//! Config and cache persistence: `$XDG_CONFIG_HOME|~/.config/sleeper-zone/`.
//!
//! - `config.json`: username, user id, tracked league ids.
//! - `players.json`: the ~5 MB Sleeper player map, refreshed at most
//!   daily (it changes at most daily and costs one big fetch).

use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::sleeper::{PlayerMap, parse_player_map};

/// How long the cached player map stays fresh.
pub const PLAYER_CACHE_TTL: Duration = Duration::from_secs(24 * 60 * 60);

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Config {
    pub username: String,
    pub user_id: String,
    pub league_ids: Vec<String>,
}

/// Base dir for config and cache files.
pub fn config_dir() -> Option<PathBuf> {
    let base = std::env::var("XDG_CONFIG_HOME")
        .ok()
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var("HOME")
                .ok()
                .map(|h| PathBuf::from(h).join(".config"))
        })?;
    Some(base.join("sleeper-zone"))
}

fn config_path() -> Option<PathBuf> {
    config_dir().map(|d| d.join("config.json"))
}

fn players_cache_path() -> Option<PathBuf> {
    config_dir().map(|d| d.join("players.json"))
}

impl Config {
    pub fn load() -> Option<Config> {
        let raw = fs::read_to_string(config_path()?).ok()?;
        serde_json::from_str(&raw).ok()
    }

    pub fn save(&self) -> std::io::Result<()> {
        let dir = config_dir().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::NotFound, "no config dir available")
        })?;
        fs::create_dir_all(&dir)?;
        let json = serde_json::to_string_pretty(self)?;
        fs::write(dir.join("config.json"), json)
    }
}

/// Load the cached player map if it exists and is younger than the TTL.
pub fn load_players_cache() -> Option<PlayerMap> {
    let path = players_cache_path()?;
    let meta = fs::metadata(&path).ok()?;
    let modified = meta.modified().ok()?;
    if modified.elapsed().ok()? > PLAYER_CACHE_TTL {
        return None;
    }
    let raw = fs::read_to_string(path).ok()?;
    parse_player_map(&raw).ok()
}

/// Persist the player map (best effort; a failed cache write just means
/// the next launch refetches).
pub fn save_players_cache(players: &PlayerMap) {
    let Some(path) = players_cache_path() else {
        return;
    };
    let Some(dir) = path.parent() else {
        return;
    };
    if fs::create_dir_all(dir).is_err() {
        return;
    }
    if let Ok(json) = serde_json::to_string(players) {
        let _ = fs::write(path, json);
    }
}

/// Seconds since epoch, for cache-age display.
pub fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_roundtrips_through_json() {
        let cfg = Config {
            username: "thecommish".into(),
            user_id: "250825917088133120".into(),
            league_ids: vec!["289646328504385536".into()],
        };
        let json = serde_json::to_string(&cfg).expect("serializes");
        let back: Config = serde_json::from_str(&json).expect("deserializes");
        assert_eq!(back.username, "thecommish");
        assert_eq!(back.user_id, "250825917088133120");
        assert_eq!(back.league_ids, vec!["289646328504385536"]);
    }

    #[test]
    fn config_resolves_under_home_config() {
        let dir = config_dir().expect("a HOME or XDG_CONFIG_HOME exists");
        assert!(dir.ends_with("sleeper-zone"), "dir: {dir:?}");
    }
}
