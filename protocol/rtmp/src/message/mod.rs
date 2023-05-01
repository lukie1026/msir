use crate::chunk::MessageHeader;
use crate::message::packet::Amf0CommandPacket;
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use bytes::{Buf, Bytes};
use error::{MessageDecodeError, MessageEncodeError};
use rml_amf0;
use rml_amf0::Amf0Value;
use std::collections::HashMap;
use std::io::Cursor;
use std::io::{self, BufRead};
use tracing::{error, info, info_span, instrument, trace};

use self::types::*;

pub mod error;
pub mod packet;
pub mod types;
pub mod request;

#[derive(Debug)]
pub struct RtmpPayload {
    pub message_type: u8,
    pub csid: u32,
    pub timestamp: u32,
    pub raw_data: Bytes,
}

#[derive(Debug)]
pub enum RtmpMessage {
    Amf0Command {
        // packet: packet::Amf0CommandPacket,
        command_name: String,
        transaction_id: f64,
        command_object: Amf0Value,
        additional_arguments: Vec<Amf0Value>,
    },
    Amf0Data {
        // packet: packet::Amf0DataPacket,
        // FIXME: need to add command_name?
        values: Vec<Amf0Value>,
    },
    UserControl {
        event_type: u16,
        event_data: u32,
        extra_data: u32,
    },
    SetWindowAckSize {
        ack_window_size: u32,
    },
    Acknowledgement {
        sequence_number: u32,
    },
    SetChunkSize {
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

impl RtmpMessage {
    // FIXME: need to judge Amf0Data?
    pub fn expect_amf(&self, specified_cmds: &[&str]) -> bool {
        if let RtmpMessage::Amf0Command { command_name, .. } = self {
            for cmd in specified_cmds {
                if command_name == *cmd {
                    return true;
                }
            }
        }
        return false;
    }
}

pub fn decode(payload: RtmpPayload) -> Result<RtmpMessage, MessageDecodeError> {
    match payload.message_type {
        msg_type::SET_CHUNK_SIZE => {
            trace!("Recv message <set_chunk_size>");
            let mut cursor = Cursor::new(payload.raw_data);
            let chunk_size = cursor.read_u32::<BigEndian>()?;

            return Ok(RtmpMessage::SetChunkSize { chunk_size });
        }
        msg_type::ABORT => {
            trace!("Recv message <abort>");
            let mut cursor = Cursor::new(payload.raw_data);
            let stream_id = cursor.read_u32::<BigEndian>()?;

            return Ok(RtmpMessage::Abort { stream_id });
        }
        msg_type::ACK => {
            trace!("Recv message <ack>");
            let mut cursor = Cursor::new(payload.raw_data);
            let sequence_number = cursor.read_u32::<BigEndian>()?;

            return Ok(RtmpMessage::Acknowledgement { sequence_number });
        }
        msg_type::USER_CONTROL => {
            trace!("Recv message <user_control>");
            let mut cursor = Cursor::new(payload.raw_data);
            let mut extra_data: u32 = 0;
            let event_type: u16 = cursor.read_u16::<BigEndian>()?;
            let event_data = cursor.read_u32::<BigEndian>()?;
            if event_type == user_ctrl_ev_type::SET_BUFFER_LENGTH {
                extra_data = cursor.read_u32::<BigEndian>()?;
            }

            return Ok(RtmpMessage::UserControl {
                event_type,
                event_data,
                extra_data,
            });
        }
        msg_type::WIN_ACK_SIZE => {
            trace!("Recv message <win_ack_size>");
            let mut cursor = Cursor::new(payload.raw_data);
            let ack_window_size = cursor.read_u32::<BigEndian>()?;

            return Ok(RtmpMessage::SetWindowAckSize { ack_window_size });
        }
        msg_type::SET_PEER_BW => {
            trace!("Recv message <set_peer_bw>");
            let mut cursor = Cursor::new(payload.raw_data);
            let size = cursor.read_u32::<BigEndian>()?;
            let limit_type = cursor.read_u8()?;

            return Ok(RtmpMessage::SetPeerBandwidth { size, limit_type });
        }
        msg_type::AUDIO => {
            trace!("Recv message <audio>");
        }
        msg_type::VIDEO => {
            trace!("Recv message <video>");
        }
        msg_type::AGGREGATE => {
            trace!("Recv message <aggregate>");
        }
        msg_type::AMF3_SHARED_OBJ | msg_type::AMF0_SHARED_OBJ => {
            trace!("Recv message <amf_shared_obj>");
        }
        msg_type::AMF3_DATA | msg_type::AMF0_DATA => {
            trace!("Recv message <amf_data>");
            let mut cursor = Cursor::new(payload.raw_data);
            let values = rml_amf0::deserialize(&mut cursor)?;

            return Ok(RtmpMessage::Amf0Data { values });
        }
        msg_type::AMF3_CMD | msg_type::AMF0_CMD => {
            trace!("Recv message <amf_cmd>");
            let mut cursor = Cursor::new(payload.raw_data);
            if payload.message_type == msg_type::AMF3_CMD {
                cursor.advance(1);
            }
            let mut arguments = rml_amf0::deserialize(&mut cursor)?;

            let command_name: String;
            let transaction_id: f64;
            let command_object: Amf0Value;
            {
                let mut arg_iterator = arguments.drain(..3);

                command_name = match arg_iterator
                    .next()
                    .ok_or(MessageDecodeError::InvalidFormat("command".to_string()))?
                {
                    Amf0Value::Utf8String(value) => value,
                    _ => return Err(MessageDecodeError::InvalidFormat("command".to_string())),
                };

                transaction_id =
                    match arg_iterator
                        .next()
                        .ok_or(MessageDecodeError::InvalidFormat(
                            "transcation_id".to_string(),
                        ))? {
                        Amf0Value::Number(value) => value,
                        _ => {
                            return Err(MessageDecodeError::InvalidFormat(
                                "transcation_id".to_string(),
                            ))
                        }
                    };

                command_object = arg_iterator
                    .next()
                    .ok_or(MessageDecodeError::InvalidFormat("command_obj".to_string()))?;
            }

            return Ok(RtmpMessage::Amf0Command {
                command_name,
                transaction_id,
                command_object,
                additional_arguments: arguments,
            });
        }
        other => {
            trace!("Recv message <unknow {}>", other);
        }
    }
    Ok(RtmpMessage::Unknown {
        type_id: payload.message_type,
        data: payload.raw_data,
    })
}

pub fn encode(
    msg: RtmpMessage,
    timestamp: u32,
    csid: u32,
) -> Result<RtmpPayload, MessageEncodeError> {
    match msg {
        RtmpMessage::Amf0Command {
            command_name,
            transaction_id,
            command_object,
            mut additional_arguments,
        } => {
            let mut values = vec![
                Amf0Value::Utf8String(command_name),
                Amf0Value::Number(transaction_id),
                command_object,
            ];

            values.append(&mut additional_arguments);
            let bytes = rml_amf0::serialize(&values)?;
            Ok(RtmpPayload {
                message_type: msg_type::AMF0_CMD,
                csid,
                timestamp,
                raw_data: Bytes::from(bytes),
            })
        }
        RtmpMessage::Amf0Data { values } => {
            let bytes = rml_amf0::serialize(&values)?;

            Ok(RtmpPayload {
                message_type: msg_type::AMF0_DATA,
                csid,
                timestamp,
                raw_data: Bytes::from(bytes),
            })
        }
        RtmpMessage::UserControl {
            event_type,
            event_data,
            extra_data,
        } => {
            let mut cursor = Cursor::new(Vec::new());
            cursor.write_u16::<BigEndian>(event_type)?;
            cursor.write_u32::<BigEndian>(event_data)?;
            if event_type == user_ctrl_ev_type::SET_BUFFER_LENGTH {
                cursor.write_u32::<BigEndian>(extra_data)?;
            }
            Ok(RtmpPayload {
                message_type: msg_type::USER_CONTROL,
                csid,
                timestamp,
                raw_data: Bytes::from(cursor.into_inner()),
            })
        }
        RtmpMessage::SetWindowAckSize { ack_window_size } => Ok(RtmpPayload {
            message_type: msg_type::WIN_ACK_SIZE,
            csid,
            timestamp,
            raw_data: fast_u32_encode(ack_window_size)?,
        }),
        RtmpMessage::Acknowledgement { sequence_number } => Ok(RtmpPayload {
            message_type: msg_type::ACK,
            csid,
            timestamp,
            raw_data: fast_u32_encode(sequence_number)?,
        }),
        RtmpMessage::SetChunkSize { chunk_size } => Ok(RtmpPayload {
            message_type: msg_type::SET_CHUNK_SIZE,
            csid,
            timestamp,
            raw_data: fast_u32_encode(chunk_size)?,
        }),
        RtmpMessage::AudioData { header, payload } => Ok(RtmpPayload {
            message_type: msg_type::AUDIO,
            csid,
            timestamp,
            raw_data: payload,
        }),
        RtmpMessage::VideoData { header, payload } => Ok(RtmpPayload {
            message_type: msg_type::VIDEO,
            csid,
            timestamp,
            raw_data: payload,
        }),
        RtmpMessage::Abort { stream_id } => Ok(RtmpPayload {
            message_type: msg_type::ABORT,
            csid,
            timestamp,
            raw_data: fast_u32_encode(stream_id)?,
        }),
        RtmpMessage::SetPeerBandwidth { size, limit_type } => {
            let mut cursor = Cursor::new(Vec::new());
            cursor.write_u32::<BigEndian>(size)?;
            cursor.write_u8(limit_type)?;
            Ok(RtmpPayload {
                message_type: msg_type::SET_PEER_BW,
                csid,
                timestamp,
                raw_data: Bytes::from(cursor.into_inner()),
            })
        }
        RtmpMessage::Unknown { type_id, data } => Ok(RtmpPayload {
            message_type: type_id,
            csid,
            timestamp,
            raw_data: data,
        }),
    }
}

fn fast_u32_encode(value: u32) -> Result<Bytes, MessageEncodeError> {
    let mut cursor = Cursor::new(Vec::new());
    cursor.write_u32::<BigEndian>(value)?;

    Ok(Bytes::from(cursor.into_inner()))
}

#[cfg(test)]
mod tests {
    // use amf::Amf0Value;

    // use super::*;

    // #[test]
    // fn test1() {
    //     let p1 = decode(Bytes::new(), MessageHeader::default()).unwrap();
    //     let p2 = encode(p1).unwrap();
    //     print!("Lukie {:?}", p2);
    // }

    // #[test]
    // fn test2() {
    //     let ca = packet::Amf0CommandPacket::ConnectApp {
    //         command_name: "connect_app".to_string(),
    //         transaction_id: 1.0,
    //         command_object: Amf0Value::Null,
    //         args: Amf0Value::Null,
    //     };
    //     let p1 = RtmpMessage::Amf0Command { packet: ca };
    //     print!("{:?}", p1)
    // }
}
