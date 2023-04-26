use bytes::BytesMut;

#[derive(Debug)]
pub struct MessageHeader {
    pub timestamp_delta: u32,
    pub payload_length: u32,
    pub message_type: u8,
    pub stream_id: u32,
    pub timestamp: u32,
    pub perfer_cid: u32,
}

#[derive(Debug)]
pub struct ChunkStream {
    pub fmt: u8,
    pub cid: u32,
    pub header: MessageHeader,
    pub extended_timestamp: bool,
    pub payload: BytesMut,
}
