use anyhow::{anyhow, Result};
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use base64::Engine;
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

pub fn random<const N: usize>() -> Result<[u8; N]> {
    let mut bytes = [0u8; N];
    getrandom::fill(&mut bytes).map_err(|error| anyhow!("no randomness: {error}"))?;
    Ok(bytes)
}

pub struct Sealed {
    pub nonce: String,
    pub secret: String,
}

pub fn seal(key: &[u8; 32], name: &str, secret: &[u8]) -> Result<Sealed> {
    let nonce = random::<24>()?;
    let sealed = XChaCha20Poly1305::new(key.into())
        .encrypt(
            XNonce::from_slice(&nonce),
            Payload {
                msg: secret,
                aad: name.as_bytes(),
            },
        )
        .map_err(|_| anyhow!("could not encrypt the secret for {name}"))?;
    Ok(Sealed {
        nonce: STANDARD.encode(nonce),
        secret: STANDARD.encode(sealed),
    })
}

pub fn open(key: &[u8; 32], name: &str, sealed: &Sealed) -> Result<Zeroizing<String>> {
    let nonce = STANDARD.decode(&sealed.nonce)?;
    let bytes = STANDARD.decode(&sealed.secret)?;
    if nonce.len() != 24 {
        return Err(anyhow!("the stored nonce for {name} is not 24 bytes"));
    }
    let plain = Zeroizing::new(
        XChaCha20Poly1305::new(key.into())
            .decrypt(
                XNonce::from_slice(&nonce),
                Payload {
                    msg: &bytes,
                    aad: name.as_bytes(),
                },
            )
            .map_err(|_| anyhow!("the secret for {name} does not open with this master key"))?,
    );
    let text = std::str::from_utf8(&plain)?;
    Ok(Zeroizing::new(text.to_string()))
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub fn hash(token: &str) -> String {
    hex(&Sha256::digest(token.as_bytes()))
}

pub fn token() -> Result<Zeroizing<String>> {
    Ok(Zeroizing::new(format!(
        "vt_{}",
        URL_SAFE_NO_PAD.encode(random::<32>()?)
    )))
}

pub fn id(prefix: &str) -> Result<String> {
    Ok(format!("{prefix}{}", hex(&random::<8>()?)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_secret_opens_only_under_its_own_name_and_key() {
        let key = [7u8; 32];
        let sealed = seal(&key, "stripe", b"sk_live_1").unwrap();
        assert_eq!(open(&key, "stripe", &sealed).unwrap().as_str(), "sk_live_1");
        assert!(open(&key, "other", &sealed).is_err());
        assert!(open(&[8u8; 32], "stripe", &sealed).is_err());
        let again = seal(&key, "stripe", b"sk_live_1").unwrap();
        assert_ne!(again.nonce, sealed.nonce);
        assert!(!sealed.secret.contains("sk_live"));
    }

    #[test]
    fn tokens_are_long_and_stored_by_hash() {
        let made = token().unwrap();
        assert!(made.starts_with("vt_") && made.len() == 46);
        assert_eq!(hash(&made).len(), 64);
        assert_ne!(hash(&made), *made);
    }
}
