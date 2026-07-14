use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt, DuplexStream};
use tokio::task::JoinHandle;
use tokio::time::timeout;
use velux2mqtt::klf200::{
    CommandId, CommandRequest, CommandTarget, ConnectionEvent, ConnectionSettings, Frame, Klf200Client, KlfError,
    NodeId, Percentage, Request, Response, SessionId, SlipDecoder, StandardParameter, slip_encode,
};

fn test_settings() -> ConnectionSettings {
    ConnectionSettings {
        request_timeout: Duration::from_millis(100),
        session_timeout: Duration::from_millis(100),
        timeout_check_interval: Duration::from_millis(5),
        ..ConnectionSettings::default()
    }
}

async fn read_request(stream: &mut DuplexStream, decoder: &mut SlipDecoder) -> Frame {
    let mut buffer = [0; 31];
    loop {
        let read = timeout(Duration::from_secs(1), stream.read(&mut buffer))
            .await
            .expect("gateway read timed out")
            .expect("gateway read failed");
        assert_ne!(read, 0, "client closed before sending the expected request");
        if let Some(decoded) = decoder.push(&buffer[..read]).into_iter().next() {
            return Frame::decode(&decoded.expect("valid SLIP request")).expect("valid KLF request");
        }
    }
}

async fn write_response(stream: &mut DuplexStream, command: CommandId, payload: &[u8]) {
    let frame = Frame::new(command, payload.to_vec()).encode().expect("encode response");
    stream
        .write_all(&slip_encode(&frame))
        .await
        .expect("write gateway response");
}

async fn write_fragmented_frames(stream: &mut DuplexStream, frames: &[(CommandId, Vec<u8>)]) {
    let mut wire = Vec::new();
    for (command, payload) in frames {
        let frame = Frame::new(*command, payload.clone()).encode().expect("encode response");
        wire.extend_from_slice(&slip_encode(&frame));
    }
    for chunk in wire.chunks(3) {
        stream.write_all(chunk).await.expect("write fragmented response");
    }
}

async fn complete_shutdown(
    client: Klf200Client,
    gateway: &mut DuplexStream,
    decoder: &mut SlipDecoder,
) -> velux2mqtt::klf200::Result<()> {
    let shutdown = tokio::spawn(async move { client.shutdown().await });
    let request = read_request(gateway, decoder).await;
    assert_eq!(request.command, CommandId::GW_HOUSE_STATUS_MONITOR_DISABLE_REQ);
    write_response(gateway, CommandId::GW_HOUSE_STATUS_MONITOR_DISABLE_CFM, &[]).await;
    shutdown.await.expect("shutdown task")
}

#[tokio::test]
async fn shutdown_closes_stream_and_reports_unconfirmed_monitor_disable() {
    let (client_stream, mut gateway) = tokio::io::duplex(4096);
    let mut settings = test_settings();
    settings.request_timeout = Duration::from_millis(20);
    let client = Klf200Client::from_stream(client_stream, settings);
    let mut decoder = SlipDecoder::default();

    let shutdown = tokio::spawn(async move { client.shutdown().await });
    let request = read_request(&mut gateway, &mut decoder).await;
    assert_eq!(request.command, CommandId::GW_HOUSE_STATUS_MONITOR_DISABLE_REQ);

    let error = timeout(Duration::from_secs(1), shutdown)
        .await
        .expect("shutdown completes")
        .expect("shutdown task joins")
        .expect_err("missing monitor confirmation is reported");
    assert_eq!(
        error,
        velux2mqtt::klf200::KlfError::RequestTimeout {
            command: CommandId::GW_HOUSE_STATUS_MONITOR_DISABLE_REQ,
        }
    );

    let mut byte = [0];
    assert_eq!(gateway.read(&mut byte).await.expect("read closed stream"), 0);
}

#[tokio::test]
async fn configured_shutdown_reboots_after_unconfirmed_monitor_disable() {
    let (client_stream, mut gateway) = tokio::io::duplex(4096);
    let mut settings = test_settings();
    settings.request_timeout = Duration::from_millis(20);
    let client = Klf200Client::from_stream(client_stream, settings);
    let mut decoder = SlipDecoder::default();

    let shutdown = tokio::spawn(async move { client.shutdown_and_reboot().await });
    let disable = read_request(&mut gateway, &mut decoder).await;
    assert_eq!(disable.command, CommandId::GW_HOUSE_STATUS_MONITOR_DISABLE_REQ);

    let reboot = read_request(&mut gateway, &mut decoder).await;
    assert_eq!(reboot.command, CommandId::GW_REBOOT_REQ);
    write_response(&mut gateway, CommandId::GW_REBOOT_CFM, &[]).await;

    timeout(Duration::from_secs(1), shutdown)
        .await
        .expect("shutdown completes")
        .expect("shutdown task joins")
        .expect("confirmed reboot recovers monitor-disable timeout");
    let mut byte = [0];
    assert_eq!(gateway.read(&mut byte).await.expect("read closed stream"), 0);
}

