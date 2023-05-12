pub mod client;
mod context;
pub mod error;
pub mod server;

#[derive(Debug, Clone)]
pub enum RtmpConnType {
    Play,
    FlvPlay,
    FmlePublish,
    FlashPublish,
    HaivisionPublish,
    Unknow,
}

impl RtmpConnType {
    pub fn is_publish(&self) -> bool {
        match self {
            RtmpConnType::FmlePublish
            | RtmpConnType::FlashPublish
            | RtmpConnType::HaivisionPublish => true,
            _ => false,
        }
    }

    pub fn is_play(&self) -> bool {
        match self {
            RtmpConnType::FlvPlay | RtmpConnType::Play => true,
            _ => false,
        }
    }
}

pub enum RtmpCtrlAction {
    // From publisher
    Republish,
    // From player
    Pause(bool),
    // From player
    Close,
}
