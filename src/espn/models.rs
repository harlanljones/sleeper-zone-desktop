//! Typed models for the ESPN scoreboard and game-summary payloads.
//!
//! Mirrors only the fields the tracker needs. ESPN payloads differ by
//! game state and grow new fields freely, so everything optional is
//! `Option`/default and unknown fields are skipped.

use serde::{Deserialize, Deserializer, Serialize};

/// Treats a JSON `null` like a missing field (ESPN nulls arrays for
/// games without the data yet).
fn vec_or_null<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Ok(Option::<Vec<T>>::deserialize(deserializer)?.unwrap_or_default())
}

/// `drives.current` is an object mid-game and a list (or null) otherwise.
/// The tracker only consumes `previous`, so anything that isn't a list
/// deserializes as empty instead of failing the whole summary.
fn current_drives<'de, D>(deserializer: D) -> Result<Vec<Drive>, D::Error>
where
    D: Deserializer<'de>,
{
    match Option::<serde_json::Value>::deserialize(deserializer)? {
        Some(serde_json::Value::Array(items)) => {
            Ok(serde_json::from_value(serde_json::Value::Array(items)).unwrap_or_default())
        }
        _ => Ok(Vec::new()),
    }
}

/// Game state machine: "pre" | "in" | "post".
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct StatusType {
    #[serde(default)]
    pub state: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub detail: Option<String>,
    #[serde(default)]
    pub completed: Option<bool>,
}

/// Game clock/status as shown on the scoreboard.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    #[serde(default)]
    pub clock: Option<f64>,
    #[serde(default)]
    pub display_clock: Option<String>,
    #[serde(default)]
    pub period: Option<u8>,
    #[serde(default, rename = "type")]
    pub status_type: Option<StatusType>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Team {
    pub id: String,
    #[serde(default)]
    pub abbreviation: Option<String>,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub short_display_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Competitor {
    pub id: String,
    #[serde(default)]
    pub home_away: Option<String>,
    /// Score arrives as a string on the scoreboard and a number in some
    /// summary shapes; kept as string and parsed by the engine.
    #[serde(default)]
    pub score: Option<String>,
    #[serde(default)]
    pub team: Option<Team>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Competition {
    #[serde(default)]
    pub date: Option<String>,
    #[serde(default)]
    pub status: Option<Status>,
    #[serde(default, deserialize_with = "vec_or_null")]
    pub competitors: Vec<Competitor>,
    #[serde(default)]
    pub venue: Option<serde_json::Map<String, serde_json::Value>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Event {
    pub id: String,
    #[serde(default)]
    pub date: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default, deserialize_with = "vec_or_null")]
    pub competitions: Vec<Competition>,
}

impl Event {
    /// Primary competition (ESPN lists exactly one for NFL games).
    pub fn competition(&self) -> Option<&Competition> {
        self.competitions.first()
    }

    /// True while the game is being played.
    pub fn is_live(&self) -> bool {
        self.competition()
            .and_then(|c| c.status.as_ref())
            .and_then(|s| s.status_type.as_ref())
            .and_then(|t| t.state.as_deref())
            .is_some_and(|s| s == "in")
    }
}

/// `GET scoreboard` response: the week's games.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Scoreboard {
    /// ESPN wraps the week: `{"number": 2, ...}`.
    #[serde(default)]
    pub week: Option<serde_json::Map<String, serde_json::Value>>,
    #[serde(default)]
    pub season: Option<serde_json::Map<String, serde_json::Value>>,
    #[serde(default, deserialize_with = "vec_or_null")]
    pub events: Vec<Event>,
}

impl Scoreboard {
    pub fn week_number(&self) -> Option<u8> {
        self.week
            .as_ref()
            .and_then(|w| w.get("number"))
            .and_then(|n| n.as_u64())
            .and_then(|n| u8::try_from(n).ok())
    }

