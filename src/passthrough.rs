use crate::state::{SharedState, FlowKey};
use crate::arp::{run_arp_handler, send_arp_request};
use pnet::datalink::{self, NetworkInterface};
use pnet::datalink::Channel::Ethernet;
use pnet::packet::ethernet::{EtherTypes, EthernetPacket, MutableEthernetPacket};
use pnet::packet::ipv4::{Ipv4Packet, MutableIpv4Packet, checksum as ipv4_checksum};
use pnet::packet::tcp::{TcpPacket, MutableTcpPacket, ipv4_checksum as tcp_ipv4_checksum};
use pnet::packet::Packet;
use std::net::IpAddr;
use std::sync::Arc;

pub fn run_passthrough_nlb(interface_name: &str, target_port: u16, state: SharedState) -> anyhow::Result<()> {
    let interfaces = datalink::interfaces();
    let interface = interfaces
        .into_iter()
        .find(|iface: &NetworkInterface| iface.name == interface_name)
        .ok_or_else(|| anyhow::anyhow!("Interface not found: {}", interface_name))?;

    let source_mac = interface.mac.ok_or_else(|| anyhow::anyhow!("Interface has no MAC address"))?;

    // Open a channel to both receive AND send packets
    let (mut tx, mut rx) = match datalink::channel(&interface, Default::default()) {
        Ok(Ethernet(tx, rx)) => (tx, rx),
        _ => return Err(anyhow::anyhow!("Failed to open Ethernet channel")),
    };

    tracing::info!("Passthrough NLB active on {} (MAC: {})", interface.name, source_mac);

    // Spawn ARP listener/resolver task
    let state_clone = Arc::clone(&state);
    let interface_clone = interface.clone();

    std::thread::spawn(move || {
        if let Err(e) = run_arp_handler(interface_clone, state_clone) {
            tracing::error!("ARP handler error: {}", e);
        }
    });

    loop {
        let packet = match rx.next() {
            Ok(p) => p,
            Err(e) => {
                tracing::error!("Failed to receive packet: {}", e);
                continue;
            }
        };

        let eth_packet = match EthernetPacket::new(packet) {
            Some(p) => p,
            None => continue,
        };

        if eth_packet.get_ethertype() == EtherTypes::Ipv4 {

            if let Some(ipv4_packet) = Ipv4Packet::new(eth_packet.payload()) {
                if let Some(tcp_packet) = TcpPacket::new(ipv4_packet.payload()) {
                    if tcp_packet.get_destination() == target_port {
                        let flow = FlowKey {
                            src_ip: ipv4_packet.get_source(),
                            dst_ip: ipv4_packet.get_destination(),
                            src_port: tcp_packet.get_source(),
                            dst_port: tcp_packet.get_destination(),
                            protocol: ipv4_packet.get_next_level_protocol().0,
                        };
                        
                        // 1. Get or assign a backend for this flow
                        let backend_addr = {
                            let mut guard = state.write().expect("Failed to acquire write lock");
                            guard.get_or_assign_backend(flow)
                        };

                        if let Some(backend_addr) = backend_addr {
                            if let IpAddr::V4(backend_ip) = backend_addr.ip() {
                                
                                // 2. Check if we have the destination MAC in ARP table
                                let dest_mac = {
                                    let guard = state.read().expect("Failed to acquire read lock");
                                    guard.get_mac(backend_ip)
                                };

                                if let Some(target_mac) = dest_mac {
                                    // 3. Create a mutable copy and rewrite headers
                                    let mut new_packet = packet.to_vec();
                                    
                                    // Rewrite Ethernet Header
                                    if let Some(mut new_eth) = MutableEthernetPacket::new(&mut new_packet) {
                                        new_eth.set_source(source_mac);
                                        new_eth.set_destination(target_mac);
                                    } else {
                                        tracing::error!("Failed to create mutable Ethernet packet");
                                        continue;
                                    }

                                    // Rewrite IPv4 Header
                                    let ipv4_start = MutableEthernetPacket::minimum_packet_size();
                                    if let Some(mut new_ipv4) = MutableIpv4Packet::new(&mut new_packet[ipv4_start..]) {
                                        new_ipv4.set_destination(backend_ip);
                                        let checksum = ipv4_checksum(&new_ipv4.to_immutable());
                                        new_ipv4.set_checksum(checksum);
                                        
                                        let ipv4_len = new_ipv4.get_header_length() as usize * 4;
                                        let tcp_start = ipv4_start + ipv4_len;

                                        // Rewrite TCP Header
                                        if let Some(mut new_tcp) = MutableTcpPacket::new(&mut new_packet[tcp_start..]) {
                                            new_tcp.set_destination(backend_addr.port());
                                            let checksum = tcp_ipv4_checksum(&new_tcp.to_immutable(), &ipv4_packet.get_source(), &backend_ip);
                                            new_tcp.set_checksum(checksum);
                                        } else {
                                            tracing::error!("Failed to create mutable TCP packet");
                                            continue;
                                        }
                                    } else {
                                        tracing::error!("Failed to create mutable IPv4 packet");
                                        continue;
                                    }

                                    tx.send_to(&new_packet, None);
                                } else {
                                    tracing::warn!("MAC unknown for {}, dropping packet and triggering ARP lookup", backend_ip);
                                    send_arp_request(&interface, backend_ip);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
