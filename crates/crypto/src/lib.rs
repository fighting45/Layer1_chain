use sha2::{Digest, Sha256};

pub type Hash = [u8; 32];

pub fn hash(data:&[u8])-> Hash {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher.finalize().into()
}