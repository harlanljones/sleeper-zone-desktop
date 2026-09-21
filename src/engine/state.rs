//! UI-facing snapshot types. Plain data, no client types leak through:
//! the egui shell renders these without knowing where they came from.

use std::collections::BTreeMap;

/// State of one NFL game as ESPN reports it.
#[derive(Debug, Clone, PartialEq)]
pub struct GameView {
    pub espn_event_id: String,
    /// "pre" | "in" | "post"
    pub state: Option<String>,
    pub period: Option<u8>,
    pub display_clock: Option<String>,
    pub home_abbr: Option<String>,
    pub away_abbr: Option<String>,
    pub home_score: Option<String>,
    pub away_score: Option<String>,
    /// Most recent scoring plays, newest last.
    pub scoring_plays: Vec<ScoringPlayView>,
}

impl GameView {
    pub fn is_live(&self) -> bool {
        self.state.as_deref() == Some("in")
    }

    /// "DAL 37–20 WSH" style one-line summary; missing parts degrade
    /// gracefully so the stream never renders blank labels.
    pub fn matchup_label(&self) -> String {
        let home = self.home_abbr.clone().unwrap_or_else(|| "?".into());
        let away = self.away_abbr.clone().unwrap_or_else(|| "?".into());
        format!(
            "{away} {}–{} {home}",
            self.away_score.clone().unwrap_or_else(|| "-".into()),
            self.home_score.clone().unwrap_or_else(|| "-".into()),
        )
    }
}

/// One rostered player with fantasy points and live game context.
#[derive(Debug, Clone, PartialEq)]
pub struct PlayerView {
    pub sleeper_id: String,
    pub name: String,
    pub position: Option<String>,
    pub nfl_team: Option<String>,
    pub points: f64,
    pub is_starter: bool,
    /// Game context for the player's NFL team, when that team played
    /// and the join succeeded.
    pub game: Option<GameRef>,
    /// Injury status as Sleeper reports it (e.g. "Q", "O", "IR").
    pub injury_status: Option<String>,
    pub number: Option<u8>,
    pub age: Option<u8>,
}

/// Reference to a game in `TrackerSnapshot::games`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct GameRef {
    pub espn_event_id: String,
    /// "pre" | "in" | "post"
    pub state: Option<String>,
}

/// One fantasy team's live scoreboard line.
#[derive(Debug, Clone, PartialEq)]
pub struct TeamView {
    pub roster_id: u8,
    /// Team name from the league's user metadata, when known.
    pub name: Option<String>,
    pub points: f64,
    pub starters: Vec<PlayerView>,
    pub bench: Vec<PlayerView>,
}

/// One draft pick resolved to a player name, owned by `roster_id`.
#[derive(Debug, Clone, PartialEq)]
pub struct PickView {
    pub round: Option<u8>,
    pub slot: Option<u16>,
    pub roster_id: Option<u8>,
    pub player: String,
}

/// One league's matchups for the tracked week.
#[derive(Debug, Clone, PartialEq)]
pub struct LeagueView {
    pub league_id: String,
    pub name: Option<String>,
    /// Matchups keyed by matchup_id, each pairing two teams.
    pub matchups: BTreeMap<u8, Vec<TeamView>>,
    /// The league's own scoring rules (Sleeper `scoring_settings`):
    /// key like "rec_td" -> points. Drives play-stream point estimates.
    pub scoring: std::collections::HashMap<String, f64>,
    /// Draft picks of the league's draft, if one exists.
    pub picks: Vec<PickView>,
    /// The config user's roster in this league, when resolvable.
    pub user_roster_id: Option<u8>,
    /// Players not on any roster: the FA / waiver pool, name-sorted.
    pub free_agents: Vec<PlayerView>,
    /// Roster slot labels in slot order (drives the league-page tables).
    pub roster_positions: Vec<String>,
    /// True when the league has a superflex slot.
    pub is_superflex: bool,
    /// Per-player fantasy context keyed by Sleeper player id.
    pub player_stats: std::collections::HashMap<String, PlayerStatsView>,
}

/// A complete, self-consistent view for one instant. Cheap to clone —
/// the UI holds the latest one behind a watch channel.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TrackerSnapshot {
    pub week: Option<u8>,
    pub season: Option<u16>,
    /// Seconds since epoch when the snapshot was assembled.
    pub fetched_at: u64,
    pub leagues: Vec<LeagueView>,
    /// Every game relevant to tracked players, keyed by ESPN event id.
    pub games: BTreeMap<String, GameView>,
}

