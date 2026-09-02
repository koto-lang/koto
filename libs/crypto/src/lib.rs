//! A cryptography module for the Koto language
//!
//! The `crypto` module provides hashing, keyed hashing (HMAC), hex encoding,
//! authenticated symmetric encryption (ChaCha20-Poly1305), and Ed25519 signatures.
//!
//! ## Binary data
//!
//! Koto strings are UTF-8, so arbitrary binary data is represented in this module as
//! hex-encoded strings. Keys, nonces, digests, ciphertexts, and signatures are all
//! represented as hex strings.
//!
//! For example, a 32-byte key is a 64-character hex string, which can be generated
//! with [`crypto.random_bytes`](#random_bytes).

use koto_runtime::{Result, derive::*, prelude::*};

pub fn make_module() -> KMap {
    let result = KMap::with_type("crypto");

    result.add_fn("blake2b", blake2b);
    result.add_fn("blake2s", blake2s);
    result.add_fn("blake3", blake3);
    result.add_fn("sha256", sha256);
    result.add_fn("sha512", sha512);
    result.add_fn("sha1", sha1);
    result.add_fn("md5", md5);

    result.add_fn("hmac", hmac);

    result.add_fn("hex_encode", hex_encode);
    result.add_fn("hex_decode", hex_decode);

    result.add_fn("random_bytes", random_bytes);

    #[cfg(feature = "encryption")]
    {
        result.add_fn("encrypt", encrypt);
        result.add_fn("decrypt", decrypt);
    }

    #[cfg(feature = "signing")]
    {
        result.add_fn("keypair", keypair);
        result.add_fn("sign", sign);
        result.add_fn("verify", verify);
    }

    result
}

fn hash_hex<D: digest::Digest + Default>(data: &[u8]) -> String {
    let mut hasher = D::default();
    hasher.update(data);
    hex::encode(hasher.finalize())
}

fn hmac_hex<M: hmac::Mac + hmac::KeyInit>(key: &[u8], message: &[u8]) -> Result<String> {
    let mut mac = match M::new_from_slice(key) {
        Ok(mac) => mac,
        Err(e) => return runtime_error!("crypto.hmac: {e}"),
    };
    mac.update(message);
    Ok(hex::encode(mac.finalize().into_bytes()))
}

#[cfg(any(feature = "encryption", feature = "signing"))]
fn decode_key<const N: usize>(key: &str, context: &str) -> Result<[u8; N]> {
    let bytes = match hex::decode(key) {
        Ok(bytes) => bytes,
        Err(e) => return runtime_error!("crypto.{context}: invalid hex input: {e}"),
    };
    let len = bytes.len();
    match bytes.try_into() {
        Ok(array) => Ok(array),
        Err(_) => runtime_error!("crypto.{context}: expected {N} bytes, found {len}"),
    }
}

