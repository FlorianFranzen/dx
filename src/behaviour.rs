use std::time::{Duration, Instant};

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
};

use crate::status::{
    Status,
    StatusConfig,
    StatusEvent,
    StatusSuccess,
    Payload,
};


enum Event {
    PeerOffline,
    PeerOnline,
    PeerStatus,
}


struct PeerInfo {
    id: PeerId,
    routing: Option<PeerRouting>,
    status: Option<PeerStatus>,
}
struct PeerRouting ( Vec<PeerId>, Instant);
struct PeerStatus ( Payload, Instant );

// We create a custom network behaviour that combines Kademlia with
// regular status requests.
#[derive(NetworkBehaviour)]
pub struct Behaviour {
    discovery: Kademlia<MemoryStore>,
    status: Status,

    #[behaviour(ignore)]
    peers: Vec<PeerInfo>,
}

impl Behaviour {
    pub fn new(id: PeerId, state: Payload ) -> Self {
        // Config and setup Kademlia
        let mut cfg = KademliaConfig::default();

        let store = MemoryStore::new(id.clone());

        let mut discovery = Kademlia::with_config(id.clone(), store, cfg);

        // Trigger bootstrap with a stable bootstrap peer
        discovery.add_address(&"QmaCpDMGvV2BGHeYERUEnRQAwe3N8SzbUtfsmvsqQLuvuJ".parse().unwrap(), "/ip4/104.131.131.82/tcp/4001".parse().unwrap());
        discovery.bootstrap();

        // Configure and setup status protocol
        let status = Status::new(StatusConfig::new( state ).with_keep_alive(true));

        Behaviour { discovery, status, peers: Vec::new() }
    }

    pub fn add_peers(&mut self, id: PeerId) {
        self.peers.push(PeerInfo{
            id: id.clone(),
            routing: None,
            status: None,
        });

        self.discovery.get_closest_peers(id.clone());
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
                                self.discovery.get_closest_peers(id.clone());
                            } else {
                                println!("Key: {:#?} => Peers: {:#?}", id, closest.peers);
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

impl NetworkBehaviourEventProcess<StatusEvent> for Behaviour {
    fn inject_event(&mut self, event: StatusEvent) {
        if let Ok(StatusSuccess::Received(status)) = event.result {
            println!("Received status '{:#?}' from {:?}", status, event.peer);
        }
    }
}
