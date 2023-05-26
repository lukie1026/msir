use anyhow::Result;
use serde_derive::Deserialize;
use std::fs;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub worker: Option<WorkerConfig>,
    pub log: Option<LogConfig>,
    pub rtmp: Option<RtmpConfig>,
    pub http: Option<HttpConfig>,
    pub api: Option<ApiConfig>,
}

impl Config {
    pub fn default() -> Self {
        Self {
            worker: Some(WorkerConfig::default()),
            log: Some(LogConfig::default()),
            rtmp: Some(RtmpConfig::default()),
            http: Some(HttpConfig::default()),
            api: Some(ApiConfig::default()),
        }
    }

    fn fill_default(&mut self) {
        self.worker = match &mut self.worker {
            None => Some(WorkerConfig::default()),
            Some(w) => {
                w.fill_default();
                Some(w.clone())
            }
        };
        self.log = match &mut self.log {
            None => Some(LogConfig::default()),
            Some(l) => {
                l.fill_default();
                Some(l.clone())
            }
        };
        self.rtmp = match &mut self.rtmp {
            None => Some(RtmpConfig::default()),
            Some(r) => {
                r.fill_default();
                Some(r.clone())
            }
        };
        self.http = match &mut self.http {
            None => Some(HttpConfig::default()),
            Some(h) => {
                h.fill_default();
                Some(h.clone())
            }
        };
        self.api = match &mut self.api {
            None => Some(ApiConfig::default()),
            Some(a) => {
                a.fill_default();
                Some(a.clone())
            }
        };
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct WorkerConfig {
    pub mode: String,
}

impl WorkerConfig {
    fn default() -> Self {
        Self {
            mode: "single".to_string(),
        }
    }
    fn fill_default(&mut self) {}
}

#[derive(Debug, Deserialize, Clone)]
pub struct LogConfig {
    pub level: String,
    pub file: Option<String>,
    pub rolling: Option<String>,
}

impl LogConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            file: None,
            rolling: None,
        }
    }
    fn fill_default(&mut self) {}
}

#[derive(Debug, Deserialize, Clone)]
pub struct RtmpConfig {
    pub listen: String,
    performance: Option<String>,
}

impl RtmpConfig {
    fn default() -> Self {
        Self {
            listen: "0.0.0.0:1935".to_string(),
            performance: Some("middle".to_string()),
        }
    }
    fn fill_default(&mut self) {
        if let None = self.performance {
            self.performance = Some("middle".to_string())
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct HttpConfig {
    pub enabled: bool,
    pub listen: Option<String>,
    pub flv: Option<HttpFlv>,
    pub hls: Option<HttpHls>,
}

impl HttpConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            listen: Some("0.0.0.0:8080".to_string()),
            flv: None,
            hls: None,
        }
    }
    fn fill_default(&mut self) {
        if let None = self.listen {
            self.listen = Some("0.0.0.0:8080".to_string())
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct HttpFlv {
    pub enabled: bool,
}

#[derive(Debug, Deserialize, Clone)]
pub struct HttpHls {
    pub enabled: bool,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ApiConfig {
    enabled: bool,
    listen: Option<String>,
}

impl ApiConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            listen: Some("0.0.0.0:1888".to_string()),
        }
    }
    fn fill_default(&mut self) {
        if let None = self.listen {
            self.listen = Some("0.0.0.0:1888".to_string())
        }
    }
}

pub fn load(path: &str) -> Result<Config> {
    let content = fs::read_to_string(path)?;
    let mut config: Config = toml::from_str(&content[..])?;
    config.fill_default();
    Ok(config)
}
