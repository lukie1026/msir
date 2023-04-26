use rtmp::{codec, message::RtmpMessage};
use tracing::{info, warn};

#[derive(Debug)]
pub struct GopCache {
    // TODO: implement iter
    pub caches: Vec<RtmpMessage>,
    max_frame: usize,
    cached_video: bool,
    continuous_audio_count: usize,

    timestamp_start: u32,
    timestamp_end: u32,
}

impl GopCache {
    pub fn new() -> Self {
        Self {
            max_frame: 2048,
            cached_video: false,
            continuous_audio_count: 0,
            caches: Vec::with_capacity(2048),

            timestamp_start: 0,
            timestamp_end: 0,
        }
    }

    pub fn cache(&mut self, msg: RtmpMessage) {
        let (is_video, timestamp, payload) = match &msg {
            RtmpMessage::VideoData {
                payload, timestamp, ..
            } => (true, timestamp, payload),
            RtmpMessage::AudioData {
                payload, timestamp, ..
            } => (false, timestamp, payload),
            _ => return,
        };
        if is_video {
            self.cached_video = true;
            self.continuous_audio_count = 0;
        }
        if !self.cached_video {
            return;
        }
        if !is_video {
            self.continuous_audio_count += 1;
        }
        if self.continuous_audio_count > 100 {
            warn!("Clear gop cache for guess pure audio overflow");
            self.clear();
            return;
        }
        if self.caches.len() > self.max_frame {
            warn!(
                "Clear gop cache for reach the max frames, threshold {}",
                self.max_frame
            );
            self.clear();
            return;
        }
        if is_video && codec::is_video_keyframe(payload) {
            self.clear();
            self.cached_video = true;
            self.timestamp_start = *timestamp;
        }
        self.timestamp_end = *timestamp;
        self.caches.push(msg);
    }

    pub fn duration(&mut self) -> u32 {
        self.timestamp_end - self.timestamp_start
    }

    fn clear(&mut self) {
        self.caches.clear();
        self.cached_video = false;
        self.continuous_audio_count = 0;
        self.timestamp_end = 0;
        self.timestamp_start = 0;
    }
}
