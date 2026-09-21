//! The egui application: setup flow and the live scoreboard.

pub mod config;

use std::collections::BTreeMap;
use std::sync::Arc;

use eframe::egui;
use tokio::sync::watch;

use crate::engine::{
    GameView, LeagueView, PickView, PlayerView, TeamView, Tracker, TrackerSnapshot,
};
use crate::espn::EspnClient;
use crate::sleeper::{SleeperClient, User};

/// One tokio runtime powering the tracker thread for the app's life.
struct Runtime {
    /// Owns the tracker task; dropped when the app exits.
    _rt: tokio::runtime::Runtime,
    rx: watch::Receiver<Option<TrackerSnapshot>>,
}

pub struct ZoneApp {
    /// None until the tracker thread is running.
    runtime: Option<Runtime>,
    setup: SetupState,
    config: config::Config,
    error: Option<String>,
    /// Per-league visibility on the Zone page (parallel to the
    /// snapshot's leagues). None = not sized yet (defaults to all-on).
    zone_filter: Option<Vec<bool>>,
    /// ESPN scoring-play ids already shown to the user; drives the
    /// NEW flash on fresh stream entries.
    seen_plays: std::collections::BTreeSet<String>,
    /// Current page; the Zone page is the default landing view.
    page: Page,
    /// Player whose detail popup is open, if any.
    popup: Option<PlayerView>,
    /// Selected roster per league page (league id -> roster id).
    league_team: BTreeMap<String, u8>,
    /// Free-agent browser: search text and position filter.
    fa_search: String,
    fa_pos: Option<String>,
    /// Roster-table sort state per slot: (column, descending).
    table_sort: BTreeMap<String, (u8, bool)>,
}

enum SetupState {
    /// Enter username.
    Username(String),
    /// Leagues fetched; awaiting selection.
    PickLeagues {
        user: User,
        leagues: Vec<crate::sleeper::League>,
        selected: Vec<bool>,
    },
    Done,
}

/// Display label for a roster slot.
fn slot_label(slot: &str) -> String {
    match slot {
        "SUPER_FLEX" => "Superflex".into(),
        "W_R" => "W/R".into(),
        "W_T" => "W/T".into(),
        other => other.to_string(),
    }
}

impl ZoneApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let config = config::Config::load().unwrap_or_default();
        let setup = if config.username.is_empty() {
            SetupState::Username(String::new())
        } else {
            SetupState::Done
        };
        let mut app = Self {
            runtime: None,
            setup,
            config,
            error: None,
            zone_filter: None,
            seen_plays: Default::default(),
            page: Page::Zone,
            popup: None,
            league_team: Default::default(),
            fa_search: String::new(),
            fa_pos: None,
            table_sort: Default::default(),
        };
        if matches!(app.setup, SetupState::Done) {
            app.start_tracking();
        }
        app
    }

    fn start_tracking(&mut self) {
        let league_ids = self.config.league_ids.clone();
        let user_id = self.config.user_id.clone();
        let (tx, rx) = watch::channel(None);
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("tokio runtime builds");
        rt.spawn(async move {
            // Player map: disk cache first (fresh < 24h), else one big
            // fetch that we persist for next launch.
            let sleeper = SleeperClient::new();
            let espn = EspnClient::new();
            let players = match config::load_players_cache() {
                Some(p) if !p.is_empty() => p,
                _ => match sleeper.players().await {
                    Ok(p) => {
                        config::save_players_cache(&p);
                        p
                    }
                    Err(e) => {
                        eprintln!("[tracker] player map fetch failed: {e}");
                        Default::default()
                    }
                },
            };
            let tracker = Arc::new(Tracker::new(
                sleeper,
                espn,
                league_ids,
                players,
                Some(user_id),
            ));
            tracker.run(tx).await;
        });
        self.runtime = Some(Runtime { _rt: rt, rx });
    }

    fn setup_username(&self) -> &str {
        match &self.setup {
            SetupState::Username(u) => u,
            _ => "",
        }
    }
}

impl eframe::App for ZoneApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            if matches!(self.setup, SetupState::Done) {
                self.ui_nav(ui);
                ui.separator();
            }
            match self.page.clone() {
                Page::Zone => self.ui_zone(ctx, ui),
                Page::Games => self.ui_games(ctx, ui),
                Page::GameDetail(event_id) => self.ui_game_detail(ctx, ui, &event_id),
                Page::League(league_id) => self.ui_league_page(ctx, ui, &league_id),
            }
        });
        self.ui_player_popup(ctx);
    }
}

