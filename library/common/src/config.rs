use crate::auth::AuthAlgorithm;
use serde_derive::Deserialize;
use std::fs;
use std::fs::File;
use std::io::BufReader;
use std::vec::Vec;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub rtmp: Option<RtmpConfig>,
    pub http: Option<HttpConfig>,
    pub edit_auth: EditAuthConfig,
    pub httpapi: Option<HttpApiConfig>,
    pub httpnotify: Option<HttpNotifierConfig>,
    pub authsecret: AuthSecretConfig,
    pub streams: Option<Vec<Streams>>,
    pub log: Option<LogConfig>,
}

impl Config {
    pub fn new(
        rtmp_port: Vec<usize>,
        http_port:Vec<usize>,
        log_level: String,
    ) -> Self {
        let mut rtmp_config: Option<RtmpConfig> = None;
        if rtmp_port.len() > 0 {
            rtmp_config = Some(RtmpConfig {
                enabled: true,
                port: rtmp_port,
                gop_num: None,
                pull: None,
                push: None,
                auth: None,
            });
        }


        let mut http_config: Option<HttpConfig> = None;
        if http_port.len() > 0 {
            http_config = Some(HttpConfig {
                enabled: true,
                port: http_port,
                need_record: false,
                auth: None,
            });
        }

        let log_config = Some(LogConfig {
            level: log_level,
            file: None,
        });
        let streams_config = Some(vec![Streams {
            app_name: "live".to_string(),
            stream_name: "test".to_string(),
            disabled: false,
            max_bitrate: None,
            on_publish_url: None,
            max_sessions: None,
        }]);

        Self {
            rtmp: rtmp_config,
            http: http_config,
            edit_auth: EditAuthConfig::default(),
            httpapi: None,
            httpnotify: None,
            authsecret: AuthSecretConfig::default(),
            streams: streams_config,
            log: log_config,
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct Streams{
    pub app_name: String,
    pub stream_name: String,
    pub disabled: bool,
    pub max_bitrate: Option<usize>,
    pub on_publish_url: Option<String>,
    pub max_sessions: Option<usize>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RtmpConfig {
    pub enabled: bool,
    pub port: Vec<usize>,
    pub gop_num: Option<usize>,
    pub pull: Option<RtmpPullConfig>,
    pub push: Option<Vec<RtmpPushConfig>>,
    pub auth: Option<AuthConfig>,
}
#[derive(Debug, Deserialize, Clone)]
pub struct RtmpPullConfig {
    pub enabled: bool,
    pub address: String,
    pub port: u16,
}
#[derive(Debug, Deserialize, Clone)]
pub struct RtmpPushConfig {
    pub enabled: bool,
    pub address: String,
    pub port: usize,
}

#[derive(Debug, Deserialize, Clone)]
pub struct HttpConfig {
    pub enabled: bool,
    pub port: Vec<usize>,
    //record or not
    pub need_record: bool,
    pub auth: Option<AuthConfig>,
}

pub enum LogLevel {
    Info,
    Warn,
    Error,
    Trace,
    Debug,
}

#[derive(Debug, Deserialize, Clone)]
pub struct LogConfig {
    pub level: String,
    pub file: Option<LogFile>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct LogFile {
    pub enabled: bool,
    pub rotate: String,
    pub path: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct HttpApiConfig {
    pub port: usize,
}

#[derive(Debug, Deserialize, Clone)]
pub struct HttpNotifierConfig {
    pub enabled: bool,
    pub on_publish: Option<String>,
    pub on_unpublish: Option<String>,
    pub on_play: Option<String>,
    pub on_stop: Option<String>,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct AuthSecretConfig {
    pub key: String,
    pub password: String,
    pub push_password: Option<String>
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct EditAuthConfig{
    pub username: String,
    pub password: String,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct AuthConfig {
    pub pull_enabled: bool,
    pub push_enabled: Option<bool>,
    pub algorithm: AuthAlgorithm,
}

use serde_json::from_reader;
pub fn load_config(path: &str) -> anyhow::Result<Config, anyhow::Error> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let config: Config = from_reader(reader)?;
    Ok(config)
}

// pub fn load(cfg_path: &String) -> Result<Config, ConfigError> {
//     let content = fs::read_to_string(cfg_path)?;
//     let decoded_config = toml::from_str(&content[..]).unwrap();
//     Ok(decoded_config)
// }

// #[test]
// fn test_toml_parse() {
//     let path = std::env::current_dir();
//     match path {
//         Ok(val) => println!("The current directory is {}\n", val.display()),
//         Err(err) => println!("{}", err),
//     }
//
//     let str = fs::read_to_string("./src/config/config.json");
//
//     match str {
//         Ok(val) => {
//             println!("++++++{val}\n");
//             let decoded: Config = toml::from_str(&val[..]).unwrap();
//             println!("whole config: {:?}", decoded);
//             let rtmp = decoded.httpnotify;
//
//             if let Some(val) = rtmp {
//                 println!("++++++{val:?}\n");
//             }
//         }
//         Err(err) => println!("======{err}"),
//     }
// }

use {
    failure::{Backtrace, Fail},
    std::{fmt, io::Error},
};
#[derive(Debug)]
pub struct ConfigError {
    pub value: ConfigErrorValue,
}

#[derive(Debug, Fail)]
pub enum ConfigErrorValue {
    #[fail(display = "IO error: {}", _0)]
    IOError(Error),
}

impl From<Error> for ConfigError {
    fn from(error: Error) -> Self {
        ConfigError {
            value: ConfigErrorValue::IOError(error),
        }
    }
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.value, f)
    }
}

impl Fail for ConfigError {
    fn cause(&self) -> Option<&dyn Fail> {
        self.value.cause()
    }

    fn backtrace(&self) -> Option<&Backtrace> {
        self.value.backtrace()
    }
}
