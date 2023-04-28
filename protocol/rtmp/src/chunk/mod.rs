use std::{cmp, collections::HashMap, io::Cursor};

use byteorder::{BigEndian, ReadBytesExt};
use bytes::{buf, Buf, Bytes, BytesMut};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};
use tracing::{error, info, instrument, trace, warn};

use crate::message::{RtmpMessage, decode};

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
const RTMP_CID_ProtocolControl: u32 = 0x02;
// The AMF0/AMF3 command message, invoke method and return the result, over NetConnection.
// generally use 0x03.
const RTMP_CID_OverConnection: u32 = 0x03;
// The AMF0/AMF3 command message, invoke method and return the result, over NetConnection,
// The midst state(we guess).
// rarely used, e.g. onStatus(NetStream.Play.Reset).
const RTMP_CID_OverConnection2: u32 = 0x04;
// The stream message(amf0/amf3), over NetStream.
// generally use 0x05.
const RTMP_CID_OverStream: u32 = 0x05;
// The stream message(amf0/amf3), over NetStream, the midst state(we guess).
// rarely used, e.g. play("mp4:mystram.f4v")
const RTMP_CID_OverStream2: u32 = 0x08;
// The stream message(video), over NetStream
// generally use 0x06.
const RTMP_CID_Video: u32 = 0x06;
// The stream message(audio), over NetStream.
// generally use 0x07.
const RTMP_CID_Audio: u32 = 0x07;

type Result<T> = std::result::Result<T, ChunkError>;

#[derive(Debug)]
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
            perfer_cid: RTMP_CID_ProtocolControl,
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
}

impl ChunkCodec {
    pub fn new(io: TcpStream) -> Self {
        Self {
            io,
            in_chunk_size: 128,
            out_chunk_size: 128,
            chunk_streams: HashMap::new(),
        }
    }
    pub async fn recv_rtmp_message(&mut self) -> Result<Option<RtmpMessage>> {
        loop {
            let payload = self.recv_interlaced_message().await?;
            match payload {
                Some(p) => {
                    let msg = decode(p)?;
                    return Ok(Some(msg));
                },
                None => continue,
            }
        }
    }
    pub fn send_rtmp_message(&mut self, msg: RtmpMessage) -> Result<()> {
        Ok(())
    }
    pub fn send_rtmp_messages(&mut self, msgs: &[RtmpMessage]) -> Result<()> {
        Ok(())
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
                    chunk.header.stream_id = mh.get_u32();
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
        let mut buffer = Vec::<u8>::with_capacity(payload_size as usize);
        self.io.read_exact(&mut buffer).await?;

        // read payload to buffer
        chunk.payload.extend_from_slice(&buffer);

        // got entire RTMP message?
        if chunk.header.payload_length == chunk.payload.len() {
            return Ok(Some(chunk.payload.split().into()));
        }

        Ok(None)
    }

    async fn recv_interlaced_message(&mut self) -> Result<Option<Bytes>> {
        let (fmt, csid) = self.read_basic_header().await?;
        let mut chunk =  match self
            .chunk_streams
            .remove_entry(&csid) {
                Some((_, v)) => v,
                None => ChunkStream::new(csid),
            };
        self.read_message_header(&mut chunk, fmt).await?;
        let payload = self.read_message_payload(&mut chunk).await;
        self.chunk_streams.insert(csid, chunk);
        payload
    }
}
