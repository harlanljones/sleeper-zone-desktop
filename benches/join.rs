//! Criterion benchmarks for the sync engine's join path: the hot loop
//! is building the abbreviation map and joining a full 12-team league
//! against the scoreboard. Before/after numbers gate any `perf:`
//! claim (AGENTS.md).

use std::collections::HashMap;
use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};

use sleeper_zone::engine::join::{build_abbreviation_map, game_view_of, resolve_game};
use sleeper_zone::espn::Scoreboard;
use sleeper_zone::sleeper::{Matchup, Player, PlayerMap};

fn scoreboard() -> Scoreboard {
    let raw = std::fs::read_to_string("testdata/espn/scoreboard.json").expect("fixture");
    serde_json::from_str(&raw).expect("parses")
}

fn matchups() -> Vec<Matchup> {
    let raw = std::fs::read_to_string("testdata/matchups.json").expect("fixture");
    serde_json::from_str(&raw).expect("parses")
}

/// Full player-map-sized fixture: the real sample repeated to ~10k
/// entries, approximating one league's join surface (players map is
/// only filtered by tracked rosters in practice).
fn player_map() -> PlayerMap {
    let sample_raw = std::fs::read_to_string("testdata/players_sample.json").expect("fixture");
    let sample: PlayerMap = serde_json::from_str(&sample_raw).expect("parses");
    let mut map: PlayerMap = HashMap::new();
    for i in 0..2_000u32 {
        for (k, p) in &sample {
            let cloned =
                serde_json::from_str::<Player>(&serde_json::to_string(p).expect("reserialize"))
                    .expect("reparses");
            map.insert(format!("{k}-{i}"), cloned);
        }
    }
    map
}

fn bench_join(c: &mut Criterion) {
    let sb = scoreboard();
    let matchups = matchups();
    let players = player_map();

    c.bench_function("build_abbreviation_map", |b| {
        b.iter(|| build_abbreviation_map(black_box(&sb)))
    });

    let abbr = build_abbreviation_map(&sb);
    c.bench_function("join_full_league", |b| {
        b.iter(|| {
            let mut joined = 0usize;
            for m in black_box(&matchups) {
                for pid in &m.players {
                    if let Some(p) = players
                        .get(pid)
                        .filter(|p| resolve_game(p, black_box(&abbr)).is_some())
                    {
                        joined += 1;
                        let _ = p;
                    }
                }
            }
            joined
        })
    });

    c.bench_function("game_views", |b| {
        b.iter(|| {
            let mut views = Vec::new();
            for e in black_box(&sb.events) {
                views.push(game_view_of(e, None, 8));
            }
            views
        })
    });
}

criterion_group!(benches, bench_join);
criterion_main!(benches);
