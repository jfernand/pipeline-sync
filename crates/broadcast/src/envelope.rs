//! The envelope contract shared with the Pipeline app: what an event envelope's hash covers, and
//! the signature that travels beside it (PL-025, design step 1).
//!
//! The app's `envelopeHash` (Kotlin, sync-core `EnvelopeHash.kt`) is the original. Every envelope
//! the app has stored was sealed with it, so this encoding has to match it byte for byte:
//! `deviceId|sequence|timestamp|` then each parent hash followed by `,`, then `|payload`, as UTF-8,
//! hashed with SHA-256 and written as lowercase hex. The parents are hashed in the order given;
//! their order is part of what was sealed. `interop/vectors/envelope-vectors.json` holds the cases
//! both implementations test against.
//!
//! A signature is ed25519, made with the authoring device's key (its iroh key), over the 32 bytes
//! the hash's hex names. It isn't part of the hash, so signing never re-seals an envelope.

use iroh::{PublicKey, SecretKey, Signature};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt::{Display, Write};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventEnvelope {
    pub hash: Hash,
    pub parent_hashes: Vec<Hash>,
    pub device_id: DeviceId,
    pub sequence: u64,
    pub timestamp_epoch_millis: u64,
    pub payload: String,
}

/// A lowercase hex SHA-256, as the app writes it.
#[derive(Eq, Hash, PartialEq, PartialOrd, Ord, Clone, Debug, Serialize, Deserialize)]
pub struct Hash(String);

impl Hash {
    pub fn new(hex: String) -> Self {
        Hash(hex)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The 32 bytes this hash names, or `None` if it isn't 64 lowercase hex digits — which no
    /// sealed envelope's hash can fail to be.
    pub fn to_bytes(&self) -> Option<[u8; 32]> {
        if self.0.len() != 64 || !self.0.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) {
            return None;
        }
        let bytes = data_encoding::HEXLOWER.decode(self.0.as_bytes()).ok()?;
        bytes.try_into().ok()
    }
}

impl Display for Hash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct DeviceId(String);

impl DeviceId {
    pub fn random() -> DeviceId {
        DeviceId(Uuid::new_v4().to_string())
    }

    pub fn new(id: String) -> DeviceId {
        DeviceId(id)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The hash an envelope with these contents is sealed with. See the module docs for the encoding.
pub fn envelope_hash(
    device_id: &DeviceId,
    sequence: u64,
    timestamp_epoch_millis: u64,
    parent_hashes: &[Hash],
    payload: &str,
) -> Hash {
    let mut input = String::new();
    write!(input, "{}|{}|{}|", device_id.0, sequence, timestamp_epoch_millis).unwrap();
    for parent in parent_hashes {
        input.push_str(&parent.0);
        input.push(',');
    }
    input.push('|');
    input.push_str(payload);
    Hash(data_encoding::HEXLOWER.encode(&Sha256::digest(input.as_bytes())))
}

impl EventEnvelope {
    /// An envelope whose hash is computed from its own contents.
    pub fn seal(
        device_id: DeviceId,
        sequence: u64,
        timestamp_epoch_millis: u64,
        parent_hashes: Vec<Hash>,
        payload: String,
    ) -> Self {
        let hash = envelope_hash(&device_id, sequence, timestamp_epoch_millis, &parent_hashes, &payload);
        Self { hash, parent_hashes, device_id, sequence, timestamp_epoch_millis, payload }
    }

    /// Whether this envelope's hash is the one its contents produce. False for one altered after
    /// it was sealed, and for a device id containing `|`, the one field that could otherwise shift
    /// a boundary in the encoding — the same rule as the app's `verify()`.
    pub fn verify(&self) -> bool {
        !self.device_id.0.contains('|')
            && self.hash
                == envelope_hash(
                    &self.device_id,
                    self.sequence,
                    self.timestamp_epoch_millis,
                    &self.parent_hashes,
                    &self.payload,
                )
    }

    /// This envelope's signature by `key`, or `None` if its hash isn't well formed (it was never
    /// sealed).
    pub fn sign(&self, key: &SecretKey) -> Option<Signature> {
        Some(key.sign(&self.hash.to_bytes()?))
    }

    /// Whether `signature` is `key`'s over this envelope's hash. It says nothing about the
    /// contents: check those with [`verify`](Self::verify).
    pub fn verify_signature(&self, signature: &Signature, key: &PublicKey) -> bool {
        match self.hash.to_bytes() {
            Some(bytes) => key.verify(&bytes, signature).is_ok(),
            None => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn golden() -> EventEnvelope {
        EventEnvelope::seal(
            DeviceId::new("device-a".into()),
            3,
            1000,
            vec![Hash::new("abc".into()), Hash::new("def".into())],
            r#"{"type":"x","note":"café"}"#.into(),
        )
    }

    #[test]
    fn matches_the_apps_golden_hash() {
        // The app's EnvelopeHashTest vector, computed there independently with Python's hashlib.
        assert_eq!(
            golden().hash.as_str(),
            "43f813844e8cc8d052c5f03a8c69e5a3903729589bac124f7b8d30709f1995a1"
        );
        assert!(golden().verify());
    }

    #[test]
    fn changing_any_part_breaks_the_hash() {
        let sealed = golden();
        let altered = [
            EventEnvelope { payload: r#"{"type":"x","note":"cafe"}"#.into(), ..sealed.clone() },
            EventEnvelope { parent_hashes: vec![Hash::new("abc".into())], ..sealed.clone() },
            EventEnvelope {
                parent_hashes: vec![Hash::new("def".into()), Hash::new("abc".into())],
                ..sealed.clone()
            },
            EventEnvelope { sequence: 4, ..sealed.clone() },
            EventEnvelope { timestamp_epoch_millis: 1001, ..sealed.clone() },
            EventEnvelope { device_id: DeviceId::new("device-b".into()), ..sealed.clone() },
            EventEnvelope { hash: Hash::new("0".repeat(64)), ..sealed.clone() },
        ];
        for envelope in altered {
            assert!(!envelope.verify(), "{envelope:?}");
        }
    }

    #[test]
    fn a_device_id_that_could_shift_a_boundary_never_verifies() {
        let ambiguous = EventEnvelope::seal(DeviceId::new("device-a|3".into()), 1000, 0, vec![], "{}".into());
        assert!(!ambiguous.verify());
    }

    #[test]
    fn a_signature_verifies_only_with_its_key_and_its_envelope() {
        let key = SecretKey::from_bytes(&[7; 32]);
        let other = SecretKey::from_bytes(&[8; 32]);
        let envelope = EventEnvelope::seal(DeviceId::new("device-a".into()), 0, 0, vec![], "{}".into());
        let signature = envelope.sign(&key).unwrap();

        assert!(envelope.verify_signature(&signature, &key.public()));
        assert!(!envelope.verify_signature(&signature, &other.public()));
        let next = EventEnvelope::seal(DeviceId::new("device-a".into()), 1, 0, vec![envelope.hash.clone()], "{}".into());
        assert!(!next.verify_signature(&signature, &key.public()));
    }

    #[test]
    fn an_unsealed_hash_cannot_be_signed() {
        let key = SecretKey::from_bytes(&[7; 32]);
        let unsealed = EventEnvelope { hash: Hash::new("abc".into()), ..golden() };
        let uppercase = EventEnvelope { hash: Hash::new(golden().hash.as_str().to_uppercase()), ..golden() };

        assert!(unsealed.sign(&key).is_none());
        assert!(uppercase.sign(&key).is_none());
    }
}