koto_fn! {
    runtime = koto_runtime;

    fn blake2b(s: &str) -> String {
        hash_hex::<blake2::Blake2b512>(s.as_bytes())
    }

    fn blake2s(s: &str) -> String {
        hash_hex::<blake2::Blake2s256>(s.as_bytes())
    }

    fn blake3(s: &str) -> String {
        let mut hasher = blake3::Hasher::new();
        hasher.update(s.as_bytes());
        hex::encode(hasher.finalize().as_bytes())
    }

    fn sha256(s: &str) -> String {
        hash_hex::<sha2::Sha256>(s.as_bytes())
    }

    fn sha512(s: &str) -> String {
        hash_hex::<sha2::Sha512>(s.as_bytes())
    }

    fn sha1(s: &str) -> String {
        hash_hex::<sha1::Sha1>(s.as_bytes())
    }

    fn md5(s: &str) -> String {
        hash_hex::<md5::Md5>(s.as_bytes())
    }

    fn hmac(algorithm: &str, key: &str, message: &str) -> Result<String> {
        let key = match hex::decode(key) {
            Ok(key) => key,
            Err(e) => return runtime_error!("crypto.hmac: invalid hex key: {e}"),
        };

        match algorithm {
            "blake2b" => hmac_hex::<hmac::SimpleHmac<blake2::Blake2b512>>(&key, message.as_bytes()),
            "blake2s" => hmac_hex::<hmac::SimpleHmac<blake2::Blake2s256>>(&key, message.as_bytes()),
            "sha256" => hmac_hex::<hmac::SimpleHmac<sha2::Sha256>>(&key, message.as_bytes()),
            "sha512" => hmac_hex::<hmac::SimpleHmac<sha2::Sha512>>(&key, message.as_bytes()),
            "sha1" => hmac_hex::<hmac::SimpleHmac<sha1::Sha1>>(&key, message.as_bytes()),
            "md5" => hmac_hex::<hmac::SimpleHmac<md5::Md5>>(&key, message.as_bytes()),
            other => runtime_error!("crypto.hmac: unsupported algorithm '{other}'"),
        }
    }

    fn hex_encode(s: &str) -> String {
        hex::encode(s.as_bytes())
    }

    fn hex_decode(s: &str) -> Result<KValue> {
        let bytes = match hex::decode(s) {
            Ok(bytes) => bytes,
            Err(e) => return runtime_error!("crypto.hex_decode: {e}"),
        };
        let list = KList::with_data(bytes.iter().map(|b| KValue::from(*b)).collect());
        Ok(list.into())
    }

    fn random_bytes(n: KNumber) -> Result<String> {
        let count = match n {
            KNumber::I64(i) if i >= 0 => i as usize,
            KNumber::F64(f) if f >= 0.0 && f.fract() == 0.0 && f < usize::MAX as f64 => f as usize,
            _ => return runtime_error!("crypto.random_bytes: expected a non-negative integer, found {n}"),
        };

        let mut bytes = vec![0u8; count];
        if let Err(e) = getrandom::fill(&mut bytes) {
            return runtime_error!("crypto.random_bytes: {e}");
        }
        Ok(hex::encode(bytes))
    }
}

#[cfg(feature = "encryption")]
koto_fn! {
    runtime = koto_runtime;

    fn encrypt(key: &str, plaintext: &str) -> Result<String> {
        use chacha20poly1305::{ChaCha20Poly1305, Nonce, aead::{Aead, KeyInit}};

        let key_bytes: [u8; 32] = decode_key(key, "encrypt")?;
        let cipher = match ChaCha20Poly1305::new_from_slice(&key_bytes) {
            Ok(cipher) => cipher,
            Err(e) => return runtime_error!("crypto.encrypt: {e}"),
        };

        let mut nonce = [0u8; 12];
        if let Err(e) = getrandom::fill(&mut nonce) {
            return runtime_error!("crypto.encrypt: {e}");
        }

        let nonce = match Nonce::try_from(&nonce[..]) {
            Ok(nonce) => nonce,
            Err(_) => return runtime_error!("crypto.encrypt: failed to create nonce"),
        };
        // The cipher's error type is deliberately opaque, so there's no detail to report here.
        let ciphertext = match cipher.encrypt(&nonce, plaintext.as_bytes()) {
            Ok(ciphertext) => ciphertext,
            Err(_) => return runtime_error!("crypto.encrypt: encryption failed"),
        };

        // The nonce is prepended to the ciphertext so that it can be recovered
        // when decrypting.
        let mut output = nonce.to_vec();
        output.extend_from_slice(&ciphertext);
        Ok(hex::encode(output))
    }

    fn decrypt(key: &str, ciphertext: &str) -> Result<String> {
        use chacha20poly1305::{ChaCha20Poly1305, Nonce, aead::{Aead, KeyInit}};

        let key_bytes: [u8; 32] = decode_key(key, "decrypt")?;
        let cipher = match ChaCha20Poly1305::new_from_slice(&key_bytes) {
            Ok(cipher) => cipher,
            Err(e) => return runtime_error!("crypto.decrypt: {e}"),
        };

        let data = match hex::decode(ciphertext) {
            Ok(data) => data,
            Err(e) => return runtime_error!("crypto.decrypt: invalid hex input: {e}"),
        };

        if data.len() < 12 {
            return runtime_error!("crypto.decrypt: ciphertext is too short");
        }
        let (nonce, ciphertext_bytes) = data.split_at(12);

        let nonce = match Nonce::try_from(nonce) {
            Ok(nonce) => nonce,
            Err(_) => return runtime_error!("crypto.decrypt: failed to create nonce"),
        };
        // Authentication failure is reported without detail to avoid implying which of the
        // possible causes applies.
        let plaintext = match cipher.decrypt(&nonce, ciphertext_bytes) {
            Ok(plaintext) => plaintext,
            Err(_) => {
                return runtime_error!(
                    "crypto.decrypt: decryption failed, the key is incorrect or the data has been modified"
                );
            }
        };

        match String::from_utf8(plaintext) {
            Ok(plaintext) => Ok(plaintext),
            Err(e) => runtime_error!("crypto.decrypt: {e}"),
        }
    }
}

