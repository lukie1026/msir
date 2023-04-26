use amf::Amf0Value;

#[derive(Debug)]
pub enum Amf0CommandPacket {
    ConnectApp {
        command_name: String,
        transaction_id: f64,
        command_object: Amf0Value,
        args: Amf0Value,
    },
    CreateStream {
        command_name: String,
        transaction_id: f64,
        command_object: Amf0Value,
    },
    Play(),
    Pause(),
    FmleStart(),
    Publish(),
    CloseStream(),
    Call(),
    ConnectAppRes(),
    CreateStreamRes(),
    FmleStartRes(),
    CallRes(),
    PlayRes{
        command_name: String,
        transaction_id: f64,
        command_object: Amf0Value,
        desc: Amf0Value,
    },
    OnStatusCall{
        command_name: String,
        transaction_id: f64,
        args: Amf0Value,
        data: Amf0Value,
    },
    OnBWDone{
        command_name: String,
        transaction_id: f64,
        args: Amf0Value,
    }
}

#[derive(Debug)]
pub enum Amf0DataPacket {
    OnMetaData{
        command_name: String,
        metadata: Amf0Value,
    },

    OnStatusData{
        command_name: String,
        data: Amf0Value,
    },
    SampleAccess{
        command_name: String,
        video_sample_access: bool,
        audio_sample_access: bool,
    },
}
