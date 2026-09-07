//! Spine wire encoding for facts — the payload a FACT entry's
//! `payload_digest` points at.
//!
//! Phase C1 of `.claude/plans/single-spine-substrate-impl.md` owns this
//! encoding: [`FactPayload`] is the postcard-serialized form of a
//! [`FactEntry`] as it rides the entry spine, versioned
//! the same way `x0k-loro`'s `DocOpPayload` is (explicit `version: u8`
//! checked on decode; unknown versions are rejected loudly instead of
//! misread).
//!
//! # Why a separate wire type
//!
//! [`FactEntry`] and [`FactValue`]
//! deliberately carry no serde derives: a field reorder or a serde
//! attribute on the in-memory type would silently change the wire bytes
//! of every fact in the system. The wire schema is this module's
//! [`FactPayload`]/[`FactPayloadValue`] pair — their field and variant
//! order IS the schema, decoupled from refactors of the in-memory types,
//! and the conversion functions are the only bridge.
//!
//! # Canonical value bytes
//!
//! [`canonical_value_bytes`] is the canonical encoding of a value alone
//! (no entity, predicate, or cause). Spine writers hash these bytes to
//! form the per-value path component of a fact's coordinate (see
//! `x0k-folio-daemon/src/spine_sink.rs`): the coordinate identity of
//! a fact is `(entity, predicate, value)` — re-asserting the same value
//! with a different `cause` (e.g. the same edge re-ingested from a
//! changed file) lands at the SAME coordinate and supersedes via LWW
//! rather than accreting a parallel fact.
//!
//! # Cause convention
//!
//! Facts ingested from a file carry the source file's content hash as
//! their cause, in the form [`file_content_cause`] produces:
//! `file-content:<blake3-hex>`. The namespace word matches
//! `x0k_types::provenance::RevisionLocator::FileContent` — the same
//! "blake3 of the bytes of a file on disk" meaning, rendered as a string
//! because `FactEntry::cause` is a producer-defined string slot.

use serde::{Deserialize, Serialize};

use crate::{FactEntry, FactValue};

/// Version tag carried inside every [`FactPayload`]. Bump on any change
/// to the payload schema. Version 2 (single-spine C3) appended the
/// [`FactPayloadValue::Retracted`] tombstone variant.
pub const FACT_PAYLOAD_VERSION: u8 = 2;

/// Oldest payload version this reader still decodes. Because the schema
/// is append-only (variants and fields are only ever added at the end),
/// every version in `MIN..=CURRENT` decodes correctly under the current
/// enum; the version check is a **ceiling** guard against bytes written
/// by a newer writer whose variants this reader cannot know.
pub const FACT_PAYLOAD_MIN_VERSION: u8 = 1;

/// Prefix of a file-content cause string (see [`file_content_cause`]).
pub const FILE_CONTENT_CAUSE_PREFIX: &str = "file-content:";

/// Prefix of a file-deleted cause string (see [`file_deleted_cause`]).
pub const FILE_DELETED_CAUSE_PREFIX: &str = "file-deleted:";

/// Errors from decoding a [`FactPayload`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FactPayloadError {
    /// The bytes did not decode as a `FactPayload` at all.
    Decode(String),
    /// The bytes decoded but declared an unknown schema version.
    UnknownVersion(u8),
}

impl core::fmt::Display for FactPayloadError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            FactPayloadError::Decode(e) => write!(f, "fact payload decode failed: {e}"),
            FactPayloadError::UnknownVersion(v) => write!(
                f,
                "unknown FactPayload version {v} (expected {FACT_PAYLOAD_VERSION})"
            ),
        }
    }
}

impl std::error::Error for FactPayloadError {}

/// Wire form of a [`FactValue`]. Variant order is the wire schema —
/// append-only; never reorder.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FactPayloadValue {
    Text(String),
    EntityRef(String),
    Boolean(bool),
    UnsignedInt(u128),
    SignedInt(i128),
    Float(f64),
    Bytes(Vec<u8>),
    Record(Vec<u8>),
    Symbol(String),
    /// Tombstone marker (single-spine C3, payload version 2): the wire
    /// form of [`FactValue::Retracted`]. Carries the retracted value so
    /// the tombstone is self-describing and first-class in fact history
    /// (typed, never value-mangled).
    Retracted(Box<FactPayloadValue>),
}

impl From<&FactValue> for FactPayloadValue {
    fn from(value: &FactValue) -> Self {
        match value {
            FactValue::Text(s) => FactPayloadValue::Text(s.clone()),
            FactValue::EntityRef(s) => FactPayloadValue::EntityRef(s.clone()),
            FactValue::Boolean(b) => FactPayloadValue::Boolean(*b),
            FactValue::UnsignedInt(v) => FactPayloadValue::UnsignedInt(*v),
            FactValue::SignedInt(v) => FactPayloadValue::SignedInt(*v),
            FactValue::Float(v) => FactPayloadValue::Float(*v),
            FactValue::Bytes(b) => FactPayloadValue::Bytes(b.clone()),
            FactValue::Record(b) => FactPayloadValue::Record(b.clone()),
            FactValue::Symbol(s) => FactPayloadValue::Symbol(s.clone()),
            FactValue::Retracted(inner) => {
                FactPayloadValue::Retracted(Box::new(Self::from(inner.as_ref())))
            }
        }
    }
}

