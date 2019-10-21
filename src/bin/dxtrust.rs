use std::env;

use dx::trust::{
    TrustStore,
    TrustedIdentity,
};

fn help() {
    println!("usage:
dxtrust list
    List keys currently in trusted peer database.
dxtrust generate <name>
    Generate new keypair for given hostname.");
}

fn list() {
    let store = TrustStore::load();

    for peer in store.ids {
        println!("{}: {}", peer.name, peer.id());
    }
}

fn generate(name: String) {
    let id = TrustedIdentity::new(name, &TrustStore::path());

    println!("{}: {}", id.name, id.id());
}

fn main() {
    let args: Vec<String> = env::args().collect();

    match args.len() {
        1 => help(),
        2 => match args[1].as_str() {
            "list" => list(),
            _ => help(),
        },
        3 => match args[1].as_str() {
            "generate" => generate(args[2].clone()),
            _ => help(),
        }
        _ => help(),
    }
}
