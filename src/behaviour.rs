use std::time::{Duration, Instant};
use std::sync::Mutex;

use libp2p::{
    PeerId,
    NetworkBehaviour,
    swarm::NetworkBehaviourEventProcess,
    kad::{
        Kademlia,
        KademliaConfig,
        KademliaEvent,
        record::store::MemoryStore
    },
    mdns::{Mdns, MdnsEvent},
};

use crate::status::{
    Status,
    StatusConfig,
    StatusEvent,
    StatusSuccess,
    Payload,
};


/// Returned events by behavior (unused)
enum Event {
    PeerOffline,
    PeerOnline,
    PeerStatus,
}

/// Internal structure used to track other peers
#[derive(Clone)]
pub struct PeerInfo {
    id: PeerId,
    routing: Option<PeerRouting>,
    status: Option<PeerStatus>,
}

#[derive(Clone)]
struct PeerRouting ( Vec<PeerId>, Instant);

#[derive(Clone)]
struct PeerStatus ( Payload, Instant );

impl PeerInfo {
    pub fn new(id: &PeerId) -> Self {
        PeerInfo {
            id: id.clone(),
            routing: None,
            status: None,
        }
    }
}


// We create a custom network behaviour that combines Kademlia with
// regular status requests.
#[derive(NetworkBehaviour)]
pub struct Behaviour {
    kad: Kademlia<MemoryStore>,
    mdns: Mdns,
    status: Status,

    #[behaviour(ignore)]
    pub peers: Mutex<Vec<PeerInfo>>,
}

impl Behaviour {
    pub fn new(id: PeerId, state: Payload ) -> Self {
        // Config and setup Kademlia
        let mut cfg = KademliaConfig::default();

        let store = MemoryStore::new(id.clone());

        let mut kad = Kademlia::with_config(id.clone(), store, cfg);

        // Trigger bootstrap with a stable bootstrap peer
        kad.add_address(&"QmaCpDMGvV2BGHeYERUEnRQAwe3N8SzbUtfsmvsqQLuvuJ".parse().unwrap(), "/ip4/104.131.131.82/tcp/4001".parse().unwrap());
        kad.bootstrap();

        // Setup mDNS discovery
        let mdns = Mdns::new().unwrap();

        // Configure and setup status protocol
        let status = Status::new(StatusConfig::new( state ).with_keep_alive(true));

        Behaviour { kad, mdns, status, peers: Mutex::new(Vec::new()) }
    }

    /// Add peer id to list of watched peers
    pub fn add_peers(&mut self, id: PeerId) {
        self.peers.lock().unwrap().push(PeerInfo::new(&id));

        self.kad.get_closest_peers(id.clone());
    }

    /// Retrieve current peer status by id
    pub fn get_peer_info(&self, id: &PeerId) -> Option<PeerInfo> {
        for peer in self.peers.lock().unwrap().iter() {
            if &peer.id == id {
                return Some(peer.clone())
            }
        }
        None
    }
}

impl NetworkBehaviourEventProcess<KademliaEvent> for Behaviour {
    fn inject_event(&mut self, event: KademliaEvent) {
        match event {
            KademliaEvent::BootstrapResult(result) => {
                match result {
                    Ok(..) => println!("Bootstrap successful!"),
                    Err(error) => println!("Bootstrap failed: {:#?}", error),
                }
            },
            KademliaEvent::GetClosestPeersResult(result) => {
                match result {
                    Ok(closest) => {
                        if let Ok(id) = PeerId::from_bytes(closest.key) {

                            if closest.peers.is_empty() {
                                self.kad.get_closest_peers(id.clone());
                            } else {
                                if let Some(info) = self.get_peer_info(&id) {
                                    //info.routing = Some(PeerRouting(closest.peers, Instant::now()));
                                    println!("Updated Kademlia Peers of {:#?}: {:#?}", id, closest.peers);
                                } else {
                                    println!("Unknown Peer {:#?}: {:#?}", id, closest.peers);
                                }
                            }
                        }
                    },
                    Err(error) => {
                        println!("Failed to look up peer: {:#?}", error);
                    },
                }
            },
            _ => (),
        }
    }
}

impl NetworkBehaviourEventProcess<MdnsEvent> for Behaviour {
    fn inject_event(&mut self, event: MdnsEvent) {
        match event {
            MdnsEvent::Discovered(list) => {
                for (peer, addr) in list {
                    // Add discovered nodes to kademlia
                    self.kad.add_address(&peer, addr.clone());

                    println!("Discovered {:#?} via {:#?}", peer, addr);
                }
            },
            MdnsEvent::Expired(list) => {
                for (peer, addr) in list {
                    println!("Expired {:#?} via {:#?}", peer, addr);
                }
            }
        }
    }
}

impl NetworkBehaviourEventProcess<StatusEvent> for Behaviour {
    fn inject_event(&mut self, event: StatusEvent) {
        if let Ok(StatusSuccess::Received(status)) = event.result {
            println!("Received status '{:#?}' from {:?}", status, event.peer);
        }
    }
}
