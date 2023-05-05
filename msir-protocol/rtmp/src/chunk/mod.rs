use std::{
    cmp,
    collections::HashMap,
    io::{Cursor, IoSlice},
};

use byteorder::{BigEndian, LittleEndian, ReadBytesExt, WriteBytesExt};
use bytes::{Buf, Bytes, BytesMut};
use rml_amf0::Amf0Value;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};
use tracing::{error, info, instrument, trace, warn};

use crate::message::{decode, encode, types::msg_type::*, RtmpMessage, RtmpPayload};

use self::error::ChunkError;

pub mod error;

const RTMP_FMT_TYPE0: u8 = 0;
const RTMP_FMT_TYPE1: u8 = 1;
const RTMP_FMT_TYPE2: u8 = 2;
const RTMP_FMT_TYPE3: u8 = 3;
const RTMP_EXTENDED_TIMESTAMP: u32 = 0xFFFFFF;
const MH_SIZES: [u32; 4] = [11, 7, 3, 0];

// The chunk stream id used for some under-layer message,
// For example, the PC(protocol control) message.
const RTMP_CID_PROTOCOL_CONTROL: u32 = 0x02;
// The AMF0/AMF3 command message, invoke method and return the result, over NetConnection.
// generally use 0x03.
const RTMP_CID_OVER_CONNECTION: u32 = 0x03;
// The AMF0/AMF3 command message, invoke method and return the result, over NetConnection,
// The midst state(we guess).
// rarely used, e.g. onStatus(NetStream.Play.Reset).
const RTMP_CID_OVER_CONNECTION2: u32 = 0x04;
// The stream message(amf0/amf3), over NetStream.
// generally use 0x05.
const RTMP_CID_OVER_STREAM: u32 = 0x05;
// The stream message(amf0/amf3), over NetStream, the midst state(we guess).
// rarely used, e.g. play("mp4:mystram.f4v")
const RTMP_CID_OVER_STREAM2: u32 = 0x08;
// The stream message(video), over NetStream
// generally use 0x06.
const RTMP_CID_VIDEO: u32 = 0x06;
// The stream message(audio), over NetStream.
// generally use 0x07.
const RTMP_CID_AUDIO: u32 = 0x07;

type Result<T> = std::result::Result<T, ChunkError>;

#[derive(Debug, Clone)]
pub struct MessageHeader {
    pub timestamp_delta: u32,
    pub payload_length: usize,
    pub message_type: u8,
    pub stream_id: u32,
    pub timestamp: u32,
    pub perfer_cid: u32,
}

impl Default for MessageHeader {
    fn default() -> Self {
        MessageHeader {
            timestamp_delta: 0,
            payload_length: 0,
            message_type: 0,
            stream_id: 0,
            timestamp: 0,
            perfer_cid: RTMP_CID_PROTOCOL_CONTROL,
        }
    }
}

#[derive(Debug)]
pub struct ChunkStream {
    pub fmt: u8,
    pub csid: u32,
    pub header: MessageHeader,
    pub extended_timestamp: bool,
    pub payload: BytesMut,
    pub msg_count: u32,
}

impl ChunkStream {
    pub fn new(csid: u32) -> Self {
        let mut mh = MessageHeader::default();
        mh.perfer_cid = csid;
        Self {
            fmt: 0,
            csid,
            header: mh,
            extended_timestamp: false,
            payload: BytesMut::new(),
            msg_count: 0,
        }
    }
}

pub struct ChunkCodec {
    io: TcpStream,
    in_chunk_size: usize,
    out_chunk_size: usize,
    chunk_streams: HashMap<u32, ChunkStream>,
    chunk_header_cache: Vec<u8>,
}

