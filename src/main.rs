use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::RwLock;

/// Represents a backend server that the load balancer will proxy traffic to.
#[derive(Debug)]
struct Backend {
    addr: SocketAddr,
    is_healthy: bool,
}

/// The global state of the load balancer, containing the list of backends and the RR index.
#[derive(Debug)]
struct State {
    backends: Vec<Backend>,
    /// The index of the next backend to select (for Round Robin).
    next_backend: usize,
}

impl State {
    /// Selects the next healthy backend using a simple Round Robin algorithm.
    fn next_backend_addr(&mut self) -> Option<SocketAddr> {
        let n = self.backends.len();
        if n == 0 {
            return None;
        }

        for _ in 0..n {
            // 1. Pick the current pointer (use modulo for safety if the backend list size changed)
            let idx = self.next_backend % n;

            // 2. ALWAYS move the pointer forward for the next request, 
            // even if this specific backend is unhealthy.
            self.next_backend = (idx + 1) % n;

            // 3. Check if the backend we picked is healthy
            if self.backends[idx].is_healthy {
                return Some(self.backends[idx].addr);
            }
        }

        None
    }
}

/// A thread-safe, shared pointer to the load balancer's state.
type SharedState = Arc<RwLock<State>>;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing for logging
    tracing_subscriber::fmt::init();

    // Task 1: Initialize the shared state with some dummy backends for now.
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

    // Task 3: Set up the TcpListener loop
    let addr = "127.0.0.1:8080";
    let listener = TcpListener::bind(addr).await?;
    tracing::info!("Listening on {}", addr);

    loop {
        let (stream, client_addr) = listener.accept().await?;
        tracing::info!("Accepted connection from {}", client_addr);

        let state_clone = Arc::clone(&state);
        tokio::spawn(async move {
            if let Err(e) = handle_connection(stream, state_clone).await {
                tracing::error!("Error handling connection from {}: {}", client_addr, e);
            }
        });
    }
}

async fn handle_connection(mut client_stream: TcpStream, state: SharedState) -> anyhow::Result<()> {
    // Select a backend
    let backend_addr = {
        let mut state_guard = state.write().await;
        state_guard.next_backend_addr()
    };

    match backend_addr {
        Some(addr) => {
            tracing::info!("Proxying to {}", addr);
            let mut backend_stream = TcpStream::connect(addr).await?;

            // Proxy data bidirectionally between client and backend
            let (from_client, from_backend) =
                tokio::io::copy_bidirectional(&mut client_stream, &mut backend_stream).await?;

            tracing::info!(
                "Connection closed. Sent {} bytes, received {} bytes from {}",
                from_client,
                from_backend,
                addr
            );
        }
        None => {
            tracing::error!("No healthy backends available");
        }
    }

    Ok(())
}