impl TrackerSnapshot {
    /// True when any joined game is being played right now.
    pub fn any_game_live(&self) -> bool {
        self.games.values().any(GameView::is_live)
    }
    /// ESPN event ids whose players sit in the leagues marked `selected`
    /// (parallel to `self.leagues`). Empty selection → no games.
    pub fn visible_game_ids(&self, selected: &[bool]) -> std::collections::BTreeSet<String> {
        let mut ids = std::collections::BTreeSet::new();
        for (league, on) in self.leagues.iter().zip(selected.iter()) {
            if !on {
                continue;
            }
            for teams in league.matchups.values() {
                for team in teams {
                    for player in team.starters.iter().chain(team.bench.iter()) {
                        if let Some(g) = &player.game {
                            ids.insert(g.espn_event_id.clone());
                        }
                    }
                }
            }
        }
        ids
    }

    /// Every game of the week, live games first, then by event id.
    /// Backs the Games page listing.
    pub fn games_ordered(&self) -> Vec<&GameView> {
        let mut games: Vec<&GameView> = self.games.values().collect();
        games.sort_by_key(|g| (!g.is_live(), g.espn_event_id.clone()));
        games
    }

    /// Games of the config user's OWN rostered players across the
    /// selected leagues. Unlike `visible_game_ids`, other teams'
    /// players in those leagues do not count.
    pub fn user_game_ids(&self, selected: &[bool]) -> std::collections::BTreeSet<String> {
        let mut ids = std::collections::BTreeSet::new();
        for (league, on) in self.leagues.iter().zip(selected.iter()) {
            if !on {
                continue;
            }
            let Some(user_roster) = league.user_roster_id else {
                continue;
            };
            for teams in league.matchups.values() {
                for team in teams.iter().filter(|t| t.roster_id == user_roster) {
                    for player in team.starters.iter().chain(team.bench.iter()) {
                        if let Some(g) = &player.game {
                            ids.insert(g.espn_event_id.clone());
                        }
                    }
                }
            }
        }
        ids
    }

    /// The Zone page's user-relevant play stream:
    /// - no league selected → every play of the week;
    /// - leagues selected → only plays that name a player on the
    ///   user's own roster in a selected league.
    pub fn user_play_stream(&self, selected: &[bool]) -> Vec<PlayLine> {
        if !selected.iter().any(|&on| on) {
            let all: std::collections::BTreeSet<String> = self.games.keys().cloned().collect();
            return self.play_stream(&all);
        }
        let game_ids = self.user_game_ids(selected);
        self.play_stream(&game_ids)
            .into_iter()
            .filter(|l| {
                l.relevant
                    .iter()
                    .any(|r| r.user && self.league_is_selected(selected, &r.league_id))
            })
            .collect()
    }

    fn league_is_selected(&self, selected: &[bool], league_id: &str) -> bool {
        self.leagues
            .iter()
            .zip(selected.iter())
            .any(|(l, on)| *on && l.league_id == league_id)
    }

    /// The play stream: every scoring play of the given games, live
    /// games first, newest play first within a game. Each line carries
    /// the rostered players it names, with fantasy-point estimates
    /// derived from each league's own scoring settings.
    pub fn play_stream(&self, game_ids: &std::collections::BTreeSet<String>) -> Vec<PlayLine> {
        let mut lines = Vec::new();
        let mut games: Vec<&GameView> = self
            .games
            .values()
            .filter(|g| game_ids.contains(&g.espn_event_id))
            .collect();
        games.sort_by_key(|g| (!g.is_live(), g.espn_event_id.clone()));
        for game in games {
            let label = game.matchup_label();
            for play in game.scoring_plays.iter().rev() {
                lines.push(PlayLine {
                    espn_event_id: game.espn_event_id.clone(),
                    id: play.id.clone(),
                    game_label: label.clone(),
                    team: play.team.clone(),
                    text: play.text.clone(),
                    live: game.is_live(),
                    caption: play.caption(),
                    relevant: self.relevant_players(play),
                });
            }
        }
        lines
    }

