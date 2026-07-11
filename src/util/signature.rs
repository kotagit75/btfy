use bitcode::{Decode, Encode};
use rsa::pkcs1v15::{Signature, VerifyingKey};
use rsa::sha2::Sha256;
use rsa::signature::{SignatureEncoding, Verifier};
use rsa::{pkcs1v15::SigningKey, signature::Signer};
use serde::{Deserialize, Serialize};

use crate::util::key::{PK, SK};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default, Encode, Decode)]
pub struct SignatureWrapper(Vec<u8>);

pub fn sign(data: &[u8], sk: SK) -> SignatureWrapper {
    let signing_key = SigningKey::<Sha256>::new(sk.key());
    let signature = signing_key.sign(data);
    SignatureWrapper(signature.to_vec())
}

pub fn verify(data: &[u8], pk: PK, signature_wrapper: &SignatureWrapper) -> bool {
    let signature_bytes: &[u8] = &signature_wrapper.0;
    let signature = Signature::try_from(signature_bytes).unwrap();
    let verifying_key = VerifyingKey::<Sha256>::new(pk.key());
    verifying_key.verify(data, &signature).is_ok()
}
