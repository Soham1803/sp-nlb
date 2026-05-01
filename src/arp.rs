use crate::state::SharedState;
use pnet::datalink::{self, NetworkInterface, MacAddr, Channel::Ethernet};
use pnet::packet::ethernet::{EtherTypes, EthernetPacket, MutableEthernetPacket};
use pnet::packet::arp::{ArpPacket, ArpOperations, MutableArpPacket, ArpHardwareTypes};
use pnet::packet::Packet;
use std::net::{IpAddr, Ipv4Addr};

pub fn run_arp_handler(interface: NetworkInterface, state: SharedState) -> anyhow::Result<()> {
    let (_tx, mut rx) = match datalink::channel(&interface, Default::default()) {
        Ok(Ethernet(tx, rx)) => (tx, rx),
        _ => return Err(anyhow::anyhow!("Failed to open Ethernet channel for ARP")),
    };

    loop {
        let packet = match rx.next() {
            Ok(p) => p,
            Err(e) => {
                tracing::error!("ARP handler failed to receive packet: {}", e);
                continue;
            }
        };
        
        let eth_packet = match EthernetPacket::new(packet) {
            Some(p) => p,
            None => continue,
        };

        if eth_packet.get_ethertype() == EtherTypes::Arp {
            if let Some(arp_packet) = ArpPacket::new(eth_packet.payload()) {
                if arp_packet.get_operation() == ArpOperations::Reply {
                    let ip = arp_packet.get_sender_proto_addr();
                    let mac = arp_packet.get_sender_hw_addr();
                    
                    let mut guard = state.write().expect("Failed to acquire write lock");
                    guard.update_arp(ip, mac);
                    tracing::info!("ARP Table Updated: {} -> {}", ip, mac);
                }
            }
        }
    }
}

pub fn send_arp_request(interface: &NetworkInterface, target_ip: Ipv4Addr) {
    let source_mac = match interface.mac {
        Some(mac) => mac,
        None => {
            tracing::error!("Cannot send ARP request: Interface has no MAC");
            return;
        }
    };
    
    let source_ip = interface.ips.iter()
        .find(|ip| ip.is_ipv4())
        .and_then(|ip| match ip.ip() { 
            IpAddr::V4(ipv4) => Some(ipv4), 
            _ => None 
        })
        .unwrap_or(Ipv4Addr::new(0, 0, 0, 0));

    let (mut tx, _rx) = match datalink::channel(interface, Default::default()) {
        Ok(Ethernet(tx, rx)) => (tx, rx),
        _ => {
            tracing::error!("Failed to open channel for ARP request");
            return;
        }
    };

    let mut ethernet_buffer = [0u8; 42]; // 14 (Eth) + 28 (ARP)
    
    let mut ethernet_packet = match MutableEthernetPacket::new(&mut ethernet_buffer) {
        Some(p) => p,
        None => return,
    };

    ethernet_packet.set_destination(MacAddr::broadcast());
    ethernet_packet.set_source(source_mac);
    ethernet_packet.set_ethertype(EtherTypes::Arp);

    let mut arp_buffer = [0u8; 28];
    let mut arp_packet = match MutableArpPacket::new(&mut arp_buffer) {
        Some(p) => p,
        None => return,
    };

    arp_packet.set_hardware_type(ArpHardwareTypes::Ethernet);
    arp_packet.set_protocol_type(EtherTypes::Ipv4);
    arp_packet.set_hw_addr_len(6);
    arp_packet.set_proto_addr_len(4);
    arp_packet.set_operation(ArpOperations::Request);
    arp_packet.set_sender_hw_addr(source_mac);
    arp_packet.set_sender_proto_addr(source_ip);
    arp_packet.set_target_hw_addr(MacAddr::zero());
    arp_packet.set_target_proto_addr(target_ip);

    ethernet_packet.set_payload(arp_packet.packet());

    if let Some(Err(e)) = tx.send_to(ethernet_packet.packet(), None) {
        tracing::error!("Failed to send ARP request: {}", e);
    }
}