impl ChunkCodec {
    pub fn new(io: TcpStream) -> Self {
        Self {
            io,
            in_chunk_size: 128,
            out_chunk_size: 128,
            chunk_streams: HashMap::new(),
            chunk_header_cache: Vec::with_capacity(16 * 128),
        }
    }
    pub fn set_in_chunk_size(&mut self, n: usize) {
        self.in_chunk_size = n;
    }
    pub fn set_out_chunk_size(&mut self, n: usize) {
        self.out_chunk_size = n;
    }
    pub async fn recv_rtmp_message(&mut self) -> Result<RtmpMessage> {
        loop {
            trace!("Receiving message...");
            let payload = self.recv_interlaced_message().await?;
            match payload {
                Some((b, mh)) => {
                    let data = RtmpPayload {
                        message_type: mh.message_type,
                        csid: mh.stream_id,
                        timestamp: mh.timestamp,
                        raw_data: b,
                    };
                    let msg = decode(data)?;
                    return Ok(msg);
                }
                None => continue,
            }
        }
    }
    pub async fn send_rtmp_message(&mut self, msg: RtmpPayload) -> Result<()> {
        let msgs = [msg];
        self.send_rtmp_messages(&msgs[0..1]).await
    }
    pub async fn send_rtmp_messages(&mut self, msgs: &[RtmpPayload]) -> Result<()> {
        for (_, msg) in msgs.into_iter().enumerate() {
            if msg.raw_data.is_empty() {
                continue;
            }
            let mut init = true;
            let total = msg.raw_data.len();
            let mut sent = 0_usize;
            loop {
                let length = cmp::min(total - sent, self.out_chunk_size);
                let (s, e) = self.add_chunk_header(msg, sent == 0, init)?;
                let mut write_array: Vec<IoSlice> = Vec::with_capacity(2);
                {
                    write_array.push(IoSlice::new(&self.chunk_header_cache[s..e]));
                    write_array.push(IoSlice::new(&msg.raw_data[sent..(sent + length)]));
                }
                trace!("Lukie write vectored len {}", write_array.len());
                self.io.write_vectored(&write_array).await?;

                init = false;
                sent += length;
                if sent >= total {
                    break;
                }
            }
        }
        Ok(())
    }
    fn add_chunk_header(
        &mut self,
        msg: &RtmpPayload,
        c0: bool,
        need_clear: bool,
    ) -> Result<(usize, usize)> {
        if need_clear {
            self.chunk_header_cache.clear();
        }
        let start = self.chunk_header_cache.len();
        let perfer_cid = get_perfer_cid(msg.message_type) as u8;
        if c0 {
            let basic_header = (RTMP_FMT_TYPE0 << 6) | (perfer_cid & 0x3F);
            let mut hlen = 1 + 3 + 3 + 1 + 4;
            WriteBytesExt::write_u8(&mut self.chunk_header_cache, basic_header)?;
            if msg.timestamp < RTMP_EXTENDED_TIMESTAMP {
                self.chunk_header_cache
                    .write_u24::<BigEndian>(msg.timestamp)?;
            } else {
                self.chunk_header_cache
                    .write_u24::<BigEndian>(RTMP_EXTENDED_TIMESTAMP)?;
            }
            self.chunk_header_cache
                .write_u24::<BigEndian>(msg.raw_data.len() as u32)?;
            WriteBytesExt::write_u8(&mut self.chunk_header_cache, msg.message_type)?;
            WriteBytesExt::write_u32::<LittleEndian>(&mut self.chunk_header_cache, msg.csid)?;
            if msg.timestamp >= RTMP_EXTENDED_TIMESTAMP {
                WriteBytesExt::write_u32::<BigEndian>(&mut self.chunk_header_cache, msg.timestamp)?;
                hlen += 4;
            }
            Ok((start, start + hlen))
        } else {
            let basic_header = (RTMP_FMT_TYPE3 << 6) | (perfer_cid & 0x3F);
            let mut hlen = 1;
            WriteBytesExt::write_u8(&mut self.chunk_header_cache, basic_header)?;
            if msg.timestamp >= RTMP_EXTENDED_TIMESTAMP {
                WriteBytesExt::write_u32::<BigEndian>(&mut self.chunk_header_cache, msg.timestamp)?;
                hlen += 4;
            }
            Ok((start, start + hlen))
        }
    }

    async fn read_basic_header(&mut self) -> Result<(u8, u32)> {
        let head = self.io.read_u8().await?;
        let csid = head & 0x3f;
        let fmt = (head >> 6) & 0x03;
        if csid > 1 {
            Ok((fmt, csid as u32))
        } else if csid == 0 {
            let mut csid = 64_u32;
            csid += self.io.read_u8().await? as u32;
            Ok((fmt, csid))
        } else {
            let mut csid = 64_u32;
            csid += self.io.read_u8().await? as u32;
            csid += self.io.read_u8().await? as u32 * 256;
            Ok((fmt, csid))
        }
    }

