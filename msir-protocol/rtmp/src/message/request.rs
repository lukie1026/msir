use url::Url;

use crate::connection::RtmpConnType;

use super::{error::ReuquestError, types::rtmp_sig};

// #[derive(Debug, Default)]
// pub struct Request2 {
//     // Remote IP
//     pub ip: Option<String>,
//     pub tc_url: String,
//     pub object_encoding: f64,
//     // The following is from tcUrl
//     pub schema: String,
//     pub host: Option<String>,
//     pub port: Option<u16>,
//     pub app: String,
//     pub param: Option<String>,
//     pub stream: Option<String>,
// }

// impl Request2 {
//     // FIXME: Maybe schema, host, app, stream, param should be &str
//     pub fn parse_from(url: String) -> Result<Self, ReuquestError> {
//         let parsed = Url::parse(&url)?;
//         let app_stream: Vec<&str> = parsed.path().splitn(3, '/').collect();
//         if app_stream.len() < 2 || app_stream[1].is_empty() {
//             return Err(ReuquestError::NotfoundApp(url));
//         }
//         let app = app_stream[1].to_string();
//         let mut stream = None;
//         if app_stream.len() > 2 && !app_stream[2].is_empty() {
//             stream = Some(app_stream[2].to_string());
//         }
//         Ok(Request2 {
//             ip: None,
//             tc_url: url,
//             object_encoding: 0.0,
//             schema: parsed.scheme().to_string(),
//             host: None, // FIXME: add host
//             port: parsed.port(),
//             app,
//             param: None, // FIXME: add param
//             stream,
//         })
//     }
// }

#[derive(Debug)]
pub struct Request {
    // Remote IP
    pub ip: Option<String>,
    // From amf::connect
    pub object_encoding: f64,
    // tcUrl: rtmp://a.com/live?key=1
    pub tc_url: Url,
    pub stream: Option<String>,
    pub conn_type: RtmpConnType,
    // From amf::play
    pub duration: u32,
}

impl Request {
    pub fn parse_from(url: String) -> Result<Self, ReuquestError> {
        // FIXME: whether to deal with srs-style url(with vhost)?
        let tc_url = Url::parse(&url)?;
        // FIXME: whether to deal with the url with stream?
        Ok(Request {
            ip: None,
            object_encoding: rtmp_sig::RTMP_SIG_AMF0_VER,
            tc_url,
            stream: None,
            conn_type: RtmpConnType::Unknow,
            duration: 0,
        })
    }
}
