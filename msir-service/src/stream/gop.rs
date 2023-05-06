use rtmp::message::RtmpMessage;

#[derive(Debug)]
pub struct GopCache {
    // TODO: implement iter
    pub caches: Vec<RtmpMessage>,
    max_frame: u32,
}

impl GopCache {
    pub fn new() -> Self {
        Self {
            max_frame: 128,
            caches: Vec::with_capacity(128),
        }
    }

    pub fn cache(&mut self, msg: RtmpMessage) {
        self.caches.push(msg);
    }
}
