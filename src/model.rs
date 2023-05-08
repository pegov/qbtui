use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::humanize::{humanize_bytes, humanize_eta, humanize_percentage};

#[derive(Debug, Deserialize, PartialEq, Eq)]
pub enum SpeedLimitsMode {
    Global,
    Alternative,
}

impl From<String> for SpeedLimitsMode {
    fn from(value: String) -> Self {
        match value.as_str() {
            "0" => Self::Global,
            "1" => Self::Alternative,
            _ => unreachable!(),
        }
    }
}

#[derive(Serialize, Debug)]
pub struct SetSpeedLimit {
    pub limit: i32,
}

// TODO: partial data
#[derive(Clone, Debug, Default, Deserialize)]
pub struct TransferInfo {
    pub dl_info_speed: i64,                  // Global download rate (bytes/s)
    pub dl_info_data: i64,                   // Data downloaded this session (bytes)
    pub up_info_speed: i64,                  // Global upload rate (bytes/s)
    pub up_info_data: i64,                   // Data uploaded this session (bytes)
    pub dl_rate_limit: i64,                  // Download rate limit (bytes/s)
    pub up_rate_limit: i64,                  // Upload rate limit (bytes/s)
    pub dht_nodes: i64,                      // DHT nodes connected to
    pub connection_status: ConnectionStatus, // Connection status. See possible values here below
    #[serde(default = "bool::default")]
    pub use_alt_speed_limits: bool, // True if alternative speed limits are enabled
}