    /// Rostered players across all tracked leagues whose last name
    /// appears in the play text and whose NFL team scored. Matching is
    /// last-name + team based because ESPN texts abbreviate names
    /// ("C.Lamb"); same-team restriction keeps the false-positive rate
    /// low for common surnames.
    fn relevant_players(&self, play: &ScoringPlayView) -> Vec<Relevance> {
        let text_lower = play.text.to_lowercase();
        let mut out = Vec::new();
        for league in &self.leagues {
            for teams in league.matchups.values() {
                for team in teams {
                    for player in team.starters.iter().chain(team.bench.iter()) {
                        let Some(last) = player.last_name() else {
                            continue;
                        };
                        let last_lower = last.to_lowercase();
                        if last_lower.len() < 3 {
                            continue; // too short to match safely
                        }
                        if !text_lower.contains(&last_lower) {
                            continue;
                        }
                        if player.nfl_team.as_deref() != play.team.as_deref() {
                            continue;
                        }
                        let est_points = estimate_points(&play.text, &last_lower, &league.scoring);
                        out.push(Relevance {
                            league_id: league.league_id.clone(),
                            league_name: league.name.clone(),
                            player: player.name.clone(),
                            starter: player.is_starter,
                            est_points,
                            user: league.user_roster_id == Some(team.roster_id),
                        });
                    }
                }
            }
        }
        out.sort_by(|a, b| {
            b.starter
                .cmp(&a.starter)
                .then(a.league_id.cmp(&b.league_id))
        });
        out
    }
}

impl PlayerView {
    /// Last token of the display name, as ESPN texts use surname only.
    pub fn last_name(&self) -> Option<&str> {
        self.name
            .rsplit_once(' ')
            .map(|(_, l)| l)
            .or(Some(&self.name))
    }
}

/// Points for one scoring rule, with Sleeper's default league settings
/// as fallback when the league didn't override the key.
fn setting(scoring: &std::collections::HashMap<String, f64>, keys: &[&str], default: f64) -> f64 {
    keys.iter()
        .find_map(|k| scoring.get(*k).copied())
        .unwrap_or(default)
}

/// First "N Yd" in the text, as f64 yards.
fn yards_in(text: &str) -> f64 {
    let bytes = text.as_bytes();
    let mut i = 0;
    while i + 4 <= bytes.len() {
        if text[i..].len() >= 3 && text[i..i + 3].eq_ignore_ascii_case(" yd") {
            let start = text[..i]
                .char_indices()
                .rev()
                .take_while(|(_, c)| c.is_ascii_digit())
                .last()
                .map(|(idx, _)| idx);
            if let Some(start) = start {
                return text[start..i].parse::<f64>().unwrap_or(0.0);
            }
        }
        i += 1;
    }
    0.0
}

/// Field-goal points from the distance bucket the league configured.
fn fg_points(yards: f64, scoring: &std::collections::HashMap<String, f64>) -> f64 {
    let (key, default) = match yards as u16 {
        0..=19 => ("fg_0_19", 3.0),
        20..=29 => ("fg_20_29", 3.0),
        30..=39 => ("fg_30_39", 3.0),
        40..=49 => ("fg_40_49", 4.0),
        _ => ("fg_50p", 5.0),
    };
    setting(scoring, &[key], default)
}

