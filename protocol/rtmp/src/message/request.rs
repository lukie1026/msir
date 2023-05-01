use url::Url;

use super::error::ReuquestError;


#[derive(Debug, Default)]
pub struct Request {
    // Remote IP
    pub ip: Option<String>,
    pub tc_url: String,
    pub object_encoding: f64,
    // The following is from tcUrl
    pub schema: String,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub app: String,
    pub param: Option<String>,
    pub stream: Option<String>,
}

impl Request {
    // FIXME: Maybe schema, host, app, stream, param should be &str
    pub fn parse_from(url: String) -> Result<Self, ReuquestError>  {
        let parsed = Url::parse(&url)?;
        let app_stream:Vec<&str> = parsed.path().splitn(3, '/').collect();
        if app_stream.len() < 2 || app_stream[1].is_empty() {
            return Err(ReuquestError::NotfoundApp(url));
        }
        let app = app_stream[1].to_string();
        let mut stream = None;
        if app_stream.len() > 2 && !app_stream[2].is_empty() {
            stream = Some(app_stream[2].to_string());
        }
        Ok(Request {
            ip: None,
            tc_url: url,
            object_encoding: 0.0,
            schema: parsed.scheme().to_string(),
            host: None, // FIXME: add host
            port: parsed.port(),
            app,
            param: None, // FIXME: add param
            stream,
        })
    }
}