impl ZoneApp {
    /// Top navigation: Zone, Games, then one tab per tracked league.
    fn ui_nav(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui
                .selectable_label(matches!(self.page, Page::Zone), "Zone")
                .clicked()
            {
                self.page = Page::Zone;
            }
            if ui
                .selectable_label(!matches!(self.page, Page::Zone | Page::League(_)), "Games")
                .clicked()
            {
                self.page = Page::Games;
            }
            // One tab per league, labeled by its (short) name.
            if let Some(snapshot) = self.current_snapshot() {
                for league in &snapshot.leagues {
                    let label = league
                        .name
                        .clone()
                        .unwrap_or_else(|| league.league_id.clone());
                    if ui
                        .selectable_label(
                            matches!(&self.page, Page::League(id) if *id == league.league_id),
                            label,
                        )
                        .clicked()
                    {
                        self.page = Page::League(league.league_id.clone());
                    }
                }
            }
        });
    }

    /// Floating player-detail popup, shared by every page's clickable
    /// player rows.
    fn ui_player_popup(&mut self, ctx: &egui::Context) {
        let Some(p) = self.popup.clone() else {
            return;
        };
        let mut open = true;
        egui::Window::new(format!("{} — details", p.name))
            .open(&mut open)
            .show(ctx, |ui| {
                egui::Grid::new("player_details")
                    .num_columns(2)
                    .show(ui, |ui| {
                        ui.label("Position");
                        ui.label(p.position.clone().unwrap_or_else(|| "?".into()));
                        ui.end_row();
                        ui.label("NFL team");
                        ui.label(p.nfl_team.clone().unwrap_or_else(|| "—".into()));
                        ui.end_row();
                        ui.label("Fantasy points");
                        ui.label(format!("{:.2}", p.points));
                        ui.end_row();
                        ui.label("Role");
                        ui.label(if p.is_starter { "starter" } else { "bench" });
                        ui.end_row();
                        if let Some(inj) = &p.injury_status {
                            ui.label("Injury status");
                            ui.colored_label(egui::Color32::YELLOW, inj.clone());
                            ui.end_row();
                        }
                        if let (Some(n), Some(a)) = (p.number, p.age) {
                            ui.label("# / age");
                            ui.label(format!("#{n}, {a}"));
                            ui.end_row();
                        }
                        if let Some(g) = &p.game {
                            let state = match g.state.as_deref() {
                                Some("in") => "live",
                                Some("post") => "final",
                                _ => "pre-game",
                            };
                            ui.label("NFL game");
                            ui.label(state.to_string());
                            ui.end_row();
                        }
                    });
            });
        if !open {
            self.popup = None;
        }
    }
}

/// Which top-level page is showing. `GameDetail` carries the ESPN event
/// id of the opened game; `League` the Sleeper league id of the tab.
#[derive(Debug, Clone, PartialEq)]
enum Page {
    Zone,
    Games,
    GameDetail(String),
    League(String),
}

impl ZoneApp {
    /// The Zone page: the app's default landing view. First-run setup
    /// overlays it; once tracking, it shows the league selector (All /
    /// Off / per-league chips), the live scoring-play stream, and the
    /// scoreboard for the selected leagues.
    fn ui_zone(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        // Setup gates the Zone page: username → leagues → live view.
        match &mut self.setup {
            SetupState::Username(_) => self.ui_username(ctx, ui),
            SetupState::PickLeagues { .. } => self.ui_pick_leagues(ui),
            SetupState::Done => self.ui_zone_live(ctx, ui),
        }
    }

    fn ui_zone_live(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        let Some(runtime) = &self.runtime else {
            ui.label("starting tracker…");
            ctx.request_repaint_after(std::time::Duration::from_millis(250));
            return;
        };
        // Snapshot: clone latest; repaint on the adaptive cadence.
        let snapshot = runtime.rx.borrow().clone();
        let Some(snapshot) = snapshot else {
            ui.heading("Loading your leagues…");
            ctx.request_repaint_after(std::time::Duration::from_secs(2));
            return;
        };
        let live = snapshot.any_game_live();
        ui.horizontal(|ui| {
            ui.heading("Sleeper Zone");
            ui.separator();
            ui.label(format!(
                "season {} week {}",
                snapshot
                    .season
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "?".into()),
                snapshot
                    .week
                    .map(|w| w.to_string())
                    .unwrap_or_else(|| "?".into()),
            ));
            let chip = if live {
                egui::Label::new(egui::RichText::new("● LIVE").color(egui::Color32::RED))
            } else {
                egui::Label::new(egui::RichText::new("○ idle").color(egui::Color32::GRAY))
            };
            ui.add(chip);
        });
        ui.separator();

