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
    pub const COMMAND_ON_BW_DONE: &str = "onBWDone";
    pub const COMMAND_ON_STATUS: &str = "onStatus";
    pub const COMMAND_RESULT: &str = "_result";
    pub const COMMAND_ERROR: &str = "_error";
    pub const COMMAND_RELEASE_STREAM: &str = "releaseStream";
    pub const COMMAND_FC_PUBLISH: &str = "FCPublish";
    pub const COMMAND_UNPUBLISH: &str = "FCUnpublish";
    pub const COMMAND_PUBLISH: &str = "publish";
    pub const COMMAND_ON_FC_PUBLISH: &str = "onFCPublish";
    pub const COMMAND_ON_FC_UNPUBLISH: &str = "onFCUnpublish";
    pub const DATA_SAMPLE_ACCESS: &str = "|RtmpSampleAccess";
}

pub mod rtmp_sig {
    pub const RTMP_SIG_FMS_VER: &str = "FMS/3,5,3,888";
    pub const RTMP_SIG_AMF0_VER: f64 = 0.0;
    pub const RTMP_SIG_CLIENT_ID: &str = "ASAICiss";
}

pub mod rtmp_status {
    pub const STATUS_LEVEL: &str = "level";
    pub const STATUS_CODE: &str = "code";
    pub const STATUS_DESCRIPTION: &str = "description";
    pub const STATUS_DETAILS: &str = "details";
    pub const STATUS_CLIENT_ID: &str = "clientid";

    pub const STATUS_LEVEL_STATUS: &str = "status";
    pub const STATUS_LEVEL_ERROR: &str = "error";

    pub const STATUS_CODE_CONNECT_SUCCESS: &str = "NetConnection.Connect.Success";
    pub const STATUS_CODE_CONNECT_REJECTED: &str = "NetConnection.Connect.Rejected";
    pub const STATUS_CODE_STREAM_RESET: &str = "NetStream.Play.Reset";
    pub const STATUS_CODE_STREAM_START: &str = "NetStream.Play.Start";
    pub const STATUS_CODE_STREAM_PAUSE: &str = "NetStream.Pause.Notify";
    pub const STATUS_CODE_STREAM_UNPAUSE: &str = "NetStream.Unpause.Notify";
    pub const STATUS_CODE_PUBLISH_START: &str = "NetStream.Publish.Start";
    pub const STATUS_CODE_DATA_START: &str = "NetStream.Data.Start";
    pub const STATUS_CODE_UNPUBLISH_SUCCESS: &str = "NetStream.Unpublish.Success";
}

pub const DEFAULT_SID: f64 = 1.0;