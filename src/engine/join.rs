//! The join: map Sleeper players onto ESPN games.
//!
//! The link key is the NFL team abbreviation: Sleeper players carry
//! `team` (e.g. "DAL"), ESPN competitors carry `team.abbreviation`.
//! Covers athletes and DST slots alike; players whose `team` is unset
//! (free agents) resolve to no game.

use std::collections::{BTreeMap, HashMap};

use crate::engine::state::{
    GameRef, GameView, PickView, PlayerStatsView, PlayerView, ScoringPlayView,
};
use crate::espn::{Competitor, Event, GameSummary, Scoreboard};
use crate::sleeper::{DraftPick, LeagueUser, Player, PlayerMap, PlayerStats, Roster};

/// abbreviation -> espn event id, from the scoreboard.
pub fn build_abbreviation_map(scoreboard: &Scoreboard) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for event in &scoreboard.events {
        for comp in &event.competitions {
            for competitor in &comp.competitors {
                if let Some(abbr) = competitor
                    .team
                    .as_ref()
                    .and_then(|t| t.abbreviation.clone())
                {
                    map.insert(abbr, event.id.clone());
                }
            }
        }
    }
    map
}

/// Resolve which ESPN game a Sleeper player's NFL team is playing.
pub fn resolve_game(player: &Player, abbr_map: &HashMap<String, String>) -> Option<String> {
    let team = player.team.as_deref()?;
    abbr_map.get(team).cloned()
}

/// Extract scoring plays from a game summary, oldest first, capped to
/// the `limit` most recent.
pub fn scoring_plays_of(summary: &GameSummary, limit: usize) -> Vec<ScoringPlayView> {
    let all: Vec<ScoringPlayView> = summary
        .scoring_plays
        .iter()
        .map(|sp| ScoringPlayView {
            id: sp.id.clone(),
            team: sp.team.as_ref().and_then(|t| t.abbreviation.clone()),
            text: sp.text.clone().unwrap_or_default(),
            period: sp.period.as_ref().and_then(|p| p.number),
            clock: sp.clock.as_ref().and_then(|c| c.display_value.clone()),
            home_score: sp.home_score,
            away_score: sp.away_score,
        })
        .collect();
    if all.len() <= limit {
        all
    } else {
        all[all.len() - limit..].to_vec()
    }
}

/// Summarize an event into a GameView, optionally enriched with the
/// game summary's scoring plays.
pub fn game_view_of(
    event: &Event,
    summary: Option<&GameSummary>,
    scoring_play_window: usize,
) -> GameView {
    let comp = event.competition();
    let status = comp.and_then(|c| c.status.as_ref());
    let mut home = None;
    let mut away = None;
    if let Some(comp) = comp {
        for competitor in &comp.competitors {
            match competitor.home_away.as_deref() {
                Some("home") => home = Some(competitor),
                Some("away") => away = Some(competitor),
                _ => {}
            }
        }
    }
    let abbr = |c: Option<&Competitor>| {
        c.and_then(|c| c.team.as_ref().and_then(|t| t.abbreviation.clone()))
    };
    let score = |c: Option<&Competitor>| c.and_then(|c| c.score.clone());
    GameView {
        espn_event_id: event.id.clone(),
        state: status
            .and_then(|s| s.status_type.as_ref())
            .and_then(|t| t.state.clone()),
        period: status.and_then(|s| s.period),
        display_clock: status.and_then(|s| s.display_clock.clone()),
        home_abbr: abbr(home),
        away_abbr: abbr(away),
        home_score: score(home),
        away_score: score(away),
        scoring_plays: summary
            .map(|s| scoring_plays_of(s, scoring_play_window))
            .unwrap_or_default(),
    }
}

/// Ordered map of event id -> summary, used by the tracker tick.
pub type SummaryMap = BTreeMap<String, GameSummary>;