        // League selection: All / Off plus a per-league chip each.
        // Snapshot league count is the source of truth; a stale filter
        // (from a previous snapshot) resets to all-on.
        let n = snapshot.leagues.len();
        let stale = self.zone_filter.as_ref().is_none_or(|f| f.len() != n);
        if stale {
            self.zone_filter = Some(vec![true; n]);
        }
        let filter = self.zone_filter.as_mut().expect("just sized");
        ui.horizontal(|ui| {
            ui.label("Leagues:");
            if ui.small_button("All leagues").clicked() {
                *filter = vec![true; n];
            }
            if ui.small_button("No leagues").clicked() {
                *filter = vec![false; n];
            }
            for (i, league) in snapshot.leagues.iter().enumerate() {
                let name = league
                    .name
                    .clone()
                    .unwrap_or_else(|| league.league_id.clone());
                ui.toggle_value(&mut filter[i], name);
            }
        });

        let selected = self.zone_filter.clone().expect("just sized");
        // Zone stream semantics: no league selected shows every play of
        // the week; otherwise only plays touching the user's own
        // rosters in the selected leagues.
        let lines = snapshot.user_play_stream(&selected);
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.heading("Play stream");
            if lines.is_empty() {
                ui.label("No scoring plays for the selected leagues yet.");
            }
            for line in lines.iter().take(50) {
                let is_new = !self.seen_plays.contains(&line.id);
                ui.horizontal_wrapped(|ui| {
                    if is_new {
                        ui.colored_label(egui::Color32::LIGHT_GREEN, "NEW");
                    }
                    if line.live {
                        ui.colored_label(egui::Color32::RED, "●");
                    }
                    if let Some(team) = &line.team {
                        ui.strong(team.clone());
                    }
                    ui.label(line.text.clone());
                    ui.small(line.caption.clone());
                    // Relevance chips: one per rostered player the play
                    // names, using that league's own scoring settings.
                    // Players in unselected leagues are not shown.
                    let rels: Vec<_> = line
                        .relevant
                        .iter()
                        .filter(|r| self.league_selected(&selected, &r.league_id, &snapshot))
                        .collect();
                    for rel in rels {
                        let star = if rel.starter { "★" } else { "○" };
                        let pts = rel
                            .est_points
                            .map(|p| format!(" +{:.1}", p))
                            .unwrap_or_default();
                        let color = if rel.starter {
                            egui::Color32::GOLD
                        } else {
                            egui::Color32::GRAY
                        };
                        let pts_suffix = pts;
                        if ui
                            .small_button(
                                egui::RichText::new(format!("{star} {}{pts_suffix}", rel.player))
                                    .color(color),
                            )
                            .clicked()
                        {
                            self.popup = Some(PlayerView {
                                sleeper_id: String::new(),
                                name: rel.player.clone(),
                                position: None,
                                nfl_team: None,
                                points: rel.est_points.unwrap_or(0.0),
                                is_starter: rel.starter,
                                game: None,
                                injury_status: None,
                                number: None,
                                age: None,
                            });
                        }
                    }
                });
            }
            // Mark everything just shown so the next snapshot's fresh
            // plays flash NEW.
            for line in &lines {
                self.seen_plays.insert(line.id.clone());
            }
            ui.add_space(8.0);
            ui.separator();
            for (i, league) in snapshot.leagues.iter().enumerate() {
                if selected[i] {
                    self.ui_league(ui, league, &snapshot.games);
                }
            }
        });
        // Remember what the user has now seen so the next snapshot's
        // fresh plays flash NEW.
        ctx.request_repaint_after(crate::engine::next_delay(live));
    }

    /// Latest tracker snapshot, when one has arrived.
    fn current_snapshot(&self) -> Option<TrackerSnapshot> {
        let runtime = self.runtime.as_ref()?;
        runtime.rx.borrow().clone()
    }

    /// The Games page: every game of the week, live first. Each row
    /// opens the game's detail subpage.
    fn ui_games(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        let Some(snapshot) = self.current_snapshot() else {
            ui.heading("Loading games…");
            ctx.request_repaint_after(std::time::Duration::from_secs(2));
            return;
        };
        ui.heading(format!(
            "Week {} — all games",
            snapshot
                .week
                .map(|w| w.to_string())
                .unwrap_or_else(|| "?".into())
        ));
        ui.separator();
        egui::ScrollArea::vertical().show(ui, |ui| {
            for game in snapshot.games_ordered() {
                let status = game.clock_or_state();
                ui.horizontal(|ui| {
                    if game.is_live() {
                        ui.colored_label(egui::Color32::RED, "●");
                    }
                    let label = format!("{}  ·  {status}", game.matchup_label());
                    if ui.selectable_label(false, label).clicked() {
                        self.page = Page::GameDetail(game.espn_event_id.clone());
                    }
                });
            }
        });
        ctx.request_repaint_after(crate::engine::next_delay(snapshot.any_game_live()));
    }

    /// One game's detail subpage: score, status, and the full scoring
    /// play list with relevance chips, newest first.
    fn ui_game_detail(&mut self, ctx: &egui::Context, ui: &mut egui::Ui, event_id: &str) {
        if ui.button("← All games").clicked() {
            self.page = Page::Games;
            return;
        }
        let Some(snapshot) = self.current_snapshot() else {
            ui.heading("Loading game…");
            ctx.request_repaint_after(std::time::Duration::from_secs(2));
            return;
        };
        let Some(game) = snapshot.games.get(event_id) else {
            ui.label("Game not found in this week's slate.");
            return;
        };
        ui.horizontal(|ui| {
            ui.heading(game.matchup_label());
            let status = game.clock_or_state();
            if game.is_live() {
                ui.colored_label(egui::Color32::RED, format!("● LIVE · {status}"));
            } else {
                ui.weak(status);
            }
        });
        ui.separator();
        ui.heading("Scoring plays");
        egui::ScrollArea::vertical().show(ui, |ui| {
            let mut ids = std::collections::BTreeSet::new();
            ids.insert(event_id.to_string());
            let lines = snapshot.play_stream(&ids);
            if lines.is_empty() {
                ui.label("No scoring plays yet.");
            }
            for line in &lines {
                ui.horizontal_wrapped(|ui| {
                    if line.live {
                        ui.colored_label(egui::Color32::RED, "●");
                    }
                    if let Some(team) = &line.team {
                        ui.strong(team.clone());
                    }
                    ui.label(line.text.clone());
                    ui.small(line.caption.clone());
                    let rels: Vec<_> = line
                        .relevant
                        .iter()
                        .filter(|r| self.league_selected_filtered(&snapshot, &r.league_id))
                        .collect();
                    for rel in rels {
                        let star = if rel.starter { "★" } else { "○" };
                        let pts = rel
                            .est_points
                            .map(|p| format!(" +{:.1}", p))
                            .unwrap_or_default();
                        let color = if rel.starter {
                            egui::Color32::GOLD
                        } else {
                            egui::Color32::GRAY
                        };
                        if ui
                            .small_button(
                                egui::RichText::new(format!("{star} {}{pts}", rel.player))
                                    .color(color),
                            )
                            .clicked()
                        {
                            self.popup = Some(PlayerView {
                                sleeper_id: String::new(),
                                name: rel.player.clone(),
                                position: None,
                                nfl_team: None,
                                points: rel.est_points.unwrap_or(0.0),
                                is_starter: rel.starter,
                                game: None,
                                injury_status: None,
                                number: None,
                                age: None,
                            });
                        }
                    }
                });
            }
        });
        ctx.request_repaint_after(crate::engine::next_delay(game.is_live()));
    }

    /// Is a league's relevance visible, honoring the Zone page's
    /// All/Off league filter when one is sized?
    fn league_selected_filtered(&self, snapshot: &TrackerSnapshot, league_id: &str) -> bool {
        match &self.zone_filter {
            None => true,
            Some(selected) => snapshot
                .leagues
                .iter()
                .zip(selected.iter())
                .any(|(l, on)| *on && l.league_id == league_id),
        }
    }

    /// Is `league_id` among the selected leagues of this snapshot?
    fn league_selected(
        &self,
        selected: &[bool],
        league_id: &str,
        snapshot: &TrackerSnapshot,
    ) -> bool {
        snapshot
            .leagues
            .iter()
            .zip(selected.iter())
            .any(|(l, on)| *on && l.league_id == league_id)
    }
}

