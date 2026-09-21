//! Async client for ESPN's public NFL endpoints (no key, undocumented).

use serde::de::DeserializeOwned;

use super::{SITE_API_BASE, WEB_API_BASE, error::EspnError, error::Result, models::*};

#[derive(Debug, Clone)]
pub struct EspnClient {
    http: reqwest::Client,
    site_base: String,
    web_base: String,
}

impl Default for EspnClient {
    fn default() -> Self {
        Self::new()
    }
}

impl EspnClient {
    pub fn new() -> Self {
        Self::with_bases(SITE_API_BASE, WEB_API_BASE)
    }

    /// Override the bases (tests point these at local fixture servers or
    /// the real hosts).
    pub fn with_bases(site_base: &str, web_base: &str) -> Self {
        Self {
            http: reqwest::Client::builder()
                // ESPN's edge (Akamai) 403s unknown client UAs but
                // allowlists libcurl-style ones; identify ourselves
                // inside a libcurl-compatible UA string.
                .user_agent(concat!(
                    "sleeper-zone-desktop/",
                    env!("CARGO_PKG_VERSION"),
                    " (libcurl/8.9.1)"
                ))
                .default_headers({
                    let mut h = reqwest::header::HeaderMap::new();
                    h.insert(
                        reqwest::header::ACCEPT,
                        "application/json".parse().expect("static header"),
                    );
                    h.insert(
                        reqwest::header::REFERER,
                        "https://www.espn.com/".parse().expect("static header"),
                    );
                    h
                })
                .build()
                .expect("static client configuration is valid"),
            site_base: site_base.trim_end_matches('/').to_string(),
            web_base: web_base.trim_end_matches('/').to_string(),
        }
    }

    async fn get<T: DeserializeOwned>(&self, url: &str) -> Result<T> {
        let resp = self.http.get(url).send().await?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(EspnError::Api {
                status: status.as_u16(),
                body,
            });
        }
        Ok(resp.json::<T>().await?)
    }

    /// Current week's scoreboard (all games, live status and scores).
    pub async fn scoreboard(&self) -> Result<Scoreboard> {
        self.get(&format!("{}/scoreboard", self.site_base)).await
    }

    /// Scoreboard restricted to a season/week: seasontype 2 is regular
    /// season.
    pub async fn scoreboard_week(&self, season: u16, week: u8) -> Result<Scoreboard> {
        self.get(&format!(
            "{}/scoreboard?seasontype=2&year={season}&week={week}",
            self.site_base
        ))
        .await
    }

    /// Full game summary: drives/plays, scoring plays, header status.
    pub async fn summary(&self, event_id: &str) -> Result<GameSummary> {
        self.get(&format!("{}/summary?event={event_id}", self.web_base))
            .await
    }
}
