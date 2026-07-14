use bytes::Bytes;
use velux2mqtt::klf200::{CommandId, Frame, ProtocolVersion, Request, Response, SlipDecoder, Version, slip_encode};

#[test]
fn request_survives_fragmented_stream_round_trip() {
    let frame = Request::GetVersion.encode().expect("encode request");
    let wire = slip_encode(&frame.encode().expect("encode envelope"));

    let mut decoder = SlipDecoder::default();
    let mut frames = Vec::new();
    for byte in wire.chunks(1) {
        frames.extend(decoder.push(byte));
    }

    assert_eq!(frames.len(), 1);
    let unescaped = frames.remove(0).expect("valid SLIP frame");
    assert_eq!(Frame::decode(&unescaped).expect("valid KLF frame"), frame);
}

#[test]
fn coalesced_response_frames_decode_independently() {
    let version = Frame::new(
        CommandId::GW_GET_VERSION_CFM,
        Bytes::from_static(&[0, 2, 0, 0, 71, 0, 6, 14, 3]),
    );
    let protocol = Frame::new(
        CommandId::GW_GET_PROTOCOL_VERSION_CFM,
        Bytes::from_static(&[0, 3, 0, 18]),
    );
    let wire = [
        slip_encode(&version.encode().expect("encode version")),
        slip_encode(&protocol.encode().expect("encode protocol version")),
    ]
    .concat();

    let mut decoder = SlipDecoder::default();
    let responses = decoder
        .push(&wire)
        .into_iter()
        .map(|frame| {
            let frame = Frame::decode(&frame.expect("valid SLIP frame")).expect("valid KLF frame");
            Response::decode(frame).expect("typed response")
        })
        .collect::<Vec<_>>();

    assert_eq!(
        responses,
        [
            Response::Version(Version {
                software: [0, 2, 0, 0, 71, 0],
                hardware: 6,
                product_group: 14,
                product_type: 3,
            }),
            Response::ProtocolVersion(ProtocolVersion { major: 3, minor: 18 }),
        ]
    );
}