/// Best-effort fantasy points one named player earns on this play,
/// under the given league's scoring settings. `last_lower` is the
/// matched player's lowercased surname; role is inferred from where the
/// name sits in the ESPN text. None when the play type carries no
/// offensive-player scoring we can model (e.g. defensive safeties).
fn estimate_points(
    text: &str,
    last_lower: &str,
    scoring: &std::collections::HashMap<String, f64>,
) -> Option<f64> {
    let lower = text.to_lowercase();
    let yards = yards_in(text);

    // Pure kicking plays first — they must not be confused by a TD
    // text's "(B.Aubrey Kick)" tail.
    if lower.contains("field goal") {
        return Some(fg_points(yards, scoring));
    }
    if lower.contains("extra point") || lower.contains(" kick") {
        // On a TD play the XP kicker's name sits right before " kick";
        // only that match earns XP. Everyone else falls through to the
        // TD branches below.
        if let Some(pos) = lower.find(last_lower)
            && lower[pos + last_lower.len()..].starts_with(" kick")
        {
            return Some(setting(scoring, &["xp"], 1.0));
        }
        if lower.contains("extra point") && !lower.contains("pass") && !lower.contains("run") {
            return Some(setting(scoring, &["xp"], 1.0));
        }
    }
    if lower.contains("2 pt") || lower.contains("two-point") {
        let key = if lower.contains("pass") {
            "2pt_pass"
        } else if lower.contains("run") {
            "2pt_rush"
        } else if lower.contains("reception") {
            "2pt_rec"
        } else {
            "2pt"
        };
        return Some(setting(scoring, &[key, "2pt"], 2.0));
    }
    if lower.contains("safety") {
        return None; // defensive scoring, out of scope for the stream
    }

    // Name position decides the role on multi-player plays: in
    // "X 24 Yd pass from Y", X (before "pass from") catches, Y throws.
    let name_pos = lower.find(last_lower).unwrap_or(0);
    if let Some(p) = lower.find("pass from") {
        if name_pos > p {
            let td = setting(scoring, &["pass_td", "td"], 4.0);
            return Some(setting(scoring, &["pass_yd"], 0.04) * yards + td);
        }
        let td = setting(scoring, &["rec_td", "td"], 6.0);
        return Some(
            setting(scoring, &["rec"], 1.0) + setting(scoring, &["rec_yd"], 0.1) * yards + td,
        );
    }
    if lower.contains("run") || lower.contains("rush") {
        let td = setting(scoring, &["rush_td", "td"], 6.0);
        return Some(setting(scoring, &["rush_yd"], 0.1) * yards + td);
    }
    if lower.contains("return") || lower.contains("fumble") || lower.contains("interception") {
        // Return/defensive TDs: attribute the plain TD only; IDP rules
        // are out of scope for the stream.
        return Some(setting(scoring, &["td"], 6.0));
    }
    // Unmodeled scoring shape: report relevance without inventing points.
    None
}

/// Per-player fantasy context shown in league-page roster tables.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PlayerStatsView {
    /// Season fantasy points per game, under the league's scoring type.
    pub avg_points: Option<f64>,
    /// Current week minus games played this season.
    pub weeks_missed: Option<u8>,
    /// Compact human stat line from this week's box score.
    pub week_line: String,
    /// True when the player's NFL team has no game this week.
    pub bye: bool,
}

/// One scoring play as shown in the play stream, joined from the ESPN
/// game summary. `id` is the ESPN scoring-play id (stable across ticks;
/// the UI uses it to flag newly-arrived plays); the scores are the
/// the running score at the moment of the play.
#[derive(Debug, Clone, PartialEq)]
pub struct ScoringPlayView {
    pub id: String,
    pub team: Option<String>,
    pub text: String,
    pub period: Option<u8>,
    pub clock: Option<String>,
    pub home_score: Option<i16>,
    pub away_score: Option<i16>,
}

impl ScoringPlayView {
    /// "Q3 · 10:01 · 17–14" caption for the line.
    pub fn caption(&self) -> String {
        let mut parts = Vec::new();
        if let Some(p) = self.period {
            parts.push(format!("Q{p}"));
        }
        if let Some(c) = &self.clock {
            parts.push(c.clone());
        }
        if let (Some(a), Some(h)) = (self.away_score, self.home_score) {
            parts.push(format!("{a}–{h}"));
        }
        parts.join(" · ")
    }
}

/// Why a play matters to one league: it names a rostered player, with a
/// fantasy-point estimate computed from that league's own
/// `scoring_settings`. Estimates are best-effort derived from the play
/// text, not Sleeper's official stat recomputation.
#[derive(Debug, Clone, PartialEq)]
pub struct Relevance {
    pub league_id: String,
    pub league_name: Option<String>,
    pub player: String,
    pub starter: bool,
    pub est_points: Option<f64>,
    /// True when the named player is on the config user's own roster
    /// in this league — drives the Zone page's user-team stream filter.
    pub user: bool,
}

/// One entry in the Zone page's play stream.
#[derive(Debug, Clone, PartialEq)]
pub struct PlayLine {
    pub espn_event_id: String,
    /// ESPN scoring-play id, stable across ticks.
    pub id: String,
    /// "DAL 37–20 WSH" style summary of the game the play belongs to.
    pub game_label: String,
    /// Scoring team abbreviation, when ESPN provides it.
    pub team: Option<String>,
    pub text: String,
    pub live: bool,
    /// Period/clock/running-score caption for the play.
    pub caption: String,
    /// Rostered players (per league) named by this play.
    pub relevant: Vec<Relevance>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    fn player(id: &str, event: &str) -> PlayerView {
        player_named(id, "Sample Guy", None, event)
    }

