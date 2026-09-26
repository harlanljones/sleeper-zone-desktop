//! Typed models for the documented Sleeper API (https://docs.sleeper.com).
//!
//! Every field that Sleeper has been observed to omit or null out carries
//! `#[serde(default)]`: upstream adds and drops fields between versions,
//! so unknown input is skipped and missing optional input is a default,
//! never a parse error.

use std::collections::HashMap;

use serde::{Deserialize, Deserializer, Serialize};

/// Accepts a JSON string or number and yields a String. Sleeper emits
/// `espn_id` both ways depending on the player, so the model must not
/// care which.
fn string_or_number<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum MaybeNumber {
        Str(String),
        Num(serde_json::Number),
    }
    match Option::<MaybeNumber>::deserialize(deserializer)? {
        None => Ok(None),
        Some(MaybeNumber::Str(s)) => Ok(Some(s)),
        Some(MaybeNumber::Num(n)) => Ok(Some(n.to_string())),
    }
}

/// Treats a JSON `null` the same as a missing field: `#[serde(default)]`
/// alone only covers absence, but Sleeper nulls out array fields too.
fn vec_or_null<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(Option::<Vec<String>>::deserialize(deserializer)?.unwrap_or_default())
}

/// Current NFL league state (season, week, scoring availability).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct NflState {
    pub week: u8,
    pub season: String,
    pub season_type: String,
    #[serde(default)]
    pub league_season: Option<String>,
    #[serde(default)]
    pub previous_season: Option<String>,
    #[serde(default)]
    pub display_week: Option<u8>,
    #[serde(default)]
    pub leg: Option<u8>,
    #[serde(default)]
    pub season_start_date: Option<String>,
    #[serde(default)]
    pub season_has_scores: Option<bool>,
}

/// A Sleeper user account.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct User {
    pub user_id: String,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub avatar: Option<String>,
    #[serde(default)]
    pub is_bot: Option<bool>,
}

/// A fantasy league. `scoring_settings` values may be ints or floats in
/// upstream JSON, so they are coerced to `f64`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct League {
    pub league_id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub season: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub sport: Option<String>,
    #[serde(default)]
    pub season_type: Option<String>,
    #[serde(default)]
    pub total_rosters: Option<u8>,
    #[serde(default)]
    pub roster_positions: Option<Vec<String>>,
    #[serde(default)]
    pub scoring_settings: Option<HashMap<String, f64>>,
    #[serde(default)]
    pub settings: Option<serde_json::Map<String, serde_json::Value>>,
    #[serde(default)]
    pub previous_league_id: Option<String>,
    #[serde(default)]
    pub draft_id: Option<String>,
    #[serde(default)]
    pub avatar: Option<String>,
}

/// Per-roster record fields used for standings and ownership lookups.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct RosterSettings {
    #[serde(default)]
    pub wins: Option<u16>,
    #[serde(default)]
    pub losses: Option<u16>,
    #[serde(default)]
    pub ties: Option<u16>,
    #[serde(default)]
    pub fpts: Option<f64>,
    #[serde(default)]
    pub fpts_decimal: Option<u16>,
    #[serde(default)]
    pub fpts_against: Option<f64>,
    #[serde(default)]
    pub fpts_against_decimal: Option<u16>,
    #[serde(default)]
    pub waiver_position: Option<u16>,
    #[serde(default)]
    pub waiver_budget_used: Option<u16>,
    #[serde(default)]
    pub total_moves: Option<u16>,
}

/// A team (roster) inside a league.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct Roster {
    pub roster_id: u8,
    pub league_id: String,
    #[serde(default)]
    pub owner_id: Option<String>,
    #[serde(default)]
    pub co_owners: Option<Vec<String>>,
    #[serde(default, deserialize_with = "vec_or_null")]
    pub players: Vec<String>,
    #[serde(default, deserialize_with = "vec_or_null")]
    pub starters: Vec<String>,
    #[serde(default)]
    pub reserve: Option<Vec<String>>,
    #[serde(default)]
    pub taxi: Option<Vec<String>>,
    #[serde(default)]
    pub keepers: Option<Vec<String>>,
    #[serde(default)]
    pub settings: Option<RosterSettings>,
}

/// A user's membership in a specific league.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct LeagueUser {
    pub user_id: String,
    pub league_id: String,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub avatar: Option<String>,
    #[serde(default)]
    pub is_owner: Option<bool>,
    #[serde(default)]
    pub metadata: Option<serde_json::Map<String, serde_json::Value>>,
}

/// One draft pick as returned by `GET draft/{id}/picks`. Unknown fields
/// (metadata, keeper flags) are skipped.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct DraftPick {
    #[serde(default)]
    pub round: Option<u8>,
    #[serde(default, rename = "draft_slot")]
    pub slot: Option<u16>,
    #[serde(default)]
    pub roster_id: Option<u8>,
    #[serde(default)]
    pub player_id: Option<String>,
    #[serde(default)]
    pub picked_by: Option<String>,
}

/// One player's stats payload: the flat per-player map the stats
/// endpoints return, e.g. `{"pass_yd": 315.0, "gp": 1.0}`.
/// Non-numeric entries (null, strings, nested maps) are skipped rather
/// than failing the multi-megabyte payload.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PlayerStats {
    pub stats: HashMap<String, f64>,
}