    pub fn season_year(&self) -> Option<u16> {
        self.season
            .as_ref()
            .and_then(|s| s.get("year"))
            .and_then(|y| y.as_u64())
            .and_then(|y| u16::try_from(y).ok())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayType {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub abbreviation: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Possession {
    #[serde(default)]
    pub down: Option<i8>,
    #[serde(default)]
    pub distance: Option<i16>,
    #[serde(default)]
    pub yards_to_endzone: Option<i16>,
    #[serde(default, rename = "shortDownDistanceText")]
    pub short_down_distance_text: Option<String>,
    #[serde(default, rename = "possessionText")]
    pub possession_text: Option<String>,
    #[serde(default)]
    pub team: Option<serde_json::Map<String, serde_json::Value>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Clock {
    #[serde(default)]
    pub value: Option<f64>,
    #[serde(default, rename = "displayValue")]
    pub display_value: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Period {
    #[serde(default)]
    pub number: Option<u8>,
}

/// One play inside a drive. `text` is the human-readable recap; the
/// per-play player stats live behind `$ref` URLs and are intentionally
/// not fetched (the fantasy points come from Sleeper).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Play {
    pub id: String,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default, rename = "type")]
    pub play_type: Option<PlayType>,
    #[serde(default)]
    pub scoring_play: Option<bool>,
    #[serde(default)]
    pub period: Option<Period>,
    #[serde(default)]
    pub clock: Option<Clock>,
    #[serde(default)]
    pub home_score: Option<i16>,
    #[serde(default)]
    pub away_score: Option<i16>,
    #[serde(default)]
    pub stat_yardage: Option<i16>,
    #[serde(default)]
    pub start: Option<Possession>,
    #[serde(default, rename = "end")]
    pub end_possession: Option<Possession>,
    #[serde(default)]
    pub is_turnover: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Drive {
    pub id: String,
    #[serde(default)]
    pub team: Option<Team>,
    #[serde(default, deserialize_with = "vec_or_null")]
    pub plays: Vec<Play>,
    #[serde(default, rename = "isScore")]
    pub is_score: Option<bool>,
    #[serde(default)]
    pub yards: Option<i16>,
    #[serde(default)]
    pub result: Option<String>,
    #[serde(default)]
    pub display_result: Option<String>,
}

/// Drives container: `current` exists mid-game, `previous` always.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Drives {
    #[serde(default, deserialize_with = "current_drives")]
    pub current: Vec<Drive>,
    #[serde(default, deserialize_with = "vec_or_null")]
    pub previous: Vec<Drive>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScoringType {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub abbreviation: Option<String>,
}

/// One scoring event (TD, FG, safety, 2pt) with running score.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScoringPlay {
    pub id: String,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default, rename = "type")]
    pub play_type: Option<PlayType>,
    #[serde(default)]
    pub scoring_type: Option<ScoringType>,
    #[serde(default)]
    pub period: Option<Period>,
    #[serde(default)]
    pub clock: Option<Clock>,
    #[serde(default)]
    pub team: Option<Team>,
    #[serde(default)]
    pub home_score: Option<i16>,
    #[serde(default)]
    pub away_score: Option<i16>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HeaderCompetition {
    #[serde(default)]
    pub status: Option<Status>,
    #[serde(default, deserialize_with = "vec_or_null")]
    pub competitors: Vec<Competitor>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Header {
    #[serde(default, deserialize_with = "vec_or_null")]
    pub competitions: Vec<HeaderCompetition>,
}

/// `GET summary?event=<id>` response.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameSummary {
    #[serde(default)]
    pub header: Option<Header>,
    #[serde(default)]
    pub drives: Option<Drives>,
    #[serde(default, deserialize_with = "vec_or_null")]
    pub scoring_plays: Vec<ScoringPlay>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_is_live_reads_status_state() {
        let e: Event = serde_json::from_str(
            r#"{"id": "1", "competitions": [{"status": {"type": {"state": "in"}}}]}"#,
        )
        .expect("parses");
        assert!(e.is_live());
        let done: Event = serde_json::from_str(
            r#"{"id": "2", "competitions": [{"status": {"type": {"state": "post"}}}]}"#,
        )
        .expect("parses");
        assert!(!done.is_live());
    }

    #[test]
    fn null_competitors_become_empty() {
        let c: Competition = serde_json::from_str(r#"{"competitors": null}"#).expect("parses");
        assert!(c.competitors.is_empty());
    }

    #[test]
    fn drives_current_object_is_tolerated() {
        // Live games carry `drives.current` as an object, not a list; the
        // summary must still parse and keep its scoring plays.
        let g: GameSummary = serde_json::from_str(
            r#"{"drives": {"current": {"id": "1", "plays": []}}, "scoringPlays": [{"id": "9", "text": "TD"}]}"#,
        )
        .expect("live-shaped summary parses");
        assert_eq!(g.scoring_plays.len(), 1);
        assert!(g.drives.unwrap().current.is_empty());
    }
}
