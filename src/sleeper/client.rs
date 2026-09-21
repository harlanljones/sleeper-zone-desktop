//! Async client for the documented Sleeper public API.
//!
//! Read-only, no auth. Sleeper's guidance is roughly 1000 calls/min; the
//! sync engine above this layer owns pacing (see the adaptive poll design
//! in the project plan) — this client performs one request per call.

use serde::de::DeserializeOwned;

use super::{API_BASE, error::Result, error::SleeperError, models::parse_player_map, models::*};

#[derive(Debug, Clone)]
pub struct SleeperClient {
    http: reqwest::Client,
    base: String,
}

impl Default for SleeperClient {
    fn default() -> Self {
        Self::new()
    }
}

impl SleeperClient {
    pub fn new() -> Self {
        Self::with_base(API_BASE)
    }

    /// Override the API base (used by tests pointing at local fixtures).
    pub fn with_base(base: &str) -> Self {
        Self {
            http: reqwest::Client::builder()
                .user_agent(concat!("sleeper-zone-desktop/", env!("CARGO_PKG_VERSION")))
                .build()
                .expect("static client configuration is valid"),
            base: base.trim_end_matches('/').to_string(),
        }
    }

    async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let url = format!("{}/{}", self.base, path.trim_start_matches('/'));
        let resp = self.http.get(&url).send().await?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(SleeperError::Api {
                status: status.as_u16(),
                body,
            });
        }
        Ok(resp.json::<T>().await?)
    }

    /// GET returning `null`-tolerant JSON: Sleeper returns a bare `null`
    /// for empty collections (e.g. a user with no leagues in a season).
    async fn get_list<T: DeserializeOwned>(&self, path: &str) -> Result<Vec<T>> {
        let opt = self.get::<Option<Vec<T>>>(path).await?.unwrap_or_default();
        Ok(opt)
    }

    /// Resolve a username (or user id) to a user account.
    pub async fn user(&self, username: &str) -> Result<Option<User>> {
        self.get(&format!("user/{username}")).await
    }

    /// All NFL leagues the user is in for a season.
    pub async fn user_leagues(&self, user_id: &str, season: &str) -> Result<Vec<League>> {
        self.get_list(&format!("user/{user_id}/leagues/nfl/{season}"))
            .await
    }

    /// League metadata.
    pub async fn league(&self, league_id: &str) -> Result<League> {
        self.get(&format!("league/{league_id}")).await
    }

    /// All rosters in a league.
    pub async fn rosters(&self, league_id: &str) -> Result<Vec<Roster>> {
        self.get_list(&format!("league/{league_id}/rosters")).await
    }

    /// All user memberships in a league.
    pub async fn league_users(&self, league_id: &str) -> Result<Vec<LeagueUser>> {
        self.get_list(&format!("league/{league_id}/users")).await
    }

    /// Draft picks of a league's draft; empty for leagues without one.
    pub async fn draft_picks(&self, draft_id: &str) -> Result<Vec<DraftPick>> {
        self.get_list(&format!("draft/{draft_id}/picks")).await
    }

    /// Season-aggregate stats for every NFL player.
    pub async fn season_stats(&self, season: &str) -> Result<StatsMap> {
        self.get(&format!("stats/nfl/regular/{season}")).await
    }

    /// Per-week stats for every NFL player.
    pub async fn week_stats(&self, season: &str, week: u8) -> Result<StatsMap> {
        self.get(&format!("stats/nfl/regular/{season}/{week}"))
            .await
    }

    /// Weekly matchup scoring entries; pair by `matchup_id` for games.
    pub async fn matchups(&self, league_id: &str, week: u8) -> Result<Vec<Matchup>> {
        self.get_list(&format!("league/{league_id}/matchups/{week}"))
            .await
    }

    /// Current NFL state (season, week).
    pub async fn nfl_state(&self) -> Result<NflState> {
        self.get("state/nfl").await
    }

    /// The full NFL player map (~5 MB, changes at most daily — cache it).
    pub async fn players(&self) -> Result<PlayerMap> {
        let raw = self.get_text("players/nfl").await?;
        Ok(parse_player_map(&raw)?)
    }

    async fn get_text(&self, path: &str) -> Result<String> {
        let url = format!("{}/{}", self.base, path.trim_start_matches('/'));
        let resp = self.http.get(&url).send().await?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(SleeperError::Api {
                status: status.as_u16(),
                body,
            });
        }
        Ok(resp.text().await?)
    }
}
