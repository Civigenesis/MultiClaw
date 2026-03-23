//! HTTP client for the [ClawHub](https://github.com/openclaw/clawhub) public skill registry (`https://clawhub.ai`).
//! Routes follow the official `clawhub` CLI ([docs/cli.md](https://github.com/openclaw/clawhub/blob/main/docs/cli.md)).

use anyhow::{Context, Result};
use std::time::Duration;

/// Default registry base URL (same as `clawhub` CLI `--registry`).
pub const DEFAULT_REGISTRY_URL: &str = "https://clawhub.ai";

#[derive(Clone)]
pub struct ClawHubRegistry {
    base_url: String,
    client: reqwest::Client,
    token: Option<String>,
}

impl ClawHubRegistry {
    pub fn new(base_url: String, timeout_ms: u64, token: Option<String>) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_millis(timeout_ms.max(1_000)))
            .user_agent("MultiClaw/0.1 (ClawHubRegistry)")
            .build()
            .context("build reqwest client for ClawHub")?;
        Ok(Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            client,
            token,
        })
    }

    fn auth_header(&self, mut req: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        if let Some(ref t) = self.token {
            let t = t.trim();
            if !t.is_empty() {
                req = req.header(reqwest::header::AUTHORIZATION, format!("Bearer {t}"));
            }
        }
        req
    }

    /// `GET /api/v1/search?q=...`
    pub async fn search(&self, q: &str) -> Result<String> {
        let url = format!("{}/api/v1/search", self.base_url);
        let req = self.client.get(url).query(&[("q", q)]);
        let req = self.auth_header(req);
        let resp = req.send().await.context("clawhub search request")?;
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            anyhow::bail!("clawhub search HTTP {}: {}", status, text);
        }
        Ok(text)
    }

    /// `GET /api/v1/skills?limit=...`
    pub async fn explore(&self, limit: u32) -> Result<String> {
        let url = format!("{}/api/v1/skills", self.base_url);
        let req = self
            .client
            .get(url)
            .query(&[("limit", limit.min(200).max(1).to_string())]);
        let req = self.auth_header(req);
        let resp = req.send().await.context("clawhub explore request")?;
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            anyhow::bail!("clawhub explore HTTP {}: {}", status, text);
        }
        Ok(text)
    }

    /// `GET /api/v1/skills/{slug}` — metadata JSON.
    pub async fn get_skill_json(&self, slug: &str) -> Result<String> {
        let slug = slug.trim().trim_start_matches('/');
        if slug.is_empty() || slug.contains("..") || slug.contains('/') {
            anyhow::bail!("invalid skill slug");
        }
        let url = format!("{}/api/v1/skills/{slug}", self.base_url);
        let req = self.client.get(url);
        let req = self.auth_header(req);
        let resp = req.send().await.context("clawhub get_skill request")?;
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            anyhow::bail!("clawhub get_skill HTTP {}: {}", status, text);
        }
        Ok(text)
    }

    /// Download a skill version as zip bytes (`GET /api/v1/download` with slug query).
    pub async fn download_zip(&self, slug: &str) -> Result<Vec<u8>> {
        let url = format!("{}/api/v1/download", self.base_url);
        let req = self.client.get(url).query(&[("slug", slug)]);
        let req = self.auth_header(req);
        let resp = req.send().await.context("clawhub download request")?;
        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            anyhow::bail!("clawhub download HTTP {}: {}", status, text);
        }
        let bytes = resp.bytes().await.context("clawhub download body")?;
        Ok(bytes.to_vec())
    }
}
