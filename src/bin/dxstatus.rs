use dx::behaviour::Behaviour;
use dx::trust::{
    TrustStore,
    TrustedIdentity,
};
use dx::status::generate_payload;

use futures::{prelude::*, future};
use libp2p::{Swarm};

use std::env;

fn help() {
    println!("usage: dxstatus <name>
    Run dx status node for supplied identity.");
}

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() != 2 {
        return help();
    }

    let name = &args[1];

    println!("Starting status node for identity '{}'", name);

    //env_logger::init();

    let store = TrustStore::load();

    // Determine peer id
    let key = store.find(name).expect("Name not in trust store");
    println!("Local peer id: {:?}", key.id());

    // Determine status
    let status = generate_payload();

    // Set up swarm
    let transport = libp2p::build_development_transport(key.key());
    let mut behaviour = Behaviour::new(key.id(), status);

    for other in store.ids.iter() {
        if &other.name != name {
            behaviour.add_peers(other.id())
        }
    }

    let mut swarm = Swarm::new(transport, behaviour, key.id());

    // Tell the swarm to listen on all interfaces and a random, OS-assigned port.
    Swarm::listen_on(&mut swarm, "/ip4/0.0.0.0/tcp/0".parse().unwrap()).unwrap();

    // Use tokio to drive the `Swarm`.
    let mut listening = false;
    tokio::run(future::poll_fn(move || -> Result<_, ()> {
        loop {
            match swarm.poll().expect("Error while polling swarm") {
                Async::Ready(Some(e)) => println!("{:?}", e),
                Async::Ready(None) | Async::NotReady => {
                    if !listening {
                        if let Some(a) = Swarm::listeners(&swarm).next() {
                            println!("Listening on {:?}", a);
                            listening = true;
                        }
                    }
                    return Ok(Async::NotReady)
                }
            }
        }
    }));
}
