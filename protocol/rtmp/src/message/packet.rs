use rml_amf0::Amf0Value;

use super::error::MessageDecodeError;

#[derive(Debug)]
pub enum Amf0CommandPacket {
    ConnectApp {
        command_name: String,
        transaction_id: f64,
        command_object: Amf0Value,
        args: Amf0Value,
    },
    ConnectAppRes {
        command_name: String,
        transaction_id: f64,
        props: Amf0Value,
        info: Amf0Value,
    },
    Call {
        command_name: String,
        transaction_id: f64,
        command_object: Amf0Value,
        args: Amf0Value,
    },
    CallRes {
        command_name: String,
        transaction_id: f64,
        command_object: Amf0Value,
        response: Vec<Amf0Value>,
    },
    CreateStream {
        command_name: String,
        transaction_id: f64,
        command_object: Amf0Value,
    },
    CreateStreamRes {
        command_name: String,
        transaction_id: f64,
        command_object: Amf0Value,
        stream_id: f64,
    },
    CloseStream {
        command_name: String,
        transaction_id: f64,
        command_object: Amf0Value,
    },
    FmleStart {
        command_name: String,
        transaction_id: f64,
        command_object: Amf0Value,
        stream_name: String,
    },
    FmleStartRes {
        command_name: String,
        transaction_id: f64,
        command_object: Amf0Value,
        args: Amf0Value,
    },
    Publish {
        command_name: String,
        transaction_id: f64,
        command_object: Amf0Value,
        stream_name: String,
        publish_type: String,
    },
    Pause {
        command_name: String,
        transaction_id: f64,
        command_object: Amf0Value,
        is_pause: bool,
        time_ms: f64,
    },
    Play {
        command_name: String,
        transaction_id: f64,
        command_object: Amf0Value,
        stream_name: String,
        start: f64,
        duration: f64,
        reset: bool,
    },
    // PlayRes{
    //     command_name: String,
    //     transaction_id: f64,
    //     command_object: Amf0Value,
    //     desc: Amf0Value,
    // },
    OnBWDone {
        command_name: String,
        transaction_id: f64,
        args: Amf0Value,
    },
    OnStatusCall {
        command_name: String,
        transaction_id: f64,
        args: Amf0Value,
        data: Amf0Value,
    },
    // ResultOrError {
    //     command_name: String,
    //     transaction_id: f64,
    //     command_object: Amf0Value,
    //     additional_arguments: Vec<Amf0Value>,
    // },
}

// pub fn to_call_res(roe: Amf0CommandPacket) -> Result<Amf0CommandPacket, MessageDecodeError> {
//     if let Amf0CommandPacket::ResultOrError {
//         command_name,
//         transaction_id,
//         command_object,
//         additional_arguments,
//     } = roe
//     {
//         Ok(Amf0CommandPacket::CallRes {
//             command_name,
//             transaction_id,
//             command_object,
//             response: additional_arguments,
//         })
//     } else {
//         Err(MessageDecodeError::InvalidFormat(String::from(
//             "result_or_error",
//         )))
//     }
// }

#[derive(Debug)]
pub enum Amf0DataPacket {
    OnMetaData {
        command_name: String,
        metadata: Amf0Value,
    },
    OnStatusData {
        command_name: String,
        data: Amf0Value,
    },
    SampleAccess {
        command_name: String,
        video_sample_access: bool,
        audio_sample_access: bool,
    },
}
