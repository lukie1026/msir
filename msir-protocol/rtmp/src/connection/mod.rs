pub mod client;
mod context;
pub mod error;
pub mod server;

#[derive(Debug)]
pub enum RtmpConnType {
    Play,
    FlvPlay,
    FmlePublish,
    FlashPublish,
    HaivisionPublish,
    Unknow,
}

impl RtmpConnType {
    pub fn is_publish(self) -> bool {
        match self {
            RtmpConnType::FmlePublish
            | RtmpConnType::FlashPublish
            | RtmpConnType::HaivisionPublish => true,
            _ => false,
        }
    }
}
