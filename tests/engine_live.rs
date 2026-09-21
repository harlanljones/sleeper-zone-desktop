//! Live engine test: one real Tracker::tick against both production
//! APIs. Ignored by default; run with `cargo test --test engine_live
//! -- --ignored`.

use sleeper_zone::engine::{Tracker, TrackerSnapshot};
use sleeper_zone::espn::EspnClient;
use sleeper_zone::sleeper::SleeperClient;

#[tokio::test]
#[ignore = "hits api.sleeper.app and ESPN live"]
async fn live_engine_tick_joins_both_apis() {
    // The 2018 Sleeper Friends League — stable, public, and its matchup
    // payloads are still served for any week.
    let sleeper = SleeperClient::new();
    let espn = EspnClient::new();
    let players = sleeper
        .players()
        .await
        .expect("player map fetch (slow, ~5 MB)");
    let tracker = Tracker::new(
        sleeper,
        espn,
        vec!["289646328504385536".to_string()],
        players,
        None,
    );

    let snapshot: TrackerSnapshot = tracker.tick().await.expect("tick succeeds");
    assert_eq!(snapshot.leagues.len(), 1);
    let league = &snapshot.leagues[0];
    assert_eq!(league.league_id, "289646328504385536");
    assert_eq!(
        league.matchups.values().map(|t| t.len()).sum::<usize>(),
        12,
        "12 rosters grouped into 6 matchups"
    );
    // Every team view carries points from Sleeper and a resolved
    // team/owner name.
    for teams in league.matchups.values() {
        assert_eq!(teams.len(), 2, "matchup pairs two teams");
        assert!(teams.iter().all(|t| t.points >= 0.0));
        assert!(
            teams.iter().all(|t| t.name.is_some()),
            "team names resolved from league users"
        );
    }
    // Game layer: the week's full slate joins (not just tracked games).
    assert!(
        !snapshot.games.is_empty(),
        "tracked players join onto current-week ESPN games"
    );
    assert!(
        snapshot.games.len() >= 14,
        "full week slate expected, got {}",
        snapshot.games.len()
    );
    assert!(snapshot.week.is_some());
    // FA/waiver pool: rostered players excluded, active NFL players kept.
    // Roster-table stats: every rostered player has a context entry.
    let rostered: usize = snapshot.leagues[0]
        .matchups
        .values()
        .map(|teams| {
            teams
                .iter()
                .map(|t| t.starters.len() + t.bench.len())
                .sum::<usize>()
        })
        .sum();
    assert_eq!(snapshot.leagues[0].player_stats.len(), rostered);
    assert!(
        !snapshot.leagues[0].free_agents.is_empty(),
        "free-agent pool should be populated"
    );
    let rostered_total: usize = league
        .matchups
        .values()
        .map(|t| {
            t.iter()
                .map(|tm| tm.starters.len() + tm.bench.len())
                .sum::<usize>()
        })
        .sum();
    assert_ne!(
        snapshot.leagues[0].free_agents.len(),
        rostered_total,
        "FA pool must exclude rostered players"
    );
}

/// The play stream data path live: ids stable, captions present, and
/// relevance estimation runs over real ESPN text without panicking.
/// Week-2 games may be idle, so only shape is asserted.
#[tokio::test]
#[ignore = "hits api.sleeper.app and ESPN live"]
async fn live_play_stream_shape() {
    let sleeper = SleeperClient::new();
    let espn = EspnClient::new();
    let players = sleeper
        .players()
        .await
        .expect("player map fetch (slow, ~5 MB)");
    let tracker = Tracker::new(
        sleeper,
        espn,
        vec!["289646328504385536".to_string()],
        players,
        None,
    );
    let snapshot = tracker.tick().await.expect("tick succeeds");
    let all_ids = snapshot.visible_game_ids(&[true]);
    let lines = snapshot.play_stream(&all_ids);
    assert!(
        !snapshot.leagues[0].scoring.is_empty(),
        "scoring settings fetched"
    );
    for line in &lines {
        assert!(!line.id.is_empty());
        assert!(!line.text.is_empty());
        for rel in &line.relevant {
            assert_eq!(rel.league_id, snapshot.leagues[0].league_id);
            assert!(!rel.player.is_empty());
            if let Some(p) = rel.est_points {
                assert!((0.0..=99.0).contains(&p), "plausible points, got {p}");
            }
        }
    }
}
