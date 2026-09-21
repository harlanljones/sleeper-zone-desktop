//! Live smoke test against ESPN's public API. Ignored by default so
//! `cargo test` stays hermetic; run with:
//!
//!   cargo test --test espn_live -- --ignored

use sleeper_zone::espn::EspnClient;

#[tokio::test]
#[ignore = "hits the real ESPN API; run with --ignored"]
async fn live_espn_api_smoke() {
    let client = EspnClient::new();

    let sb = client.scoreboard().await.expect("live scoreboard fetch");
    assert!(!sb.events.is_empty(), "scoreboard always lists events");
    assert!(
        sb.week_number().is_some(),
        "scoreboard carries current week"
    );

    // Summary for a completed-or-live game: drives should exist.
    let with_pbp = sb
        .events
        .iter()
        .find(|e| !e.is_live())
        .or_else(|| sb.events.first())
        .expect("at least one event");
    let summary = client
        .summary(&with_pbp.id)
        .await
        .expect("live summary fetch");
    if with_pbp.competition().is_some_and(|c| {
        c.status
            .as_ref()
            .and_then(|s| s.status_type.as_ref())
            .and_then(|t| t.completed)
            == Some(true)
    }) {
        assert!(
            summary
                .drives
                .as_ref()
                .is_some_and(|d| !d.previous.is_empty()),
            "completed game has recorded drives"
        );
    }

    // Week-targeted query works too.
    let week = sb.week_number().expect("week");
    let season = sb.season_year().expect("season year");
    let week_sb = client
        .scoreboard_week(season, week)
        .await
        .expect("week scoreboard fetch");
    assert!(!week_sb.events.is_empty());
}