impl ZoneApp {
    fn ui_username(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        ui.heading("Sleeper Zone Desktop");
        ui.separator();
        ui.label("Enter your Sleeper username to find your leagues.");
        let mut go = false;
        if let SetupState::Username(name) = &mut self.setup {
            let response = ui.add(egui::TextEdit::singleline(name).hint_text("username"));
            go = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            ui.add_space(8.0);
            go |= ui.button("Find leagues").clicked();
        }
        if let Some(err) = &self.error {
            ui.colored_label(egui::Color32::RED, err);
        }
        if go {
            // One-shot setup flow: short-lived runtime, blocking the UI
            // for ~2 requests is acceptable before any live tracking.
            let username = self.setup_username().to_string();
            let result = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .map_err(|e| e.to_string())
                .and_then(|rt| {
                    rt.block_on(async {
                        let sleeper = SleeperClient::new();
                        match sleeper.user(&username).await {
                            Ok(Some(u)) => {
                                let season = sleeper
                                    .nfl_state()
                                    .await
                                    .map(|s| s.season)
                                    .unwrap_or_default();
                                sleeper
                                    .user_leagues(&u.user_id, &season)
                                    .await
                                    .map_err(|e| e.to_string())
                                    .map(|ls| (u, ls))
                            }
                            Ok(None) => Err(format!("no Sleeper user named '{username}'")),
                            Err(e) => Err(e.to_string()),
                        }
                    })
                });
            match result {
                Ok((user, leagues)) => {
                    self.error = None;
                    let n = leagues.len();
                    self.setup = SetupState::PickLeagues {
                        user,
                        leagues,
                        selected: vec![true; n],
                    };
                }
                Err(e) => self.error = Some(e),
            }
            ctx.request_repaint();
        }
    }

