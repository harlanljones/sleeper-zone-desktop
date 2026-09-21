//! Sleeper API client: models, errors, HTTP access.

pub mod client;
pub mod error;
pub mod models;

pub use client::SleeperClient;
pub use error::{Result, SleeperError};
pub use models::parse_player_map;
pub use models::{
    DraftPick, League, LeagueUser, Matchup, NflState, Player, PlayerMap, PlayerStats, Roster,
    StatsMap, User,
};

/// Base URL of the documented Sleeper public API.
pub const API_BASE: &str = "https://api.sleeper.app/v1";
