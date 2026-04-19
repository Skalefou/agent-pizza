use std::collections::HashMap;
use std::net::{SocketAddr, UdpSocket};
use std::sync::{Arc, RwLock};
use std::time::Duration;

use log::{debug, info, warn};

use crate::node::state::{NodeState, PeerInfo};
use crate::protocol::gossip::{AnnouncePayload, CborAddr, Check, GossipMessage, PeerVersion, TaggedLastSeen};

const GOSSIP_INTERVAL: Duration = Duration::from_secs(2);
const PEER_TIMEOUT: Duration = Duration::from_secs(10);
const UDP_BUF: usize = 65_507;

pub fn start_gossip_service(
    state: Arc<RwLock<NodeState>>,
    socket: UdpSocket,
    bootstrap_peers: Vec<SocketAddr>,
) -> (std::thread::JoinHandle<()>, std::thread::JoinHandle<()>) {
    let socket_send = socket.try_clone().expect("clone UdpSocket");
    let socket_recv = socket;
    let state_send = Arc::clone(&state);
    let state_recv = Arc::clone(&state);

    let send_h = std::thread::spawn(move || send_loop(state_send, socket_send, bootstrap_peers));
    let recv_h = std::thread::spawn(move || recv_loop(state_recv, socket_recv));
    (send_h, recv_h)
}

fn build_announce(state: &NodeState) -> GossipMessage {
    let peer_addrs: Vec<CborAddr> = state.peers.values().map(|p| CborAddr(p.host.clone())).collect();
    GossipMessage::Announce(AnnouncePayload {
        node_addr: CborAddr(state.host.clone()),
        capabilities: state.capabilities.clone(),
        recipes: state.recipe_names(),
        peers: peer_addrs,
        version: PeerVersion {
            counter: state.generation,
            generation: state.node_generation_id,
        },
    })
}

fn build_check(state: &NodeState) -> Check {
    Check {
        last_seen: TaggedLastSeen(HashMap::new()),
        version: PeerVersion {
            counter: state.generation,
            generation: state.node_generation_id,
        },
    }
}

fn send_loop(state: Arc<RwLock<NodeState>>, socket: UdpSocket, bootstrap: Vec<SocketAddr>) {
    loop {
        let (announce, targets) = {
            let s = state.read().unwrap();
            let msg = build_announce(&s);
            let mut addrs = s.peer_hosts();
            for bp in &bootstrap {
                let t = bp.to_string();
                if !addrs.contains(&t) { addrs.push(t); }
            }
            (msg, addrs)
        };

        let mut payload = Vec::new();
        if let Err(e) = ciborium::ser::into_writer(&announce, &mut payload) {
            warn!("gossip: sérialisation échouée: {}", e);
            std::thread::sleep(GOSSIP_INTERVAL);
            continue;
        }

        for target in &targets {
            if let Ok(addr) = target.parse::<SocketAddr>() {
                if let Err(e) = socket.send_to(&payload, addr) {
                    debug!("gossip: send_to {} failed: {}", addr, e);
                }
            }
        }

        state.write().unwrap().evict_stale_peers(PEER_TIMEOUT);
        std::thread::sleep(GOSSIP_INTERVAL);
    }
}

fn recv_loop(state: Arc<RwLock<NodeState>>, socket: UdpSocket) {
    let mut buf = vec![0u8; UDP_BUF];
    socket.set_read_timeout(Some(Duration::from_secs(1))).ok();

    loop {
        let (n, src) = match socket.recv_from(&mut buf) {
            Ok(v) => v,
            Err(ref e)
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut => continue,
            Err(e) => { warn!("gossip: recv_from: {}", e); continue; }
        };

        match ciborium::de::from_reader(&buf[..n]) {
            Ok(msg) => handle_message(&state, msg, src),
            Err(e) => debug!("gossip: décode échoué ({} bytes depuis {}): {}", n, src, e),
        }
    }
}

fn send_udp(socket: &UdpSocket, msg: &GossipMessage, target: SocketAddr) {
    let mut buf = Vec::new();
    if ciborium::ser::into_writer(msg, &mut buf).is_ok() {
        let _ = socket.send_to(&buf, target);
    }
}

fn handle_message(state: &Arc<RwLock<NodeState>>, msg: GossipMessage, src: SocketAddr) {
    match msg {
        GossipMessage::Announce(p) => {
            let my_host = state.read().unwrap().host.clone();
            if p.node_addr.0 == my_host { return; }
            info!("gossip: Announce de {} (caps={:?}, gen={})", p.node_addr.0, p.capabilities, p.version.generation);
            let mut s = state.write().unwrap();
            s.upsert_peer(p.node_addr.0.clone(), PeerInfo {
                host: p.node_addr.0,
                capabilities: p.capabilities,
                recipes: p.recipes,
                generation: p.version.generation,
                last_seen: std::time::Instant::now(),
            });
        }

        GossipMessage::Ping(_check) => {
            debug!("gossip: Ping depuis {}", src);
            let pong = {
                let s = state.read().unwrap();
                GossipMessage::Pong(build_check(&s))
            };
            if let Ok(reply_sock) = UdpSocket::bind("0.0.0.0:0") {
                send_udp(&reply_sock, &pong, src);
            }
        }

        GossipMessage::Pong(check) => {
            debug!("gossip: Pong depuis {}", src);
            let mut s = state.write().unwrap();
            if let Some(peer) = s.peers.get_mut(&src.to_string()) {
                peer.last_seen = std::time::Instant::now();
                peer.generation = check.version.generation;
            }
        }
    }
}