    fn player_named(id: &str, name: &str, team: Option<&str>, event: &str) -> PlayerView {
        PlayerView {
            sleeper_id: id.into(),
            name: name.into(),
            position: None,
            nfl_team: team.map(|t| t.into()),
            points: 0.0,
            is_starter: true,
            injury_status: None,
            number: None,
            age: None,
            game: Some(GameRef {
                espn_event_id: event.into(),
                state: Some("in".into()),
            }),
        }
    }

    fn team(roster: u8, players: Vec<PlayerView>) -> TeamView {
        TeamView {
            roster_id: roster,
            name: None,
            points: 0.0,
            starters: players,
            bench: vec![],
        }
    }

    fn scoring_play(id: &str, team: &str, text: &str) -> ScoringPlayView {
        ScoringPlayView {
            id: id.into(),
            team: Some(team.into()),
            text: text.into(),
            period: Some(3),
            clock: Some("5:00".into()),
            home_score: Some(20),
            away_score: Some(17),
        }
    }

    fn snapshot() -> TrackerSnapshot {
        let mut a_matchups = BTreeMap::new();
        a_matchups.insert(1u8, vec![team(1, vec![player("p1", "g1")])]);
        let mut b_matchups = BTreeMap::new();
        b_matchups.insert(1u8, vec![team(2, vec![player("p2", "g2")])]);
        TrackerSnapshot {
            week: Some(2),
            season: Some(2026),
            fetched_at: 0,
            leagues: vec![
                LeagueView {
                    league_id: "A".into(),
                    name: Some("Alpha".into()),
                    matchups: a_matchups,
                    scoring: Default::default(),
                    picks: vec![],
                    user_roster_id: None,
                    free_agents: vec![],
                    roster_positions: vec![],
                    is_superflex: false,
                    player_stats: Default::default(),
                },
                LeagueView {
                    league_id: "B".into(),
                    name: Some("Beta".into()),
                    matchups: b_matchups,
                    scoring: Default::default(),
                    picks: vec![],
                    user_roster_id: None,
                    free_agents: vec![],
                    roster_positions: vec![],
                    is_superflex: false,
                    player_stats: Default::default(),
                },
            ],
            games: BTreeMap::from([
                (
                    "g1".into(),
                    GameView {
                        espn_event_id: "g1".into(),
                        state: Some("in".into()),
                        period: Some(3),
                        display_clock: Some("5:00".into()),
                        home_abbr: Some("DAL".into()),
                        away_abbr: Some("WSH".into()),
                        home_score: Some("20".into()),
                        away_score: Some("17".into()),
                        scoring_plays: vec![
                            scoring_play("s1", "WSH", "first play"),
                            scoring_play("s2", "DAL", "second play"),
                        ],
                    },
                ),
                (
                    "g2".into(),
                    GameView {
                        espn_event_id: "g2".into(),
                        state: Some("post".into()),
                        period: None,
                        display_clock: None,
                        home_abbr: Some("KC".into()),
                        away_abbr: Some("LV".into()),
                        home_score: None,
                        away_score: None,
                        scoring_plays: vec![scoring_play("s3", "KC", "kc play")],
                    },
                ),
            ]),
        }
    }

    #[test]
    fn visible_game_ids_respects_league_selection() {
        let snap = snapshot();
        let all = snap.visible_game_ids(&[true, true]);
        assert_eq!(all, BTreeSet::from(["g1".into(), "g2".into()]));
        let only_a = snap.visible_game_ids(&[true, false]);
        assert_eq!(only_a, BTreeSet::from(["g1".into()]));
        let none = snap.visible_game_ids(&[false, false]);
        assert!(none.is_empty(), "off means no games");
    }

    #[test]
    fn play_stream_is_newest_first_within_each_game() {
        let snap = snapshot();
        let ids = snap.visible_game_ids(&[true, true]);
        let lines = snap.play_stream(&ids);
        let g1: Vec<&str> = lines
            .iter()
            .filter(|l| l.espn_event_id == "g1")
            .map(|l| l.text.as_str())
            .collect();
        assert_eq!(g1, vec!["second play", "first play"]);
        assert_eq!(lines[0].game_label, "WSH 17–20 DAL");
        assert!(lines[0].live);
    }

