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
        let mut tc_url = Url::parse(&url)?;

        let app;
        let mut stream;
        let mut conn_type = RtmpConnType::Unknow;
        {
            let app_stream: Vec<&str> = tc_url.path().splitn(3, '/').collect();
            if app_stream.len() < 2 || app_stream[1].is_empty() {
                return Err(ReuquestError::NotfoundApp);
            }
            app = String::from("/") + app_stream[1];
            stream = None;
            if app_stream.len() > 2 && !app_stream[2].is_empty() {
                let s = app_stream[2];
                if s.len() >= 4 && &s[(s.len() - 4)..] == ".flv" {
                    stream = Some(s[0..(s.len() - 4)].to_string());
                    conn_type = RtmpConnType::FlvPlay;
                } else {
                    stream = Some(s.to_string());
                }
            }
        }
        tc_url.set_path(&app);

        Ok(Request {
            ip: None,
            // object_encoding: rtmp_sig::RTMP_SIG_AMF0_VER,
            tc_url,
            stream,
            conn_type,
            duration: 0,
        })
    }

    pub fn app_stream(&self) -> String {
        return format!(
            "/{}/{}",
            match self.tc_url.path_segments() {
                Some(split) => {
                    let apps: Vec<&str> = split.collect();
                    apps[0]
                }
                None => "",
            },
            match &self.stream {
                Some(s) => s,
                None => "",
            }
        );
    }

    pub fn stream(&self) -> &str {
        match &self.stream {
            Some(s) => s,
            None => "",
        }
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test1() {
        let url = String::from("rtmp://1.1.1.1/live/stream/12");
        let req = Request::parse_from(url.clone()).unwrap();
        println!(
            "Lukie 1 path={:?} stream={}",
            req.tc_url.path(),
            req.stream()
        );
    }
    #[test]
    fn test2() {
        let url = String::from("rtmp://1.1.1.1/live/stream/");
        let req = Request::parse_from(url.clone()).unwrap();
        println!(
            "Lukie 2 path={:?} stream={}",
            req.tc_url.path(),
            req.stream()
        );
    }
    #[test]
    fn test3() {
        let url = String::from("rtmp://1.1.1.1/live/stream");
        let req = Request::parse_from(url.clone()).unwrap();
        println!(
            "Lukie 3 path={:?} stream={}",
            req.tc_url.path(),
            req.stream()
        );
    }
    #[test]
    fn test4() {
        let url = String::from("rtmp://1.1.1.1/live/");
        let req = Request::parse_from(url.clone()).unwrap();
        println!(
            "Lukie 4 path={:?} stream={:?}",
            req.tc_url.path(),
            req.stream
        );
    }
    #[test]
    fn test5() {
        let url = String::from("rtmp://1.1.1.1/live");
        let req = Request::parse_from(url.clone()).unwrap();
        println!(
            "Lukie 5 path={:?} stream={:?}",
            req.tc_url.path(),
            req.stream
        );
    }
    // #[test]
    fn test6() {
        let url = String::from("rtmp://1.1.1.1/");
        let req = Request::parse_from(url.clone()).unwrap();
        println!(
            "Lukie 6 path={:?} stream={}",
            req.tc_url.path(),
            req.stream()
        );
    }
    // #[test]
    fn test7() {
        let url = String::from("rtmp://1.1.1.1");
        let req = Request::parse_from(url.clone()).unwrap();
        println!(
            "Lukie 7 path={:?} stream={}",
            req.tc_url.path(),
            req.stream()
        );
    }
    #[test]
    fn test8() {
        let url = String::from("rtmp://1.1.1.1/live/stream.flv");
        let req = Request::parse_from(url.clone()).unwrap();
        println!(
            "Lukie 8 path={:?} stream={:?}",
            req.tc_url.path(),
            req.stream
        );
    }
    #[test]
    fn test9() {
        let url = String::from("rtmp://1.1.1.1/live/.flv");
        let req = Request::parse_from(url.clone()).unwrap();
        println!(
            "Lukie 9 path={:?} stream={:?}",
            req.tc_url.path(),
            req.stream
        );
    }
    #[test]
    fn test10() {
        let url = String::from("rtmp://1.1.1.1/live/flv");
        let req = Request::parse_from(url.clone()).unwrap();
        println!(
            "Lukie 10 path={:?} stream={:?}",
            req.tc_url.path(),
            req.stream
        );
    }
    // #[test]
    // fn test11() {
    //     let url = String::from("/live/a.flv?ab=ba");
    //     let req = Request::parse_from(url.clone()).unwrap();
    //     println!("Lukie 11 path={:?} stream={:?}", req.tc_url.path(), req.stream);
    // }
}
