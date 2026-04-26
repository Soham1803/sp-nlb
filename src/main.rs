mod state;
mod proxy;
mod health;
mod passthrough;

use std::sync::Arc;
use tokio::sync::RwLock;

use crate::state::{Backend, State, SharedState};
use crate::health::run_health_checker;
use crate::passthrough::run_passthrough_nlb;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing for logging
    tracing_subscriber::fmt::init();

    // Initialize the shared state with some dummy backends for now.
    let state: SharedState = Arc::new(RwLock::new(State {
        backends: vec![
            Backend {
                addr: "127.0.0.1:8081".parse()?,
                is_healthy: true,
            },
            Backend {
                addr: "127.0.0.1:8082".parse()?,
                is_healthy: true,
            },
        ],
        next_backend: 0,
    }));

    // Phase 2: Spawn health checker
    let state_for_checker = Arc::clone(&state);
    tokio::spawn(async move {
        run_health_checker(state_for_checker).await;
    });

    // Phase 3: Passthrough NLB
    let state_for_passthrough = Arc::clone(&state);
    tokio::task::spawn_blocking(move || {
        if let Err(e) = run_passthrough_nlb("lo", 8080, state_for_passthrough) {
            tracing::error!("Passthrough nlb error: {}", e);
        }
    });

    tracing::info!("NLB started in Passthrough mode on port 8080");

    // Keep the main thread alive since we no longer have a listener loop here
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
    }
}