    fn ui_pick_leagues(&mut self, ui: &mut egui::Ui) {
        ui.heading("Pick your leagues");
        ui.separator();
        let mut any = false;
        if let SetupState::PickLeagues {
            user,
            leagues,
            selected,
        } = &mut self.setup
        {
            ui.label(format!(
                "Signed in as {} ({})",
                user.display_name.as_deref().unwrap_or("?"),
                user.username.as_deref().unwrap_or("?")
            ));
            if leagues.is_empty() {
                ui.label("No NFL leagues found for the current season.");
            }
            for (i, league) in leagues.iter().enumerate() {
                let mut on = selected[i];
                let label = format!(
                    "{}  ·  {} rosters  ·  season {}",
                    league.name.as_deref().unwrap_or(league.league_id.as_str()),
                    league.total_rosters.unwrap_or(0),
                    league.season.as_deref().unwrap_or("?"),
                );
                if ui.checkbox(&mut on, label).changed() {
                    selected[i] = on;
                }
                any |= on;
            }
            ui.add_space(8.0);
            let start = ui.add_enabled(any, egui::Button::new("Start tracking"));
            if start.clicked() {
                self.config.username = user.username.clone().unwrap_or_default();
                self.config.user_id = user.user_id.clone();
                self.config.league_ids = leagues
                    .iter()
                    .zip(selected.iter())
                    .filter(|(_, s)| **s)
                    .map(|(l, _)| l.league_id.clone())
                    .collect();
                if let Err(e) = self.config.save() {
                    self.error = Some(format!("could not save config: {e}"));
                }
                self.setup = SetupState::Done;
                self.start_tracking();
            }
        }
        if let Some(err) = &self.error {
            ui.colored_label(egui::Color32::RED, err);
        }
    }

    fn ui_league(
        &mut self,
        ui: &mut egui::Ui,
        league: &LeagueView,
        games: &std::collections::BTreeMap<String, GameView>,
    ) {
        ui.heading(league.name.as_deref().unwrap_or(&league.league_id));
        for teams in league.matchups.values() {
            if teams.len() == 2 {
                self.ui_matchup(ui, &teams[0], &teams[1], games);
            } else {
                for t in teams {
                    self.ui_team_line(ui, t, games);
                }
            }
            ui.add_space(6.0);
        }
        ui.separator();
    }

    fn ui_matchup(
        &mut self,
        ui: &mut egui::Ui,
        a: &TeamView,
        b: &TeamView,
        games: &std::collections::BTreeMap<String, GameView>,
    ) {
        egui::Grid::new(format!("m{}-{}", a.roster_id, b.roster_id))
            .num_columns(4)
            .striped(true)
            .show(ui, |ui| {
                self.ui_team_line(ui, a, games);
                ui.end_row();
                self.ui_team_line(ui, b, games);
                ui.end_row();
            });
    }