#[tokio::test]
async fn serializes_requests_while_delivering_fragmented_notifications() {
    let (client_stream, mut gateway) = tokio::io::duplex(4096);
    let client = Klf200Client::from_stream(client_stream, test_settings());
    let mut events = client.subscribe();
    let mut decoder = SlipDecoder::default();

    let version_client = client.clone();
    let version = tokio::spawn(async move { version_client.version().await });
    tokio::task::yield_now().await;
    let protocol_client = client.clone();
    let protocol = tokio::spawn(async move { protocol_client.protocol_version().await });

    let first = read_request(&mut gateway, &mut decoder).await;
    assert_eq!(first.command, CommandId::GW_GET_VERSION_REQ);
    assert!(
        timeout(Duration::from_millis(20), read_request(&mut gateway, &mut decoder))
            .await
            .is_err(),
        "the second request was written before the first confirmation"
    );

    let mut position = vec![0; 20];
    position[0] = 7;
    position[1] = 4;
    position[2..4].copy_from_slice(&Percentage::from_percent(25).raw().to_be_bytes());
    position[4..6].copy_from_slice(&Percentage::from_percent(75).raw().to_be_bytes());
    write_fragmented_frames(
        &mut gateway,
        &[
            (CommandId::GW_NODE_STATE_POSITION_CHANGED_NTF, position),
            (CommandId::GW_GET_VERSION_CFM, vec![0, 2, 0, 0, 71, 0, 6, 14, 3]),
        ],
    )
    .await;

    assert_eq!(version.await.expect("version task").expect("version").hardware, 6);
    let second = read_request(&mut gateway, &mut decoder).await;
    assert_eq!(second.command, CommandId::GW_GET_PROTOCOL_VERSION_REQ);
    write_response(&mut gateway, CommandId::GW_GET_PROTOCOL_VERSION_CFM, &[0, 3, 0, 18]).await;
    assert_eq!(protocol.await.expect("protocol task").expect("protocol").minor, 18);

    let event = events.recv().await.expect("position event");
    assert!(matches!(
        event,
        ConnectionEvent::Notification(Response::NodeStatePosition(position))
            if position.node_id == NodeId::new(7)
    ));

    complete_shutdown(client, &mut gateway, &mut decoder)
        .await
        .expect("clean shutdown");
}

#[tokio::test]
async fn collects_node_discovery_until_finished_notification() {
    let (client_stream, mut gateway) = tokio::io::duplex(4096);
    let client = Klf200Client::from_stream(client_stream, test_settings());
    let mut decoder = SlipDecoder::default();

    let discovery_client = client.clone();
    let discovery = tokio::spawn(async move { discovery_client.discover_nodes().await });
    let request = read_request(&mut gateway, &mut decoder).await;
    assert_eq!(request.command, CommandId::GW_GET_ALL_NODES_INFORMATION_REQ);
    write_response(&mut gateway, CommandId::GW_GET_ALL_NODES_INFORMATION_CFM, &[0, 2]).await;
    write_response(
        &mut gateway,
        CommandId::GW_GET_ALL_NODES_INFORMATION_NTF,
        &node_information_payload(3, "Kitchen"),
    )
    .await;
    write_response(
        &mut gateway,
        CommandId::GW_GET_ALL_NODES_INFORMATION_NTF,
        &node_information_payload(8, "Office"),
    )
    .await;
    write_response(&mut gateway, CommandId::GW_GET_ALL_NODES_INFORMATION_FINISHED_NTF, &[]).await;

    let nodes = discovery.await.expect("discovery task").expect("discovery result");
    assert_eq!(nodes.len(), 2);
    assert_eq!(nodes[0].name, "Kitchen");
    assert_eq!(nodes[1].node_id, NodeId::new(8));

    complete_shutdown(client, &mut gateway, &mut decoder)
        .await
        .expect("clean shutdown");
}

