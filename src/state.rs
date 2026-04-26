use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Represents a backend server that the load balancer will proxy traffic to.
#[derive(Debug, Clone)]
pub struct Backend {
    pub addr: SocketAddr,
    pub is_healthy: bool,
}

/// The global state of the load balancer, containing the list of backends and the RR index.
#[derive(Debug)]
pub struct State {
    pub backends: Vec<Backend>,
    /// The index of the next backend to select (for Round Robin).
    pub next_backend: usize,
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
}

/// A thread-safe, shared pointer to the load balancer's state.
pub type SharedState = Arc<RwLock<State>>;
