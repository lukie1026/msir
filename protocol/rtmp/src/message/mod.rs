use std::io;

use crate::chunk::MessageHeader;
use amf::{Value, Version};
use bytes::Bytes;
use error::{MessageDecodeError, MessageEncodeError};
use tracing::{trace, info, error, info_span, instrument};

use self::types::msg_type::*;

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

pub fn decode(data: Bytes, mh: MessageHeader) -> Result<RtmpMessage, MessageDecodeError> {
    match mh.message_type {
        SET_CHUNK_SIZE => {
            trace!("Recv message <set_chunk_size>");
        },
        ABORT => {
            trace!("Recv message <abort>");
        },
        ACK => {
            trace!("Recv message <ack>");
        },
        USER_CONTROL => {
            trace!("Recv message <user_control>");
        },
        WIN_ACK_SIZE => {
            trace!("Recv message <win_ack_size>");
        },
        SET_PEER_BW => {
            trace!("Recv message <set_peer_bw>");
        },
        AUDIO => {
            trace!("Recv message <audio>");
        },
        VIDEO => {
            trace!("Recv message <video>");
        },
        AMF3_DATA => {
            trace!("Recv message <amf3_data>");
        },
        AMF3_SHARED_OBJ => {
            trace!("Recv message <amf3_shared_obj>");
        },
        AMF3_CMD | AMF0_CMD => {
            trace!("Recv message <amf_cmd>");
        },
        AMF0_DATA => {
            trace!("Recv message <amf0_data>");
        },
        AMF0_SHARED_OBJ => {
            trace!("Recv message <afm0_shared_obj>");
        },
        AGGREGATE => {
            trace!("Recv message <aggregate>");
        },
        other => {
            trace!("Recv message <unknow {}>", other);
        }
    }
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
        let p1 = decode(Bytes::new(), MessageHeader::default()).unwrap();
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
