//! Sync engine: joins the Sleeper fantasy layer with the ESPN game
//! layer and produces UI-ready snapshots on an adaptive poll cadence.

pub mod join;
pub mod state;
pub mod tracker;

pub use state::{
    GameRef, GameView, LeagueView, PickView, PlayerView, ScoringPlayView, TeamView, TrackerSnapshot,
};
pub use tracker::{IDLE_POLL, SCORING_PLAY_WINDOW, TRACKED_POLL, Tracker, next_delay};
