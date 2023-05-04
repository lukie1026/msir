use rtmp::message::RtmpMessage;

pub struct GopCache {
    caches: Vec<RtmpMessage>,
    max_frame: u32,
}

impl GopCache {
    pub fn new() -> Self {
        Self {
            max_frame: 128,
            caches: Vec::with_capacity(128),
        }
    }

    pub fn cache(msg: RtmpMessage) {}
}
