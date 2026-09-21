//! The tracker: fetches both APIs, joins them, publishes snapshots.
//!
//! The Sleeper→ESPN link is the player's NFL team abbreviation against
//! ESPN competitor abbreviations. (Sleeper's `espn_id` is an athlete id
//! and ESPN's per-play athlete data sits behind `$ref` URLs we don't
//! fetch, so fantasy points stay Sleeper-owned and game context is
//! team-level. That is exactly the Sleeper-Zone granularity we chose.)

use std::collections::{BTreeMap, HashMap};
use std::convert::Infallible;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tokio::sync::watch;

use crate::engine::join;
use crate::engine::state::{
    GameRef, GameView, LeagueView, PickView, PlayerStatsView, PlayerView, TeamView, TrackerSnapshot,
};
use crate::espn::EspnClient;
use crate::sleeper::{Matchup, PlayerMap, SleeperClient};

/// Poll cadence while any tracked game is live.
pub const TRACKED_POLL: Duration = Duration::from_secs(15);
/// Poll cadence when nothing is live.
pub const IDLE_POLL: Duration = Duration::from_secs(5 * 60);

/// Adaptive delay: fresh data fast during games, quiet otherwise.
pub fn next_delay(any_live: bool) -> Duration {
    if any_live { TRACKED_POLL } else { IDLE_POLL }
}

/// Maximum scoring plays kept per game in the snapshot.
pub const SCORING_PLAY_WINDOW: usize = 24;

/// Intermediate per-league fetch result before the join step.
struct FetchedLeague {
    league_id: String,
    name: Option<String>,
    scoring: std::collections::HashMap<String, f64>,
    matchups: Vec<Matchup>,
    owner_names: std::collections::HashMap<u8, String>,
    picks: Vec<PickView>,
    user_roster_id: Option<u8>,
    rostered: std::collections::HashSet<String>,
    roster_positions: Vec<String>,
    is_superflex: bool,
}

pub struct Tracker {
    sleeper: SleeperClient,
    espn: EspnClient,
    league_ids: Vec<String>,
    /// Player map, refreshed daily by the caller.
    players: PlayerMap,
    /// Config user's Sleeper id, used to flag their roster per league.
    user_id: Option<String>,
    /// Cached stats payloads (season + current week) with fetch times.
    stats_cache: std::sync::Mutex<StatsCache>,
}

/// TTL-cached stats payloads; both are multi-MB, so refetch sparingly.
#[derive(Default)]
struct StatsCache {
    season: Option<(String, std::time::Instant, crate::sleeper::StatsMap)>,
    week: Option<(String, u8, std::time::Instant, crate::sleeper::StatsMap)>,
}

const SEASON_STATS_TTL: std::time::Duration = std::time::Duration::from_secs(30 * 60);
const WEEK_STATS_TTL: std::time::Duration = std::time::Duration::from_secs(60);

impl Tracker {
    pub fn new(
        sleeper: SleeperClient,
        espn: EspnClient,
        league_ids: Vec<String>,
        players: PlayerMap,
        user_id: Option<String>,
    ) -> Self {
        Self {
            sleeper,
            espn,
            league_ids,
            players,
            user_id: user_id.filter(|u| !u.is_empty()),
            stats_cache: std::sync::Mutex::default(),
        }
    }

    /// Replace the cached player map (call ~daily).
    pub fn update_players(&mut self, players: PlayerMap) {
        self.players = players;
    }

