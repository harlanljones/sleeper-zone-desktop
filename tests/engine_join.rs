//! Engine join tests: recorded Sleeper league payloads joined against
//! the recorded ESPN scoreboard, pinned end-to-end.

use std::collections::{BTreeMap, HashMap};

use sleeper_zone::engine::join::{build_abbreviation_map, game_view_of, resolve_game};
use sleeper_zone::engine::{GameRef, PlayerView, TrackerSnapshot};
use sleeper_zone::espn::Scoreboard;
use sleeper_zone::sleeper::{Matchup, Player, PlayerMap};

fn load<T: serde::de::DeserializeOwned>(name: &str) -> T {
    let raw = std::fs::read_to_string(format!("testdata/{name}"))
        .unwrap_or_else(|e| panic!("fixture {name}: {e}"));
    serde_json::from_str(&raw).unwrap_or_else(|e| panic!("parse {name}: {e}"))
}

/// A DAL player joined through the scoreboard must land in the WAS@DAL
/// game, and a DST slot ("PHI" roster id) in the PHI game.
#[test]
fn sleeper_players_join_onto_espn_games() {
    let sb: Scoreboard = load("espn/scoreboard.json");
    let abbr = build_abbreviation_map(&sb);

    let lamb: Player =
        serde_json::from_str(r#"{"first_name": "CeeDee", "last_name": "Lamb", "team": "DAL"}"#)
            .unwrap();
    let eagles_dst: Player = serde_json::from_str(
        r#"{"first_name": "Philadelphia", "last_name": "Eagles", "team": "PHI"}"#,
    )
    .unwrap();
    assert_eq!(resolve_game(&lamb, &abbr).as_deref(), Some("401872944"));
    assert_eq!(
        resolve_game(&eagles_dst, &abbr).as_deref(),
        Some("401872939")
    );
}

/// End-to-end team-view join built the way Tracker::team_view does,
/// using the recorded matchups fixture and a trimmed player map.
#[test]
fn team_view_carries_points_starter_flags_and_game_state() {
    let sb: Scoreboard = load("espn/scoreboard.json");
    let abbr = build_abbreviation_map(&sb);
    let mut players: PlayerMap = HashMap::new();
    for (id, raw) in [
        (
            "4035",
            r#"{"first_name": "CeeDee", "last_name": "Lamb", "team": "DAL", "position": "WR"}"#,
        ),
        (
            "PHI",
            r#"{"first_name": "Philadelphia", "last_name": "Eagles", "team": "PHI", "position": "DEF"}"#,
        ),
        ("9999", r#"{"first_name": "Ghost", "team": null}"#),
    ] {
        players.insert(
            id.to_string(),
            serde_json::from_str::<Player>(raw).expect("player parses"),
        );
    }
    let matchups: Vec<Matchup> = load("matchups.json");
    let m = &matchups[0]; // roster 1, 148.04 points
    assert_eq!(m.roster_id, 1);

    // Rebuild the team view exactly as Tracker does.
    let games: BTreeMap<String, sleeper_zone::engine::GameView> = sb
        .events
        .iter()
        .map(|e| (e.id.clone(), game_view_of(e, None, 8)))
        .collect();
    let starters: Vec<String> = m.starters.clone();
    let views: Vec<PlayerView> = m
        .players
        .iter()
        .map(|pid| {
            let player = players.get(pid);
            PlayerView {
                sleeper_id: pid.clone(),
                name: player.map(Player::display_name).unwrap_or_default(),
                position: player.and_then(|p| p.position.clone()),
                nfl_team: player.and_then(|p| p.team.clone()),
                points: m.players_points.get(pid).copied().unwrap_or(0.0),
                is_starter: starters.contains(pid),
                game: player.and_then(|p| resolve_game(p, &abbr)).and_then(|eid| {
                    games.get(&eid).map(|g| GameRef {
                        espn_event_id: eid,
                        state: g.state.clone(),
                    })
                }),
                injury_status: player.and_then(|p| p.injury_status.clone()),
                number: player.and_then(|p| p.number),
                age: player.and_then(|p| p.age),
            }
        })
        .collect();

    let starters_view: Vec<&PlayerView> = views.iter().filter(|v| v.is_starter).collect();
    assert_eq!(starters_view.len(), 9, "recorded matchup has 9 starters");
    let lamb_view = views
        .iter()
        .find(|v| v.sleeper_id == "4035")
        .expect("Lamb in matchup");
    assert!(lamb_view.is_starter);
    assert_eq!(
        lamb_view.game.as_ref().unwrap().state.as_deref(),
        Some("post")
    );
    assert_eq!(lamb_view.game.as_ref().unwrap().espn_event_id, "401872944");
    let phi_dst = views.iter().find(|v| v.sleeper_id == "PHI").unwrap();
    assert_eq!(phi_dst.game.as_ref().unwrap().espn_event_id, "401872939");
    // Ghost player has no NFL team: no game context, but still listed.
    let ghost = views.iter().find(|v| v.sleeper_id == "9999");
    if let Some(g) = ghost {
        assert!(g.game.is_none());
    }
}

#[test]
fn snapshot_live_detection_switches_cadence() {
    use sleeper_zone::engine::{IDLE_POLL, TRACKED_POLL, next_delay};

    let mut snapshot = TrackerSnapshot::default();
    assert_eq!(next_delay(snapshot.any_game_live()), IDLE_POLL);
    snapshot.games.insert(
        "e1".into(),
        sleeper_zone::engine::GameView {
            espn_event_id: "e1".into(),
            state: Some("in".into()),
            period: Some(2),
            display_clock: None,
            home_abbr: None,
            away_abbr: None,
            home_score: None,
            away_score: None,
            scoring_plays: vec![],
        },
    );
    assert!(snapshot.any_game_live());
    assert_eq!(next_delay(snapshot.any_game_live()), TRACKED_POLL);
}
