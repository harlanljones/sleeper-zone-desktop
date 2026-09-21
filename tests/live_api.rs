//! Live smoke test against the real Sleeper API. Ignored by default so
//! `cargo test` stays hermetic; run with:
//!
//!   cargo test --test live_api -- --ignored
//!
//! Exercises every documented endpoint this milestone depends on and
//! asserts structural sanity (not volatile fantasy numbers) against
//! production.

use sleeper_zone::sleeper::{SleeperClient, User};

const LIVE_LEAGUE: &str = "289646328504385536"; // Sleeperbot Dynasty (docs example, still live)

#[tokio::test]
#[ignore = "hits the real api.sleeper.app; run with --ignored"]
async fn live_sleeper_api_smoke() {
    let client = SleeperClient::new();

    // Known user resolves with a stable identity.
    let bot: User = client
        .user("sleeperbot")
        .await
        .expect("user fetch")
        .expect("sleeperbot exists");
    assert_eq!(bot.display_name.as_deref(), Some("SleeperBot"));

    // NFL state is live and season-shaped.
    let state = client.nfl_state().await.expect("state fetch");
    assert!(!state.season.is_empty());
    assert!(state.week >= 1);

    // A user with no leagues in a season returns null -> empty vec.
    let empty = client
        .user_leagues(&bot.user_id, "1972")
        .await
        .expect("null-tolerant leagues fetch");
    assert!(empty.is_empty());

    // League, rosters, users, matchups all resolve with sane structure.
    let league = client.league(LIVE_LEAGUE).await.expect("league fetch");
    assert!(!league.league_id.is_empty());

    let rosters = client.rosters(LIVE_LEAGUE).await.expect("rosters fetch");
    assert!(!rosters.is_empty());
    assert!(rosters.iter().all(|r| !r.players.is_empty()));

    let users = client
        .league_users(LIVE_LEAGUE)
        .await
        .expect("league users fetch");
    assert!(!users.is_empty());

    let matchups = client
        .matchups(LIVE_LEAGUE, 1)
        .await
        .expect("matchups fetch");
    assert!(!matchups.is_empty());
    assert!(matchups.iter().all(|m| m.points.is_finite()));

    // Player map resolves and is the expected order of magnitude.
    let players = client.players().await.expect("players fetch");
    assert!(players.len() > 10_000, "player map should be large");
}