    async fn read_message_header(&mut self, chunk: &mut ChunkStream, fmt: u8) -> Result<()> {
        let first_chunk_of_msg = chunk.payload.len() == 0;
        if chunk.msg_count == 0 && fmt != RTMP_FMT_TYPE0 {
            if fmt == RTMP_FMT_TYPE1 {
                warn!("Fresh chunk start wiht fmt=1");
            } else {
                return Err(ChunkError::InvalidFmtRule1(fmt, chunk.csid));
            }
        }
        // when exists cache msg, means got an partial message,
        // the fmt must not be type0 which means new message.
        if !first_chunk_of_msg && fmt == RTMP_FMT_TYPE0 {
            return Err(ChunkError::InvalidFmtRule2(fmt, chunk.csid));
        }

        let mh_size = MH_SIZES[fmt as usize] as usize;
        let mut mh = BytesMut::with_capacity(mh_size);
        mh.resize(mh_size, 0);
        self.io.read_exact(&mut mh).await?;
        if fmt <= RTMP_FMT_TYPE2 {
            chunk.header.timestamp_delta = Cursor::new(mh.split_to(3)).read_u24::<BigEndian>()?;
            chunk.extended_timestamp = chunk.header.timestamp_delta >= RTMP_EXTENDED_TIMESTAMP;
            if !chunk.extended_timestamp {
                if fmt == RTMP_FMT_TYPE0 {
                    chunk.header.timestamp = chunk.header.timestamp_delta;
                } else {
                    chunk.header.timestamp += chunk.header.timestamp_delta;
                }
            }
            if fmt <= RTMP_FMT_TYPE1 {
                let payload_length = Cursor::new(mh.split_to(3)).read_u24::<BigEndian>()? as usize;
                if !first_chunk_of_msg && chunk.header.payload_length != payload_length {
                    return Err(ChunkError::InvalidMsgLengthRule1(
                        chunk.header.payload_length,
                        payload_length,
                    ));
                }

                chunk.header.payload_length = payload_length;
                chunk.payload.reserve(payload_length);
                chunk.header.message_type = mh.get_u8();

                if fmt == RTMP_FMT_TYPE0 {
                    chunk.header.stream_id = mh.get_u32_le();
                }
            }
        } else {
            // update the timestamp even fmt=3 for first chunk packet
            if first_chunk_of_msg && !chunk.extended_timestamp {
                chunk.header.timestamp += chunk.header.timestamp_delta;
            }
        }

        // read extended-timestamp
        if chunk.extended_timestamp {
            let timestamp = self.io.read_u32().await?;
            if !first_chunk_of_msg
                && chunk.header.timestamp > 0
                && timestamp != chunk.header.timestamp
            {
                return Err(ChunkError::InvalidExTimestamp);
            } else {
                chunk.header.timestamp = timestamp;
            }
        }
        chunk.header.timestamp &= 0x7fffffff;

        chunk.msg_count += 1;
        Ok(())
    }

    async fn read_message_payload(&mut self, chunk: &mut ChunkStream) -> Result<Option<Bytes>> {
        // empty message
        if chunk.header.payload_length <= 0 {
            trace!(
                "Get an empty RTMP message(type={})",
                chunk.header.message_type
            );
            return Ok(Some(chunk.payload.split().into()));
        }

        // the chunk payload size.
        let mut payload_size = chunk.header.payload_length - chunk.payload.len();
        payload_size = cmp::min(payload_size, self.in_chunk_size);

        // create msg payload if not initialized
        let mut buffer = Vec::<u8>::with_capacity(payload_size);
        unsafe { buffer.set_len(payload_size) };
        self.io.read_exact(&mut buffer).await?;

        // read payload to buffer
        chunk.payload.extend_from_slice(&buffer);

        // got entire RTMP message?
        if chunk.header.payload_length == chunk.payload.len() {
            trace!(
                "Reading payload finish, read={}, total={}",
                chunk.payload.len(),
                chunk.header.payload_length
            );
            return Ok(Some(chunk.payload.split().into()));
        }

        trace!(
            "Read payload continue, read={}, total={}",
            chunk.payload.len(),
            chunk.header.payload_length
        );
        Ok(None)
    }

