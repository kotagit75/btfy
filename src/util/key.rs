use rsa::{
    RsaPrivateKey, RsaPublicKey,
    pkcs1::{DecodeRsaPrivateKey, DecodeRsaPublicKey, EncodeRsaPrivateKey, EncodeRsaPublicKey},
    rand_core,
};
use serde::{Deserialize, Serialize};

use crate::util::signature::{SignatureWrapper, sign, verify};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct PK {
    pub der: String,
}
impl std::fmt::Display for PK {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.der)
    }
}
impl PK {
    pub fn new(pk: RsaPublicKey) -> Self {
        PK {
            der: hex::encode(pk.to_pkcs1_der().unwrap()),
        }
    }
    pub fn key(&self) -> RsaPublicKey {
        RsaPublicKey::from_pkcs1_der(&hex::decode(self.der.clone()).unwrap()).unwrap()
    }
    pub fn verify(&self, data: &[u8], signature: &SignatureWrapper) -> bool {
        verify(data, self.clone(), signature)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SK {
    pub der: String,
}
impl std::fmt::Display for SK {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.der)
    }
}
impl SK {
    pub fn new(sk: RsaPrivateKey) -> Self {
        SK {
            der: hex::encode(sk.to_pkcs1_der().unwrap().to_bytes()),
        }
    }
    pub fn key(&self) -> RsaPrivateKey {
        RsaPrivateKey::from_pkcs1_der(&hex::decode(self.der.clone()).unwrap()).unwrap()
    }
    pub fn to_pk(&self) -> PK {
        PK::new(self.key().to_public_key())
    }
    pub fn sign(&self, data: &[u8]) -> SignatureWrapper {
        sign(data, self.clone())
    }
}

pub fn generate_sk(bits: usize) -> SK {
    let mut rng = rand_core::OsRng;
    let priv_key = RsaPrivateKey::new(&mut rng, bits).expect("failed to generate a key");
    SK::new(priv_key)
}