#[cfg(feature = "signing")]
koto_fn! {
    runtime = koto_runtime;

    fn keypair() -> Result<KMap> {
        let mut seed = [0u8; 32];
        if let Err(e) = getrandom::fill(&mut seed) {
            return runtime_error!("crypto.keypair: {e}");
        }

        let signing_key = ed25519_dalek::SigningKey::from_bytes(&seed);
        let verifying_key = signing_key.verifying_key();

        let result = KMap::with_type("keypair");
        result.insert("secret", hex::encode(signing_key.to_bytes()));
        result.insert("public", hex::encode(verifying_key.to_bytes()));
        Ok(result)
    }

    fn sign(secret_key: &str, message: &str) -> Result<String> {
        use ed25519_dalek::Signer;

        let seed: [u8; 32] = decode_key(secret_key, "sign")?;
        let signing_key = ed25519_dalek::SigningKey::from_bytes(&seed);
        let signature = signing_key.sign(message.as_bytes());
        Ok(hex::encode(signature.to_bytes()))
    }

    fn verify(public_key: &str, message: &str, signature: &str) -> Result<bool> {
        use ed25519_dalek::Verifier;

        let public_key: [u8; 32] = decode_key(public_key, "verify")?;
        let verifying_key = match ed25519_dalek::VerifyingKey::from_bytes(&public_key) {
            Ok(key) => key,
            Err(e) => return runtime_error!("crypto.verify: invalid public key: {e}"),
        };

        let signature_bytes: [u8; 64] = decode_key(signature, "verify")?;
        let signature = ed25519_dalek::Signature::from_bytes(&signature_bytes);

        Ok(verifying_key
            .verify(message.as_bytes(), &signature)
            .is_ok())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blake2b() {
        assert_eq!(
            hash_hex::<blake2::Blake2b512>(b"hello"),
            "e4cfa39a3d37be31c59609e807970799caa68a19bfaa15135f165085e01d41a65ba1e1b146aeb6bd0092b49eac214c103ccfa3a365954bbbe52f74a2b3620c94"
        );
    }

    #[test]
    fn sha256() {
        assert_eq!(
            hash_hex::<sha2::Sha256>(b"hello"),
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
    }

    #[test]
    fn hmac_blake2b() {
        let key = [
            0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d,
            0x0e, 0x0f, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x1b,
            0x1c, 0x1d, 0x1e, 0x1f,
        ];
        assert_eq!(
            hmac_hex::<hmac::SimpleHmac<blake2::Blake2b512>>(&key, b"hello").unwrap(),
            "32519f3c0d076330d5b9acadf44097f9f462bfc42955c4169866f96398d3632d2cea55f1a8207a36a2bf88eb364e8a4eada28f650f8ad2d27a78a25cee5ab531"
        );
    }
}
