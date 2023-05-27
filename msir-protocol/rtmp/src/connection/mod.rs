use serde_derive::Serialize;

pub mod client;
mod context;
pub mod error;
pub mod server;

#[derive(Debug, Clone, Serialize)]
pub enum RtmpConnType {
    Play,
    FlvPlay,
    Pull,
    FmlePublish,
    FlashPublish,
    HaivisionPublish,
    Unknow,
}

impl RtmpConnType {
    pub fn is_publish(&self) -> bool {
        match self {
            RtmpConnType::Pull
            | RtmpConnType::FmlePublish
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

    pub fn as_str(&self) -> &str {
        return match self {
            RtmpConnType::Pull => "pull",
            RtmpConnType::Play => "play",
            RtmpConnType::FlvPlay => "flv-play",
            RtmpConnType::FmlePublish => "publish",
            RtmpConnType::FlashPublish => "publish",
            RtmpConnType::HaivisionPublish => "publish",
            RtmpConnType::Unknow => "unknow",
        };
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