/// roster_id -> best-effort team/owner display name, from the league's
/// rosters and users. Falls back to username, then "R<id>".
pub fn build_owner_names(rosters: &[Roster], users: &[LeagueUser]) -> HashMap<u8, String> {
    let by_user: HashMap<&str, &LeagueUser> =
        users.iter().map(|u| (u.user_id.as_str(), u)).collect();
    let mut names = HashMap::new();
    for roster in rosters {
        let name = roster
            .owner_id
            .as_deref()
            .and_then(|owner| by_user.get(owner))
            .and_then(|u| {
                u.metadata
                    .as_ref()
                    .and_then(|m| m.get("team_name"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .or_else(|| u.display_name.clone())
                    .or_else(|| u.username.clone())
            })
            .unwrap_or_else(|| format!("R{}", roster.roster_id));
        names.insert(roster.roster_id, name);
    }
    names
}

/// Resolve draft picks to player names (Sleeper player map), keeping
/// the owning roster id.
pub fn resolve_picks(picks: &[DraftPick], players: &PlayerMap) -> Vec<PickView> {
    picks
        .iter()
        .map(|p| PickView {
            round: p.round,
            slot: p.slot,
            roster_id: p.roster_id,
            player: p
                .player_id
                .as_deref()
                .and_then(|id| players.get(id))
                .map(crate::sleeper::Player::display_name)
                .unwrap_or_else(|| "Unknown".to_string()),
        })
        .collect()
}

/// The roster owned by `user_id`, if any (empty user id matches none).
pub fn user_roster(rosters: &[Roster], user_id: &str) -> Option<u8> {
    if user_id.is_empty() {
        return None;
    }
    rosters
        .iter()
        .find(|r| r.owner_id.as_deref() == Some(user_id))
        .map(|r| r.roster_id)
}

/// Fantasy-point key for a league's scoring type, from its `rec` rule:
/// 1.0+ = PPR, 0.5 = half, else standard.
pub fn pts_key(scoring: &std::collections::HashMap<String, f64>) -> &'static str {
    match scoring.get("rec").copied() {
        Some(r) if r >= 1.0 => "pts_ppr",
        Some(r) if r > 0.0 => "pts_half_ppr",
        _ => "pts_std",
    }
}

/// Position groups of a roster slot label, e.g. FLEX -> RB/WR/TE.
pub fn slot_positions(slot: &str) -> &'static [&'static str] {
    match slot {
        "FLEX" => &["RB", "WR", "TE"],
        "SUPER_FLEX" | "Q_W_R_T" => &["QB", "RB", "WR", "TE"],
        "W_R" => &["WR", "RB"],
        "W_T" => &["WR", "TE"],
        "QB" => &["QB"],
        "RB" => &["RB"],
        "WR" => &["WR"],
        "TE" => &["TE"],
        "K" => &["K"],
        "DEF" => &["DEF"],
        _ => &[],
    }
}

/// Build the roster-table stats view for one player: season average
/// points, weeks missed, a compact this-week stat line, and a bye flag
/// (the player's NFL team has no game in this week's scoreboard).
pub fn stats_view(
    player: Option<&Player>,
    season_stats: Option<&PlayerStats>,
    week_stats: Option<&PlayerStats>,
    pts_key: &str,
    week: u8,
    abbr_map: &HashMap<String, String>,
) -> PlayerStatsView {
    // Season: avg = season points / games played.
    let (avg_points, weeks_missed) = match season_stats {
        Some(s) => {
            let gp = s.get("gp");
            let pts = s.get(pts_key);
            let avg = if gp > 0.0 { Some(pts / gp) } else { None };
            let missed = if gp > 0.0 {
                Some(week.saturating_sub(gp as u8))
            } else {
                None
            };
            (avg, missed)
        }
        None => (None, None),
    };
    // Bye: the player has an NFL team, but that team has no game this week.
    let bye = player
        .and_then(|p| p.team.clone())
        .is_some_and(|t| !abbr_map.contains_key(&t));
    // This-week stat line, position-aware.
    let position = player.and_then(|p| p.position.clone()).unwrap_or_default();
    let week_line = week_stats
        .map(|s| week_line(s, &position))
        .unwrap_or_default();
    PlayerStatsView {
        avg_points,
        weeks_missed,
        week_line,
        bye,
    }
}

