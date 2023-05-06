use rtmp::{codec, message::RtmpMessage};
use tracing::{info, warn};

#[derive(Debug)]
pub struct GopCache {
    // TODO: implement iter
    pub caches: Vec<RtmpMessage>,
    max_frame: usize,
    cached_video: bool,
    continuous_audio_count: usize,
}

impl GopCache {
    pub fn new() -> Self {
        Self {
            max_frame: 128,
            cached_video: false,
            continuous_audio_count: 0,
            caches: Vec::with_capacity(128),
        }
    }

    pub fn cache(&mut self, msg: RtmpMessage) {
        let (is_video, payload) = match &msg {
            RtmpMessage::VideoData { payload, .. } => (true, payload),
            RtmpMessage::AudioData { payload, .. } => (false, payload),
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
            return;
        }
        if self.caches.len() > self.max_frame {
            return;
        }
        if is_video && codec::is_video_keyframe(payload) {
            self.clear();
            self.cached_video = true;
        }
        self.caches.push(msg);
    }

    fn clear(&mut self) {
        self.caches.clear();
        self.cached_video = false;
        self.continuous_audio_count = 0;
    }
}