    async fn recv_interlaced_message(&mut self) -> Result<Option<(Bytes, MessageHeader)>> {
        let (fmt, csid) = self.read_basic_header().await?;
        let mut chunk = match self.chunk_streams.remove_entry(&csid) {
            Some((_, v)) => v,
            None => ChunkStream::new(csid),
        };
        trace!(
            "Read basic header, fmt={} csid={}, chunk{:?}",
            fmt,
            csid,
            chunk
        );
        self.read_message_header(&mut chunk, fmt).await?;
        trace!(
            "Read message header, fmt={} csid={}, chunk{:?}",
            fmt,
            csid,
            chunk
        );
        let payload = self.read_message_payload(&mut chunk).await;
        trace!("Read message payload, {:?}", payload);
        let mh = chunk.header.clone();
        self.chunk_streams.insert(csid, chunk);

        match payload {
            Ok(p) => match p {
                Some(b) => Ok(Some((b, mh))),
                None => Ok(None),
            },
            Err(e) => Err(e),
        }
    }

    // FIXME: test
    pub fn handle_connect(&mut self, msg: RtmpMessage) -> (String, bool) {
        match msg {
            RtmpMessage::Amf0Command {
                command_name,
                command_object,
                transaction_id,
                additional_arguments,
            } => {
                if command_name != crate::message::types::amf0_command_type::COMMAND_CONNECT {
                    return ("notfound1".to_string(), false);
                }
                let mut properties = match command_object {
                    Amf0Value::Object(properties) => properties,
                    _ => return ("invalid".to_string(), false),
                };

                let app_name = match properties.remove("app") {
                    Some(value) => match value {
                        Amf0Value::Utf8String(mut app) => {
                            if app.ends_with("/") {
                                app.pop();
                            }

                            return (app, true);
                        }
                        _ => return ("invalid2".to_string(), false),
                    },
                    None => return ("invalid3".to_string(), false),
                };
            }
            _ => ("notfound2".to_string(), false),
        }
    }

    // FIXME: test
    pub async fn relay_connect(&mut self, app: String) -> Result<()> {
        let mut command_object_properties = HashMap::new();
        command_object_properties.insert(
            "fmsVer".to_string(),
            Amf0Value::Utf8String("FMS/3,0,1,1233".to_string()),
        );
        command_object_properties.insert("capabilities".to_string(), Amf0Value::Number(31.0));

        let description = "Successfully connected on app: ".to_string() + &app;
        let mut additional_properties = create_status_object(
            "status",
            "NetConnection.Connect.Success",
            description.as_ref(),
        );
        additional_properties.insert("objectEncoding".to_string(), Amf0Value::Number(0.0));

        let message = RtmpMessage::Amf0Command {
            command_name: "_result".to_string(),
            transaction_id: 1.0,
            command_object: Amf0Value::Object(command_object_properties),
            additional_arguments: vec![Amf0Value::Object(additional_properties)],
        };

        let payload = encode(message, 0, 0)?;
        // let payload = message.into_message_payload(self.get_epoch(), 0)?;
        // let packet = self.serializer.serialize(&payload, false, false)?;

        // Ok(vec![ServerSessionResult::OutboundResponse(packet)])

        self.send_rtmp_message(payload).await
    }
}

fn get_perfer_cid(typ: u8) -> u32 {
    match typ {
        SET_CHUNK_SIZE | ABORT | ACK | USER_CONTROL | WIN_ACK_SIZE | SET_PEER_BW => {
            RTMP_CID_PROTOCOL_CONTROL
        }
        AMF0_DATA | AMF0_SHARED_OBJ | AMF0_CMD | AMF3_DATA | AMF3_SHARED_OBJ | AMF3_CMD => {
            RTMP_CID_OVER_CONNECTION
        }
        AUDIO => RTMP_CID_AUDIO,
        VIDEO => RTMP_CID_VIDEO,
        _ => RTMP_CID_PROTOCOL_CONTROL,
    }
}

// FIXME: test
fn create_status_object(level: &str, code: &str, description: &str) -> HashMap<String, Amf0Value> {
    let mut properties = HashMap::new();
    properties.insert(
        "level".to_string(),
        Amf0Value::Utf8String(level.to_string()),
    );
    properties.insert("code".to_string(), Amf0Value::Utf8String(code.to_string()));
    properties.insert(
        "description".to_string(),
        Amf0Value::Utf8String(description.to_string()),
    );
    properties
}