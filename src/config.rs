use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Config {
    pub api_key: Option<String>,
    pub host: Option<String>,
}

impl Config {
    pub fn host(&self) -> &str {
        self.host.as_deref().unwrap_or("http://localhost:8384")
    }
}

fn config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("syncthing-cli")
        .join("config.json")
}

fn syncthing_config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("syncthing")
        .join("config.xml")
}

pub fn load_config() -> Result<Config> {
    let path = config_path();
    if path.exists() {
        let content = fs::read_to_string(&path)?;
        return Ok(serde_json::from_str(&content)?);
    }
    Ok(Config::default())
}

pub fn save_config(config: &Config) -> Result<()> {
    let path = config_path();
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    fs::write(path, serde_json::to_string_pretty(config)?)?;
    Ok(())
}

pub fn get_api_key() -> Result<String> {
    // First check our config
    let config = load_config()?;
    if let Some(key) = config.api_key {
        return Ok(key);
    }

    // Fall back to reading from syncthing's config.xml
    let st_config = syncthing_config_path();
    if st_config.exists() {
        let content = fs::read_to_string(&st_config)
            .context("Failed to read syncthing config.xml")?;

        // Simple extraction - look for <apikey>...</apikey>
        if let Some(start) = content.find("<apikey>") {
            let start = start + 8;
            if let Some(end) = content[start..].find("</apikey>") {
                return Ok(content[start..start + end].to_string());
            }
        }
    }

    anyhow::bail!(
        "No API key found. Either configure with 'syncthing config --api-key <KEY>' \
         or ensure syncthing is running with config at {:?}",
        st_config
    )
}
