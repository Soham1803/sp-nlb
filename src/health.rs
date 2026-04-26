use crate::state::SharedState;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::time::{self, timeout};

pub async fn run_health_checker(state: SharedState) {
    let mut interval = time::interval(Duration::from_secs(5));
    
    loop {
        interval.tick().await;
        
        // We take a read lock first to get the list of backends
        let backends: Vec<(usize, SocketAddr)> = {
            let state_guard = state.read().await;
            state_guard.backends.iter()
                .enumerate()
                .map(|(i, b)| (i, b.addr))
                .collect()
        };

        for (idx, addr) in backends {
            let is_healthy = check_backend(addr).await;
            
            // Only take a write lock if the status has actually changed
            let mut state_guard = state.write().await;
            if state_guard.backends[idx].is_healthy != is_healthy {
                state_guard.backends[idx].is_healthy = is_healthy;

                if is_healthy {
                    tracing::info!("Backend {} is now HEALTHY", addr);
                } else {
                    tracing::warn!("Backend {} is now UNHEALTHY", addr);
                }
            }
        }
    }
}

async fn check_backend(addr: SocketAddr) -> bool {
    // Try to connect to the backend with a 1-second timeout
    match timeout(Duration::from_secs(1), TcpStream::connect(addr)).await {
        Ok(Ok(_)) => true,
        _ => false,
    }
}
