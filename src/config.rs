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
        self.host.as_deref().unwrap_or("http://[::1]:8384")
    }
}

fn config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("syncthing-cli")
        .join("config.json")
}

fn syncthing_config_path() -> PathBuf {
    #[cfg(windows)]
    {
        return windows_syncthing_config_path(
            &dirs::data_local_dir().unwrap_or_else(|| PathBuf::from(".")),
        );
    }

    #[cfg(target_os = "linux")]
    {
        let candidates = linux_syncthing_config_paths();
        if let Some(path) = existing_syncthing_config_path(&candidates) {
            return path;
        }
        return dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("syncthing")
            .join("config.xml");
    }

    #[cfg(not(any(windows, target_os = "linux")))]
    {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("syncthing")
            .join("config.xml")
    }
}

fn windows_syncthing_config_path(local_app_data: &std::path::Path) -> PathBuf {
    local_app_data.join("Syncthing").join("config.xml")
}

#[cfg(target_os = "linux")]
fn linux_syncthing_config_paths() -> Vec<PathBuf> {
    let state_dir = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| dirs::home_dir().map(|home| home.join(".local").join("state")));
    let config_dir = dirs::config_dir();

    let mut paths = Vec::new();
    if let Some(state_dir) = state_dir {
        paths.push(state_dir.join("syncthing").join("config.xml"));
    }
    if let Some(config_dir) = config_dir {
        paths.push(config_dir.join("syncthing").join("config.xml"));
    }
    paths
}

#[cfg(target_os = "linux")]
fn existing_syncthing_config_path(candidates: &[PathBuf]) -> Option<PathBuf> {
    candidates.iter().find(|path| path.is_file()).cloned()
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
    get_local_api_key()
}

pub fn get_local_api_key() -> Result<String> {
    let st_config = syncthing_config_path();
    extract_api_key_from_path(&st_config)
}

pub fn get_local_host() -> Result<String> {
    let st_config = syncthing_config_path();
    extract_gui_address_from_path(&st_config)
}

pub fn extract_api_key_from_path(path: &PathBuf) -> Result<String> {
    if path.exists() {
        let content = fs::read_to_string(path).context("Failed to read syncthing config.xml")?;
        return extract_api_key_from_xml(&content);
    }

    anyhow::bail!(
        "No API key found. Either configure with 'syncthing config --api-key <KEY>' \
         or ensure syncthing is running with config at {:?}",
        path
    )
}

pub fn extract_gui_address_from_path(path: &PathBuf) -> Result<String> {
    if path.exists() {
        let content = fs::read_to_string(path).context("Failed to read syncthing config.xml")?;
        return extract_gui_address_from_xml(&content);
    }

    anyhow::bail!("No Syncthing GUI address found in config at {:?}", path)
}

pub fn extract_api_key_from_xml(content: &str) -> Result<String> {
    if let Some(start) = content.find("<apikey>") {
        let start = start + 8;
        if let Some(end) = content[start..].find("</apikey>") {
            return Ok(content[start..start + end].trim().to_string());
        }
    }
    anyhow::bail!("No apikey element found in config")
}

pub fn extract_gui_address_from_xml(content: &str) -> Result<String> {
    let Some(gui_start) = content.find("<gui") else {
        anyhow::bail!("No gui element found in config");
    };
    let gui = &content[gui_start..];
    if let Some(start) = gui.find("<address>") {
        let start = start + 9;
        if let Some(end) = gui[start..].find("</address>") {
            let address = gui[start..start + end].trim();
            if !address.is_empty() {
                return Ok(address.to_string());
            }
        }
    }
    anyhow::bail!("No gui address element found in config")
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
        assert_eq!(config.host(), "http://[::1]:8384");
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
    fn test_extract_gui_address_from_xml() {
        let xml = r#"
<configuration version="37">
    <gui enabled="true" tls="false" debugging="false" sendBasicAuthPrompt="false">
        <address>[::1]:8384</address>
        <apikey>abc123def456</apikey>
    </gui>
</configuration>
"#;
        let address = extract_gui_address_from_xml(xml).unwrap();
        assert_eq!(address, "[::1]:8384");
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
    fn windows_syncthing_config_path_uses_local_app_data() {
        assert_eq!(
            windows_syncthing_config_path(std::path::Path::new("C:/Users/Alice/AppData/Local")),
            PathBuf::from("C:/Users/Alice/AppData/Local/Syncthing/config.xml")
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn syncthing_config_path_prefers_existing_state_path() {
        let dir = tempdir().unwrap();
        let state_path = dir.path().join("state/syncthing/config.xml");
        let config_path = dir.path().join("config/syncthing/config.xml");
        fs::create_dir_all(state_path.parent().unwrap()).unwrap();
        fs::create_dir_all(config_path.parent().unwrap()).unwrap();
        fs::write(&state_path, "<configuration />").unwrap();
        fs::write(&config_path, "<configuration />").unwrap();

        assert_eq!(
            existing_syncthing_config_path(&[state_path.clone(), config_path]),
            Some(state_path)
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn syncthing_config_path_uses_config_path_when_state_file_is_absent() {
        let dir = tempdir().unwrap();
        let state_path = dir.path().join("state/syncthing/config.xml");
        let config_path = dir.path().join("config/syncthing/config.xml");
        fs::create_dir_all(config_path.parent().unwrap()).unwrap();
        fs::write(&config_path, "<configuration />").unwrap();

        assert_eq!(
            existing_syncthing_config_path(&[state_path, config_path.clone()]),
            Some(config_path)
        );
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
    fn test_extract_gui_address_from_path() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.xml");

        let xml = "<configuration><gui><address>[::1]:8384</address></gui></configuration>";
        fs::write(&path, xml).unwrap();

        let address = extract_gui_address_from_path(&path).unwrap();
        assert_eq!(address, "[::1]:8384");
    }

    #[test]
    fn test_extract_api_key_from_missing_file() {
        let path = PathBuf::from("/nonexistent/config.xml");
        let result = extract_api_key_from_path(&path);
        assert!(result.is_err());
    }
}
