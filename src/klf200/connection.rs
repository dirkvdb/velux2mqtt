use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::CryptoProvider;
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{CertificateError, ClientConfig, DigitallySignedStruct, SignatureScheme};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::{Mutex, broadcast, mpsc, oneshot};
use tokio::time::{Instant, interval, timeout};
use tokio_rustls::TlsConnector;

use super::{
    CommandId, CommandKind, Frame, GatewayState, GroupInformation, KlfError, NodeGroupRequest, NodeId, NodeInformation,
    ProtocolVersion, Request, Response, Result, SessionId, SlipDecoder, Version, slip_encode,
};

const DEFAULT_KLF_PORT: u16 = 51_200;

#[derive(Clone, Copy, Debug)]
pub struct ConnectionSettings {
    pub request_timeout: Duration,
    pub session_timeout: Duration,
    pub command_buffer: usize,
    pub event_buffer: usize,
    pub incoming_buffer: usize,
    pub read_buffer_size: usize,
    pub maximum_slip_frame_length: usize,
    pub timeout_check_interval: Duration,
}

impl Default for ConnectionSettings {
    fn default() -> Self {
        Self {
            request_timeout: Duration::from_secs(5),
            session_timeout: Duration::from_mins(2),
            command_buffer: 16,
            event_buffer: 64,
            incoming_buffer: 32,
            read_buffer_size: 4096,
            maximum_slip_frame_length: 1024,
            timeout_check_interval: Duration::from_millis(50),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Klf200Config {
    pub host: String,
    pub port: u16,
    pub certificate: Option<PathBuf>,
    pub connect_timeout: Duration,
    pub connection: ConnectionSettings,
}

impl Klf200Config {
    #[must_use]
    pub fn new(host: impl Into<String>) -> Self {
        Self {
            host: host.into(),
            port: DEFAULT_KLF_PORT,
            certificate: None,
            connect_timeout: Duration::from_secs(10),
            connection: ConnectionSettings::default(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConnectionEvent {
    Notification(Response),
    UnexpectedResponse(Response),
    SessionTimedOut(SessionId),
    ProtocolError(String),
    Disconnected(String),
}

#[derive(Clone)]
pub struct Klf200Client {
    command_tx: mpsc::Sender<ActorCommand>,
    event_tx: broadcast::Sender<ConnectionEvent>,
    request_gate: Arc<Mutex<()>>,
    notification_timeout: Duration,
    operation_timeout: Duration,
}

impl fmt::Debug for Klf200Client {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("Klf200Client").finish_non_exhaustive()
    }
}

impl Klf200Client {
    /// Connects to a KLF 200 over TLS and starts the connection actor.
    ///
    /// When `config.certificate` is absent, certificate and hostname validation are disabled to
    /// support gateways with self-signed or expired certificates. This mode is vulnerable to
    /// machine-in-the-middle attacks and emits a warning.
    ///
    /// # Errors
    ///
    /// Returns an error when TCP connection, certificate loading, TLS configuration, or the TLS
    /// handshake fails or times out.
    pub async fn connect(config: Klf200Config) -> Result<Self> {
        let tcp_stream = timeout(
            config.connect_timeout,
            TcpStream::connect((config.host.as_str(), config.port)),
        )
        .await
        .map_err(|_| KlfError::ConnectTimeout)?
        .map_err(|error| io_error(&error))?;
        tcp_stream.set_nodelay(true).map_err(|error| io_error(&error))?;

        let tls_config = build_tls_config(config.certificate.as_ref())?;
        let server_name = ServerName::try_from(config.host.clone()).map_err(|error| KlfError::Tls {
            message: error.to_string(),
        })?;
        let tls_stream = timeout(
            config.connect_timeout,
            TlsConnector::from(Arc::new(tls_config)).connect(server_name, tcp_stream),
        )
        .await
        .map_err(|_| KlfError::ConnectTimeout)?
        .map_err(|error| KlfError::Tls {
            message: error.to_string(),
        })?;

        Ok(Self::from_stream(tls_stream, config.connection))
    }

    /// Starts a KLF connection actor around an already connected async byte stream.
    ///
    /// This is useful for alternative transports and deterministic in-memory tests. Production
    /// callers normally use [`Klf200Client::connect`].
    pub fn from_stream<S>(stream: S, settings: ConnectionSettings) -> Self
    where
        S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    {
        let command_capacity = settings.command_buffer.max(1);
        let event_capacity = settings.event_buffer.max(1);
        let (command_tx, command_rx) = mpsc::channel(command_capacity);
        let (event_tx, _) = broadcast::channel(event_capacity);

        tokio::spawn(run_actor(stream, command_rx, event_tx.clone(), settings));

        Self {
            command_tx,
            event_tx,
            request_gate: Arc::new(Mutex::new(())),
            notification_timeout: settings.request_timeout,
            operation_timeout: settings.session_timeout,
        }
    }

    #[must_use]
    pub fn subscribe(&self) -> broadcast::Receiver<ConnectionEvent> {
        self.event_tx.subscribe()
    }

    /// Sends one request and waits for its matching confirmation.
    ///
    /// Calls through this handle and its clones are serialized. Notifications remain available
    /// through [`Klf200Client::subscribe`] while a confirmation is pending.
    ///
    /// # Errors
    ///
    /// Returns an encoding, connection, duplicate-session, or request-timeout error.
    pub async fn send(&self, request: Request) -> Result<Response> {
        let _guard = self.request_gate.lock().await;
        self.send_unlocked(request).await
    }

    /// Authenticates with the gateway WLAN password.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid password length, transport failure, timeout, rejection, or an
    /// unexpected confirmation type.
    pub async fn login(&self, password: impl Into<String>) -> Result<()> {
        match self.send(Request::password_enter(password)?).await? {
            Response::PasswordEntered { accepted: true } => Ok(()),
            Response::PasswordEntered { accepted: false } => Err(KlfError::AuthenticationRejected),
            response => Err(unexpected_response("password login", &response)),
        }
    }

    /// Requests gateway firmware and hardware version information.
    ///
    /// # Errors
    ///
    /// Returns a connection or timeout error, or an error if the gateway confirms with an
    /// unexpected response type.
    pub async fn version(&self) -> Result<Version> {
        match self.send(Request::GetVersion).await? {
            Response::Version(version) => Ok(version),
            response => Err(unexpected_response("version request", &response)),
        }
    }

    /// Requests the KLF API protocol version.
    ///
    /// # Errors
    ///
    /// Returns a connection or timeout error, or an error for an unexpected response type.
    pub async fn protocol_version(&self) -> Result<ProtocolVersion> {
        match self.send(Request::GetProtocolVersion).await? {
            Response::ProtocolVersion(version) => Ok(version),
            response => Err(unexpected_response("protocol version request", &response)),
        }
    }

    /// Requests the current gateway state, suitable for use as a heartbeat.
    ///
    /// # Errors
    ///
    /// Returns a connection or timeout error, or an error for an unexpected response type.
    pub async fn gateway_state(&self) -> Result<GatewayState> {
        match self.send(Request::GetState).await? {
            Response::GatewayState(state) => Ok(state),
            response => Err(unexpected_response("gateway state request", &response)),
        }
    }

    /// Discovers all nodes and collects notifications until the finished notification arrives.
    ///
    /// The shared request gate remains held for the complete multi-frame operation so another
    /// request cannot overtake its notifications.
    ///
    /// # Errors
    ///
    /// Returns an error when discovery is rejected, the event stream lags or disconnects, the
    /// operation times out, or the initial request fails.
    pub async fn discover_nodes(&self) -> Result<Vec<NodeInformation>> {
        let _guard = self.request_gate.lock().await;
        let mut events = self.subscribe();
        match self.send_unlocked(Request::GetAllNodesInformation).await? {
            Response::AllNodesInformationAccepted { status: 0, .. } => {}
            Response::AllNodesInformationAccepted { status, .. } => {
                return Err(KlfError::DiscoveryRejected { status });
            }
            response => return Err(unexpected_response("node discovery", &response)),
        }

        timeout(self.operation_timeout, async {
            let mut nodes = Vec::new();
            loop {
                match events.recv().await {
                    Ok(ConnectionEvent::Notification(Response::NodeInformation(node))) => nodes.push(node),
                    Ok(ConnectionEvent::Notification(Response::Acknowledgement { command }))
                        if command == CommandId::GW_GET_ALL_NODES_INFORMATION_FINISHED_NTF =>
                    {
                        return Ok(nodes);
                    }
                    Ok(ConnectionEvent::Disconnected(_)) | Err(broadcast::error::RecvError::Closed) => {
                        return Err(KlfError::ConnectionClosed);
                    }
                    Ok(ConnectionEvent::ProtocolError(message)) => {
                        return Err(KlfError::Protocol { message });
                    }
                    Ok(_) => {}
                    Err(broadcast::error::RecvError::Lagged(_)) => {
                        return Err(KlfError::InvalidRequest {
                            message: "node discovery event receiver lagged",
                        });
                    }
                }
            }
        })
        .await
        .map_err(|_| KlfError::RequestTimeout {
            command: CommandId::GW_GET_ALL_NODES_INFORMATION_REQ,
        })?
    }

    /// Reads the current information record for one KLF node.
    ///
    /// # Errors
    ///
    /// Returns an error when the request is rejected, the notification stream lags or disconnects,
    /// the operation times out, or the confirmation references a different node.
    pub async fn node_information(&self, node_id: NodeId) -> Result<NodeInformation> {
        let _guard = self.request_gate.lock().await;
        let mut events = self.subscribe();
        match self.send_unlocked(Request::GetNodeInformation(node_id)).await? {
            Response::NodeInformationAccepted {
                status: 0,
                node_id: confirmed,
            } if confirmed == node_id => {}
            Response::NodeInformationAccepted { status: 0, .. } => {
                return Err(KlfError::InvalidRequest {
                    message: "node information confirmation referenced another node",
                });
            }
            Response::NodeInformationAccepted { status, .. } => {
                return Err(KlfError::DiscoveryRejected { status });
            }
            response => return Err(unexpected_response("node information request", &response)),
        }

        timeout(self.notification_timeout, async {
            loop {
                match events.recv().await {
                    Ok(ConnectionEvent::Notification(Response::NodeInformation(information)))
                        if information.node_id == node_id =>
                    {
                        return Ok(information);
                    }
                    Ok(ConnectionEvent::Disconnected(_)) | Err(broadcast::error::RecvError::Closed) => {
                        return Err(KlfError::ConnectionClosed);
                    }
                    Ok(ConnectionEvent::ProtocolError(message)) => {
                        return Err(KlfError::Protocol { message });
                    }
                    Ok(_) => {}
                    Err(broadcast::error::RecvError::Lagged(_)) => {
                        return Err(KlfError::InvalidRequest {
                            message: "node information event receiver lagged",
                        });
                    }
                }
            }
        })
        .await
        .map_err(|_| KlfError::RequestTimeout {
            command: CommandId::GW_GET_NODE_INFORMATION_REQ,
        })?
    }

    /// Discovers all configured KLF groups and collects notifications until completion.
    ///
    /// The shared request gate remains held for the complete multi-frame operation. A gateway
    /// reporting that no groups are available is returned as an empty collection.
    ///
    /// # Errors
    ///
    /// Returns an error when discovery is rejected, the event stream lags or disconnects, the
    /// operation times out, or the initial request fails. The confirmation count is advisory because
    /// some KLF firmware reports a value that differs from the completed notification stream.
    pub async fn discover_groups(&self) -> Result<Vec<GroupInformation>> {
        let _guard = self.request_gate.lock().await;
        let mut events = self.subscribe();
        let expected = match self
            .send_unlocked(Request::NodeGroup(NodeGroupRequest::GetAllGroups {
                use_filter: false,
                group_type: 0,
            }))
            .await?
        {
            Response::AllGroupsAccepted(accepted) => match accepted.status {
                0 => usize::from(accepted.total_groups),
                2 => return Ok(Vec::new()),
                status => {
                    return Err(KlfError::DiscoveryRejected {
                        status: status.to_be_bytes()[0],
                    });
                }
            },
            response => return Err(unexpected_response("group discovery", &response)),
        };

        timeout(self.operation_timeout, async {
            let mut groups = Vec::with_capacity(expected);
            loop {
                match events.recv().await {
                    Ok(ConnectionEvent::Notification(Response::GroupInformation(notification)))
                        if notification.command == CommandId::GW_GET_ALL_GROUPS_INFORMATION_NTF =>
                    {
                        groups.push(notification.information);
                    }
                    Ok(ConnectionEvent::Notification(Response::Acknowledgement { command }))
                        if command == CommandId::GW_GET_ALL_GROUPS_INFORMATION_FINISHED_NTF =>
                    {
                        if groups.len() != expected {
                            log::warn!(
                                "KLF group discovery reported {expected} groups but delivered {}; using the completed notification stream",
                                groups.len()
                            );
                        }
                        return Ok(groups);
                    }
                    Ok(ConnectionEvent::Disconnected(_)) | Err(broadcast::error::RecvError::Closed) => {
                        return Err(KlfError::ConnectionClosed);
                    }
                    Ok(ConnectionEvent::ProtocolError(message)) => {
                        return Err(KlfError::Protocol { message });
                    }
                    Ok(_) => {}
                    Err(broadcast::error::RecvError::Lagged(_)) => {
                        return Err(KlfError::InvalidRequest {
                            message: "group discovery event receiver lagged",
                        });
                    }
                }
            }
        })
        .await
        .map_err(|_| KlfError::RequestTimeout {
            command: CommandId::GW_GET_ALL_GROUPS_INFORMATION_REQ,
        })?
    }

    /// Disables house monitoring and cleanly closes the actor-owned stream.
    ///
    /// # Errors
    ///
    /// The stream is closed even when the gateway does not confirm monitor disable. In that case,
    /// the confirmation error is returned because disconnecting with monitoring enabled can leave
    /// a KLF 200 unable to complete subsequent TLS handshakes.
    pub async fn shutdown(&self) -> Result<()> {
        self.shutdown_inner(false).await
    }

    /// Disables house monitoring, requests a gateway reboot, and closes the stream.
    ///
    /// A reboot can recover KLF units whose firmware otherwise becomes unavailable after a
    /// monitored connection closes. It interrupts the complete gateway and is therefore kept as
    /// an explicit operation rather than the default [`Klf200Client::shutdown`] behavior.
    ///
    /// # Errors
    ///
    /// The stream is closed even when monitor disable or reboot is not confirmed. A successful
    /// reboot confirmation supersedes a monitor-disable error because the reboot resets that
    /// monitor state.
    pub async fn shutdown_and_reboot(&self) -> Result<()> {
        self.shutdown_inner(true).await
    }

    async fn shutdown_inner(&self, reboot: bool) -> Result<()> {
        let _guard = self.request_gate.lock().await;
        let monitor_result = match self.send_unlocked(Request::HouseStatusMonitorDisable).await {
            Ok(Response::Acknowledgement { command }) if command == CommandId::GW_HOUSE_STATUS_MONITOR_DISABLE_CFM => {
                Ok(())
            }
            Ok(response) => Err(unexpected_response("house monitor disable", &response)),
            Err(error) => Err(error),
        };
        let reboot_result = if reboot {
            match self.send_unlocked(Request::Reboot).await {
                Ok(Response::Acknowledgement { command }) if command == CommandId::GW_REBOOT_CFM => Ok(()),
                Ok(response) => Err(unexpected_response("gateway reboot", &response)),
                Err(error) => Err(error),
            }
        } else {
            Ok(())
        };

        let (reply_tx, reply_rx) = oneshot::channel();
        let shutdown_result = match self.command_tx.send(ActorCommand::Shutdown { reply: reply_tx }).await {
            Ok(()) => reply_rx.await.map_err(|_| KlfError::ConnectionClosed)?,
            Err(_) => {
                return reboot_result.and(monitor_result).and(Err(KlfError::ClientClosed));
            }
        };

        shutdown_result?;
        match reboot_result {
            Err(error) => Err(error),
            Ok(()) if reboot => Ok(()),
            Ok(()) => monitor_result,
        }
    }

    async fn send_unlocked(&self, request: Request) -> Result<Response> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.command_tx
            .send(ActorCommand::Send {
                request,
                reply: reply_tx,
            })
            .await
            .map_err(|_| KlfError::ConnectionClosed)?;
        reply_rx.await.map_err(|_| KlfError::ConnectionClosed)?
    }
}

fn unexpected_response(operation: &'static str, response: &Response) -> KlfError {
    KlfError::UnexpectedResponse {
        operation,
        command: response_command(response),
    }
}

fn response_command(response: &Response) -> CommandId {
    match response {
        Response::ErrorNotification { .. } => CommandId::GW_ERROR_NTF,
        Response::Acknowledgement { command }
        | Response::Unknown { command, .. }
        | Response::OperationStatus { command, .. } => *command,
        Response::PasswordEntered { .. } => CommandId::GW_PASSWORD_ENTER_CFM,
        Response::PasswordChanged { .. } => CommandId::GW_PASSWORD_CHANGE_CFM,
        Response::PasswordChangedNotification(_) => CommandId::GW_PASSWORD_CHANGE_NTF,
        Response::Version(_) => CommandId::GW_GET_VERSION_CFM,
        Response::ProtocolVersion(_) => CommandId::GW_GET_PROTOCOL_VERSION_CFM,
        Response::GatewayState(_) => CommandId::GW_GET_STATE_CFM,
        Response::NetworkSetup(_) => CommandId::GW_GET_NETWORK_SETUP_CFM,
        Response::LocalTime(_) => CommandId::GW_GET_LOCAL_TIME_CFM,
        Response::SystemTableData(_) => CommandId::GW_CS_GET_SYSTEMTABLE_DATA_NTF,
        Response::NodeDiscoveryResult(_) => CommandId::GW_CS_DISCOVER_NODES_NTF,
        Response::NodesRemoved { .. } => CommandId::GW_CS_REMOVE_NODES_CFM,
        Response::ControllerCopyResult(_) => CommandId::GW_CS_CONTROLLER_COPY_NTF,
        Response::KeyChangeResult(result) => result.command,
        Response::PgcJob(_) => CommandId::GW_CS_PGC_JOB_NTF,
        Response::SystemTableUpdate(_) => CommandId::GW_CS_SYSTEM_TABLE_UPDATE_NTF,
        Response::ConfigurationActivationResult(_) => CommandId::GW_CS_ACTIVATE_CONFIGURATION_MODE_CFM,
        Response::NodeMutationResult(result) => result.command,
        Response::GroupOperationResult(result) => result.command,
        Response::GroupInformation(notification) => notification.command,
        Response::GroupChange(_) => CommandId::GW_GROUP_INFORMATION_CHANGED_NTF,
        Response::AllGroupsAccepted(_) => CommandId::GW_GET_ALL_GROUPS_INFORMATION_CFM,
        Response::GroupDeleted(_) => CommandId::GW_GROUP_DELETED_NTF,
        Response::SessionCommandResult(result) => result.command,
        Response::WinkFinished { .. } => CommandId::GW_WINK_SEND_NTF,
        Response::LimitationStatus(_) => CommandId::GW_LIMITATION_STATUS_NTF,
        Response::ModeNotification(_) => CommandId::GW_MODE_SEND_NTF,
        Response::SceneStatusResult(result) => result.command,
        Response::SceneInitializationResult(_) => CommandId::GW_INITIALIZE_SCENE_NTF,
        Response::SceneObjectResult(result) => result.command,
        Response::SceneListAccepted { .. } => CommandId::GW_GET_SCENE_LIST_CFM,
        Response::SceneList(_) => CommandId::GW_GET_SCENE_LIST_NTF,
        Response::SceneInformation(_) => CommandId::GW_GET_SCENE_INFOAMATION_NTF,
        Response::SceneSessionResult(result) => result.command,
        Response::SceneChange(_) => CommandId::GW_SCENE_INFORMATION_CHANGED_NTF,
        Response::ProductGroupResult(_) => CommandId::GW_ACTIVATE_PRODUCTGROUP_CFM,
        Response::ProductGroupNotification(_) => CommandId::GW_ACTIVATE_PRODUCTGROUP_NTF,
        Response::ContactInputLinks(_) => CommandId::GW_GET_CONTACT_INPUT_LINK_LIST_CFM,
        Response::ContactInputOperationResult(result) => result.command,
        Response::ActivationLogHeader(_) => CommandId::GW_GET_ACTIVATION_LOG_HEADER_CFM,
        Response::ActivationLogEntry(entry) => entry.command,
        Response::MultipleActivationLogResult(_) => CommandId::GW_GET_MULTIPLE_ACTIVATION_LOG_LINES_CFM,
        Response::NodeInformationAccepted { .. } => CommandId::GW_GET_NODE_INFORMATION_CFM,
        Response::AllNodesInformationAccepted { .. } => CommandId::GW_GET_ALL_NODES_INFORMATION_CFM,
        Response::NodeInformation(_) => CommandId::GW_GET_ALL_NODES_INFORMATION_NTF,
        Response::NodeInformationChanged(_) => CommandId::GW_NODE_INFORMATION_CHANGED_NTF,
        Response::NodeStatePosition(_) => CommandId::GW_NODE_STATE_POSITION_CHANGED_NTF,
        Response::CommandAccepted { .. } => CommandId::GW_COMMAND_SEND_CFM,
        Response::CommandRunStatus(_) => CommandId::GW_COMMAND_RUN_STATUS_NTF,
        Response::CommandRemainingTime { .. } => CommandId::GW_COMMAND_REMAINING_TIME_NTF,
        Response::SessionFinished { .. } => CommandId::GW_SESSION_FINISHED_NTF,
        Response::StatusAccepted { .. } => CommandId::GW_STATUS_REQUEST_CFM,
        Response::StatusNotification(_) => CommandId::GW_STATUS_REQUEST_NTF,
    }
}

enum ActorCommand {
    Send {
        request: Request,
        reply: oneshot::Sender<Result<Response>>,
    },
    Shutdown {
        reply: oneshot::Sender<Result<()>>,
    },
}

struct QueuedRequest {
    request: Request,
    reply: oneshot::Sender<Result<Response>>,
}

struct PendingRequest {
    command: CommandId,
    expected_confirmation: CommandId,
    session_id: Option<SessionId>,
    deadline: Instant,
    reply: oneshot::Sender<Result<Response>>,
}

enum Incoming {
    Frame(Frame),
    ProtocolError(KlfError),
    Closed(KlfError),
}

async fn run_actor<S>(
    stream: S,
    mut command_rx: mpsc::Receiver<ActorCommand>,
    event_tx: broadcast::Sender<ConnectionEvent>,
    settings: ConnectionSettings,
) where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let (reader, mut writer) = tokio::io::split(stream);
    let (incoming_tx, mut incoming_rx) = mpsc::channel(settings.incoming_buffer.max(1));
    let reader_task = tokio::spawn(read_frames(reader, incoming_tx, settings));
    let mut requests = VecDeque::<QueuedRequest>::new();
    let mut pending = None::<PendingRequest>;
    let mut sessions = HashMap::<SessionId, Instant>::new();
    let mut timeout_tick = interval(settings.timeout_check_interval.max(Duration::from_millis(1)));
    let mut shutdown_reply = None;
    let mut disconnect_reason = None;

    'actor: loop {
        if pending.is_none()
            && let Err(error) = dispatch_next(
                &mut requests,
                &mut pending,
                &sessions,
                &mut writer,
                settings.request_timeout,
            )
            .await
        {
            disconnect_reason = Some(error.to_string());
            break 'actor;
        }

        tokio::select! {
            command = command_rx.recv() => match command {
                Some(ActorCommand::Send { request, reply }) => requests.push_back(QueuedRequest { request, reply }),
                Some(ActorCommand::Shutdown { reply }) => {
                    shutdown_reply = Some(reply);
                    break;
                }
                None => break,
            },
            incoming = incoming_rx.recv() => match incoming {
                Some(Incoming::Frame(frame)) => {
                    handle_frame(
                        frame,
                        &mut pending,
                        &mut sessions,
                        settings.session_timeout,
                        &event_tx,
                    );
                }
                Some(Incoming::ProtocolError(error)) => {
                    let _ = event_tx.send(ConnectionEvent::ProtocolError(error.to_string()));
                }
                Some(Incoming::Closed(error)) => {
                    disconnect_reason = Some(error.to_string());
                    break;
                }
                None => {
                    disconnect_reason = Some(KlfError::ConnectionClosed.to_string());
                    break;
                }
            },
            _ = timeout_tick.tick() => {
                expire_request(&mut pending, &mut sessions);
                expire_sessions(&mut sessions, &event_tx);
            }
        }
    }

    reader_task.abort();
    fail_outstanding(&mut pending, &mut requests);
    while let Ok(command) = command_rx.try_recv() {
        if let ActorCommand::Send { reply, .. } = command {
            let _ = reply.send(Err(KlfError::ConnectionClosed));
        }
    }
    let shutdown_result = writer.shutdown().await.map_err(|error| io_error(&error));

    if let Some(reason) = disconnect_reason {
        let _ = event_tx.send(ConnectionEvent::Disconnected(reason));
    }
    if let Some(reply) = shutdown_reply {
        let _ = reply.send(shutdown_result);
    }
}

async fn dispatch_next<W>(
    requests: &mut VecDeque<QueuedRequest>,
    pending: &mut Option<PendingRequest>,
    sessions: &HashMap<SessionId, Instant>,
    writer: &mut W,
    request_timeout: Duration,
) -> Result<()>
where
    W: AsyncWrite + Unpin,
{
    while let Some(queued) = requests.pop_front() {
        let command = queued.request.command_id();
        let Some(expected_confirmation) = command.expected_confirmation() else {
            let _ = queued.reply.send(Err(KlfError::MissingConfirmation { command }));
            continue;
        };
        let session_id = queued.request.session_id();
        if let Some(session_id) = session_id
            && sessions.contains_key(&session_id)
        {
            let _ = queued.reply.send(Err(KlfError::SessionIdInUse {
                session_id: session_id.get(),
            }));
            continue;
        }

        let frame = match queued.request.encode().and_then(|frame| frame.encode()) {
            Ok(frame) => frame,
            Err(error) => {
                let _ = queued.reply.send(Err(error));
                continue;
            }
        };
        if let Err(error) = writer.write_all(&slip_encode(&frame)).await {
            let _ = queued.reply.send(Err(KlfError::ConnectionClosed));
            return Err(io_error(&error));
        }

        let now = Instant::now();
        *pending = Some(PendingRequest {
            command,
            expected_confirmation,
            session_id,
            deadline: now + request_timeout,
            reply: queued.reply,
        });
        break;
    }
    Ok(())
}

fn handle_frame(
    frame: Frame,
    pending: &mut Option<PendingRequest>,
    sessions: &mut HashMap<SessionId, Instant>,
    session_timeout: Duration,
    event_tx: &broadcast::Sender<ConnectionEvent>,
) {
    let command = frame.command;
    if pending
        .as_ref()
        .is_some_and(|request| request.expected_confirmation == command)
    {
        let decoded = Response::decode(frame);
        if decoded.as_ref().is_ok_and(|response| {
            pending
                .as_ref()
                .is_some_and(|request| !confirmation_session_matches(request.session_id, response))
        }) {
            if let Ok(response) = decoded {
                let _ = event_tx.send(ConnectionEvent::UnexpectedResponse(response));
            }
            return;
        }
        let Some(request) = pending.take() else {
            return;
        };
        match decoded {
            Ok(response) => {
                if let Some(session_id) = request.session_id {
                    if session_was_accepted(&response) {
                        sessions.insert(session_id, Instant::now() + session_timeout);
                    } else {
                        sessions.remove(&session_id);
                    }
                }
                let _ = request.reply.send(Ok(response));
            }
            Err(error) => {
                if let Some(session_id) = request.session_id {
                    sessions.remove(&session_id);
                }
                let _ = request.reply.send(Err(error));
            }
        }
        return;
    }

    match Response::decode(frame) {
        Ok(response) if command.kind() == CommandKind::Notification => {
            if let Some(session_id) = terminal_session_id(&response) {
                sessions.remove(&session_id);
            }
            let _ = event_tx.send(ConnectionEvent::Notification(response));
        }
        Ok(response) => {
            let _ = event_tx.send(ConnectionEvent::UnexpectedResponse(response));
        }
        Err(error) => {
            let _ = event_tx.send(ConnectionEvent::ProtocolError(error.to_string()));
        }
    }
}

fn confirmation_session_matches(expected: Option<SessionId>, response: &Response) -> bool {
    expected.is_none_or(|expected| response_session_id(response) == Some(expected))
}

fn response_session_id(response: &Response) -> Option<SessionId> {
    match response {
        Response::CommandAccepted { session_id, .. } | Response::StatusAccepted { session_id, .. } => Some(*session_id),
        Response::SessionCommandResult(result) => Some(result.session_id),
        Response::SceneSessionResult(result) => Some(result.session_id),
        Response::ProductGroupResult(result) => Some(result.session_id),
        _ => None,
    }
}

fn session_was_accepted(response: &Response) -> bool {
    match response {
        Response::CommandAccepted { status, .. } | Response::StatusAccepted { status, .. } => *status == 1,
        Response::SessionCommandResult(result) => match result.command {
            CommandId::GW_WINK_SEND_CFM
            | CommandId::GW_SET_LIMITATION_CFM
            | CommandId::GW_GET_LIMITATION_STATUS_CFM => result.status == 1,
            CommandId::GW_MODE_SEND_CFM => result.status == 0,
            _ => false,
        },
        Response::SceneSessionResult(result) => result.status == 0,
        Response::ProductGroupResult(result) => result.status == 0,
        _ => false,
    }
}

fn terminal_session_id(response: &Response) -> Option<SessionId> {
    match response {
        Response::SessionFinished { session_id } | Response::WinkFinished { session_id } => Some(*session_id),
        _ => None,
    }
}

fn expire_request(pending: &mut Option<PendingRequest>, sessions: &mut HashMap<SessionId, Instant>) {
    if pending
        .as_ref()
        .is_some_and(|request| request.deadline <= Instant::now())
    {
        let Some(request) = pending.take() else {
            return;
        };
        if let Some(session_id) = request.session_id {
            sessions.remove(&session_id);
        }
        let _ = request.reply.send(Err(KlfError::RequestTimeout {
            command: request.command,
        }));
    }
}

fn expire_sessions(sessions: &mut HashMap<SessionId, Instant>, event_tx: &broadcast::Sender<ConnectionEvent>) {
    let now = Instant::now();
    let expired = sessions
        .iter()
        .filter_map(|(session_id, deadline)| (*deadline <= now).then_some(*session_id))
        .collect::<Vec<_>>();
    for session_id in expired {
        sessions.remove(&session_id);
        let _ = event_tx.send(ConnectionEvent::SessionTimedOut(session_id));
    }
}

fn fail_outstanding(pending: &mut Option<PendingRequest>, requests: &mut VecDeque<QueuedRequest>) {
    if let Some(request) = pending.take() {
        let _ = request.reply.send(Err(KlfError::ConnectionClosed));
    }
    for request in requests.drain(..) {
        let _ = request.reply.send(Err(KlfError::ConnectionClosed));
    }
}

async fn read_frames<R>(reader: R, incoming_tx: mpsc::Sender<Incoming>, settings: ConnectionSettings)
where
    R: AsyncRead + Unpin + Send + 'static,
{
    let mut reader = reader;
    let mut decoder = SlipDecoder::new(settings.maximum_slip_frame_length.max(1));
    let mut buffer = vec![0; settings.read_buffer_size.max(1)];
    loop {
        let read = match reader.read(&mut buffer).await {
            Ok(0) => {
                let _ = incoming_tx.send(Incoming::Closed(KlfError::ConnectionClosed)).await;
                return;
            }
            Ok(read) => read,
            Err(error) => {
                let _ = incoming_tx.send(Incoming::Closed(io_error(&error))).await;
                return;
            }
        };
        for decoded in decoder.push(&buffer[..read]) {
            let incoming = match decoded.and_then(|frame| Frame::decode(&frame)) {
                Ok(frame) => Incoming::Frame(frame),
                Err(error) => Incoming::ProtocolError(error),
            };
            if incoming_tx.send(incoming).await.is_err() {
                return;
            }
        }
    }
}

fn build_tls_config(certificate: Option<&PathBuf>) -> Result<ClientConfig> {
    if let Some(certificate) = certificate {
        let file = File::open(certificate).map_err(|error| io_error(&error))?;
        let mut reader = BufReader::new(file);
        let certificates = rustls_pemfile::certs(&mut reader)
            .collect::<std::result::Result<Vec<CertificateDer<'static>>, _>>()
            .map_err(|error| io_error(&error))?;
        if certificates.is_empty() {
            return Err(KlfError::EmptyCertificateFile);
        }
        Ok(ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(PinnedServerCertificateVerification::new(certificates))
            .with_no_client_auth())
    } else {
        log::warn!(
            "KLF certificate verification is disabled; configure --certificate or V2M_KLF_CERTIFICATE to pin the gateway certificate"
        );
        Ok(ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(SkipServerCertificateVerification::new())
            .with_no_client_auth())
    }
}

#[derive(Debug)]
struct SkipServerCertificateVerification(Arc<CryptoProvider>);

impl SkipServerCertificateVerification {
    fn new() -> Arc<Self> {
        Arc::new(Self(Arc::new(rustls::crypto::ring::default_provider())))
    }
}

#[derive(Debug)]
struct PinnedServerCertificateVerification {
    certificates: Vec<CertificateDer<'static>>,
    provider: Arc<CryptoProvider>,
}

impl PinnedServerCertificateVerification {
    fn new(certificates: Vec<CertificateDer<'static>>) -> Arc<Self> {
        Arc::new(Self {
            certificates,
            provider: Arc::new(rustls::crypto::ring::default_provider()),
        })
    }
}

impl ServerCertVerifier for PinnedServerCertificateVerification {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> std::result::Result<ServerCertVerified, rustls::Error> {
        if self
            .certificates
            .iter()
            .any(|certificate| certificate.as_ref() == end_entity.as_ref())
        {
            Ok(ServerCertVerified::assertion())
        } else {
            Err(rustls::Error::InvalidCertificate(
                CertificateError::ApplicationVerificationFailure,
            ))
        }
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        certificate: &CertificateDer<'_>,
        digitally_signed: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(
            message,
            certificate,
            digitally_signed,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        certificate: &CertificateDer<'_>,
        digitally_signed: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(
            message,
            certificate,
            digitally_signed,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider.signature_verification_algorithms.supported_schemes()
    }
}

impl ServerCertVerifier for SkipServerCertificateVerification {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> std::result::Result<ServerCertVerified, rustls::Error> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        certificate: &CertificateDer<'_>,
        digitally_signed: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(
            message,
            certificate,
            digitally_signed,
            &self.0.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        certificate: &CertificateDer<'_>,
        digitally_signed: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(
            message,
            certificate,
            digitally_signed,
            &self.0.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.0.signature_verification_algorithms.supported_schemes()
    }
}

fn io_error(error: &std::io::Error) -> KlfError {
    KlfError::Io {
        message: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_insecure_and_pinned_tls_configs() {
        assert!(build_tls_config(None).is_ok());

        let certificate = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("cert/velux-cert.pem");
        assert!(build_tls_config(Some(&certificate)).is_ok());
    }
}