    /// One full fetch + join cycle.
    pub async fn tick(&self) -> crate::sleeper::Result<TrackerSnapshot> {
        let state = self.sleeper.nfl_state().await?;
        let week = state.week;
        let season = state.season.parse::<u16>().ok();

        // Fantasy layer: matchups + scoring rules for every tracked league.
        let mut fetched: Vec<FetchedLeague> = Vec::with_capacity(self.league_ids.len());
        for league_id in &self.league_ids {
            // Name + scoring settings come from the league payload;
            // settings are what the play-stream estimates use, so a
            // failed league fetch degrades to default-scoring estimates.
            let (name, scoring, draft_id, roster_positions) =
                match self.sleeper.league(league_id).await.ok() {
                    Some(l) => (
                        l.name,
                        l.scoring_settings.unwrap_or_default(),
                        l.draft_id,
                        l.roster_positions.unwrap_or_default(),
                    ),
                    None => (None, Default::default(), None, vec![]),
                };
            let is_superflex = roster_positions.iter().any(|p| p == "SUPER_FLEX");
            let matchups = self.sleeper.matchups(league_id, week).await?;
            // Team names + draft picks for the league tab; failures
            // degrade to unnamed teams and an empty picks list.
            let rosters = self.sleeper.rosters(league_id).await.unwrap_or_default();
            let users = self
                .sleeper
                .league_users(league_id)
                .await
                .unwrap_or_default();
            let owner_names = join::build_owner_names(&rosters, &users);
            let user_roster_id = self
                .user_id
                .as_deref()
                .and_then(|uid| join::user_roster(&rosters, uid));
            let rostered: std::collections::HashSet<String> = rosters
                .iter()
                .flat_map(|r| r.players.iter().cloned())
                .collect();
            let picks = match &draft_id {
                Some(id) => join::resolve_picks(
                    &self.sleeper.draft_picks(id).await.unwrap_or_default(),
                    &self.players,
                ),
                None => Vec::new(),
            };
            fetched.push(FetchedLeague {
                league_id: league_id.clone(),
                name,
                scoring,
                matchups,
                owner_names,
                picks,
                user_roster_id,
                rostered,
                roster_positions,
                is_superflex,
            });
        }

        // Game layer: the week's full slate. Every game gets a summary
        // and a GameView so the Games page and its detail subpages work
        // for all matchups, not just the ones tracked players are in.
        let scoreboard = self.espn.scoreboard().await.ok();
        let abbr_map = scoreboard
            .as_ref()
            .map(join::build_abbreviation_map)
            .unwrap_or_default();

        let mut summaries: BTreeMap<String, crate::espn::GameSummary> = BTreeMap::new();
        if let Some(sb) = &scoreboard {
            for event in &sb.events {
                if let Ok(summary) = self.espn.summary(&event.id).await {
                    summaries.insert(event.id.clone(), summary);
                }
            }
        }

        let games: BTreeMap<String, GameView> = scoreboard
            .as_ref()
            .map(|sb| {
                sb.events
                    .iter()
                    .map(|e| {
                        let view = join::game_view_of(e, summaries.get(&e.id), SCORING_PLAY_WINDOW);
                        (e.id.clone(), view)
                    })
                    .collect()
            })
            .unwrap_or_default();

        // Join into views.
        let season_str = season.map(|s| s.to_string()).unwrap_or_default();
        let (season_stats, week_stats) = self.load_stats(&season_str, week).await;
        let mut leagues = Vec::with_capacity(fetched.len());
        for fl in fetched {
            leagues.push(self.league_view(fl, &abbr_map, &games, week, &season_stats, &week_stats));
        }

        Ok(TrackerSnapshot {
            week: Some(week),
            season,
            fetched_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
            leagues,
            games,
        })
    }

    /// Fetch (or reuse cached) season + current-week stats payloads.
    async fn load_stats(
        &self,
        season: &str,
        week: u8,
    ) -> (crate::sleeper::StatsMap, crate::sleeper::StatsMap) {
        let now = std::time::Instant::now();
        let cached_season = {
            let cache = self.stats_cache.lock().unwrap_or_else(|e| e.into_inner());
            match &cache.season {
                Some((s, at, map)) if s == season && now.duration_since(*at) < SEASON_STATS_TTL => {
                    Some(map.clone())
                }
                _ => None,
            }
        };
        let season_stats = match cached_season {
            Some(map) => map,
            None => match self.sleeper.season_stats(season).await {
                Ok(map) => {
                    let mut cache = self.stats_cache.lock().unwrap_or_else(|e| e.into_inner());
                    cache.season = Some((season.to_string(), now, map.clone()));
                    map
                }
                Err(_) => Default::default(),
            },
        };
        let cached_week = {
            let cache = self.stats_cache.lock().unwrap_or_else(|e| e.into_inner());
            match &cache.week {
                Some((s, w, at, map))
                    if s == season && *w == week && now.duration_since(*at) < WEEK_STATS_TTL =>
                {
                    Some(map.clone())
                }
                _ => None,
            }
        };
        let week_stats = match cached_week {
            Some(map) => map,
            None => match self.sleeper.week_stats(season, week).await {
                Ok(map) => {
                    let mut cache = self.stats_cache.lock().unwrap_or_else(|e| e.into_inner());
                    cache.week = Some((season.to_string(), week, now, map.clone()));
                    map
                }
                Err(_) => Default::default(),
            },
        };
        (season_stats, week_stats)
    }

