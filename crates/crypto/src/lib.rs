use sha2::{Digest, Sha256};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use rand::rngs::OsRng;

pub type Hash = [u8; 32];

pub fn hash(data:&[u8])-> Hash {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher.finalize().into()
}

pub struct KeyPair {
    pub signing_key: SigningKey,
}
impl KeyPair {
    pub fn generate()->Self{
        Self {
            signing_key:SigningKey::generate(&mut OsRng),
        }
    }
    pub fn public_key(&self)->VerifyingKey{
        self.signing_key.verifying_key()
    }
    pub fn sign(&self , message: &[u8])->Signature{
        self.signing_key.sign(message)
    }
}
pub fn  verify(public_key:&VerifyingKey, message:&[u8], signature:&Signature)->bool{
    public_key.verify(message, signature).is_ok()
}
pub fn address_from_pubkey(public_key:&VerifyingKey)->[u8;20]{
    let h: [u8; 32] = hash(public_key.as_bytes());
    let mut addr: [u8; 20]=[0u8;20];
    addr.copy_from_slice(&h[..20]);
    addr
}

#[cfg(test)]
mod tests {


use super::*;

    #[test]
    fn test_hash_is_deterministic(){
        let h1 = hash(b"hello");
        let h2 = hash(b"hello");
        assert_eq!(h1, h2);
    }
    #[test]
    fn test_different_inputs_give_different_outputs(){
        let h1 = hash(b"hello");
        let h2 = hash(b"world");
        assert_ne!(h1, h2);
    }
    #[test]
    fn test_verify_and_sign(){
        let keypair = KeyPair::generate();
        let message = b"transfer 10 coins to usama";
        let signature = keypair.sign(message);
        assert!(verify(&keypair.public_key(), message, &signature));
    }
    #[test]
    fn test_wrong_message_fails_verify(){
        let keypair = KeyPair::generate();
        let signature = keypair.sign(b"real message");
        assert!(!verify(&keypair.public_key(),b"fake message", &signature));
    }
    #[test]
    fn tes_address_is_20_bytes(){
        let keypair = KeyPair::generate();
        let addr = address_from_pubkey(&keypair.public_key());
        assert_eq!(addr.len(),20);
    }
}