# sp_nlb: High-Performance Network Load Balancer in Rust

A high-performance Layer 4 Network Load Balancer (NLB) built in Rust using `tokio` for asynchronous I/O and `pnet` for low-level packet manipulation. This project is built with production standards in mind, focusing on educational clarity and architectural integrity.

> **Status:** This project is in active development. It has successfully moved from a basic TCP Proxy to a Stateful Layer 3/4 Passthrough NLB.

## 🚀 Current Implementation Features

### 1. Proxy Mode (Layer 4)
*   **TCP Termination**: Fully terminates client connections.
*   **Asynchronous I/O**: Utilizes `tokio::io::copy_bidirectional` for high-throughput data transfer.
*   **Concurrency**: Spawns lightweight green threads for every connection.

### 2. Passthrough Mode (Layer 3/4) - *Experimental*
*   **Raw Sockets**: Uses `pnet` to intercept Ethernet frames directly from the data link layer.
*   **Packet Mutation**: Manually rewrites IPv4 and TCP headers (DNAT).
*   **Checksum Calculation**: Custom implementation for IPv4 and TCP pseudo-header checksums.
*   **Bypassing Kernel**: Injects modified packets directly back into the network interface.

### 3. Core Engine
*   **Stateful Connection Tracking**: Uses a synchronized `HashMap` to ensure "sticky sessions," mapping clients to consistent backends.
*   **Round Robin Selection**: A health-aware selection algorithm for distributing load.
*   **Async Health Checker**: A background task that proactively monitors backend health with configurable timeouts.
*   **Modular Architecture**: Cleanly separated modules for `state`, `proxy`, `health`, and `passthrough`.

## 🛠 Project Structure

```text
src/
├── main.rs          # Entry point and orchestration
├── state.rs         # Shared state and selection logic
├── proxy.rs         # Layer 4 proxy implementation
├── health.rs        # Background health monitoring
└── passthrough.rs   # Raw packet manipulation (DNAT)
```

## 🚦 Getting Started

### Prerequisites
*   Rust (Edition 2024)
*   `libpcap` development headers (for `pnet`)
*   Root/Sudo privileges (for Passthrough mode)

### Running the Project
```bash
# To run in Proxy Mode (default loopback)
cargo run

# To run in Passthrough Mode (requires root for raw sockets)
sudo ./target/debug/sp_nlb
```

## 🗺 Roadmap to Production

This project is evolving from a single-host simulation to a distributed network system. Upcoming phases include:

- [ ] **Linux Network Namespaces**: Moving beyond `lo` to test across isolated virtual networks.
- [ ] **Full NAT (Return Path)**: Implementing SNAT to handle backend-to-client traffic.
- [ ] **DSR (Direct Server Return)**: Implementing Layer 2 forwarding for extreme performance.
- [ ] **CLI Configuration**: Moving from hardcoded backends to dynamic configuration (YAML/CLI).
- [ ] **eBPF Integration**: Moving packet filtering into the kernel for XDP-powered speeds.

---
*Built for learning, aimed for performance.*