    fn league_view(
        &self,
        fl: FetchedLeague,
        abbr_map: &HashMap<String, String>,
        games: &BTreeMap<String, GameView>,
        week: u8,
        season_stats: &crate::sleeper::StatsMap,
        week_stats: &crate::sleeper::StatsMap,
    ) -> LeagueView {
        let FetchedLeague {
            league_id,
            name,
            scoring,
            matchups,
            owner_names,
            picks,
            user_roster_id,
            rostered,
            roster_positions,
            is_superflex,
        } = fl;
        // Roster-table context for every rostered player of the league.
        let pts_key = join::pts_key(&scoring);
        let mut player_stats: HashMap<String, PlayerStatsView> = HashMap::new();
        for m in &matchups {
            for pid in &m.players {
                if player_stats.contains_key(pid) {
                    continue;
                }
                let view = join::stats_view(
                    self.players.get(pid),
                    season_stats.get(pid),
                    week_stats.get(pid),
                    pts_key,
                    week,
                    abbr_map,
                );
                player_stats.insert(pid.clone(), view);
            }
        }
        let mut by_matchup: BTreeMap<u8, Vec<TeamView>> = BTreeMap::new();
        for m in &matchups {
            let team = self.team_view(m, &owner_names, abbr_map, games);
            by_matchup.entry(m.matchup_id).or_default().push(team);
        }
        LeagueView {
            league_id,
            name,
            matchups: by_matchup,
            scoring,
            picks,
            user_roster_id,
            free_agents: join::free_agents(&self.players, &rostered, abbr_map, games),
            roster_positions,
            is_superflex,
            player_stats,
        }
    }

    fn team_view(
        &self,
        m: &Matchup,
        owner_names: &std::collections::HashMap<u8, String>,
        abbr_map: &HashMap<String, String>,
        games: &BTreeMap<String, GameView>,
    ) -> TeamView {
        let starters: Vec<String> = m.starters.clone();
        let mut starter_views = Vec::new();
        let mut bench_views = Vec::new();
        for player_id in &m.players {
            let points = m.players_points.get(player_id).copied().unwrap_or(0.0);
            let is_starter = starters.contains(player_id);
            let player = self.players.get(player_id);
            let view = PlayerView {
                sleeper_id: player_id.clone(),
                name: player
                    .map(crate::sleeper::Player::display_name)
                    .unwrap_or_else(|| format!("#{player_id}")),
                position: player.and_then(|p| p.position.clone()),
                nfl_team: player.and_then(|p| p.team.clone()),
                points,
                is_starter,
                game: player
                    .and_then(|p| join::resolve_game(p, abbr_map))
                    .and_then(|event_id| {
                        games.get(&event_id).map(|g| GameRef {
                            espn_event_id: event_id,
                            state: g.state.clone(),
                        })
                    }),
                injury_status: player.and_then(|p| p.injury_status.clone()),
                number: player.and_then(|p| p.number),
                age: player.and_then(|p| p.age),
            };
            if is_starter {
                starter_views.push(view);
            } else {
                bench_views.push(view);
            }
        }
        starter_views.sort_by(|a, b| {
            b.points
                .partial_cmp(&a.points)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        TeamView {
            roster_id: m.roster_id,
            name: owner_names.get(&m.roster_id).cloned(),
            points: m.points,
            starters: starter_views,
            bench: bench_views,
        }
    }

    /// Run the adaptive poll loop, publishing snapshots to a watch
    /// channel. Never returns normally.
    pub async fn run(self: Arc<Self>, tx: watch::Sender<Option<TrackerSnapshot>>) -> Infallible {
        loop {
            match self.tick().await {
                Ok(snapshot) => {
                    let any_live = snapshot.any_game_live();
                    let _ = tx.send(Some(snapshot));
                    tokio::time::sleep(next_delay(any_live)).await;
                }
                Err(e) => {
                    // Transient API failures must not kill the loop:
                    // back off briefly, keep the last snapshot visible.
                    eprintln!("[tracker] tick failed, retrying: {e}");
                    tokio::time::sleep(Duration::from_secs(10)).await;
                }
            }
        }
    }
}