    /// One team's scoreboard line; every player is clickable and opens
    /// the detail popup.
    fn ui_team_line(
        &mut self,
        ui: &mut egui::Ui,
        team: &TeamView,
        games: &std::collections::BTreeMap<String, GameView>,
    ) {
        ui.horizontal_wrapped(|ui| {
            ui.monospace(format!("R{}", team.roster_id));
            if let Some(name) = &team.name {
                ui.strong(name.clone());
            }
            ui.strong(format!("{:.2}", team.points));
            for p in team.starters.iter().take(9) {
                let mut text = format!("{} {:.1}", p.name, p.points);
                if let Some(clock) = p
                    .game
                    .as_ref()
                    .and_then(|g| games.get(&g.espn_event_id))
                    .filter(|game| game.is_live())
                    .map(|game| game.clock_or_state())
                {
                    text.push_str(&format!("  [{clock}]"));
                }
                if ui.small_button(text).clicked() {
                    self.popup = Some(p.clone());
                }
            }
        });
    }

    /// Full team block for a league tab: every rostered player,
    /// starters and bench, all clickable.
    /// Per-position roster tables for one team: one table per roster slot
    /// (FLEX and SUPER_FLEX when the league uses them), with this week's
    /// points, season average, weeks missed, bye flag, and a stat line.
    fn ui_roster_tables(
        &mut self,
        ui: &mut egui::Ui,
        team: &TeamView,
        league: &LeagueView,
        games: &std::collections::BTreeMap<String, GameView>,
    ) {
        ui.horizontal(|ui| {
            ui.heading(
                team.name
                    .clone()
                    .unwrap_or_else(|| format!("Roster {}", team.roster_id)),
            );
            ui.strong(format!("{:.2} pts", team.points));
        });
        // All rostered players, most points first; each player is listed
        // under the FIRST matching slot so primary slots win over FLEX.
        let mut pool: Vec<&PlayerView> = team
            .starters
            .iter()
            .chain(team.bench.iter())
            .filter(|p| !p.sleeper_id.is_empty())
            .collect();
        pool.sort_by(|a, b| b.points.total_cmp(&a.points));
        let slots: Vec<String> = league.roster_positions.clone();
        let mut shown: Vec<&PlayerView> = Vec::new();
        for slot in &slots {
            let group = crate::engine::join::slot_positions(slot);
            if group.is_empty() {
                continue;
            }
            let mut rows: Vec<&&PlayerView> = pool
                .iter()
                .filter(|p| !shown.contains(*p))
                .filter(|p| {
                    p.position
                        .as_deref()
                        .is_some_and(|pos| group.contains(&pos))
                })
                .collect();
            if rows.is_empty() {
                continue;
            }
            // Sortable columns: 0 Player, 1 Pts, 2 Avg, 3 Missed, 4 Bye,
            // 5 This week. Header click sorts; clicking the active column
            // flips direction. Defaults to points, descending.
            let (sort_col, desc) = self.table_sort.get(slot).copied().unwrap_or((1, true));
            let cmp = |a: &&PlayerView, b: &&PlayerView| {
                let sa = league.player_stats.get(&a.sleeper_id);
                let sb = league.player_stats.get(&b.sleeper_id);
                match sort_col {
                    0 => a.name.cmp(&b.name),
                    1 => a.points.total_cmp(&b.points),
                    2 => {
                        let av = sa.and_then(|s| s.avg_points).unwrap_or(f64::MIN);
                        let bv = sb.and_then(|s| s.avg_points).unwrap_or(f64::MIN);
                        av.total_cmp(&bv)
                    }
                    3 => {
                        let av = sa.and_then(|s| s.weeks_missed).unwrap_or(u8::MAX);
                        let bv = sb.and_then(|s| s.weeks_missed).unwrap_or(u8::MAX);
                        av.cmp(&bv)
                    }
                    4 => sa.is_some_and(|s| s.bye).cmp(&sb.is_some_and(|s| s.bye)),
                    _ => {
                        let at = sa.map(|s| s.week_line.clone()).unwrap_or_default();
                        let bt = sb.map(|s| s.week_line.clone()).unwrap_or_default();
                        at.cmp(&bt)
                    }
                }
            };
            rows.sort_by(|a, b| cmp(a, b));
            if desc {
                rows.reverse();
            }
            for row in &rows {
                shown.push(*row);
            }
            ui.add_space(4.0);
            ui.heading(slot_label(slot));
            const COLS: [&str; 6] = ["Player", "Pts", "Avg", "Missed", "Bye", "This week"];
            // Columns that read naturally as descending (flip default on
            // first click): numeric and boolean ones.
            const DESC_DEFAULT: [bool; 6] = [false, true, true, false, true, false];
            egui::Grid::new(format!("roster_grid_{slot}"))
                .striped(true)
                .num_columns(6)
                .show(ui, |ui| {
                    for (col, label) in COLS.iter().enumerate() {
                        let active = sort_col as usize == col;
                        let arrow = if active {
                            if desc { " ▾" } else { " ▴" }
                        } else {
                            ""
                        };
                        if ui
                            .selectable_label(active, format!("{label}{arrow}"))
                            .clicked()
                        {
                            let next = if active {
                                (sort_col, !desc)
                            } else {
                                (col as u8, DESC_DEFAULT[col])
                            };
                            self.table_sort.insert(slot.clone(), next);
                        }
                    }
                    ui.end_row();
                    for p in &rows {
                        let stats = league.player_stats.get(&p.sleeper_id);
                        self.ui_player_cell(ui, p, games);
                        ui.strong(format!("{:.1}", p.points));
                        ui.label(match stats.and_then(|s| s.avg_points) {
                            Some(avg) => format!("{avg:.1}"),
                            None => "-".into(),
                        });
                        ui.label(
                            stats
                                .and_then(|s| s.weeks_missed)
                                .map(|m| m.to_string())
                                .unwrap_or_else(|| "-".into()),
                        );
                        ui.label(
                            stats
                                .map(|s| if s.bye { "BYE" } else { "-" })
                                .unwrap_or("-"),
                        );
                        ui.label(
                            stats
                                .map(|s| s.week_line.clone())
                                .unwrap_or_else(|| "-".into()),
                        );
                        ui.end_row();
                    }
                });
        }
        // Any player whose position doesn't fit a slot (IR, DNP, unknown).
        let leftover: Vec<&&PlayerView> = pool.iter().filter(|p| !shown.contains(*p)).collect();
        if !leftover.is_empty() {
            ui.add_space(4.0);
            ui.heading("Other");
            for p in &leftover {
                self.ui_player_row(ui, p, games);
            }
        }
        ui.add_space(4.0);
    }

