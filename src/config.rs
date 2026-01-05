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
    extract_api_key_from_path(&st_config)
}

pub fn extract_api_key_from_path(path: &PathBuf) -> Result<String> {
    if path.exists() {
        let content = fs::read_to_string(path)
            .context("Failed to read syncthing config.xml")?;
        return extract_api_key_from_xml(&content);
    }

    anyhow::bail!(
        "No API key found. Either configure with 'syncthing config --api-key <KEY>' \
         or ensure syncthing is running with config at {:?}",
        path
    )
}

pub fn extract_api_key_from_xml(content: &str) -> Result<String> {
    if let Some(start) = content.find("<apikey>") {
        let start = start + 8;
        if let Some(end) = content[start..].find("</apikey>") {
            return Ok(content[start..start + end].to_string());
        }
    }
    anyhow::bail!("No apikey element found in config")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.api_key, None);
        assert_eq!(config.host, None);
        assert_eq!(config.host(), "http://localhost:8384");
    }

    #[test]
    fn test_config_with_custom_host() {
        let config = Config {
            api_key: None,
            host: Some("http://192.168.1.100:8384".to_string()),
        };
        assert_eq!(config.host(), "http://192.168.1.100:8384");
    }

    #[test]
    fn test_extract_api_key_from_xml() {
        let xml = r#"
<configuration version="37">
    <gui enabled="true" tls="false" debugging="false" sendBasicAuthPrompt="false">
        <address>127.0.0.1:8384</address>
        <apikey>abc123def456</apikey>
    </gui>
</configuration>
"#;
        let key = extract_api_key_from_xml(xml).unwrap();
        assert_eq!(key, "abc123def456");
    }

    #[test]
    fn test_extract_api_key_missing() {
        let xml = "<configuration></configuration>";
        let result = extract_api_key_from_xml(xml);
        assert!(result.is_err());
    }

    #[test]
    fn test_save_and_load_config() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.json");

        let config = Config {
            api_key: Some("test-key".to_string()),
            host: Some("http://test:8384".to_string()),
        };

        // Save
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&path, serde_json::to_string_pretty(&config).unwrap()).unwrap();

        // Load
        let content = fs::read_to_string(&path).unwrap();
        let loaded: Config = serde_json::from_str(&content).unwrap();

        assert_eq!(loaded.api_key, Some("test-key".to_string()));
        assert_eq!(loaded.host, Some("http://test:8384".to_string()));
    }

    #[test]
    fn test_extract_api_key_from_path() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.xml");

        let xml = "<configuration><gui><apikey>mykey123</apikey></gui></configuration>";
        fs::write(&path, xml).unwrap();

        let key = extract_api_key_from_path(&path).unwrap();
        assert_eq!(key, "mykey123");
    }

    #[test]
    fn test_extract_api_key_from_missing_file() {
        let path = PathBuf::from("/nonexistent/config.xml");
        let result = extract_api_key_from_path(&path);
        assert!(result.is_err());
    }
}
