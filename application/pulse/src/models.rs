use serde::{Deserialize, Serialize};
use commonlib::config::{AuthSecretConfig, EditAuthConfig, HttpApiConfig, HttpConfig, HttpNotifierConfig, LogConfig, RtmpConfig};

#[warn(unused_imports)]
#[derive(Debug, serde::Deserialize, Clone)]
pub struct Data {
    pub name: String,
    pub url: String,
    pub preview: String,
    pub udp_url: String,
    #[serde(rename = "type")]
    pub stream_type: String,
    pub video_resolution: Option<String>,
    pub video_bitrate: Option<u32>,
    pub audio_bitrate: Option<u32>,
    pub audio_samplerate: Option<u32>,
    pub audio_channels: Option<u32>,
    pub audio_codec: String,
    pub video_codec: String,
    pub video_fps: Option<u32>,
}

#[derive(Debug, serde::Deserialize)]
pub struct APIResponse {
    pub status_code: u32,
    pub message: String,
    pub error: String,
    pub data: Vec<Data>,
}

#[derive(Serialize, Debug, Deserialize)]
pub struct SuccessResponse {
    pub status: bool,
    pub message: String,
    pub data: AllData
}
#[derive(Serialize, Debug, Deserialize)]
pub struct ErrorResponse {
    pub status: bool,
    pub message: String,
}

#[derive(Serialize, Debug, Deserialize)]
pub struct AllData {
    pub system: SystemOs,
    pub network: Vec<NetworkInfo>,
    pub components: Vec<ComponentData>,
    pub disk: Vec<Disk>,
}

#[derive(Serialize, Debug, Deserialize)]
pub struct Disk {
    pub name: String,
    pub file_system: String,
    pub total_space: u64,
    pub available_space: u64,
}
#[derive(Serialize, Debug, Deserialize)]
pub struct ComponentData {
    pub name: String,
    pub temperature: String,
}

#[derive(Serialize, Debug, Deserialize)]
pub struct SystemData {
    pub total_memory: u64,
    pub used_memory: u64,
    pub total_swap: u64,
    pub used_swap: u64,
    pub uptime: u64,
    pub cpu_percentage: f32,
    pub cpu_temp: i32,
    pub ram_percentage: f32,
    pub boot_time: u64,
}

#[derive(Serialize, Debug, Deserialize)]
pub struct SystemOs {
    pub name: String,
    pub kernel_version: String,
    pub os_version: String,
    pub  host_name: String,
    pub cpus: i32,
    pub processor: String,
    pub  stats: SystemData,
}

#[derive(Serialize, Debug, Deserialize)]
pub struct NetworkInfo {
    pub index: u32,
    pub name: String,
    pub mac: String,
    pub ip_addr: Vec<pnet::ipnetwork::IpNetwork>,
    pub flags: u32,
    pub total_received : u64,
    pub total_transmitted: u64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Config0 {
    pub rtmp: Option<RtmpConfig>,
    pub http: Option<HttpConfig>,
    pub edit_auth: EditAuthConfig,
    pub httpapi: Option<HttpApiConfig>,
    pub httpnotify: Option<HttpNotifierConfig>,
    pub authsecret: AuthSecretConfig,
    pub log: Option<LogConfig>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Config2 {
    pub rtmp: Vec<usize>,
    pub http: Vec<usize>,
    pub edit_auth: EditAuthConfig,
    pub httpapi: Option<HttpApiConfig>,
    pub httpnotify: Option<HttpNotifierConfig>,
    pub authsecret: AuthSecretConfig,
    pub log: Option<LogConfig>,
}