impl TransferInfo {
    pub fn to_stats_string(&self, host: &str) -> String {
        let connection_status_icon = match self.connection_status {
            ConnectionStatus::Connected => "🔗",
            ConnectionStatus::Firewalled => "🌢",
            ConnectionStatus::Disconnected => "⏏",
        };
        let human_dl_speed = humanize_bytes(self.dl_info_speed as f64) + "/s";
        let human_up_speed = humanize_bytes(self.up_info_speed as f64) + "/s";
        let human_dl_data = humanize_bytes(self.dl_info_data as f64);
        let human_up_data = humanize_bytes(self.up_info_data as f64);
        let speed_limits_mode = if self.use_alt_speed_limits {
            "ALT"
        } else {
            "GLO"
        };
        let alt_dl_rate_limit = if self.use_alt_speed_limits {
            format!("[{}]", humanize_bytes(self.dl_rate_limit as f64) + "/s")
        } else {
            "".to_owned()
        };
        let alt_up_rate_limit = if self.use_alt_speed_limits {
            format!("[{}]", humanize_bytes(self.up_rate_limit as f64) + "/s")
        } else {
            "".to_owned()
        };
        format!("DHT: {} nodes | {host} {connection_status_icon} | ⯯ {human_dl_speed} {alt_dl_rate_limit} ({human_dl_data}) | 🠝 {human_up_speed} {alt_up_rate_limit} ({human_up_data}) | {speed_limits_mode} |", self.dht_nodes)
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
pub enum ConnectionStatus {
    #[serde(rename = "connected")]
    Connected,
    #[serde(rename = "firewalled")]
    Firewalled,
    #[serde(rename = "disconnected")]
    #[default]
    Disconnected,
}

#[derive(Clone, Debug, Serialize)]
pub struct GetTorrentListParams {
    filter: Option<String>, // Filter torrent list by state. Allowed state filters: all, downloading, seeding, completed, paused, active, inactive, resumed, stalled, stalled_uploading, stalled_downloading, errored
    category: Option<String>, // Get torrents with the given category (empty string means "without category"; no "category" parameter means "any category" <- broken until #11748 is resolved). Remember to URL-encode the category name. For example, My category becomes My%20category
    tag: Option<String>, // Get torrents with the given tag (empty string means "without tag"; no "tag" parameter means "any tag". Remember to URL-encode the category name. For example, My tag becomes My%20tag
    sort: Option<String>, // torrents by given key. They can be sorted using any field of the response's JSON array (which are documented below) as the sort key.
    reverse: Option<bool>, // Enable reverse sorting. Defaults to false
    limit: Option<i32>,   // Limit the number of torrents returned
    offset: Option<i32>,  // Set offset (if less than 0, offset from end)
    hashes: Option<String>, // Filter by hashes. Can contain multiple hashes separated by |
}

#[derive(Clone, Debug, Deserialize)]
pub struct TorrentInfo {
    pub added_on: i64,
    pub amount_left: i64,
    pub category: String,
    pub completed: i64,
    pub completion_on: i64,
    pub content_path: String,
    pub dlspeed: i64,
    pub downloaded: i64,
    pub eta: i64,
    pub hash: String,
    pub magnet_uri: String,
    pub name: String,
    pub num_complete: u64,   // seeds all
    pub num_incomplete: u64, // leechs all
    pub num_leechs: u64,     // leechs connected to
    pub num_seeds: u64,      // seeds connected to
    pub progress: f64,
    pub save_path: String,
    pub size: i64,
    pub state: TorrentInfoState,
    pub upspeed: i64,
}

impl TorrentInfo {
    pub fn to_row(&self) -> Vec<String> {
        let size_in_bytes = humanize_bytes(self.size as f64);
        let progress_percentage = humanize_percentage(self.progress);
        let seeds_info = format!("{} ({})", self.num_seeds, self.num_complete);
        let leechs_info = format!("{} ({})", self.num_leechs, self.num_incomplete);
        let dl_in_bytes_per_sec = humanize_bytes(self.dlspeed as f64) + "/s";
        let up_in_bytes_per_sec = humanize_bytes(self.upspeed as f64) + "/s";
        let eta = humanize_eta(self.eta);

        vec![
            self.category.clone(),
            self.state.to_icon().to_owned(),
            self.name.clone(),
            size_in_bytes,
            progress_percentage,
            seeds_info,
            leechs_info,
            dl_in_bytes_per_sec,
            up_in_bytes_per_sec,
            eta,
        ]
    }

    pub fn to_info_page(&self) -> String {
        vec![
            format!("Name: {}", self.name),
            format!("Size: {}", humanize_bytes(self.size as f64)),
            format!("Save path: {}", self.save_path),
            format!("Hash: {}", self.hash),
        ]
        .join("\n")
    }

    pub fn is_running(&self) -> bool {
        self.state != TorrentInfoState::PausedUp && self.state != TorrentInfoState::PausedDl
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct TorrentInfoSync {
    pub added_on: Option<i64>,
    pub amount_left: Option<i64>,
    pub category: Option<String>,
    pub completed: Option<i64>,
    pub completion_on: Option<i64>,
    pub content_path: Option<String>, // path
    pub downloaded: Option<i64>,
    pub eta: Option<i64>,
    pub name: Option<String>,
    pub progress: Option<f64>,
    pub save_path: Option<String>,
    pub state: Option<TorrentInfoState>,
    pub size: Option<i64>,
    pub dlspeed: Option<i64>,
    pub upspeed: Option<i64>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct TransferInfoSync {
    pub dl_info_speed: Option<i64>, // Global download rate (bytes/s)
    pub dl_info_data: Option<i64>,  // Data downloaded this session (bytes)
    pub up_info_speed: Option<i64>, // Global upload rate (bytes/s)
    pub up_info_data: Option<i64>,  // Data uploaded this session (bytes)
    pub dl_rate_limit: Option<i64>, // Download rate limit (bytes/s)
    pub up_rate_limit: Option<i64>, // Upload rate limit (bytes/s)
    pub dht_nodes: Option<i64>,     // DHT nodes connected to
    pub connection_status: Option<ConnectionStatus>, // Connection status. See possible values here below
    pub use_alt_speed_limits: Option<bool>, // Connection status. See possible values here below
}

// src/base/bittorrent/torrent.h - TorrentState
// src/webui/api/serialize/serialize_torrent.cpp
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
pub enum TorrentInfoState {
    #[serde(rename = "unknown")]
    Unknown = -1,

    #[serde(rename = "forcedDL")]
    ForcedDl,
    #[serde(rename = "downloading")]
    Downloading,
    #[serde(rename = "forcedMetaDL")]
    ForcedMetaDL,
    #[serde(rename = "metaDL")]
    MetaDl,
    #[serde(rename = "stalledDL")]
    StalledDl,

    #[serde(rename = "forcedUP")]
    ForcedUp,
    #[serde(rename = "uploading")]
    Uploading,
    #[serde(rename = "stalledUP")]
    StalledUp,

    #[serde(rename = "checkingResumeData")]
    CheckingResumeData,
    #[serde(rename = "queuedDL")]
    QueuedDl,
    #[serde(rename = "queuedUP")]
    QueuedUp,

    #[serde(rename = "checkingUP")]
    CheckingUp,
    #[serde(rename = "checkingDL")]
    CheckingDl,

    #[serde(rename = "pausedDL")]
    PausedDl,
    #[serde(rename = "pausedUP")]
    PausedUp,

    #[serde(rename = "moving")]
    Moving,

    #[serde(rename = "missingFiles")]
    MissingFiles,
    #[serde(rename = "error")]
    Error,

    // deprecated
    #[serde(rename = "allocating")]
    Allocating,
}

impl TorrentInfoState {
    pub fn to_icon(&self) -> &'static str {
        // qBittorrent/src/gui/transferlistmodel.cpp
        // qBittorrent/src/icons
        match self {
            Self::Downloading | Self::ForcedDl | Self::MetaDl | Self::ForcedMetaDL => "⯯",
            Self::StalledDl => "⯯", // another color
            Self::StalledUp => "⯭",
            Self::Uploading | Self::ForcedUp => "🠝",
            Self::PausedDl => "⏸",
            Self::PausedUp => "✔",
            Self::QueuedDl | Self::QueuedUp => "⏱",
            Self::CheckingDl
            | Self::CheckingUp
            | Self::CheckingResumeData
            | Self::Moving
            | Self::Allocating => "🗘",
            Self::Unknown | Self::MissingFiles | Self::Error => "!",
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct TorrentProperties {
    pub save_path: String,             // 	Torrent save path
    pub creation_date: i32,            // 	Torrent creation date (Unix timestamp)
    pub piece_size: i32,               // 	Torrent piece size (bytes)
    pub comment: String,               // 	Torrent comment
    pub total_wasted: i32,             // 	Total data wasted for torrent (bytes)
    pub total_uploaded: i32,           // 	Total data uploaded for torrent (bytes)
    pub total_uploaded_session: i32,   // 	Total data uploaded this session (bytes)
    pub total_downloaded: i32,         // 	Total data downloaded for torrent (bytes)
    pub total_downloaded_session: i32, // 	Total data downloaded this session (bytes)
    pub up_limit: i32,                 // 	Torrent upload limit (bytes/s)
    pub dl_limit: i32,                 // 	Torrent download limit (bytes/s)
    pub time_elapsed: i32,             // 	Torrent elapsed time (seconds)
    pub seeding_time: i32,             // 	Torrent elapsed time while complete (seconds)
    pub nb_connections: i32,           // 	Torrent connection count
    pub nb_connections_limit: i32,     // 	Torrent connection count limit
    pub share_ratio: f64,              // 	Torrent share ratio
    pub addition_date: i32,            // 	When this torrent was added (unix timestamp)
    pub completion_date: i32,          // 	Torrent completion date (unix timestamp)
    pub created_by: String,            // 	Torrent creator
    pub dl_speed_avg: i32,             // 	Torrent average download speed (bytes/second)
    pub dl_speed: i32,                 // 	Torrent download speed (bytes/second)
    pub eta: i32,                      // 	Torrent ETA (seconds)
    pub last_seen: i32,                // 	Last seen complete date (unix timestamp)
    pub peers: i32,                    // 	Number of peers connected to
    pub peers_total: i32,              // 	Number of peers in the swarm
    pub pieces_have: i32,              // 	Number of pieces owned
    pub pieces_num: i32,               // 	Number of pieces of the torrent
    pub reannounce: i32,               // 	Number of seconds until the next announce
    pub seeds: i32,                    // 	Number of seeds connected to
    pub seeds_total: i32,              // 	Number of seeds in the swarm
    pub total_size: i32,               // 	Torrent total size (bytes)
    pub up_speed_avg: i32,             // 	Torrent average upload speed (bytes/second)
    pub up_speed: i32,                 // 	Torrent upload speed (bytes/second)
}

#[derive(Debug, Deserialize)]
pub struct TorrentFile {
    pub index: i32, // File index
    pub name: String, // File name (including relative path)
                    // TODO
                    // pub size: i64,             // File size (bytes)
                    // pub progress: f64,         // File progress (percentage/100)
                    // pub priority: Priority,    // File priority. See possible values here below
                    // pub is_seed: Option<bool>, // True if file is seeding/complete
                    // pub piece_range: Vec<i32>, // The first number is the starting piece index and the second number is the ending piece index (inclusive)
                    // pub availability: f64,     // Percentage of file pieces currently available (percentage/100)
}

#[derive(Debug, Deserialize)]
pub enum Priority {
    DoNotDownload = 0,
    Normal = 1,
    High = 6,
    Maximal = 7,
}

#[derive(Clone, Debug, Serialize)]
pub struct GetTorrentFilesParams {
    hash: String,
}

impl From<String> for GetTorrentFilesParams {
    fn from(hash: String) -> Self {
        Self { hash }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct Category {
    pub name: String,
    #[serde(rename = "savePath")]
    pub save_path: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct DeleteTorrentParams {
    pub hashes: String,
    #[serde(rename = "deleteFiles")]
    pub delete_files: bool,
}

#[derive(Clone, Debug, Deserialize)]
pub struct MainData {
    pub rid: i64,
    pub full_update: Option<bool>,
    pub torrents: Option<HashMap<String, TorrentInfoSync>>,
    pub torrents_removed: Option<Vec<String>>,
    pub categories: Option<HashMap<String, Category>>,
    pub categories_removed: Option<Vec<String>>,
    pub server_state: Option<TransferInfoSync>,
}

#[derive(Clone, Debug, Serialize)]
pub struct GetMainDataParams {
    pub rid: i64,
}

#[derive(Serialize)]
pub struct Hashes {
    pub hashes: String,
}

impl From<&[&str]> for Hashes {
    fn from(value: &[&str]) -> Self {
        Self {
            hashes: value.join("|"),
        }
    }
}