/// Compact human stat line for this week, keyed by position group.
fn week_line(s: &PlayerStats, position: &str) -> String {
    let mut parts: Vec<String> = Vec::new();
    match position {
        "QB" => {
            if s.get("pass_att") > 0.0 {
                parts.push(format!(
                    "{}/{} pass",
                    s.get("pass_cmp") as u32,
                    s.get("pass_att") as u32
                ));
            }
            for (key, label) in [
                ("pass_yd", "pass yd"),
                ("pass_td", "pass TD"),
                ("pass_int", "INT"),
                ("rush_yd", "rush yd"),
                ("rush_td", "rush TD"),
            ] {
                if s.get(key) > 0.0 {
                    parts.push(format!("{} {}", fmt_num(s.get(key)), label));
                }
            }
        }
        "K" => {
            for key in s.stats.keys() {
                if key.starts_with("fgm") || key == "xp" || key == "xpm" {
                    parts.push(format!("{key} {}", fmt_num(s.get(key))));
                }
            }
            if parts.is_empty() {
                parts.push(format!("{} pts", fmt_num(s.get("pts_std"))));
            }
        }
        "DEF" => {
            parts.push(format!("{} pts", fmt_num(s.get("pts_std"))));
        }
        _ => {
            for (key, label) in [
                ("rush_att", "att"),
                ("rush_yd", "rush yd"),
                ("rush_td", "rush TD"),
                ("rec", "rec"),
                ("rec_yd", "rec yd"),
                ("rec_td", "rec TD"),
            ] {
                if s.get(key) > 0.0 {
                    parts.push(format!("{} {}", fmt_num(s.get(key)), label));
                }
            }
        }
    }
    if parts.is_empty() {
        "-".to_string()
    } else {
        parts.join(", ")
    }
}

/// Trim trailing zeros off stat numbers for display.
fn fmt_num(n: f64) -> String {
    if (n - n.round()).abs() < f64::EPSILON {
        format!("{}", n as i64)
    } else {
        format!("{n:.1}")
    }
}