#[tokio::test]
async fn reads_individual_node_information_notification() {
    let (client_stream, mut gateway) = tokio::io::duplex(4096);
    let client = Klf200Client::from_stream(client_stream, test_settings());
    let mut decoder = SlipDecoder::default();

    let information_client = client.clone();
    let information = tokio::spawn(async move { information_client.node_information(NodeId::new(3)).await });
    let request = read_request(&mut gateway, &mut decoder).await;
    assert_eq!(request.command, CommandId::GW_GET_NODE_INFORMATION_REQ);
    assert_eq!(request.payload.as_ref(), &[3]);
    write_response(&mut gateway, CommandId::GW_GET_NODE_INFORMATION_CFM, &[0, 3]).await;
    write_response(
        &mut gateway,
        CommandId::GW_GET_NODE_INFORMATION_NTF,
        &node_information_payload(3, "Kitchen"),
    )
    .await;

    let information = information.await.expect("information task").expect("node information");
    assert_eq!(information.node_id, NodeId::new(3));
    assert_eq!(information.name, "Kitchen");

    complete_shutdown(client, &mut gateway, &mut decoder)
        .await
        .expect("clean shutdown");
}

#[tokio::test]
async fn collects_group_discovery_until_finished_notification() {
    let (client_stream, mut gateway) = tokio::io::duplex(4096);
    let client = Klf200Client::from_stream(client_stream, test_settings());
    let mut decoder = SlipDecoder::default();

    let discovery_client = client.clone();
    let discovery = tokio::spawn(async move { discovery_client.discover_groups().await });
    let request = read_request(&mut gateway, &mut decoder).await;
    assert_eq!(request.command, CommandId::GW_GET_ALL_GROUPS_INFORMATION_REQ);
    assert_eq!(request.payload.as_ref(), &[0, 0]);
    // KLF firmware can report a count that does not match the completed notification stream.
    write_response(&mut gateway, CommandId::GW_GET_ALL_GROUPS_INFORMATION_CFM, &[0, 3]).await;
    write_response(
        &mut gateway,
        CommandId::GW_GET_ALL_GROUPS_INFORMATION_NTF,
        &group_information_payload(4, "Ground floor", &[3, 8]),
    )
    .await;
    write_response(
        &mut gateway,
        CommandId::GW_GET_ALL_GROUPS_INFORMATION_NTF,
        &group_information_payload(7, "Awning blind", &[11]),
    )
    .await;
    write_response(&mut gateway, CommandId::GW_GET_ALL_GROUPS_INFORMATION_FINISHED_NTF, &[]).await;

    let groups = discovery.await.expect("discovery task").expect("discovery result");
    assert_eq!(groups.len(), 2);
    assert_eq!(groups[0].name, "Ground floor");
    assert!(groups[0].actuators.contains(3));
    assert!(groups[0].actuators.contains(8));
    assert_eq!(groups[1].group_id.get(), 7);

    complete_shutdown(client, &mut gateway, &mut decoder)
        .await
        .expect("clean shutdown");
}

#[tokio::test]
async fn treats_no_configured_groups_as_empty_discovery() {
    let (client_stream, mut gateway) = tokio::io::duplex(4096);
    let client = Klf200Client::from_stream(client_stream, test_settings());
    let mut decoder = SlipDecoder::default();

    let discovery_client = client.clone();
    let discovery = tokio::spawn(async move { discovery_client.discover_groups().await });
    assert_eq!(
        read_request(&mut gateway, &mut decoder).await.command,
        CommandId::GW_GET_ALL_GROUPS_INFORMATION_REQ
    );
    write_response(&mut gateway, CommandId::GW_GET_ALL_GROUPS_INFORMATION_CFM, &[2, 0]).await;

    assert!(
        discovery
            .await
            .expect("discovery task")
            .expect("empty discovery")
            .is_empty()
    );
    complete_shutdown(client, &mut gateway, &mut decoder)
        .await
        .expect("clean shutdown");
}

#[tokio::test]
async fn recovers_after_timeout_and_routes_late_confirmation_as_unexpected() {
    let mut settings = test_settings();
    settings.request_timeout = Duration::from_millis(30);
    let (client_stream, mut gateway) = tokio::io::duplex(4096);
    let client = Klf200Client::from_stream(client_stream, settings);
    let mut events = client.subscribe();
    let mut decoder = SlipDecoder::default();

    let state_client = client.clone();
    let state = tokio::spawn(async move { state_client.gateway_state().await });
    assert_eq!(
        read_request(&mut gateway, &mut decoder).await.command,
        CommandId::GW_GET_STATE_REQ
    );
    assert_eq!(
        state.await.expect("state task"),
        Err(KlfError::RequestTimeout {
            command: CommandId::GW_GET_STATE_REQ
        })
    );

    let version_client = client.clone();
    let version = tokio::spawn(async move { version_client.version().await });
    assert_eq!(
        read_request(&mut gateway, &mut decoder).await.command,
        CommandId::GW_GET_VERSION_REQ
    );
    write_response(&mut gateway, CommandId::GW_GET_STATE_CFM, &[2, 0, 0, 0, 0, 0]).await;
    write_response(
        &mut gateway,
        CommandId::GW_GET_VERSION_CFM,
        &[0, 2, 0, 0, 71, 0, 6, 14, 3],
    )
    .await;

    assert_eq!(version.await.expect("version task").expect("version").hardware, 6);
    assert!(matches!(
        events.recv().await.expect("unexpected response event"),
        ConnectionEvent::UnexpectedResponse(Response::GatewayState(_))
    ));

    complete_shutdown(client, &mut gateway, &mut decoder)
        .await
        .expect("clean shutdown");
}

