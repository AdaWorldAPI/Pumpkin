use crate::{SHOULD_STOP, server::Server};
use std::{
    sync::{Arc, OnceLock, atomic::Ordering},
    time::{Duration, Instant},
};
use tokio::time::sleep;

pub struct Ticker;

fn idle_tick_interval() -> Option<Duration> {
    static INTERVAL: OnceLock<Option<Duration>> = OnceLock::new();
    *INTERVAL.get_or_init(|| {
        std::env::var("PUMPKIN_IDLE_TICK_MS").map_or_else(
            |_| Some(Duration::from_millis(1000)),
            |value| {
                value
                    .parse::<u64>()
                    .ok()
                    .and_then(|millis| (millis > 0).then_some(Duration::from_millis(millis)))
            },
        )
    })
}

impl Ticker {
    /// IMPORTANT: Run this in a new thread/tokio task.
    pub async fn run(server: &Arc<Server>) {
        let mut last_tick = Instant::now();
        while !SHOULD_STOP.load(Ordering::Relaxed) {
            let tick_start_time = Instant::now();
            let manager = &server.tick_rate_manager;
            let no_players_online = !server.has_n_players(1);

            manager.tick();

            // Now server.tick() handles both player/network ticking (always)
            // and world logic ticking (conditionally based on freeze state)
            if manager.is_sprinting() {
                // A sprint is active, so we tick.
                manager.start_sprint_tick_work();
                server.tick().await;

                // After ticking, end the work and check if the sprint is over.
                if manager.end_sprint_tick_work() {
                    // This was the last sprint tick. Finish the sprint and restore the previous state.
                    manager.finish_tick_sprint(server).await;
                }
            } else {
                // Always call tick - it will internally decide what to tick based on frozen state
                server.tick().await;
            }

            // Record the total time this tick took
            let tick_duration_nanos = tick_start_time.elapsed().as_nanos() as i64;
            server.update_tick_times(tick_duration_nanos).await;

            // Sleep logic remains the same
            let now = Instant::now();
            let elapsed = now.duration_since(last_tick);

            let tick_interval = if manager.is_sprinting() {
                Duration::ZERO
            } else if no_players_online {
                idle_tick_interval()
                    .unwrap_or_else(|| Duration::from_nanos(manager.nanoseconds_per_tick() as u64))
            } else {
                Duration::from_nanos(manager.nanoseconds_per_tick() as u64)
            };

            if let Some(sleep_time) = tick_interval.checked_sub(elapsed)
                && !sleep_time.is_zero()
            {
                if no_players_online {
                    // Keep idle sleeps interruptible so the first joining player wakes
                    // the simulation quickly even with a large idle interval configured.
                    let mut remaining = sleep_time;
                    while !remaining.is_zero()
                        && !SHOULD_STOP.load(Ordering::Relaxed)
                        && !server.has_n_players(1)
                    {
                        let step = remaining.min(Duration::from_millis(50));
                        sleep(step).await;
                        remaining = remaining.saturating_sub(step);
                    }
                } else {
                    sleep(sleep_time).await;
                }
            }

            last_tick = Instant::now();
        }
        log::debug!("Ticker stopped");
    }
}