    #[test]
    fn play_stream_hides_games_from_unselected_leagues() {
        let snap = snapshot();
        let only_a = snap.visible_game_ids(&[true, false]);
        let lines = snap.play_stream(&only_a);
        assert!(lines.iter().all(|l| l.espn_event_id == "g1"));
        assert_eq!(lines.len(), 2);
    }
    #[test]
    fn play_line_carries_id_caption_and_running_score() {
        let snap = snapshot();
        let ids = snap.visible_game_ids(&[true]);
        let lines = snap.play_stream(&ids);
        let line = lines.iter().find(|l| l.id == "s1").unwrap();
        assert_eq!(line.caption, "Q3 · 5:00 · 17–20");
    }

    #[test]
    fn td_pass_relevance_estimates_receiver_and_passer_from_league_settings() {
        let mut a_matchups = BTreeMap::new();
        a_matchups.insert(
            1u8,
            vec![team(
                1,
                vec![
                    player_named("p1", "CeeDee Lamb", Some("DAL"), "g1"),
                    player_named("p2", "Dak Prescott", Some("DAL"), "g1"),
                ],
            )],
        );
        let mut snap = snapshot();
        // PPR league: rec=1.0, rec_yd=0.2, pass_yd default.
        snap.leagues[0].matchups = a_matchups;
        snap.leagues[0].scoring = [("rec", 1.0), ("rec_yd", 0.2)]
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect();
        let text = "C.Lamb 24 Yd pass from D.Prescott (B.Aubrey Kick)";
        snap.games.get_mut("g1").unwrap().scoring_plays = vec![scoring_play("td1", "DAL", text)];
        let ids = snap.visible_game_ids(&[true]);
        let lines = snap.play_stream(&ids);
        assert_eq!(lines.len(), 1);
        let mut rel = lines[0].relevant.clone();
        rel.sort_by(|x, y| x.player.cmp(&y.player));
        assert_eq!(rel.len(), 2, "receiver and passer both match");
        let lamb = rel.iter().find(|r| r.player == "CeeDee Lamb").unwrap();
        // 1.0 rec + 24 * 0.2 yds + 6 TD
        assert!(
            (lamb.est_points.unwrap() - 11.8).abs() < 1e-9,
            "got {:?}",
            lamb.est_points
        );
        let dak = rel.iter().find(|r| r.player == "Dak Prescott").unwrap();
        // 24 * 0.04 default pass_yd + 4 pass_td
        assert!(
            (dak.est_points.unwrap() - 4.96).abs() < 1e-9,
            "got {:?}",
            dak.est_points
        );
    }

    #[test]
    fn league_settings_differ_between_leagues_on_same_play() {
        let mut a_matchups = BTreeMap::new();
        a_matchups.insert(
            1u8,
            vec![team(
                1,
                vec![player_named("p1", "Jalen Hurts", Some("PHI"), "g1")],
            )],
        );
        let mut b_matchups = BTreeMap::new();
        b_matchups.insert(
            1u8,
            vec![team(
                2,
                vec![player_named("p2", "Jalen Hurts", Some("PHI"), "g1")],
            )],
        );
        let mut snap = snapshot();
        snap.leagues[0].matchups = a_matchups;
        snap.leagues[0].scoring = [("rush_td", 6.0)]
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect();
        snap.leagues[1].matchups = b_matchups;
        snap.leagues[1].scoring = [("rush_td", 8.0)]
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect();
        snap.games.get_mut("g1").unwrap().scoring_plays = vec![scoring_play(
            "td2",
            "PHI",
            "J.Hurts 1 Yd run (J.Elliott Kick)",
        )];
        let ids = snap.visible_game_ids(&[true, true]);
        let lines = snap.play_stream(&ids);
        let pts: Vec<f64> = lines[0]
            .relevant
            .iter()
            .map(|r| r.est_points.unwrap())
            .collect();
        assert_eq!(pts.len(), 2);
        // 1 Yd run: rush_yd 0.1 * 1 + rush_td per league.
        assert!(
            pts.iter().any(|p| (p - 6.1).abs() < 1e-9),
            "league A: {pts:?}"
        );
        assert!(
            pts.iter().any(|p| (p - 8.1).abs() < 1e-9),
            "league B: {pts:?}"
        );
    }

