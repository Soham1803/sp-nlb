# sp_nlb: High-Performance Network Load Balancer in Rust

A high-performance Layer 4 Network Load Balancer (NLB) built in Rust using `tokio` for asynchronous I/O and `pnet` for low-level packet manipulation. This project is built with production standards in mind, focusing on architectural integrity and "real-wire" networking capability.

> **Status:** **Production-Ready L3/L4 Data Plane**. The project has evolved from a basic TCP Proxy to a Stateful Layer 2/3/4 Passthrough NLB with dynamic ARP resolution and session persistence.

## 🚀 Core Features

### 1. Stateful Passthrough Mode (L2/L3/L4)
*   **Raw Sockets & L2 Rewriting**: Uses `pnet` to intercept and rewrite Ethernet frames. It modifies Source/Destination MAC addresses, enabling traffic redirection across physical network segments.
*   **Stateful Connection Tracking**: Implements a **5-tuple Flow Table** (`src/state.rs`) mapping `(SrcIP, DstIP, SrcPort, DstPort, Protocol)` to backends for consistent "sticky sessions."
*   **Dynamic ARP Resolution**: 
    *   **ARP Handler**: Background listener for ARP replies to populate the MAC cache.
    *   **Active Discovery**: Triggers ARP requests for unknown backend IPs to resolve destination MACs on-the-fly.
*   **Non-Blocking Data Plane**: Utilizes `std::sync::RwLock` and $O(1)$ table lookups to ensure the packet loop remains lightning-fast.

### 2. Maintenance & Resilience
*   **Automated Reaper Task**: A background worker that prunes stale TCP flows (5-min TTL) and aged ARP entries (1-hour TTL) to maintain a lean memory footprint.
*   **Async Health Checker**: Proactively monitors backend health with configurable timeouts, ensuring traffic is only routed to healthy nodes.
*   **Robust Error Handling**: Production-grade implementation in `src/passthrough.rs` with zero `unwrap()` calls in the fast path; malformed packets are gracefully logged and skipped.

### 3. Legacy Proxy Mode (Layer 4)
*   **TCP Termination**: Fully terminates client connections and proxies data bidirectionally.
*   **Asynchronous I/O**: High-throughput transfer using `tokio::io::copy_bidirectional`.

## 🛠 Project Structure

```text
src/
├── main.rs          # Entry point and task orchestration
├── state.rs         # Stateful Flow/ARP tables and cleanup logic
├── arp.rs           # ARP packet generation and reply handling
├── passthrough.rs   # Raw packet mutation (DNAT + L2 Rewriting)
├── proxy.rs         # Layer 4 proxy implementation
└── health.rs        # Background health monitoring
```

## 🚦 Getting Started

### Prerequisites
*   Rust (Edition 2024)
*   `libpcap` development headers (for `pnet`)
*   Root/Sudo privileges (required for raw sockets and Passthrough mode)

### Running the Project
```bash
# To run in Passthrough Mode (requires root for raw sockets)
# By default, it listens on interface 'lo' (loopback) port 8080
sudo cargo run
```

## 🗺 Roadmap to Production

- [x] **Stateful Connection Tracking**: Sticky sessions via 5-tuple hashing.
- [x] **L2 MAC Rewriting**: Support for real-network deployments.
- [x] **ARP Resolver**: Dynamic discovery of backend MAC addresses.
- [x] **Entry Reaper**: Automated cleanup of stale sessions and ARP cache.
- [ ] **Config Management**: Dynamic backend configuration via YAML or CLI flags.
- [ ] **Direct Server Return (DSR)**: Optimizing return paths for high-throughput workloads.
- [ ] **eBPF Integration**: Exploring XDP for kernel-level packet filtering.

---
*Built for learning, optimized for performance.*