#[tokio::test]
async fn tracks_session_ids_until_finished_and_rejects_duplicates() {
    let (client_stream, mut gateway) = tokio::io::duplex(4096);
    let client = Klf200Client::from_stream(client_stream, test_settings());
    let mut events = client.subscribe();
    let mut decoder = SlipDecoder::default();
    let session_id = SessionId::new(9).expect("nonzero session");

    let first = send_cover_command(&client, session_id);
    assert_eq!(
        read_request(&mut gateway, &mut decoder).await.command,
        CommandId::GW_COMMAND_SEND_REQ
    );
    write_response(&mut gateway, CommandId::GW_COMMAND_SEND_CFM, &[0, 9, 1]).await;
    assert!(matches!(
        first.await.expect("first command task").expect("first command"),
        Response::CommandAccepted { status: 1, .. }
    ));

    assert_eq!(
        send_cover_command(&client, session_id)
            .await
            .expect("duplicate command task"),
        Err(KlfError::SessionIdInUse { session_id: 9 })
    );
    assert!(
        timeout(Duration::from_millis(20), read_request(&mut gateway, &mut decoder))
            .await
            .is_err(),
        "duplicate session was sent to the gateway"
    );

    write_response(&mut gateway, CommandId::GW_SESSION_FINISHED_NTF, &[0, 9]).await;
    assert!(matches!(
        events.recv().await.expect("session finished event"),
        ConnectionEvent::Notification(Response::SessionFinished { session_id: finished }) if finished == session_id
    ));

    let second = send_cover_command(&client, session_id);
    assert_eq!(
        read_request(&mut gateway, &mut decoder).await.command,
        CommandId::GW_COMMAND_SEND_REQ
    );
    write_response(&mut gateway, CommandId::GW_COMMAND_SEND_CFM, &[0, 9, 0]).await;
    assert!(matches!(
        second.await.expect("second command task").expect("second command"),
        Response::CommandAccepted { status: 0, .. }
    ));

    complete_shutdown(client, &mut gateway, &mut decoder)
        .await
        .expect("clean shutdown");
}

#[tokio::test]
async fn ignores_confirmation_with_the_wrong_session_id() {
    let (client_stream, mut gateway) = tokio::io::duplex(4096);
    let client = Klf200Client::from_stream(client_stream, test_settings());
    let mut events = client.subscribe();
    let mut decoder = SlipDecoder::default();
    let session_id = SessionId::new(9).expect("nonzero session");

    let mut command = send_cover_command(&client, session_id);
    assert_eq!(
        read_request(&mut gateway, &mut decoder).await.command,
        CommandId::GW_COMMAND_SEND_REQ
    );
    write_response(&mut gateway, CommandId::GW_COMMAND_SEND_CFM, &[0, 10, 1]).await;
    assert!(matches!(
        events.recv().await.expect("mismatched confirmation event"),
        ConnectionEvent::UnexpectedResponse(Response::CommandAccepted {
            session_id: received,
            status: 1,
        }) if received == SessionId::new(10).expect("nonzero")
    ));
    assert!(
        timeout(Duration::from_millis(20), &mut command).await.is_err(),
        "wrong-session confirmation completed the pending request"
    );

    write_response(&mut gateway, CommandId::GW_COMMAND_SEND_CFM, &[0, 9, 1]).await;
    assert!(matches!(
        command.await.expect("command task").expect("correct confirmation"),
        Response::CommandAccepted {
            session_id: received,
            status: 1,
        } if received == session_id
    ));
    write_response(&mut gateway, CommandId::GW_SESSION_FINISHED_NTF, &[0, 9]).await;
    assert!(matches!(
        events.recv().await.expect("session finished event"),
        ConnectionEvent::Notification(Response::SessionFinished { session_id: finished }) if finished == session_id
    ));

    complete_shutdown(client, &mut gateway, &mut decoder)
        .await
        .expect("clean shutdown");
}

