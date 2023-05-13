use super::error::ReuquestError;
use crate::connection::RtmpConnType;
use url::Url;

#[derive(Debug)]
pub struct Request {
    // Remote IP
    pub ip: Option<String>,
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
            // object_encoding: rtmp_sig::RTMP_SIG_AMF0_VER,
            tc_url,
            stream: None,
            conn_type: RtmpConnType::Unknow,
            duration: 0,
        })
    }

    pub fn app_stream(&self) -> String {
        return format!(
            "{}/{}",
            self.tc_url.path(),
            match &self.stream {
                Some(s) => s,
                None => "",
            }
        );
    }
}