    #[test]
    fn field_goal_uses_distance_bucket_from_settings() {
        let mut a_matchups = BTreeMap::new();
        a_matchups.insert(
            1u8,
            vec![team(
                1,
                vec![player_named("p1", "Jake Bates", Some("DET"), "g1")],
            )],
        );
        let mut snap = snapshot();
        snap.leagues[0].matchups = a_matchups;
        snap.games.get_mut("g1").unwrap().scoring_plays =
            vec![scoring_play("fg1", "DET", "J.Bates 55 Yd field goal")];
        let ids = snap.visible_game_ids(&[true]);
        let lines = snap.play_stream(&ids);
        let rel = &lines[0].relevant;
        assert_eq!(rel.len(), 1);
        assert_eq!(rel[0].est_points, Some(5.0)); // fg_50p default
    }

    #[test]
    fn pat_kicker_gets_xp_and_non_kicker_is_not_confused() {
        let mut a_matchups = BTreeMap::new();
        a_matchups.insert(
            1u8,
            vec![team(
                1,
                vec![player_named("p1", "Brandon Aubrey", Some("DAL"), "g1")],
            )],
        );
        let mut snap = snapshot();
        snap.leagues[0].matchups = a_matchups;
        snap.games.get_mut("g1").unwrap().scoring_plays = vec![scoring_play(
            "pat1",
            "DAL",
            "B.Aubrey kicks the extra point",
        )];
        let ids = snap.visible_game_ids(&[true]);
        let lines = snap.play_stream(&ids);
        assert_eq!(lines[0].relevant[0].est_points, Some(1.0));
    }

    #[test]
    fn opponent_team_players_do_not_match() {
        let mut a_matchups = BTreeMap::new();
        a_matchups.insert(
            1u8,
            vec![team(
                1,
                vec![player_named("p1", "Terry McLaurin", Some("WSH"), "g1")],
            )],
        );
        let mut snap = snapshot();
        snap.leagues[0].matchups = a_matchups;
        snap.games.get_mut("g1").unwrap().scoring_plays = vec![scoring_play(
            "td3",
            "DAL",
            "C.Lamb 10 Yd run (B.Aubrey Kick)",
        )];
        let ids = snap.visible_game_ids(&[true]);
        let lines = snap.play_stream(&ids);
        assert!(
            lines[0].relevant.is_empty(),
            "WSH player must not match a DAL play"
        );
    }
    #[test]
    fn games_ordered_lists_live_games_first() {
        let snap = snapshot(); // g1 live, g2 post
        let ordered = snap.games_ordered();
        let ids: Vec<&str> = ordered.iter().map(|g| g.espn_event_id.as_str()).collect();
        assert_eq!(ids, vec!["g1", "g2"]);
    }
    #[test]
    fn user_stream_with_no_leagues_shows_all_plays() {
        let snap = snapshot();
        let all = snap.play_stream(&snap.games.keys().cloned().collect());
        let none_selected = snap.user_play_stream(&[false, false]);
        assert_eq!(none_selected.len(), all.len());
    }

    #[test]
    fn user_stream_keeps_only_user_team_plays() {
        let mut a_matchups = BTreeMap::new();
        a_matchups.insert(
            1u8,
            vec![team(
                1,
                vec![player_named("p1", "CeeDee Lamb", Some("DAL"), "g1")],
            )],
        );
        let mut snap = snapshot();
        snap.leagues[0].matchups = a_matchups;
        snap.leagues[0].user_roster_id = Some(1);
        snap.games.get_mut("g1").unwrap().scoring_plays = vec![
            scoring_play("t1", "DAL", "C.Lamb 24 Yd pass from D.Prescott (kick)"),
            scoring_play("t2", "WSH", "T.McLaurin 10 Yd run (kick)"),
        ];
        let lines = snap.user_play_stream(&[true, false]);
        assert_eq!(lines.len(), 1, "only the user's player's play survives");
        assert_eq!(lines[0].id, "t1");
        assert!(lines[0].relevant.iter().any(|r| r.user));
    }

    #[test]
    fn user_stream_empty_without_user_roster() {
        let snap = snapshot(); // user_roster_id is None everywhere
        assert!(snap.user_play_stream(&[true, false]).is_empty());
    }
}
