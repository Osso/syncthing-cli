#![allow(dead_code)]

use anyhow::{Context, Result};
use serde_json::Value;

pub struct Client {
    http: reqwest::Client,
    api_key: String,
    base_url: String,
}

impl Client {
    pub fn new(api_key: &str, base_url: &str) -> Result<Self> {
        let http = reqwest::Client::builder()
            .danger_accept_invalid_certs(true) // Syncthing uses self-signed certs
            .build()?;
        Ok(Self {
            http,
            api_key: api_key.to_string(),
            base_url: base_url.trim_end_matches('/').to_string(),
        })
    }

    async fn get(&self, endpoint: &str) -> Result<Value> {
        let url = format!("{}{}", self.base_url, endpoint);
        let resp = self
            .http
            .get(&url)
            .header("X-API-Key", &self.api_key)
            .send()
            .await
            .context("Failed to send request")?;

        if !resp.status().is_success() {
            anyhow::bail!("API error: {}", resp.status());
        }

        resp.json().await.context("Failed to parse response")
    }

    async fn post(&self, endpoint: &str, body: Option<&Value>) -> Result<Value> {
        let url = format!("{}{}", self.base_url, endpoint);
        let mut req = self.http.post(&url).header("X-API-Key", &self.api_key);

        if let Some(b) = body {
            req = req.json(b);
        }

        let resp = req.send().await.context("Failed to send request")?;

        if !resp.status().is_success() {
            anyhow::bail!("API error: {}", resp.status());
        }

        // Some POST endpoints return empty response
        let text = resp.text().await?;
        if text.is_empty() {
            Ok(Value::Null)
        } else {
            serde_json::from_str(&text).context("Failed to parse response")
        }
    }

    // System endpoints
    pub async fn status(&self) -> Result<Value> {
        self.get("/rest/system/status").await
    }

    pub async fn version(&self) -> Result<Value> {
        self.get("/rest/system/version").await
    }

    pub async fn connections(&self) -> Result<Value> {
        self.get("/rest/system/connections").await
    }

    pub async fn errors(&self) -> Result<Value> {
        self.get("/rest/system/error").await
    }

    pub async fn clear_errors(&self) -> Result<Value> {
        self.post("/rest/system/error/clear", None).await
    }

    pub async fn restart(&self) -> Result<Value> {
        self.post("/rest/system/restart", None).await
    }

    pub async fn shutdown(&self) -> Result<Value> {
        self.post("/rest/system/shutdown", None).await
    }

    // Config endpoints
    pub async fn config(&self) -> Result<Value> {
        self.get("/rest/config").await
    }

    pub async fn config_folders(&self) -> Result<Value> {
        self.get("/rest/config/folders").await
    }

    pub async fn config_devices(&self) -> Result<Value> {
        self.get("/rest/config/devices").await
    }

    // Database endpoints
    pub async fn db_status(&self, folder: &str) -> Result<Value> {
        self.get(&format!("/rest/db/status?folder={}", folder))
            .await
    }

    pub async fn db_completion(&self) -> Result<Value> {
        self.get("/rest/db/completion").await
    }

    pub async fn db_need(&self, folder: &str) -> Result<Value> {
        self.get(&format!("/rest/db/need?folder={}", folder)).await
    }

    pub async fn db_scan(&self, folder: &str) -> Result<Value> {
        self.post(&format!("/rest/db/scan?folder={}", folder), None)
            .await
    }

    pub async fn db_scan_all(&self) -> Result<Value> {
        self.post("/rest/db/scan", None).await
    }

    // Stats endpoints
    pub async fn stats_device(&self) -> Result<Value> {
        self.get("/rest/stats/device").await
    }

    pub async fn stats_folder(&self) -> Result<Value> {
        self.get("/rest/stats/folder").await
    }

    // Cluster endpoints
    pub async fn pending_devices(&self) -> Result<Value> {
        self.get("/rest/cluster/pending/devices").await
    }

    pub async fn pending_folders(&self) -> Result<Value> {
        self.get("/rest/cluster/pending/folders").await
    }

    // Folder endpoints
    pub async fn folder_errors(&self, folder: &str) -> Result<Value> {
        self.get(&format!("/rest/folder/errors?folder={}", folder))
            .await
    }

