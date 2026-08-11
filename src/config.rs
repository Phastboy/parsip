use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub listen_port: u16,
    pub shared_dir: PathBuf,
    pub downloads_dir: PathBuf,
    pub log_file: PathBuf,
    pub nickname: String,
}

impl Default for Config {
    fn default() -> Self {
        let base = std::env::var("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("."));
        let parsip_dir = base.join(".parsip");

        Self {
            listen_port: 9000,
            shared_dir: parsip_dir.join("shared"),
            downloads_dir: parsip_dir.join("downloads"),
            log_file: parsip_dir.join("parsip.log"),
            nickname: "Anonymous".to_string(),
        }
    }
}

impl Config {
    pub fn load() -> Self {
        let base = std::env::var("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("."));
        let config_path = base.join(".parsip").join("config.toml");

        if let Ok(content) = fs::read_to_string(&config_path) {
            if let Ok(config) = toml::from_str(&content) {
                return config;
            } else {
                eprintln!("Warning: Failed to parse config.toml, using defaults.");
            }
        }

        Self::default()
    }

    pub fn save(&self) -> Result<(), std::io::Error> {
        let base = std::env::var("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("."));
        let parsip_dir = base.join(".parsip");
        std::fs::create_dir_all(&parsip_dir)?;

        let config_path = parsip_dir.join("config.toml");
        let content = toml::to_string(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;

        fs::write(config_path, content)
    }
}
