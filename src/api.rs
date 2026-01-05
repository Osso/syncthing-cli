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
        let http = reqwest::Client::builder().build()?;
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
        self.get(&format!("/rest/db/status?folder={}", folder)).await
    }

    pub async fn db_completion(&self) -> Result<Value> {
        self.get("/rest/db/completion").await
    }

    pub async fn db_need(&self, folder: &str) -> Result<Value> {
        self.get(&format!("/rest/db/need?folder={}", folder)).await
    }

    pub async fn db_scan(&self, folder: &str) -> Result<Value> {
        self.post(&format!("/rest/db/scan?folder={}", folder), None).await
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
        self.get(&format!("/rest/folder/errors?folder={}", folder)).await
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
