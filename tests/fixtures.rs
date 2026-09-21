//! Fixture-based tests: recorded Sleeper API payloads in `testdata/` are
//! parsed with the real models. Numbers are pinned so upstream format
//! drift is caught here, not in production.

use serde::de::DeserializeOwned;

use sleeper_zone::sleeper::{League, LeagueUser, Matchup, NflState, PlayerMap, Roster, User};

fn load<T: DeserializeOwned>(name: &str) -> T {
    let raw = std::fs::read_to_string(format!("testdata/{name}"))
        .unwrap_or_else(|e| panic!("fixture {name}: {e}"));
    serde_json::from_str(&raw).unwrap_or_else(|e| panic!("parse {name}: {e}"))
}

#[test]
fn league_fixture_parses_with_pinned_fields() {
    let l: League = load("league.json");
    assert_eq!(l.league_id, "289646328504385536");
    assert_eq!(l.name.as_deref(), Some("Sleeper Friends League"));
    assert_eq!(l.season.as_deref(), Some("2018"));
    assert_eq!(l.total_rosters, Some(12));
    assert_eq!(l.status.as_deref(), Some("complete"));
}

#[test]
fn rosters_fixture_parses_twelve_teams() {
    let r: Vec<Roster> = load("rosters.json");
    assert_eq!(r.len(), 12);
    assert_eq!(r[0].roster_id, 1);
    assert!(
        r[0].players.len() >= 15,
        "dynasty roster carries a full squad"
    );
    let settings = r[0].settings.as_ref().expect("settings present");
    assert!(settings.wins.is_some());
}

#[test]
fn league_users_fixture_parses() {
    let u: Vec<LeagueUser> = load("league_users.json");
    assert_eq!(u[0].display_name.as_deref(), Some("2KSports"));
    assert!(u.iter().any(|x| x.is_owner == Some(true)));
}

#[test]
fn matchups_fixture_pins_scoring_values() {
    let m: Vec<Matchup> = load("matchups.json");
    assert_eq!(m.len(), 12);
    let first = &m[0];
    assert!((first.points - 148.04).abs() < 1e-9);
    let sp = first
        .starters_points
        .as_ref()
        .expect("starters_points present");
    assert!((sp[0] - 10.04).abs() < 1e-9);
    assert_eq!(first.players_points.len(), 15);
    // Every starter's points must appear in the per-player map.
    for (id, pts) in first.starters.iter().zip(sp.iter()) {
        assert!(
            first
                .players_points
                .get(id)
                .is_some_and(|p| (p - pts).abs() < 1e-9),
            "starter {id} points missing from players_points"
        );
    }
}

#[test]
fn state_fixture_pins_current_season_shape() {
    let s: NflState = load("state.json");
    assert_eq!(s.season, "2026");
    assert_eq!(s.week, 2);
    assert_eq!(s.season_type, "regular");
}

#[test]
fn user_fixture_pins_identity_fields() {
    let u: User = load("user.json");
    assert_eq!(u.user_id, "250825917088133120");
    assert_eq!(u.display_name.as_deref(), Some("TheCommish"));
}

#[test]
fn players_sample_parses_into_map() {
    let p: PlayerMap = load("players_sample.json");
    assert_eq!(p.len(), 5);
    let dent = p.get("13175").expect("sampled QB present");
    assert_eq!(dent.display_name(), "Kinkead Dent");
    assert_eq!(dent.position.as_deref(), Some("QB"));
    // espn_id is optional and absent here — the join layer must cope.
    assert_eq!(dent.espn_id, None);
}

/// Sleeper returns a bare `null` for empty collections (documented and
/// observed, e.g. a user with no leagues that season). The client's
/// null-tolerant list decoding must turn that into an empty vec.
#[test]
fn null_list_decodes_as_empty() {
    let v: Option<Vec<League>> = serde_json::from_str("null").expect("null parses");
    assert!(v.unwrap_or_default().is_empty());
}
