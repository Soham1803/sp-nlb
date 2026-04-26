use crate::state::SharedState;
use pnet::datalink::{self, NetworkInterface};
use pnet::datalink::Channel::Ethernet;
use pnet::packet::ethernet::{EtherTypes, EthernetPacket, MutableEthernetPacket};
use pnet::packet::ipv4::{Ipv4Packet, MutableIpv4Packet, checksum as ipv4_checksum};
use pnet::packet::tcp::{TcpPacket, MutableTcpPacket, ipv4_checksum as tcp_ipv4_checksum};
use pnet::packet::Packet;
use std::net::{IpAddr, SocketAddr};

pub fn run_passthrough_nlb(interface_name: &str, target_port: u16, state: SharedState) -> anyhow::Result<()> {
    let interfaces = datalink::interfaces();
    let interface = interfaces
        .into_iter()
        .find(|iface: &NetworkInterface| iface.name == interface_name)
        .ok_or_else(|| anyhow::anyhow!("Interface not found: {}", interface_name))?;

    // Open a channel to both receive AND send packets
    let (mut tx, mut rx) = match datalink::channel(&interface, Default::default()) {
        Ok(Ethernet(tx, rx)) => (tx, rx),
        _ => return Err(anyhow::anyhow!("Failed to open Ethernet channel")),
    };

    tracing::info!("Passthrough NLB active on {}...", interface.name);

    loop {
        let packet = rx.next()?;
        let eth_packet = EthernetPacket::new(packet).ok_or_else(|| anyhow::anyhow!("Malformed Ethernet packet"))?;

        if eth_packet.get_ethertype() == EtherTypes::Ipv4 {
            if let Some(ipv4_packet) = Ipv4Packet::new(eth_packet.payload()) {
                if let Some(tcp_packet) = TcpPacket::new(ipv4_packet.payload()) {
                    if tcp_packet.get_destination() == target_port {
                        let client_addr = SocketAddr::new(IpAddr::V4(ipv4_packet.get_source()), tcp_packet.get_source());
                        
                        // 1. Get or assign a backend for this connection
                        let backend_addr = {
                            let mut guard = futures::executor::block_on(state.write());
                            guard.get_or_assign_backend(client_addr)
                        };

                        if let Some(backend_addr) = backend_addr {
                            if let IpAddr::V4(backend_ip) = backend_addr.ip() {
                                
                                // 2. Create a mutable copy of the packet to modify
                                let mut new_packet = packet.to_vec();
                                
                                // 3. Modify IPv4 Header
                                {
                                    let mut new_ipv4 = MutableIpv4Packet::new(&mut new_packet[MutableEthernetPacket::minimum_packet_size()..]).unwrap();
                                    new_ipv4.set_destination(backend_ip);
                                    
                                    // Recalculate IP Checksum
                                    let checksum = ipv4_checksum(&new_ipv4.to_immutable());
                                    new_ipv4.set_checksum(checksum);
                                }

                                // 4. Modify TCP Header (Checksum depends on the new IP)
                                {
                                    let ipv4_len = MutableIpv4Packet::new(&mut new_packet[MutableEthernetPacket::minimum_packet_size()..]).unwrap().get_header_length() as usize * 4;
                                    let tcp_start = MutableEthernetPacket::minimum_packet_size() + ipv4_len;
                                    
                                    let mut new_tcp = MutableTcpPacket::new(&mut new_packet[tcp_start..]).unwrap();
                                    new_tcp.set_destination(backend_addr.port());

                                    // Recalculate TCP Checksum using the pseudo-header
                                    let source_ip = ipv4_packet.get_source();
                                    let checksum = tcp_ipv4_checksum(&new_tcp.to_immutable(), &source_ip, &backend_ip);
                                    new_tcp.set_checksum(checksum);
                                }

                                // 5. Inject the modified packet back into the network
                                tx.send_to(&new_packet, None);
                                
                                tracing::info!("Redirected packet to {}", backend_addr);
                            }
                        }
                    }
                }
            }
        }
    }
}
