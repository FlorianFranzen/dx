use super::protocol::Payload;

use rand::{distributions, prelude::*};

/// Generate random status payload, use as dummy for now
pub fn generate_payload() -> Payload {
    thread_rng().sample(distributions::Standard)
}
