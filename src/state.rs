use std::collections::HashMap;
use std::net::{SocketAddr, Ipv4Addr};
use std::sync::{Arc, RwLock};
use std::time::Instant;
use pnet::datalink::MacAddr;

/// Represents a backend server that the load balancer will proxy traffic to.
#[derive(Debug, Clone)]
pub struct Backend {
    pub addr: SocketAddr,
    pub is_healthy: bool,
}

/// 5-tuple for flow tracking
#[derive(Hash, Eq, PartialEq, Debug, Clone, Copy)]
pub struct FlowKey {
    pub src_ip: Ipv4Addr,
    pub dst_ip: Ipv4Addr,
    pub src_port: u16,
    pub dst_port: u16,
    pub protocol: u8,
}

#[derive(Debug, Clone)]
pub struct FlowEntry {
    pub backend_addr: SocketAddr,
    pub last_seen: Instant,
}

#[derive(Debug, Clone)]
pub struct ArpEntry {
    pub mac: MacAddr,
    pub last_seen: Instant,
}

/// The global state of the load balancer.
#[derive(Debug)]
pub struct State {
    pub backends: Vec<Backend>,
    /// The index of the next backend to select (for Round Robin).
    pub next_backend: usize,
    /// Flow Table for sticky sessions (mapping 5-tuple to flow info)
    pub flow_table: HashMap<FlowKey, FlowEntry>,
    /// ARP Table mapping IPv4 addresses to MAC addresses
    pub arp_table: HashMap<Ipv4Addr, ArpEntry>,
}

impl State {
    pub fn new(backends: Vec<Backend>) -> Self {
        Self {
            backends,
            next_backend: 0,
            flow_table: HashMap::new(),
            arp_table: HashMap::new(),
        }
    }

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

    /// Returns the assigned backend for a flow, or assigns a new one via Round Robin.
    pub fn get_or_assign_backend(&mut self, flow: FlowKey) -> Option<SocketAddr> {
        if let Some(entry) = self.flow_table.get_mut(&flow) {
            entry.last_seen = Instant::now();
            return Some(entry.backend_addr);
        }

        if let Some(backend_addr) = self.next_backend_addr() {
            self.flow_table.insert(flow, FlowEntry {
                backend_addr,
                last_seen: Instant::now(),
            });
            tracing::info!("New flow: mapping {:?} to {}", flow, backend_addr);
            return Some(backend_addr);
        }

        None
    }

    /// Update or insert an ARP entry
    pub fn update_arp(&mut self, ip: Ipv4Addr, mac: MacAddr) {
        self.arp_table.insert(ip, ArpEntry {
            mac,
            last_seen: Instant::now(),
        });
    }

    /// Get MAC address for an IP
    pub fn get_mac(&self, ip: Ipv4Addr) -> Option<MacAddr> {
        self.arp_table.get(&ip).map(|entry| entry.mac)
    }

    /// Cleanup stale flows and ARP entries
    pub fn cleanup(&mut self, flow_timeout: std::time::Duration, arp_timeout: std::time::Duration) {
        let now = Instant::now();
        
        // Cleanup flows
        let initial_flows = self.flow_table.len();
        self.flow_table.retain(|_, entry| now.duration_since(entry.last_seen) < flow_timeout);
        let cleaned_flows = initial_flows - self.flow_table.len();
        
        // Cleanup ARP entries
        let initial_arp = self.arp_table.len();
        self.arp_table.retain(|_, entry| now.duration_since(entry.last_seen) < arp_timeout);
        let cleaned_arp = initial_arp - self.arp_table.len();

        if cleaned_flows > 0 || cleaned_arp > 0 {
            tracing::info!("Reaper: cleaned up {} flows and {} ARP entries", cleaned_flows, cleaned_arp);
        }
    }
}

/// A thread-safe, shared pointer to the load balancer's state.
/// Using std::sync::RwLock for synchronous packet loop performance.
pub type SharedState = Arc<RwLock<State>>;
