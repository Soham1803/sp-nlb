mod state;
mod proxy;
mod health;
mod passthrough;
mod arp;

use std::sync::{Arc, RwLock};

use crate::state::{Backend, State, SharedState};
use crate::health::run_health_checker;
use crate::passthrough::run_passthrough_nlb;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing for logging
    tracing_subscriber::fmt::init();

    // Initialize the shared state
    let state: SharedState = Arc::new(RwLock::new(State::new(vec![
        Backend {
            addr: "127.0.0.1:8081".parse()?,
            is_healthy: true,
        },
        Backend {
            addr: "127.0.0.1:8082".parse()?,
            is_healthy: true,
        },
    ])));

    // Phase 2: Spawn health checker
    let state_for_checker = Arc::clone(&state);
    tokio::spawn(async move {
        run_health_checker(state_for_checker).await;
    });

    // Phase 3: Passthrough NLB
    let state_for_passthrough = Arc::clone(&state);
    // Note: We might want to pass the real interface name here in production
    tokio::task::spawn_blocking(move || {
        if let Err(e) = run_passthrough_nlb("lo", 8080, state_for_passthrough) {
            tracing::error!("Passthrough nlb error: {}", e);
        }
    });

    // Phase 4: Spawn Reaper task
    let state_for_reaper = Arc::clone(&state);
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(30));
        loop {
            interval.tick().await;
            {
                let mut guard = state_for_reaper.write().expect("Failed to acquire write lock for reaper");
                guard.cleanup(
                    tokio::time::Duration::from_secs(300), // 5 minutes for flows
                    tokio::time::Duration::from_secs(3600) // 1 hour for ARP entries
                );
            }
        }
    });

    tracing::info!("NLB started in Passthrough mode on port 8080");

    // Keep the main thread alive
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
    }
}
