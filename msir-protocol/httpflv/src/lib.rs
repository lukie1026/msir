use byteorder::{BigEndian, WriteBytesExt};
use bytes::Bytes;
use error::FlvMuxerError;
use rtmp::message::{encode, RtmpMessage};

pub mod error;

const FLV_HEADER: [u8; 9] = [
    0x46, // 'F'
    0x4c, // 'L'
    0x56, // 'V'
    0x01, // version
    0x05, // audio and video tag
    0x00, 0x00, 0x00, 0x09, // header size
];

// 8 = audio
const FRAME_TYPE_AUDIO: u8 = 8;
// 8 = video
const FRAME_TYPE_VIDEO: u8 = 9;
// 18 = script data
const FRAME_TYPE_SCRIPT: u8 = 18;

const FLV_TAG_HEADER_SIZE: usize = 11;

pub struct FlvTransmuxer {
    header_written: bool,
    send_bytes: u64,
    audio_count: u64,
    video_count: u64,
}

impl FlvTransmuxer {
    pub fn new() -> Self {
        Self {
            header_written: false,
            send_bytes: 0,
            audio_count: 0,
            video_count: 0,
        }
    }

    pub fn write_tags(
        &mut self,
        msgs: &[RtmpMessage],
        total: usize,
    ) -> Result<Vec<u8>, FlvMuxerError> {
        let mut cache = Vec::with_capacity(total + msgs.len() * 15 + 20);

        // Write FLV header first
        if !self.header_written {
            // FLV header
            cache.extend_from_slice(&FLV_HEADER);
            // Previous tag header
            cache.write_u32::<BigEndian>(0)?;
            self.header_written = true;
        }

        // Write frames
        for msg in msgs {
            match msg {
                RtmpMessage::AudioData {
                    payload, timestamp, ..
                } => {
                    self.write_audio(&mut cache, payload, *timestamp)?;
                    self.audio_count += 1;
                }
                RtmpMessage::VideoData {
                    payload, timestamp, ..
                } => {
                    self.write_video(&mut cache, payload, *timestamp)?;
                    self.video_count += 1;
                }
                RtmpMessage::Amf0Data { .. } => {
                    if msg.is_metadata() {
                        let data = encode(msg.clone(), 0, 0)?;
                        self.write_metadata(&mut cache, &data.raw_data)?
                    }
                }
                _ => {}
            }
        }

        self.send_bytes += cache.len() as u64;

        Ok(cache)
    }

    fn write_video(
        &mut self,
        cache: &mut Vec<u8>,
        data: &Bytes,
        timestamp: u32,
    ) -> Result<(), FlvMuxerError> {
        cache.write_u8(FRAME_TYPE_VIDEO)?;
        cache.write_u24::<BigEndian>(data.len() as u32)?;
        cache.write_u24::<BigEndian>(timestamp & 0xFFFFFF)?;
        cache.write_u8(((timestamp >> 24) & 0xFF) as u8)?;
        cache.write_u24::<BigEndian>(0)?;

        cache.extend(data);

        cache.write_u32::<BigEndian>((FLV_TAG_HEADER_SIZE + data.len()) as u32)?;
        Ok(())
    }

    fn write_audio(
        &mut self,
        cache: &mut Vec<u8>,
        data: &Bytes,
        timestamp: u32,
    ) -> Result<(), FlvMuxerError> {
        cache.write_u8(FRAME_TYPE_AUDIO)?;
        cache.write_u24::<BigEndian>(data.len() as u32)?;
        cache.write_u24::<BigEndian>(timestamp & 0xFFFFFF)?;
        cache.write_u8(((timestamp >> 24) & 0xFF) as u8)?;
        cache.write_u24::<BigEndian>(0)?;

        cache.extend(data);

        cache.write_u32::<BigEndian>((FLV_TAG_HEADER_SIZE + data.len()) as u32)?;
        Ok(())
    }

    fn write_metadata(&mut self, cache: &mut Vec<u8>, data: &Bytes) -> Result<(), FlvMuxerError> {
        cache.write_u8(FRAME_TYPE_SCRIPT)?;
        cache.write_u24::<BigEndian>(data.len() as u32)?;
        cache.write_u24::<BigEndian>(0)?;
        cache.write_u8(0)?;
        cache.write_u24::<BigEndian>(0)?;

        cache.extend(data);

        cache.write_u32::<BigEndian>((FLV_TAG_HEADER_SIZE + data.len()) as u32)?;
        Ok(())
    }

    pub fn get_send_bytes(&mut self) -> u64 {
        self.send_bytes
    }

    pub fn get_audio_count(&mut self) -> u64 {
        self.audio_count
    }

    pub fn get_video_count(&mut self) -> u64 {
        self.video_count
    }
}
