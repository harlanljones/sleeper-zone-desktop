//! Fixture-based tests for the ESPN client models. Payloads recorded
//! live (2026-09-20, week 2) into `testdata/espn/`; numbers are pinned
//! so upstream format drift is caught here.

use serde::de::DeserializeOwned;

use sleeper_zone::espn::{GameSummary, Scoreboard};

fn load<T: DeserializeOwned>(name: &str) -> T {
    let raw = std::fs::read_to_string(format!("testdata/espn/{name}"))
        .unwrap_or_else(|e| panic!("fixture {name}: {e}"));
    serde_json::from_str(&raw).unwrap_or_else(|e| panic!("parse {name}: {e}"))
}

fn find_event<'a>(sb: &'a Scoreboard, id: &str) -> &'a sleeper_zone::espn::Event {
    sb.events
        .iter()
        .find(|e| e.id == id)
        .unwrap_or_else(|| panic!("event {id} in scoreboard fixture"))
}

#[test]
fn scoreboard_fixture_pins_week_two() {
    let sb: Scoreboard = load("scoreboard.json");
    assert_eq!(sb.events.len(), 16, "all week-2 games listed");
    assert_eq!(sb.week_number(), Some(2));
    assert_eq!(sb.season_year(), Some(2026));
}

#[test]
fn finished_game_pins_scores_and_state() {
    let sb: Scoreboard = load("scoreboard.json");
    let e = find_event(&sb, "401872944"); // WAS @ DAL
    assert_eq!(
        e.name.as_deref(),
        Some("Washington Commanders at Dallas Cowboys")
    );
    let comp = e.competition().expect("competition");
    assert_eq!(comp.competitors.len(), 2);
    let dal = comp
        .competitors
        .iter()
        .find(|c| {
            c.team
                .as_ref()
                .is_some_and(|t| t.abbreviation.as_deref() == Some("DAL"))
        })
        .expect("DAL competitor");
    assert_eq!(dal.score.as_deref(), Some("37"));
    assert!(dal.home_away.as_deref() == Some("home"));
    let status = comp.status.as_ref().and_then(|s| s.status_type.as_ref());
    assert_eq!(status.and_then(|t| t.state.as_deref()), Some("post"));
    assert!(!e.is_live());
}

#[test]
fn upcoming_game_pins_pre_state() {
    let sb: Scoreboard = load("scoreboard.json");
    // Some events in the fixture must be pre-state (SNF/MNF games).
    let pres: Vec<_> = sb
        .events
        .iter()
        .filter(|e| {
            e.competition()
                .and_then(|c| c.status.as_ref())
                .and_then(|s| s.status_type.as_ref())
                .and_then(|t| t.state.as_deref())
                == Some("pre")
        })
        .collect();
    assert!(!pres.is_empty(), "at least one not-yet-started game");
}

#[test]
fn summary_fixture_pins_drives_and_scoring_plays() {
    let g: GameSummary = load("summary.json");
    let drives = g.drives.as_ref().expect("drives present");
    assert!(
        drives.current.is_empty(),
        "finished game has no current drive"
    );
    assert_eq!(drives.previous.len(), 17);
    let first = &drives.previous[0];
    assert_eq!(
        first.team.as_ref().and_then(|t| t.abbreviation.as_deref()),
        Some("DAL")
    );
    assert_eq!(first.plays.len(), 11);
    assert_eq!(first.yards, Some(66));

    // First scoring play: CeeDee Lamb 3-yard TD catch, DAL, Q1 10:01.
    let sp = &g.scoring_plays[0];
    assert_eq!(
        sp.text.as_deref(),
        Some("CeeDee Lamb 3 Yd pass from Dak Prescott (Brandon Aubrey Kick)")
    );
    assert_eq!(
        sp.team.as_ref().and_then(|t| t.abbreviation.as_deref()),
        Some("DAL")
    );
    assert_eq!(sp.period.as_ref().and_then(|p| p.number), Some(1));
    assert_eq!(
        sp.clock.as_ref().and_then(|c| c.display_value.as_deref()),
        Some("10:01")
    );
    assert_eq!(sp.home_score, Some(7));
    assert_eq!(sp.away_score, Some(0));

    // Scoring plays count is substantial for a 37-20 game.
    assert!(g.scoring_plays.len() >= 8, "got {}", g.scoring_plays.len());
}

#[test]
fn summary_fixture_pins_header_status() {
    let g: GameSummary = load("summary.json");
    let header = g.header.as_ref().expect("header");
    let comp = &header.competitions[0];
    assert_eq!(comp.competitors.len(), 2);
    let dal = comp
        .competitors
        .iter()
        .find(|c| {
            c.team
                .as_ref()
                .is_some_and(|t| t.abbreviation.as_deref() == Some("DAL"))
        })
        .expect("DAL in header");
    assert_eq!(dal.score.as_deref(), Some("37"));
    let status = comp.status.as_ref().and_then(|s| s.status_type.as_ref());
    assert_eq!(status.and_then(|t| t.completed), Some(true));
}

#[test]
fn play_text_carries_player_names_for_join_hints() {
    let g: GameSummary = load("summary.json");
    let drives = g.drives.as_ref().expect("drives");
    let td = drives.previous[0]
        .plays
        .iter()
        .find(|p| p.scoring_play == Some(true))
        .expect("TD play in first drive");
    assert_eq!(
        td.play_type
            .as_ref()
            .and_then(|t| t.abbreviation.as_deref()),
        Some("TD")
    );
    let text = td.text.as_deref().expect("play text");
    assert!(text.contains("C.Lamb"), "text: {text}");
    assert!(text.contains("TOUCHDOWN"));
    assert_eq!(td.stat_yardage, Some(3));
}
