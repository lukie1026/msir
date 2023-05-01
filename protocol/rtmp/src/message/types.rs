pub mod msg_type {
    pub const SET_CHUNK_SIZE: u8 = 1;
    pub const ABORT: u8 = 2;
    pub const ACK: u8 = 3;
    pub const USER_CONTROL: u8 = 4;
    pub const WIN_ACK_SIZE: u8 = 5;
    pub const SET_PEER_BW: u8 = 6;

    pub const AUDIO: u8 = 8;
    pub const VIDEO: u8 = 9;

    pub const AMF3_DATA: u8 = 15;
    pub const AMF3_SHARED_OBJ: u8 = 16;
    pub const AMF3_CMD: u8 = 17;

    pub const AMF0_DATA: u8 = 18;
    pub const AMF0_SHARED_OBJ: u8 = 19;
    pub const AMF0_CMD: u8 = 20;

    pub const AGGREGATE: u8 = 22;
}

pub mod peer_bw_limit_type {
    pub const HARD: u8 = 0;
    pub const SOFT: u8 = 1;
    pub const DYNAMIC: u8 = 2;
}

pub mod user_ctrl_ev_type {
    pub const STREAM_BEGIN: u16 = 0;
    pub const STREAM_EOF: u16 = 1;
    pub const STREAM_DRY: u16 = 2;
    pub const SET_BUFFER_LENGTH: u16 = 3;
    pub const STREAM_IS_RECORDED: u16 = 4;
    pub const PING_REQUEST: u16 = 6;
    pub const PING_RESPONSE: u16 = 7;
    pub const FMS_EVENT_0: u16 = 0x1a;
}

// The amf0 command message, command name macros
pub mod amf0_command_type {
    pub const COMMAND_CONNECT: &str = "connect";
    pub const COMMAND_CREATE_STREAM: &str = "createStream";
    pub const COMMAND_CLOSE_STREAM: &str = "closeStream";
    pub const COMMAND_PLAY: &str = "play";
    pub const COMMAND_PAUSE: &str = "pause";
    pub const ON_BW_DONE: &str = "onBWDone";
    pub const ON_STATUS: &str = "onStatus";
    pub const RESULT: &str = "_result";
    pub const ERROR: &str = "_error";
    pub const RELEASE_STREAM: &str = "releaseStream";
    pub const FC_PUBLISH: &str = "FCPublish";
    pub const UNPUBLISH: &str = "FCUnpublish";
    pub const PUBLISH: &str = "publish";
    pub const SAMPLE_ACCESS: &str = "|RtmpSampleAccess";
}
