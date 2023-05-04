use crate::chunk::MessageHeader;

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use bytes::{Buf, Bytes};
use error::{MessageDecodeError, MessageEncodeError};
use rml_amf0;
use rml_amf0::Amf0Value;

use std::{collections::HashMap, io::Cursor};

use tracing::{error, info, info_span, instrument, trace};

use self::types::{amf0_command_type::*, rtmp_sig::*, rtmp_status::*, *};

pub mod error;
pub mod packet;
pub mod request;
pub mod types;

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
        // header: MessageHeader,
        stream_id: u32,
        timestamp: u32,
        payload: Bytes,
    },
    VideoData {
        // header: MessageHeader,
        stream_id: u32,
        timestamp: u32,
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
    pub fn new_null(transaction_id: f64) -> Self {
        return RtmpMessage::Amf0Command {
            command_name: "".to_string(),
            transaction_id,
            command_object: Amf0Value::Null,
            additional_arguments: vec![],
        };
    }
    pub fn new_create_stream_res(transaction_id: f64) -> Self {
        return RtmpMessage::Amf0Command {
            command_name: COMMAND_RESULT.to_string(),
            transaction_id,
            command_object: Amf0Value::Null,
            additional_arguments: vec![Amf0Value::Number(DEFAULT_SID)],
        };
    }
    pub fn new_release_stream_res(transaction_id: f64) -> Self {
        return RtmpMessage::Amf0Command {
            command_name: COMMAND_RESULT.to_string(),
            transaction_id,
            command_object: Amf0Value::Null,
            additional_arguments: vec![Amf0Value::Undefined],
        };
    }
    pub fn new_fcpublish_res(transaction_id: f64) -> Self {
        return RtmpMessage::Amf0Command {
            command_name: COMMAND_RESULT.to_string(),
            transaction_id,
            command_object: Amf0Value::Null,
            additional_arguments: vec![Amf0Value::Undefined],
        };
    }
    pub fn new_sample_access() -> Self {
        return RtmpMessage::Amf0Data {
            values: vec![
                Amf0Value::Utf8String(DATA_SAMPLE_ACCESS.to_string()),
                Amf0Value::Boolean(true),
                Amf0Value::Boolean(true),
            ],
        };
    }
    pub fn new_on_fcpublish() -> Self {
        return RtmpMessage::Amf0Command {
            command_name: COMMAND_ON_FC_PUBLISH.to_string(),
            transaction_id: 0.0,
            command_object: Amf0Value::Null,
            additional_arguments: vec![fast_create_amf0_obj(vec![
                (
                    STATUS_CODE,
                    Amf0Value::Utf8String(STATUS_CODE_PUBLISH_START.to_string()),
                ),
                (
                    STATUS_DESCRIPTION,
                    Amf0Value::Utf8String("Started publishing stream.".to_string()),
                ),
            ])],
        };
    }
    pub fn new_on_status_publish_start() -> Self {
        return RtmpMessage::Amf0Command {
            command_name: COMMAND_ON_STATUS.to_string(),
            transaction_id: 0.0,
            command_object: Amf0Value::Null,
            additional_arguments: vec![fast_create_amf0_obj(vec![
                (
                    STATUS_LEVEL,
                    Amf0Value::Utf8String(STATUS_LEVEL_STATUS.to_string()),
                ),
                (
                    STATUS_CODE,
                    Amf0Value::Utf8String(STATUS_CODE_PUBLISH_START.to_string()),
                ),
                (
                    STATUS_DESCRIPTION,
                    Amf0Value::Utf8String("Started publishing stream.".to_string()),
                ),
                (
                    STATUS_CLIENT_ID,
                    Amf0Value::Utf8String(RTMP_SIG_CLIENT_ID.to_string()),
                ),
            ])],
        };
    }
    pub fn new_on_status_play_reset() -> Self {
        return RtmpMessage::Amf0Command {
            command_name: COMMAND_ON_STATUS.to_string(),
            transaction_id: 0.0,
            command_object: Amf0Value::Null,
            additional_arguments: vec![fast_create_amf0_obj(vec![
                (
                    STATUS_LEVEL,
                    Amf0Value::Utf8String(STATUS_LEVEL_STATUS.to_string()),
                ),
                (
                    STATUS_CODE,
                    Amf0Value::Utf8String(STATUS_CODE_STREAM_RESET.to_string()),
                ),
                (
                    STATUS_DESCRIPTION,
                    Amf0Value::Utf8String("Playing and resetting stream.".to_string()),
                ),
                (STATUS_DETAILS, Amf0Value::Utf8String("stream".to_string())),
                (
                    STATUS_CLIENT_ID,
                    Amf0Value::Utf8String(RTMP_SIG_CLIENT_ID.to_string()),
                ),
            ])],
        };
    }
    pub fn new_on_status_play_start() -> Self {
        return RtmpMessage::Amf0Command {
            command_name: COMMAND_ON_STATUS.to_string(),
            transaction_id: 0.0,
            command_object: Amf0Value::Null,
            additional_arguments: vec![fast_create_amf0_obj(vec![
                (
                    STATUS_LEVEL,
                    Amf0Value::Utf8String(STATUS_LEVEL_STATUS.to_string()),
                ),
                (
                    STATUS_CODE,
                    Amf0Value::Utf8String(STATUS_CODE_STREAM_START.to_string()),
                ),
                (
                    STATUS_DESCRIPTION,
                    Amf0Value::Utf8String("Started playing stream.".to_string()),
                ),
                (STATUS_DETAILS, Amf0Value::Utf8String("stream".to_string())),
                (
                    STATUS_CLIENT_ID,
                    Amf0Value::Utf8String(RTMP_SIG_CLIENT_ID.to_string()),
                ),
            ])],
        };
    }
    pub fn new_on_status_data_start() -> Self {
        return RtmpMessage::Amf0Data {
            values: vec![
                Amf0Value::Utf8String(COMMAND_ON_STATUS.to_string()),
                fast_create_amf0_obj(vec![(
                    STATUS_CODE,
                    Amf0Value::Utf8String(STATUS_CODE_DATA_START.to_string()),
                )]),
            ],
        };
    }
    pub fn new_connect_app_res(object_encoding: f64) -> Self {
        return RtmpMessage::Amf0Command {
            command_name: COMMAND_RESULT.to_string(),
            transaction_id: 1.0,
            command_object: fast_create_amf0_obj(vec![
                (
                    "fmsVer",
                    Amf0Value::Utf8String(RTMP_SIG_FMS_VER.to_string()),
                ),
                ("capabilities", Amf0Value::Number(127.0)),
                ("mode", Amf0Value::Number(1.0)),
            ]),
            additional_arguments: vec![fast_create_amf0_obj(vec![
                (
                    STATUS_LEVEL,
                    Amf0Value::Utf8String(STATUS_LEVEL_STATUS.to_string()),
                ),
                (
                    STATUS_CODE,
                    Amf0Value::Utf8String(STATUS_CODE_CONNECT_SUCCESS.to_string()),
                ),
                (
                    STATUS_DESCRIPTION,
                    Amf0Value::Utf8String("Connection succeeded".to_string()),
                ),
                ("objectEncoding", Amf0Value::Number(object_encoding)),
                (
                    "data",
                    fast_create_amf0_obj(vec![
                        ("msir_version", Amf0Value::Utf8String("v0.1.0".to_string())),
                        ("msir_auther", Amf0Value::Utf8String("Lukie".to_string())),
                    ]),
                ),
            ])],
        };
    }

    // FIXME: need to judge Amf0Data?
    pub fn expect_amf(&self, specified_cmds: &[&str]) -> bool {
        let all_cmds = specified_cmds.len() == 0;
        if let RtmpMessage::Amf0Command { command_name, .. } = self {
            if all_cmds {
                return true;
            }
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
            return Ok(RtmpMessage::AudioData {
                stream_id: payload.csid,
                timestamp: payload.timestamp,
                payload: payload.raw_data,
            });
        }
        msg_type::VIDEO => {
            trace!("Recv message <video>");
            return Ok(RtmpMessage::VideoData {
                stream_id: payload.csid,
                timestamp: payload.timestamp,
                payload: payload.raw_data,
            });
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
            let cmd = match command_name.is_empty() {
                true => Amf0Value::Null,
                false => Amf0Value::Utf8String(command_name),
            };
            let mut values = vec![cmd, Amf0Value::Number(transaction_id), command_object];

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
        RtmpMessage::AudioData {
            payload,
            stream_id,
            timestamp,
        } => Ok(RtmpPayload {
            message_type: msg_type::AUDIO,
            csid: stream_id,
            timestamp,
            raw_data: payload,
        }),
        RtmpMessage::VideoData {
            payload,
            stream_id,
            timestamp,
        } => Ok(RtmpPayload {
            message_type: msg_type::VIDEO,
            csid: stream_id,
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

fn fast_create_amf0_obj(values: Vec<(&str, Amf0Value)>) -> Amf0Value {
    let mut map = HashMap::new();
    for (k, v) in values {
        map.insert(k.to_string(), v);
    }
    Amf0Value::Object(map)
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
