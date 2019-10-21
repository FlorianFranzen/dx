use std::path::{Path, PathBuf};
use std::fs;
use std::ffi::OsStr;

use dirs;

use libp2p::{
    identity::{
        PublicKey,
        Keypair,
        ed25519,
    },
    PeerId,
};


/// Entry in trusted peer database
pub struct TrustedIdentity {
    pub name: String,
    public: PublicKey,
}

impl TrustedIdentity {
    /// Generate a new identity and save it to path
    pub fn new(name: String, path: &Path) -> Self {
        let key = match Keypair::generate_ed25519() {
            Keypair::Ed25519(key) => key,
            _ => panic!("Failed to generate key."),
        };

        let prefix = path.join(&name);
        fs::write(prefix.with_extension("key"), key.encode().to_vec()).unwrap();
        fs::write(prefix.with_extension("pub"), key.public().encode()).unwrap();

        let public = PublicKey::Ed25519(key.public());

        TrustedIdentity { name, public }
    }

    /// Load an excisting identity from .pub file
    pub fn load(file: &Path) -> Self {
        let data = fs::read(file).unwrap();
        let key = ed25519::PublicKey::decode(&data).unwrap();
        let public = PublicKey::Ed25519(key);

        let name = file.file_stem().unwrap()
            .to_owned().into_string().unwrap();

        TrustedIdentity{ name, public }
    }

    /// Compute peer id from identity
    pub fn id(&self) -> PeerId {
        PeerId::from_public_key(self.public.clone())
    }
}


/// Trusted peer database
pub struct TrustStore {
    pub ids: Vec<TrustedIdentity>,
}

impl TrustStore {
    /// Returns default trust store path
    pub fn path() -> PathBuf {
        dirs::home_dir().unwrap().join(".dx/") // FixMe: Only works on Linux
    }

    /// Load trust database from default path
    pub fn load() -> Self {
        fs::create_dir_all(Self::path()).unwrap();
        
        let mut ids: Vec<TrustedIdentity> = Vec::new();
        for entry in fs::read_dir(Self::path()).unwrap() {
            let path = entry.unwrap().path();

            if path.extension().and_then(OsStr::to_str) == Some("pub") {
                ids.push(TrustedIdentity::load(&path));
            }
        }

        TrustStore{ids}
    }

}