impl From<FactPayloadValue> for FactValue {
    fn from(value: FactPayloadValue) -> Self {
        match value {
            FactPayloadValue::Text(s) => FactValue::Text(s),
            FactPayloadValue::EntityRef(s) => FactValue::EntityRef(s),
            FactPayloadValue::Boolean(b) => FactValue::Boolean(b),
            FactPayloadValue::UnsignedInt(v) => FactValue::UnsignedInt(v),
            FactPayloadValue::SignedInt(v) => FactValue::SignedInt(v),
            FactPayloadValue::Float(v) => FactValue::Float(v),
            FactPayloadValue::Bytes(b) => FactValue::Bytes(b),
            FactPayloadValue::Record(b) => FactValue::Record(b),
            FactPayloadValue::Symbol(s) => FactValue::Symbol(s),
            FactPayloadValue::Retracted(inner) => {
                FactValue::Retracted(Box::new(Self::from(*inner)))
            }
        }
    }
}

/// Wire payload of one FACT entry on the spine.
///
/// Field order is the wire schema — append-only; never reorder. The
/// entity and predicate are carried redundantly with the entry's path
/// components so the payload is self-describing: a fold holding only
/// payload bytes (e.g. after blob sync) reconstructs the full
/// [`FactEntry`] without re-parsing path components.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FactPayload {
    /// Schema version — [`FACT_PAYLOAD_VERSION`].
    pub version: u8,
    /// Subject entity URI.
    pub entity: String,
    /// Predicate, in its stored spelling.
    pub predicate: String,
    /// Typed value.
    pub value: FactPayloadValue,
    /// Provenance/cause reference (see [`file_content_cause`]).
    pub cause: Option<String>,
}

impl FactPayload {
    /// Build the wire payload for a fact.
    pub fn from_fact(fact: &FactEntry) -> Self {
        Self {
            version: FACT_PAYLOAD_VERSION,
            entity: fact.entity.clone(),
            predicate: fact.predicate.clone(),
            value: FactPayloadValue::from(&fact.value),
            cause: fact.cause.clone(),
        }
    }

    /// Recover the in-memory fact.
    pub fn into_fact(self) -> FactEntry {
        FactEntry {
            entity: self.entity,
            predicate: self.predicate,
            value: self.value.into(),
            cause: self.cause,
        }
    }

    /// Encode to bytes via postcard.
    pub fn to_bytes(&self) -> Vec<u8> {
        postcard::to_allocvec(self).expect("FactPayload round-trips via postcard derive")
    }

    /// Decode from bytes, rejecting unknown schema versions. Versions
    /// in [`FACT_PAYLOAD_MIN_VERSION`]`..=`[`FACT_PAYLOAD_VERSION`] are
    /// accepted: the schema is append-only, so older payloads decode
    /// correctly under the current enum; newer ones are rejected loudly.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, FactPayloadError> {
        let payload: FactPayload =
            postcard::from_bytes(bytes).map_err(|e| FactPayloadError::Decode(e.to_string()))?;
        if !(FACT_PAYLOAD_MIN_VERSION..=FACT_PAYLOAD_VERSION).contains(&payload.version) {
            return Err(FactPayloadError::UnknownVersion(payload.version));
        }
        Ok(payload)
    }
}

/// Canonical bytes of a value alone — what spine writers hash to form
/// the per-value path component of a fact coordinate (see module docs).
///
/// A **retraction's canonical value bytes are the retracted value's**:
/// the [`FactValue::Retracted`] wrapper is unwrapped (recursively)
/// before encoding. This is the C3 coordinate decision — the tombstone
/// reproduces the digest leaf of the fact it retracts, so it lands at
/// the same `(entity, predicate, value)` coordinate and the same
/// relation-graph cell, where the dominance rule pits the two directly.
/// The wire payload still carries the `Retracted` wrapper (the
/// tombstone stays typed and first-class); only the coordinate/cell
/// identity unwraps.
pub fn canonical_value_bytes(value: &FactValue) -> Vec<u8> {
    let mut value = value;
    while let FactValue::Retracted(inner) = value {
        value = inner;
    }
    postcard::to_allocvec(&FactPayloadValue::from(value))
        .expect("FactPayloadValue round-trips via postcard derive")
}

/// Render a file-content cause string from a blake3 hex digest of the
/// source file's bytes: `file-content:<blake3-hex>` (lowercase hex).
pub fn file_content_cause(blake3_hex: &str) -> String {
    format!("{FILE_CONTENT_CAUSE_PREFIX}{}", blake3_hex.to_lowercase())
}

