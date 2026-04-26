use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Represents a backend server that the load balancer will proxy traffic to.
#[derive(Debug, Clone)]
pub struct Backend {
    pub addr: SocketAddr,
    pub is_healthy: bool,
}

/// The global state of the load balancer.
#[derive(Debug)]
pub struct State {
    pub backends: Vec<Backend>,
    /// The index of the next backend to select (for Round Robin).
    pub next_backend: usize,
    /// Maps (Client IP, Client Port) to the selected Backend Address.
    pub connections: HashMap<SocketAddr, SocketAddr>,
}

impl State {
    /// Selects the next healthy backend using a simple Round Robin algorithm.
    pub fn next_backend_addr(&mut self) -> Option<SocketAddr> {
        let n = self.backends.len();
        if n == 0 {
            return None;
        }

        for _ in 0..n {
            let idx = self.next_backend % n;
            self.next_backend = (idx + 1) % n;

            if self.backends[idx].is_healthy {
                return Some(self.backends[idx].addr);
            }
        }
        None
    }

    /// Returns the assigned backend for a client, or assigns a new one via Round Robin.
    pub fn get_or_assign_backend(&mut self, client_addr: SocketAddr) -> Option<SocketAddr> {
        if let Some(&backend_addr) = self.connections.get(&client_addr) {
            return Some(backend_addr);
        }

        if let Some(backend_addr) = self.next_backend_addr() {
            self.connections.insert(client_addr, backend_addr);
            tracing::info!("New connection: mapping {} to {}", client_addr, backend_addr);
            return Some(backend_addr);
        }

        None
    }
}

/// A thread-safe, shared pointer to the load balancer's state.
pub type SharedState = Arc<RwLock<State>>;