    fn ui_player_cell(
        &mut self,
        ui: &mut egui::Ui,
        p: &PlayerView,
        games: &std::collections::BTreeMap<String, GameView>,
    ) {
        let mut text = p.name.clone();
        if let Some(inj) = &p.injury_status {
            text.push_str(&format!(" [{inj}]"));
        }
        if p.game
            .as_ref()
            .and_then(|g| games.get(&g.espn_event_id))
            .is_some_and(|game| game.is_live())
        {
            text.push_str("  ●");
        }
        if ui.link(text).clicked() {
            self.popup = Some(p.clone());
        }
    }

    fn ui_player_row(
        &mut self,
        ui: &mut egui::Ui,
        p: &PlayerView,
        games: &std::collections::BTreeMap<String, GameView>,
    ) {
        ui.horizontal(|ui| {
            let mut text = format!(
                "{} · {} · {:.1}",
                p.name,
                p.position.clone().unwrap_or_else(|| "?".into()),
                p.points
            );
            if let Some(inj) = &p.injury_status {
                text.push_str(&format!("  [{inj}]"));
            }
            if let Some(clock) = p
                .game
                .as_ref()
                .and_then(|g| games.get(&g.espn_event_id))
                .filter(|game| game.is_live())
                .map(|game| game.clock_or_state())
            {
                text.push_str(&format!("  ({clock})"));
            }
            if ui.small_button(text).clicked() {
                self.popup = Some(p.clone());
            }
        });
    }

