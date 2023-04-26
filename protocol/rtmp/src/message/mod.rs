use crate::chunk::MessageHeader;
use bytes::Bytes;
use error::{MessageDecodeError, MessageEncodeError};

pub mod packet;
pub mod types;
pub mod error;

#[derive(Debug)]
pub enum RtmpMessage {
    Amf0Command {
        packet: packet::Amf0CommandPacket,
    },
    Amf0Data {
        packet: packet::Amf0DataPacket,
    },
    UserControl {
        event_type: u16,
        event_data: u32,
        extra_data: u32,
    },
    SetWindowAckSize{
        ack_window_size: u32,
    },
    Acknowledgement{
        sequence_number: u32,
    },
    SetChunkSize{
        chunk_size: u32,
    },
    AudioData {
        header: MessageHeader,
        payload: Bytes,
    },
    VideoData {
        header: MessageHeader,
        payload: Bytes,
    },
    Abort {
        stream_id: u32,
    },
    SetPeerBandwidth {
        size: u32,
        limit_type: u8,
    },
    Unknown {
        type_id: u8,
        data: Bytes,
    },
}

pub fn decode(data: Bytes) -> Result<RtmpMessage, MessageDecodeError> {
    Ok(RtmpMessage::Unknown {
        type_id: 88,
        data,
    })
}

pub fn encode(msg: RtmpMessage) -> Result<Bytes, MessageEncodeError> {
        println!("Lukie {:?}", msg);
        Ok(Bytes::new())
    }

#[cfg(test)]
mod tests {
    use amf::Amf0Value;

    use super::*;

    #[test]
    fn test1() {
        let p1 = decode(Bytes::new()).unwrap();
        let p2 = encode(p1).unwrap();
        print!("Lukie {:?}", p2);
    }

    #[test]
    fn test2() {
        let ca = packet::Amf0CommandPacket::ConnectApp {
            command_name: "connect_app".to_string(),
            transaction_id: 1.0,
            command_object: Amf0Value::Null,
            args: Amf0Value::Null,
        };
        let p1 = RtmpMessage::Amf0Command { packet: ca };
        print!("{:?}", p1)
    }
}