    // Events
    pub async fn events(&self, since: Option<u64>, limit: Option<u32>) -> Result<Value> {
        let mut url = "/rest/events".to_string();
        let mut params = Vec::new();
        if let Some(s) = since {
            params.push(format!("since={}", s));
        }
        if let Some(l) = limit {
            params.push(format!("limit={}", l));
        }
        if !params.is_empty() {
            url.push('?');
            url.push_str(&params.join("&"));
        }
        self.get(&url).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn test_status() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/rest/system/status"))
            .and(header("X-API-Key", "test-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "alloc": 12345678,
                "sys": 23456789,
                "uptime": 3600
            })))
            .mount(&mock_server)
            .await;

        let client = Client::new("test-key", &mock_server.uri()).unwrap();
        let result = client.status().await.unwrap();

        assert_eq!(result["uptime"], 3600);
        assert_eq!(result["alloc"], 12345678);
    }

    #[tokio::test]
    async fn test_version() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/rest/system/version"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "version": "v1.23.0",
                "longVersion": "syncthing v1.23.0"
            })))
            .mount(&mock_server)
            .await;

        let client = Client::new("test-key", &mock_server.uri()).unwrap();
        let result = client.version().await.unwrap();

        assert_eq!(result["version"], "v1.23.0");
    }

    #[tokio::test]
    async fn test_config_folders() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/rest/config/folders"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                {"id": "folder1", "label": "Documents", "paused": false},
                {"id": "folder2", "label": "Photos", "paused": true}
            ])))
            .mount(&mock_server)
            .await;

        let client = Client::new("test-key", &mock_server.uri()).unwrap();
        let result = client.config_folders().await.unwrap();

        let folders = result.as_array().unwrap();
        assert_eq!(folders.len(), 2);
        assert_eq!(folders[0]["label"], "Documents");
    }

    #[tokio::test]
    async fn test_config_devices() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/rest/config/devices"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                {"deviceID": "ABC123", "name": "Laptop"},
                {"deviceID": "DEF456", "name": "Phone"}
            ])))
            .mount(&mock_server)
            .await;

        let client = Client::new("test-key", &mock_server.uri()).unwrap();
        let result = client.config_devices().await.unwrap();

        let devices = result.as_array().unwrap();
        assert_eq!(devices.len(), 2);
        assert_eq!(devices[0]["name"], "Laptop");
    }

    #[tokio::test]
    async fn test_db_completion() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/rest/db/completion"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "completion": 100.0,
                "globalBytes": 1000000,
                "needBytes": 0
            })))
            .mount(&mock_server)
            .await;

        let client = Client::new("test-key", &mock_server.uri()).unwrap();
        let result = client.db_completion().await.unwrap();

        assert_eq!(result["completion"], 100.0);
        assert_eq!(result["needBytes"], 0);
    }

    #[tokio::test]
    async fn test_errors() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/rest/system/error"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "errors": [
                    {"when": "2024-01-01T00:00:00Z", "message": "Test error"}
                ]
            })))
            .mount(&mock_server)
            .await;

        let client = Client::new("test-key", &mock_server.uri()).unwrap();
        let result = client.errors().await.unwrap();

        let errors = result["errors"].as_array().unwrap();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0]["message"], "Test error");
    }

    #[tokio::test]
    async fn test_post_scan() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/rest/db/scan"))
            .respond_with(ResponseTemplate::new(200).set_body_string(""))
            .mount(&mock_server)
            .await;

        let client = Client::new("test-key", &mock_server.uri()).unwrap();
        let result = client.db_scan_all().await.unwrap();

        assert_eq!(result, Value::Null);
    }

    #[tokio::test]
    async fn test_api_error() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/rest/system/status"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&mock_server)
            .await;

        let client = Client::new("bad-key", &mock_server.uri()).unwrap();
        let result = client.status().await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("401"));
    }

    #[tokio::test]
    async fn test_pending_devices() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/rest/cluster/pending/devices"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
            .mount(&mock_server)
            .await;

        let client = Client::new("test-key", &mock_server.uri()).unwrap();
        let result = client.pending_devices().await.unwrap();

        assert!(result.as_object().unwrap().is_empty());
    }
}
