//! Binary entrypoint: launches the egui app, or runs a headless check.

fn main() {
    env_logger::try_init().ok();

    // `--check <username>`: headless live smoke — resolve the user,
    // fetch the player map, run one tick against the current leagues,
    // print a summary. Exits nonzero on failure.
    let args: Vec<String> = std::env::args().collect();
    if args.len() == 3 && args[1] == "--check" {
        std::process::exit(headless_check(&args[2]));
    }
    if args.len() == 3 && args[1] == "--check-league" {
        std::process::exit(headless_check_league(&args[2]));
    }

    let native = eframe::NativeOptions::default();
    eframe::run_native(
        "Sleeper Zone Desktop",
        native,
        Box::new(|cc| Ok(Box::new(sleeper_zone::app::ZoneApp::new(cc)))),
    )
    .expect("egui app runs");
}

/// League-id headless check: skips username resolution (most public
/// users have no current-season leagues), tracks the given league.
fn headless_check_league(league_id: &str) -> i32 {
    let rt = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("runtime: {e}");
            return 1;
        }
    };
    rt.block_on(async {
        let sleeper = sleeper_zone::sleeper::SleeperClient::new();
        let espn = sleeper_zone::espn::EspnClient::new();
        let players = match sleeper.players().await {
            Ok(p) => p,
            Err(e) => {
                eprintln!("player map: {e}");
                return 1;
            }
        };
        let tracker = sleeper_zone::engine::Tracker::new(
            sleeper,
            espn,
            vec![league_id.to_string()],
            players,
            None,
        );
        match tracker.tick().await {
            Ok(snapshot) => {
                println!(
                    "check ok: leagues={} games={} week={:?} fetched_at={}",
                    snapshot.leagues.len(),
                    snapshot.games.len(),
                    snapshot.week,
                    snapshot.fetched_at,
                );
                0
            }
            Err(e) => {
                eprintln!("tick: {e}");
                1
            }
        }
    })
}

fn headless_check(username: &str) -> i32 {
    let rt = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("runtime: {e}");
            return 1;
        }
    };
    rt.block_on(async {
        let sleeper = sleeper_zone::sleeper::SleeperClient::new();
        let user = match sleeper.user(username).await {
            Ok(Some(u)) => u,
            Ok(None) => {
                eprintln!("no such user: {username}");
                return 1;
            }
            Err(e) => {
                eprintln!("user fetch: {e}");
                return 1;
            }
        };
        let season = match sleeper.nfl_state().await {
            Ok(s) => s.season,
            Err(e) => {
                eprintln!("state fetch: {e}");
                return 1;
            }
        };
        let leagues = match sleeper.user_leagues(&user.user_id, &season).await {
            Ok(l) if !l.is_empty() => l,
            Ok(_) => {
                eprintln!("user has no {season} leagues; headless check needs one");
                return 1;
            }
            Err(e) => {
                eprintln!("leagues fetch: {e}");
                return 1;
            }
        };
        let players = match sleeper.players().await {
            Ok(p) => p,
            Err(e) => {
                eprintln!("player map: {e}");
                return 1;
            }
        };
        let espn = sleeper_zone::espn::EspnClient::new();
        let league_ids = leagues.iter().map(|l| l.league_id.clone()).collect();
        let tracker = sleeper_zone::engine::Tracker::new(sleeper, espn, league_ids, players, None);
        match tracker.tick().await {
            Ok(snapshot) => {
                let matchups: usize = snapshot.leagues.iter().map(|l| l.matchups.len()).sum();
                println!(
                    "check ok: user={} leagues={} matchups={} games={} week={:?}",
                    user.username.as_deref().unwrap_or(username),
                    snapshot.leagues.len(),
                    matchups,
                    snapshot.games.len(),
                    snapshot.week,
                );
                0
            }
            Err(e) => {
                eprintln!("tick: {e}");
                1
            }
        }
    })
}