impl<'de> Deserialize<'de> for PlayerStats {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = Option::<serde_json::Map<String, serde_json::Value>>::deserialize(deserializer)?;
        let stats = raw
            .unwrap_or_default()
            .into_iter()
            .filter_map(|(k, v)| v.as_f64().map(|n| (k, n)))
            .collect();
        Ok(PlayerStats { stats })
    }
}

/// player id -> stats payload, as returned by the stats endpoints.
pub type StatsMap = HashMap<String, PlayerStats>;

impl PlayerStats {
    pub fn get(&self, key: &str) -> f64 {
        self.stats.get(key).copied().unwrap_or(0.0)
    }
}

/// One roster's scoring entry for a week. Paired entries by `matchup_id`
/// form a head-to-head game; `players_points` carries per-player fantasy
/// points already recomputed by Sleeper under the league's settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct Matchup {
    pub matchup_id: u8,
    pub roster_id: u8,
    pub points: f64,
    #[serde(default)]
    pub custom_points: Option<f64>,
    #[serde(default, deserialize_with = "vec_or_null")]
    pub starters: Vec<String>,
    #[serde(default)]
    pub starters_points: Option<Vec<f64>>,
    #[serde(default, deserialize_with = "vec_or_null")]
    pub players: Vec<String>,
    #[serde(default)]
    pub players_points: HashMap<String, f64>,
}

/// The fields of a player we need to render and to join with ESPN data.
/// The full `/players/nfl` payload has dozens more fields per entry; they
/// are skipped on parse.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct Player {
    #[serde(default)]
    pub first_name: Option<String>,
    #[serde(default)]
    pub last_name: Option<String>,
    #[serde(default)]
    pub position: Option<String>,
    #[serde(default, deserialize_with = "vec_or_null")]
    pub fantasy_positions: Vec<String>,
    #[serde(default)]
    pub team: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub injury_status: Option<String>,
    #[serde(default, deserialize_with = "string_or_number")]
    pub espn_id: Option<String>,
    #[serde(default)]
    pub number: Option<u8>,
    #[serde(default)]
    pub age: Option<u8>,
}

impl Player {
    /// Full display name, best-effort.
    pub fn display_name(&self) -> String {
        match (&self.first_name, &self.last_name) {
            (Some(f), Some(l)) => format!("{f} {l}"),
            (Some(f), None) => f.clone(),
            (None, Some(l)) => l.clone(),
            (None, None) => "Unknown".to_string(),
        }
    }
}

/// Player map keyed by Sleeper player id, as returned by `/players/nfl`.
pub type PlayerMap = HashMap<String, Player>;

/// Parse the player map tolerantly: upstream emits bare `null` values
/// for some ids (defensive team stubs and pruned entries); those are
/// dropped rather than failing the whole ~5 MB decode.
pub fn parse_player_map(raw: &str) -> serde_json::Result<PlayerMap> {
    let raw: HashMap<String, Option<Player>> = serde_json::from_str(raw)?;
    Ok(raw
        .into_iter()
        .filter_map(|(k, v)| v.map(|p| (k, p)))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matchup_parses_missing_optional_fields() {
        let m: Matchup =
            serde_json::from_str(r#"{"matchup_id": 2, "roster_id": 1, "points": 20.0}"#)
                .expect("minimal matchup must parse");
        assert_eq!(m.matchup_id, 2);
        assert_eq!(m.roster_id, 1);
        assert!((m.points - 20.0).abs() < f64::EPSILON);
        assert!(m.players_points.is_empty());
        assert!(m.custom_points.is_none());
    }

    #[test]
    fn league_accepts_unknown_fields() {
        let l: League =
            serde_json::from_str(r#"{"league_id": "1", "name": "X", "brand_new_field": {"a": 1}}"#)
                .expect("unknown fields must be skipped, not rejected");
        assert_eq!(l.league_id, "1");
        assert_eq!(l.name.as_deref(), Some("X"));
    }

    #[test]
    fn player_map_drops_null_entries() {
        let raw = r#"{"1": {"first_name": "A"}, "2": null, "3": {"espn_id": 42}}"#;
        let map = parse_player_map(raw).expect("null-tolerant parse");
        assert_eq!(map.len(), 2);
        assert_eq!(map["3"].espn_id.as_deref(), Some("42"));
    }

    #[test]
    fn player_display_name_handles_missing_parts() {
        let p: Player = serde_json::from_str(r#"{"first_name": "Josh"}"#).expect("parses");
        assert_eq!(p.display_name(), "Josh");
        let empty: Player = serde_json::from_str(r#"{}"#).expect("parses");
        assert_eq!(empty.display_name(), "Unknown");
    }

    #[test]
    fn player_espn_id_accepts_string_or_number() {
        let a: Player = serde_json::from_str(r#"{"espn_id": "16736"}"#).expect("string form");
        let b: Player = serde_json::from_str(r#"{"espn_id": 16736}"#).expect("number form");
        assert_eq!(a.espn_id.as_deref(), Some("16736"));
        assert_eq!(b.espn_id.as_deref(), Some("16736"));
        let none: Player = serde_json::from_str(r#"{"espn_id": null}"#).expect("null form");
        assert_eq!(none.espn_id, None);
    }

    #[test]
    fn user_parses_bot_flag_as_nullable() {
        let u: User = serde_json::from_str(
            r#"{"user_id": "42", "display_name": "SleeperBot", "is_bot": true}"#,
        )
        .expect("parses");
        assert_eq!(u.is_bot, Some(true));
        assert_eq!(u.username, None);
    }
}
