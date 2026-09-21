//! ESPN public API client.
//!
//! These endpoints power ESPN's own website: no key, undocumented, but
//! stable in practice and the only free real-time NFL play-by-play feed.
//! All models tolerate missing/null fields — ESPN's payloads vary by
//! game state (pre/in/post) and version.

pub mod client;
pub mod error;
pub mod models;

pub use client::EspnClient;
pub use error::{EspnError, Result};
pub use models::{
    Clock, Competition, Competitor, Drive, Drives, Event, GameSummary, Header, HeaderCompetition,
    Period, Play, PlayType, Possession, Scoreboard, ScoringPlay, ScoringType, Status, StatusType,
    Team,
};

/// Site scoreboard API (games list, live status, scores).
pub const SITE_API_BASE: &str = "https://site.api.espn.com/apis/site/v2/sports/football/nfl";
/// Site web API (per-game summary: drives, plays, scoring plays).
pub const WEB_API_BASE: &str = "https://site.web.api.espn.com/apis/site/v2/sports/football/nfl";