#[tokio::test]
async fn expires_sessions_that_never_finish() {
    let mut settings = test_settings();
    settings.session_timeout = Duration::from_millis(30);
    let (client_stream, mut gateway) = tokio::io::duplex(4096);
    let client = Klf200Client::from_stream(client_stream, settings);
    let mut events = client.subscribe();
    let mut decoder = SlipDecoder::default();
    let session_id = SessionId::new(10).expect("nonzero session");

    let first = send_cover_command(&client, session_id);
    assert_eq!(
        read_request(&mut gateway, &mut decoder).await.command,
        CommandId::GW_COMMAND_SEND_REQ
    );
    write_response(&mut gateway, CommandId::GW_COMMAND_SEND_CFM, &[0, 10, 1]).await;
    assert!(matches!(
        first.await.expect("first command task").expect("first command"),
        Response::CommandAccepted { status: 1, .. }
    ));

    assert_eq!(
        timeout(Duration::from_millis(200), events.recv())
            .await
            .expect("session timeout event deadline")
            .expect("session timeout event"),
        ConnectionEvent::SessionTimedOut(session_id)
    );

    let second = send_cover_command(&client, session_id);
    assert_eq!(
        read_request(&mut gateway, &mut decoder).await.command,
        CommandId::GW_COMMAND_SEND_REQ
    );
    write_response(&mut gateway, CommandId::GW_COMMAND_SEND_CFM, &[0, 10, 0]).await;
    assert!(matches!(
        second.await.expect("second command task").expect("second command"),
        Response::CommandAccepted { status: 0, .. }
    ));

    complete_shutdown(client, &mut gateway, &mut decoder)
        .await
        .expect("clean shutdown");
}

#[tokio::test]
async fn connection_loss_fails_pending_and_queued_requests() {
    let (client_stream, mut gateway) = tokio::io::duplex(4096);
    let client = Klf200Client::from_stream(client_stream, test_settings());
    let mut decoder = SlipDecoder::default();

    let first_client = client.clone();
    let first = tokio::spawn(async move { first_client.send(Request::GetVersion).await });
    tokio::task::yield_now().await;
    let second_client = client.clone();
    let second = tokio::spawn(async move { second_client.send(Request::GetProtocolVersion).await });
    assert_eq!(
        read_request(&mut gateway, &mut decoder).await.command,
        CommandId::GW_GET_VERSION_REQ
    );
    drop(gateway);

    assert_eq!(
        first.await.expect("first request task"),
        Err(KlfError::ConnectionClosed)
    );
    assert_eq!(
        second.await.expect("second request task"),
        Err(KlfError::ConnectionClosed)
    );
}

fn send_cover_command(
    client: &Klf200Client,
    session_id: SessionId,
) -> JoinHandle<velux2mqtt::klf200::Result<Response>> {
    let client = client.clone();
    tokio::spawn(async move {
        let target = CommandTarget::new([NodeId::new(4)])?;
        client
            .send(Request::CommandSend {
                command: CommandRequest::new(session_id, StandardParameter::Relative(Percentage::FULLY_CLOSED)),
                target,
            })
            .await
    })
}

fn node_information_payload(node_id: u8, name: &str) -> Vec<u8> {
    let mut payload = vec![0; 124];
    payload[0] = node_id;
    payload[4..4 + name.len()].copy_from_slice(name.as_bytes());
    payload[69..71].copy_from_slice(&0x0080_u16.to_be_bytes());
    payload[71] = 1;
    payload[72] = 2;
    payload[76..84].copy_from_slice(&[0, 1, 2, 3, 4, 5, 6, node_id]);
    payload[84] = 5;
    payload[85..87].copy_from_slice(&Percentage::from_percent(50).raw().to_be_bytes());
    payload[87..89].copy_from_slice(&Percentage::from_percent(50).raw().to_be_bytes());
    for offset in [89, 91, 93, 95] {
        payload[offset..offset + 2].copy_from_slice(&0xF7FF_u16.to_be_bytes());
    }
    payload
}

fn group_information_payload(group_id: u8, name: &str, node_ids: &[u8]) -> Vec<u8> {
    let mut payload = vec![0; 99];
    payload[0] = group_id;
    payload[1..3].copy_from_slice(&u16::from(group_id).to_be_bytes());
    payload[3] = 1;
    payload[4..4 + name.len()].copy_from_slice(name.as_bytes());
    payload[70] = 0;
    payload[71] = u8::try_from(node_ids.len()).expect("test group fits");
    for node_id in node_ids {
        let index = usize::from(*node_id);
        payload[72 + index / 8] |= 1 << (index % 8);
    }
    payload[97..99].copy_from_slice(&1_u16.to_be_bytes());
    payload
}