/// Parse a cause string produced by [`file_content_cause`], returning
/// the blake3 hex digest if the cause is a file-content cause.
pub fn parse_file_content_cause(cause: &str) -> Option<&str> {
    cause.strip_prefix(FILE_CONTENT_CAUSE_PREFIX)
}

/// Render a file-deleted cause string: `file-deleted:<blake3-hex>` of
/// the **last seen content** of the deleted file. Carried by the
/// tombstones a file deletion emits — there is no new file content to
/// cite, so the retraction names the content whose disappearance caused
/// it.
pub fn file_deleted_cause(blake3_hex: &str) -> String {
    format!("{FILE_DELETED_CAUSE_PREFIX}{}", blake3_hex.to_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payload_roundtrips_every_value_kind() {
        let values = [
            FactValue::Text("hello".into()),
            FactValue::EntityRef("x0k:seed/x".into()),
            FactValue::Boolean(true),
            FactValue::UnsignedInt(u128::MAX),
            FactValue::SignedInt(-42),
            FactValue::Float(2.5),
            FactValue::Bytes(vec![0, 1, 2]),
            FactValue::Record(vec![9, 9]),
            FactValue::Symbol("x0k:folio/status".into()),
        ];
        for value in values {
            let fact = FactEntry {
                entity: "x0k:design/example".into(),
                predicate: "motivatedBy".into(),
                value: value.clone(),
                cause: Some(file_content_cause("ABCDEF")),
            };
            let payload = FactPayload::from_fact(&fact);
            let bytes = payload.to_bytes();
            let back = FactPayload::from_bytes(&bytes)
                .expect("decodes")
                .into_fact();
            assert_eq!(back, fact);
        }
    }

    #[test]
    fn unknown_version_is_rejected() {
        let mut payload = FactPayload::from_fact(&FactEntry::new(
            "x0k:design/x",
            "p",
            FactValue::Text("v".into()),
        ));
        payload.version = 99;
        let bytes = postcard::to_allocvec(&payload).unwrap();
        assert_eq!(
            FactPayload::from_bytes(&bytes),
            Err(FactPayloadError::UnknownVersion(99))
        );
    }

    #[test]
    fn canonical_value_bytes_distinguish_type_not_cause() {
        // Same string under Text vs EntityRef must hash differently…
        assert_ne!(
            canonical_value_bytes(&FactValue::Text("x0k:x".into())),
            canonical_value_bytes(&FactValue::EntityRef("x0k:x".into())),
        );
        // …and the bytes depend on the value alone (no cause/entity).
        assert_eq!(
            canonical_value_bytes(&FactValue::Text("a".into())),
            canonical_value_bytes(&FactValue::Text("a".into())),
        );
    }

    #[test]
    fn retraction_payload_roundtrips_typed() {
        let fact = FactEntry::new(
            "x0k:design/x",
            "motivatedBy",
            FactValue::EntityRef("x0k:commitment/a".into()),
        )
        .to_retraction(Some(file_deleted_cause("AA")));
        let bytes = FactPayload::from_fact(&fact).to_bytes();
        let back = FactPayload::from_bytes(&bytes)
            .expect("decodes")
            .into_fact();
        assert_eq!(back, fact);
        assert!(back.is_retraction());
        assert_eq!(
            back.value.retracted_value(),
            Some(&FactValue::EntityRef("x0k:commitment/a".into()))
        );
    }

    #[test]
    fn retraction_canonical_bytes_match_retracted_value() {
        // The C3 coordinate decision: a tombstone's canonical value
        // bytes ARE the retracted value's, so it reproduces the digest
        // leaf of the fact it retracts.
        let value = FactValue::EntityRef("x0k:commitment/a".into());
        let tomb = FactValue::Retracted(Box::new(value.clone()));
        assert_eq!(canonical_value_bytes(&tomb), canonical_value_bytes(&value));
        // Idempotent under nesting.
        let nested = FactValue::Retracted(Box::new(tomb));
        assert_eq!(
            canonical_value_bytes(&nested),
            canonical_value_bytes(&value)
        );
    }

    #[test]
    fn older_payload_version_still_decodes() {
        let mut payload = FactPayload::from_fact(&FactEntry::new(
            "x0k:design/x",
            "p",
            FactValue::Text("v".into()),
        ));
        payload.version = FACT_PAYLOAD_MIN_VERSION;
        let bytes = postcard::to_allocvec(&payload).unwrap();
        assert!(FactPayload::from_bytes(&bytes).is_ok());
    }

    #[test]
    fn file_content_cause_roundtrips() {
        let cause = file_content_cause("DEADBEEF");
        assert_eq!(cause, "file-content:deadbeef");
        assert_eq!(parse_file_content_cause(&cause), Some("deadbeef"));
        assert_eq!(parse_file_content_cause("entity:foo"), None);
    }
}