    /// A league tab: full team rosters plus the draft picks (when the
    /// league has a draft).
    fn ui_league_page(&mut self, ctx: &egui::Context, ui: &mut egui::Ui, league_id: &str) {
        let Some(snapshot) = self.current_snapshot() else {
            ui.heading("Loading league…");
            ctx.request_repaint_after(std::time::Duration::from_secs(2));
            return;
        };
        let Some(league) = snapshot
            .leagues
            .iter()
            .find(|l| l.league_id == league_id)
            .cloned()
        else {
            ui.label("League not found in the current snapshot.");
            return;
        };
        ui.heading(league.name.as_deref().unwrap_or(&league.league_id));
        ui.separator();
        egui::ScrollArea::vertical().show(ui, |ui| {
            // Team selector: defaults to the user's own roster.
            let mut teams: Vec<&TeamView> = league.matchups.values().flatten().collect();
            teams.sort_by_key(|t| t.roster_id);
            let default_roster = league
                .user_roster_id
                .or_else(|| teams.first().map(|t| t.roster_id));
            let mut selected = match self.league_team.get(league_id) {
                Some(id) if teams.iter().any(|t| t.roster_id == *id) => *id,
                _ => default_roster.unwrap_or(0),
            };
            let current = teams.iter().find(|t| t.roster_id == selected);
            let label = current
                .map(|t| {
                    format!(
                        "{} (R{}, {:.2} pts)",
                        t.name.as_deref().unwrap_or("Team"),
                        t.roster_id,
                        t.points
                    )
                })
                .unwrap_or_else(|| "Team".to_string());
            ui.horizontal(|ui| {
                ui.label("Team:");
                egui::ComboBox::from_id_salt("league_team_select")
                    .selected_text(label)
                    .show_ui(ui, |ui| {
                        for t in &teams {
                            let text = format!(
                                "{} (R{}, {:.2})",
                                t.name.as_deref().unwrap_or("Team"),
                                t.roster_id,
                                t.points
                            );
                            ui.selectable_value(&mut selected, t.roster_id, text);
                        }
                    });
            });
            self.league_team.insert(league_id.to_string(), selected);
            let current = teams.iter().find(|t| t.roster_id == selected);
            ui.separator();
            if let Some(t) = current {
                self.ui_roster_tables(ui, t, &league, &snapshot.games);
            }
            ui.separator();
            ui.heading("Draft picks");
            let team_picks: Vec<&PickView> = league
                .picks
                .iter()
                .filter(|p| p.roster_id == Some(selected))
                .collect();
            if team_picks.is_empty() {
                ui.label("No draft picks available for this team.");
            } else {
                for pick in team_picks {
                    ui.monospace(format!(
                        "Rd {} · Slot {} · {}",
                        pick.round
                            .map(|r| r.to_string())
                            .unwrap_or_else(|| "?".into()),
                        pick.slot
                            .map(|s| s.to_string())
                            .unwrap_or_else(|| "?".into()),
                        pick.player
                    ));
                }
            }
            ui.add_space(8.0);
            ui.separator();
            self.ui_free_agents(ui, &league);
        });
        ctx.request_repaint_after(crate::engine::next_delay(snapshot.any_game_live()));
    }

    /// Free-agent / waiver browser: position filter, search box, and a
    /// clickable list of available players.
    fn ui_free_agents(&mut self, ui: &mut egui::Ui, league: &LeagueView) {
        ui.heading("Available players (FA / waivers)");
        ui.horizontal(|ui| {
            ui.label("Filter:");
            for pos in ["QB", "RB", "WR", "TE"] {
                let active = self.fa_pos.as_deref() == Some(pos);
                if ui.selectable_label(active, pos).clicked() {
                    self.fa_pos = if active { None } else { Some(pos.to_string()) };
                }
            }
            if ui.button("clear pos").clicked() {
                self.fa_pos = None;
            }
        });
        ui.text_edit_singleline(&mut self.fa_search);
        let search = self.fa_search.to_lowercase();
        let matches: Vec<&PlayerView> = league
            .free_agents
            .iter()
            .filter(|p| {
                self.fa_pos
                    .as_deref()
                    .is_none_or(|f| p.position.as_deref() == Some(f))
            })
            .filter(|p| search.is_empty() || p.name.to_lowercase().contains(&search))
            .take(100)
            .collect();
        ui.label(format!(
            "{} shown (of {} available)",
            matches.len(),
            league.free_agents.len()
        ));
        egui::Grid::new("fa_grid")
            .striped(true)
            .num_columns(4)
            .show(ui, |ui| {
                for p in matches {
                    let pos = p.position.clone().unwrap_or_default();
                    let team = p.nfl_team.clone().unwrap_or_default();
                    if ui.small_button(p.name.clone()).clicked() {
                        self.popup = Some(p.clone());
                    }
                    ui.label(pos);
                    ui.label(team);
                    if let Some(inj) = &p.injury_status {
                        ui.colored_label(egui::Color32::YELLOW, inj.clone());
                    } else {
                        ui.label("");
                    }
                    ui.end_row();
                }
            });
    }
}

impl GameView {
    fn clock_or_state(&self) -> String {
        match (&self.display_clock, self.period) {
            (Some(c), Some(p)) => format!("Q{p} {c}"),
            (Some(c), None) => c.clone(),
            (None, Some(p)) => format!("Q{p}"),
            (None, None) => self.state.clone().unwrap_or_default(),
        }
    }
}