/// Player views for every mappable NFL player NOT on a roster: the
/// free-agent / waiver pool. Sorted by name; game context resolved the
/// same way as rostered players so live games show up.
pub fn free_agents(
    players: &PlayerMap,
    rostered: &std::collections::HashSet<String>,
    abbr_map: &HashMap<String, String>,
    games: &BTreeMap<String, GameView>,
) -> Vec<PlayerView> {
    let mut out: Vec<PlayerView> = players
        .iter()
        .filter(|(id, p)| !rostered.contains(*id) && p.position.is_some() && p.team.is_some())
        .map(|(id, p)| PlayerView {
            sleeper_id: id.clone(),
            name: p.display_name(),
            position: p.position.clone(),
            nfl_team: p.team.clone(),
            points: 0.0,
            is_starter: false,
            game: resolve_game(p, abbr_map).and_then(|event_id| {
                games.get(&event_id).map(|g| GameRef {
                    espn_event_id: event_id,
                    state: g.state.clone(),
                })
            }),
            injury_status: p.injury_status.clone(),
            number: p.number,
            age: p.age,
        })
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::espn::Scoreboard;

    fn scoreboard_fixture() -> Scoreboard {
        let raw =
            std::fs::read_to_string("testdata/espn/scoreboard.json").expect("scoreboard fixture");
        serde_json::from_str(&raw).expect("parses")
    }

    #[test]
    fn abbreviation_map_covers_both_teams_of_every_game() {
        let sb = scoreboard_fixture();
        let map = build_abbreviation_map(&sb);
        assert_eq!(map.len(), 32, "16 games x 2 teams");
        let event_id = map.get("DAL").expect("DAL present");
        assert_eq!(event_id, "401872944");
        assert_eq!(map.get("WSH"), Some(event_id), "same game");
    }

    #[test]
    fn resolve_game_matches_by_team_abbreviation() {
        let sb = scoreboard_fixture();
        let map = build_abbreviation_map(&sb);
        let player: Player =
            serde_json::from_str(r#"{"first_name": "CeeDee", "last_name": "Lamb", "team": "DAL"}"#)
                .expect("parses");
        assert_eq!(resolve_game(&player, &map).as_deref(), Some("401872944"));

        let free_agent: Player =
            serde_json::from_str(r#"{"first_name": "Nobody"}"#).expect("parses");
        assert_eq!(resolve_game(&free_agent, &map), None);
    }

    #[test]
    fn game_view_of_extracts_home_away_and_status() {
        let sb = scoreboard_fixture();
        let event = sb.events.iter().find(|e| e.id == "401872944").unwrap();
        let view = game_view_of(event, None, 8);
        assert_eq!(view.espn_event_id, "401872944");
        assert_eq!(view.home_abbr.as_deref(), Some("DAL"));
        assert_eq!(view.away_abbr.as_deref(), Some("WSH"));
        assert_eq!(view.home_score.as_deref(), Some("37"));
        assert_eq!(view.away_score.as_deref(), Some("20"));
        assert_eq!(view.state.as_deref(), Some("post"));
        assert!(!view.is_live());
    }
    #[test]
    fn owner_names_prefer_team_name_over_display_name() {
        let roster: Roster =
            serde_json::from_str(r#"{"roster_id": 3, "league_id": "L", "owner_id": "u1"}"#)
                .unwrap();
        let user: LeagueUser = serde_json::from_str(
            r#"{"user_id": "u1", "league_id": "L", "display_name": "Dan", "metadata": {"team_name": "Tune Squad"}}"#,
        )
        .unwrap();
        let names = build_owner_names(std::slice::from_ref(&roster), std::slice::from_ref(&user));
        assert_eq!(names.get(&3).map(String::as_str), Some("Tune Squad"));
    }

    #[test]
    fn owner_names_fall_back_to_username_then_roster_id() {
        let roster: Roster = serde_json::from_str(r#"{"roster_id": 7, "league_id": "L"}"#).unwrap();
        let orphan: Roster = serde_json::from_str(r#"{"roster_id": 8, "league_id": "L"}"#).unwrap();
        let user: LeagueUser =
            serde_json::from_str(r#"{"user_id": "u9", "league_id": "L", "username": "wavy"}"#)
                .unwrap();
        let roster9: Roster =
            serde_json::from_str(r#"{"roster_id": 9, "league_id": "L", "owner_id": "u9"}"#)
                .unwrap();
        let names = build_owner_names(std::slice::from_ref(&roster9), std::slice::from_ref(&user));
        assert_eq!(names.get(&9).map(String::as_str), Some("wavy"));
        let names2 = build_owner_names(std::slice::from_ref(&orphan), std::slice::from_ref(&user));
        assert_eq!(names2.get(&8).map(String::as_str), Some("R8"));
        let _ = roster;
    }

    #[test]
    fn resolve_picks_names_players_and_keeps_roster() {
        let players = crate::sleeper::parse_player_map(
            r#"{"4046": {"first_name": "CeeDee", "last_name": "Lamb"}}"#,
        )
        .expect("player map");
        let pick: DraftPick = serde_json::from_str(
            r#"{"round": 1, "draft_slot": 4, "roster_id": 2, "player_id": "4046"}"#,
        )
        .unwrap();
        let views = resolve_picks(std::slice::from_ref(&pick), &players);
        assert_eq!(views.len(), 1);
        assert_eq!(views[0].player, "CeeDee Lamb");
        assert_eq!(views[0].roster_id, Some(2));
        assert_eq!(views[0].round, Some(1));
    }
    #[test]
    fn user_roster_matches_owner_or_none() {
        let r1: Roster =
            serde_json::from_str(r#"{"roster_id": 1, "league_id": "L", "owner_id": "me"}"#)
                .unwrap();
        assert_eq!(user_roster(std::slice::from_ref(&r1), "me"), Some(1));
        assert_eq!(user_roster(std::slice::from_ref(&r1), "other"), None);
        assert_eq!(user_roster(std::slice::from_ref(&r1), ""), None);
    }

    #[test]
    fn free_agents_exclude_rostered_and_sort_by_name() {
        let players = crate::sleeper::parse_player_map(
            r#"{"1": {"first_name": "Zed", "last_name": "FA", "position": "WR", "team": "DAL"},
                "2": {"first_name": "Al", "last_name": "Rostered", "position": "QB", "team": "KC"},
                "3": {"first_name": "No", "last_name": "Team", "position": "RB"}}"#,
        )
        .expect("player map");
        let rostered = ["2".to_string()].into_iter().collect();
        let fa = free_agents(&players, &rostered, &HashMap::new(), &BTreeMap::new());
        assert_eq!(fa.len(), 1, "team-less player excluded, rostered excluded");
        assert_eq!(fa[0].name, "Zed FA");
    }
    #[test]
    fn pts_key_follows_rec_rule() {
        let ppr = [("rec".to_string(), 1.0)].into_iter().collect();
        let half = [("rec".to_string(), 0.5)].into_iter().collect();
        let std = std::collections::HashMap::new();
        assert_eq!(pts_key(&ppr), "pts_ppr");
        assert_eq!(pts_key(&half), "pts_half_ppr");
        assert_eq!(pts_key(&std), "pts_std");
    }

    #[test]
    fn slot_positions_maps_flex_and_superflex() {
        assert_eq!(slot_positions("FLEX"), &["RB", "WR", "TE"]);
        assert_eq!(slot_positions("SUPER_FLEX"), &["QB", "RB", "WR", "TE"]);
        assert_eq!(slot_positions("QB"), &["QB"]);
    }

    #[test]
    fn stats_view_averages_missed_and_bye() {
        let season = crate::sleeper::PlayerStats {
            stats: [("gp".to_string(), 2.0), ("pts_ppr".to_string(), 30.0)]
                .into_iter()
                .collect(),
        };
        let week = crate::sleeper::PlayerStats {
            stats: [
                ("rec".to_string(), 5.0),
                ("rec_yd".to_string(), 80.0),
                ("rec_td".to_string(), 1.0),
            ]
            .into_iter()
            .collect(),
        };
        let player: Player = serde_json::from_str(
            r#"{"first_name": "A", "last_name": "B", "position": "WR", "team": "DAL"}"#,
        )
        .unwrap();
        // DAL is in the abbr map (not a bye); week 3, gp 2 → 1 missed.
        let abbr = [("DAL".to_string(), "e1".to_string())]
            .into_iter()
            .collect();
        let v = stats_view(
            Some(&player),
            Some(&season),
            Some(&week),
            "pts_ppr",
            3,
            &abbr,
        );
        assert_eq!(v.avg_points, Some(15.0));
        assert_eq!(v.weeks_missed, Some(1));
        assert!(!v.bye);
        assert!(v.week_line.contains("5 rec"), "got: {}", v.week_line);
        assert!(v.week_line.contains("80 rec yd"));
    }

    #[test]
    fn stats_view_flags_bye_when_team_absent_from_scoreboard() {
        let player: Player = serde_json::from_str(
            r#"{"first_name": "A", "last_name": "B", "position": "QB", "team": "MIA"}"#,
        )
        .unwrap();
        let abbr = [("DAL".to_string(), "e1".to_string())]
            .into_iter()
            .collect();
        let v = stats_view(Some(&player), None, None, "pts_std", 2, &abbr);
        assert!(v.bye);
    }
}
