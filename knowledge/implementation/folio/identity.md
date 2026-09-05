---
x0k:
  format: folio/v1
  id: x0k:implementation/folio/identity
  type: implementation
  status: draft
  summary: The identity half of an x0k URI — class, slug, and the fragment that names a part of what the slug names — parsed and rendered by the format library itself, so a document can say who it is without a substrate underneath it.
  concerns: [folio, identity, uri, publishing, format]
  tangle:
    crate: x0k-folio
    root: src/entity_id.rs
  edges:
    implements:
      - x0k:design/publish-a-region-as-a-repository
    cites:
      - x0k:architecture/publication-projection
      - x0k:implementation/folio/colophon
      - x0k:implementation/folio/checking
---
# Who a document says it is

Every folio/v1 envelope opens by naming itself:

```yaml
id: x0k:design/publish-a-region-as-a-repository
```

and every edge it declares names another entity the same way. That
string is the whole of a document's identity, and until this module
existed the format library could not read it. `Colophon` kept `id` as a
`String` and left the grammar to whoever consumed the envelope — which
in practice meant one consumer, the daemon, promoting the string to
`x0k_types::EntityUri`. So a publication could ship the format and
ship the vocabulary and still not be able to tell a well-formed id from
a typo, because the type that knew the grammar lived in a sixty-module
crate pulling ed25519, tokio, rkyv and postcard, and pulling that in
would have defeated the point of publishing a format library at all.

This module is the smaller half of that type, placed where it means
something. `x0k:architecture/publication-projection` §6 states the rule
it follows: **a type crossing the publication boundary is split at its
meaning; it is not carried whole or refused whole.** `EntityUri` has two
halves. Its identity half is a class and an identifier — `x0k:<class>/<slug>` —
which is folio/v1 syntax. Its `locator` half pins a URI to a content
state, and its variants name Loro frontiers, Automerge heads, Dialog-DB
snapshots, jj commits, parquet manifests: the substrate's storage
vocabulary, which cannot cross and should not. `EntityId` is the first
half alone.

## Two places, one grammar, for now

`x0k_types::EntityUri` still exists and still owns both halves; the
daemon still promotes envelope strings to it. So the grammar is written
down twice, and that is a deliberate, temporary state rather than an
oversight. The resolution named in the ADR is the split itself — the
locator half stays in the substrate, `x0k-types` comes to read this
grammar rather than own it — and it is not this module's to perform,
because `EntityUri`'s locator half has eleven files across six crates
behind it. What this module buys immediately is that the published
format library can validate an id. What it owes is that the duplication
ends when the substrate side is cut.

Two measurements make the split safe to make in this order. No authored
document id in the corpus carries a locator suffix, and the typed
envelope layer — the exact consumer at issue — never reads the field.
The parity that must hold meanwhile is narrow and testable: for any URI
carrying neither a locator nor a fragment, `EntityId` and `EntityUri`
must parse the same string to the same class and identifier, and render
the same string back. Both exceptions are deliberate and both are
stated where they are made — the locator under *Parsing*, the fragment
in the section after this one.

```rust {#module-doc}
//! `EntityId` — the identity half of an x0k entity URI.
//!
//! Grammar:
//!
//! ```text
//! x0k:<class>/<identifier>[#<fragment>]
//! ```
//!
//! This is `x0k_types::EntityUri` without its `locator` half. The
//! locator pins a URI to a content state (a Loro frontier, a jj commit,
//! an object-store hash) and belongs to the substrate that owns those
//! states; the class and identifier are folio/v1 syntax and belong with
//! the format. `x0k:architecture/publication-projection` §6 is the rule:
//! a type crossing the publication boundary is split at its meaning.
//!
//! Forward-compatible: unknown classes parse cleanly. Class semantics —
//! which resolver handles a class, whether its identifier is itself a
//! content hash, what one of its fragments names — live above this
//! module. The parser knows the shape and nothing else.
//!
//! An `@<locator>` suffix is a parse error here rather than an ignored
//! tail, because silently dropping a pin would turn "this exact revision"
//! into "whatever is current" without telling anyone.

use std::fmt;
use std::str::FromStr;
```

## A fragment names a part of what the identifier names

An id can address something smaller than a whole entity, and the corpus
was already doing it in two unrelated places before this module read the
character. A publication severs a crate's feature by naming
`x0k:software-module/x0k-folio#plugins`; a transclusion pulls one
section of a design in by naming
`x0k:design/publish-a-region-as-a-repository#read-an-affordance-out-of-a-document`.
Both consumers split on `#` by hand, and two hand-rolled splits of the
same character are the signal that the grammar owns it.

So the grammar has a third part: `x0k:<class>/<identifier>[#<fragment>]`.
What a fragment *means* is class-specific and stays above this module —
a cargo feature for a software module, a heading-path slug for a
document — which is the same division the class itself already gets.
What the parser owns is that the fragment is a distinct part and not a
tail of the identifier, because absorbing it silently is the defect the
locator arm below exists to refuse: a resolver handed
`publish-a-region-as-a-repository#read-an-affordance-out-of-a-document`
as an identifier looks for a file by that name and finds nothing, and
nothing in the parse told it that a part of the string was addressing a
part of the document.

This costs the renderer one character. `#` joins the encode set, so an
identifier that genuinely contains one carries it as `%23` and the
separator stays unambiguous — the same trade `@` already makes, and the
second place this pair's grammar deliberately parts from `EntityUri`,
which models neither.

## The type

Two owned `String`s and an optional third, all public, because every
consumer wants the parts rather than the whole: a checker compares the
class against a vocabulary, a resolver looks the identifier up on disk,
a projector reads the fragment to cut a section out, a renderer prints
them back.

```rust {#entity-id}
/// A parsed x0k entity id: a class, a class-specific identifier, and an
/// optional fragment naming a part of what the identifier names.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EntityId {
    /// Kebab-case class name. Not validated at parse time so unknown
    /// classes round-trip; class registration happens above this layer.
    pub class: String,
    /// Class-specific stable identifier. Decoded — percent-escapes from
    /// the string form are unescaped here.
    pub identifier: String,
    /// The `#fragment` suffix, decoded, naming a part of the entity: a
    /// cargo feature on a software module, a heading-path slug on a
    /// document. What it selects is class-specific and lives above this
    /// module; that it is a part and not a tail of the identifier is
    /// this module's business. `None` means the whole entity.
    pub fragment: Option<String>,
}

impl EntityId {
    /// Construct directly, addressing the whole entity. Does no class
    /// validation.
    pub fn new(class: impl Into<String>, identifier: impl Into<String>) -> Self {
        Self {
            class: class.into(),
            identifier: identifier.into(),
            fragment: None,
        }
    }

    /// The same id qualified by `fragment` — the shape a projected
    /// section takes when it is named after the document it was cut from.
    pub fn with_fragment(self, fragment: impl Into<String>) -> Self {
        Self {
            fragment: Some(fragment.into()),
            ..self
        }
    }

    /// The whole entity this id names a part of. Identity when there is
    /// no fragment, so a caller can ask unconditionally.
    pub fn without_fragment(&self) -> Self {
        Self {
            class: self.class.clone(),
            identifier: self.identifier.clone(),
            fragment: None,
        }
    }
}
```

## Rendering

Display is the inverse of parsing, with one asymmetry worth naming: a
bare `/` inside an identifier parses (the split takes only the first
one) but re-renders as `%2F`. The corpus relies on the parse — the
literate documents are ided `x0k:implementation/folio/identity`, a
class of `implementation` over an identifier of `folio/identity` — so
tightening the parser would reject a live convention, and tightening
the renderer would break parity with `EntityUri`, which encodes the
same way. The honest statement is that this pair round-trips
*strings that were rendered by it*, and parses a slightly wider
language than it renders. A test pins that shape so it stays a known
edge rather than a surprise.

```rust {#display}
impl fmt::Display for EntityId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("x0k:")?;
        f.write_str(&self.class)?;
        f.write_str("/")?;
        write_encoded(f, &self.identifier)?;
        if let Some(fragment) = &self.fragment {
            f.write_str("#")?;
            write_encoded(f, fragment)?;
        }
        Ok(())
    }
}
```

## Parsing

Four rejections, in the order a malformed string trips them: no scheme,
no separator, an empty half, whitespace. The locator check comes before
the fragment split, and deliberately: it is asked of everything after
the class, so a pin hiding behind a fragment (`x0k:a/b#c@d`) is refused
by the same rule rather than parsed into a fragment nobody can honour.
It is the first of this parser's two deliberate differences from
`EntityUri` — the substrate accepts `@<locator>` and resolves it; the
format library sees a pin it cannot honour and says so. The fragment
split is the second, and it is a *widening*: `EntityUri` reads
`a/b#c` as an identifier of `b#c`, and this parser reads it as a part of
`b`. An empty fragment (`x0k:a/b#`) refuses rather than degrading to
`None`, for the reason the locator refuses: a `#` that was typed meant
something, and the parse that quietly forgets it is the one nobody
debugs.

```rust {#from-str}
impl FromStr for EntityId {
    type Err = EntityIdError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let rest = input
            .strip_prefix("x0k:")
            .ok_or_else(|| EntityIdError::MissingScheme(input.to_string()))?;

        // Split off the class first: a bare `/` inside the identifier is
        // absorbed by the identifier, matching `EntityUri`.
        let (class, raw_identifier) = rest
            .split_once('/')
            .ok_or_else(|| EntityIdError::MissingSeparator(input.to_string()))?;

        if class.is_empty() {
            return Err(EntityIdError::EmptyClass(input.to_string()));
        }

        // A bare `@` is the substrate's locator separator. An identifier
        // that genuinely contains one carries it as `%40`, so anything
        // unencoded here is a pin this layer must refuse rather than drop.
        // Asked before the fragment split so a pin behind a fragment is
        // refused too, rather than riding along inside one.
        if let Some((_, locator)) = rest.split_once('@') {
            return Err(EntityIdError::LocatorSuffix {
                input: input.to_string(),
                locator: locator.to_string(),
            });
        }

        // The fragment names a part of the entity. Splitting it off here
        // is what keeps it from being absorbed into the identifier, where
        // a resolver would look for a document by the whole string.
        let (raw_identifier, raw_fragment) = match raw_identifier.split_once('#') {
            Some((id, frag)) => (id, Some(frag)),
            None => (raw_identifier, None),
        };

        if raw_identifier.is_empty() {
            return Err(EntityIdError::EmptyIdentifier(input.to_string()));
        }
        if matches!(raw_fragment, Some(f) if f.is_empty()) {
            return Err(EntityIdError::EmptyFragment(input.to_string()));
        }
        if raw_identifier.chars().any(char::is_whitespace)
            || raw_fragment.is_some_and(|f| f.chars().any(char::is_whitespace))
        {
            return Err(EntityIdError::WhitespaceInIdentifier(input.to_string()));
        }

        let decode = |s: &str| {
            decode_percent(s).map_err(|reason| EntityIdError::InvalidEscape {
                input: input.to_string(),
                reason,
            })
        };
        let identifier = decode(raw_identifier)?;
        let fragment = match raw_fragment {
            Some(f) => Some(decode(f)?),
            None => None,
        };

        Ok(Self {
            class: class.to_string(),
            identifier,
            fragment,
        })
    }
}
```

## Errors

Each variant carries the offending input, because these surface in a
check report over a whole corpus where "invalid id" without the string
is useless.

```rust {#error}
/// Errors produced while parsing an [`EntityId`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntityIdError {
    MissingScheme(String),
    MissingSeparator(String),
    EmptyClass(String),
    EmptyIdentifier(String),
    /// The string ends in a bare `#`. A fragment that was typed named
    /// something; degrading it to "the whole entity" loses that silently.
    EmptyFragment(String),
    WhitespaceInIdentifier(String),
    InvalidEscape {
        input: String,
        reason: String,
    },
    /// The string carries an `@<locator>` suffix pinning it to a content
    /// state. That half of the grammar belongs to the substrate; the
    /// format library refuses rather than silently drops the pin.
    LocatorSuffix {
        input: String,
        locator: String,
    },
}

impl fmt::Display for EntityIdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingScheme(v) => write!(f, "id `{v}` is missing the `x0k:` scheme prefix"),
            Self::MissingSeparator(v) => {
                write!(f, "id `{v}` is missing the `/` separator after the class")
            }
            Self::EmptyClass(v) => write!(f, "id `{v}` has an empty class"),
            Self::EmptyIdentifier(v) => write!(f, "id `{v}` has an empty identifier"),
            Self::EmptyFragment(v) => write!(f, "id `{v}` has an empty `#` fragment"),
            Self::WhitespaceInIdentifier(v) => {
                write!(f, "id `{v}` has whitespace in the identifier")
            }
            Self::InvalidEscape { input, reason } => {
                write!(f, "id `{input}` has invalid percent-escape: {reason}")
            }
            Self::LocatorSuffix { input, locator } => write!(
                f,
                "id `{input}` carries the content-state locator `@{locator}`; \
                 that half of the URI grammar belongs to the substrate, not to \
                 the document format"
            ),
        }
    }
}

impl std::error::Error for EntityIdError {}
```

## Percent-encoding

Byte-for-byte the substrate's rule but for one character: encode the
structural separators (`/`, `@`, `#`), the escape sentinel (`%`),
whitespace, and everything outside printable ASCII. Decoding is the
inverse and rejects a truncated or non-hex escape rather than passing
it through. `#` is the addition, and it is forced: a fragment separator
that can also appear raw inside an identifier is not a separator. The
substrate's renderer does not encode it, so the two agree on every
string that has no `#` in it — which is every authored id in the corpus
measured today — and the divergence is the same shape as the locator's,
written down rather than discovered.

```rust {#percent}
/// Whether a byte must be percent-encoded. Disallowed: the separators
/// `/`, `@` and `#`, whitespace, `%` (escape sentinel), and any
/// non-printable / non-ASCII byte. `x0k_types::entity_uri`'s rule but
/// for `#`, which that renderer does not treat as a separator; the two
/// agree on every string without one.
fn must_encode(byte: u8) -> bool {
    match byte {
        b'/' | b'@' | b'%' | b'#' => true,
        b' ' | b'\t' | b'\n' | b'\r' => true,
        // Printable ASCII range, exclusive of the special chars above.
        0x21..=0x7e => false,
        _ => true,
    }
}

fn write_encoded(f: &mut fmt::Formatter<'_>, s: &str) -> fmt::Result {
    for byte in s.as_bytes() {
        if must_encode(*byte) {
            write!(f, "%{:02X}", byte)?;
        } else {
            f.write_str(std::str::from_utf8(std::slice::from_ref(byte)).unwrap_or(""))?;
        }
    }
    Ok(())
}

fn decode_percent(s: &str) -> Result<String, String> {
    let bytes = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'%' {
            if i + 2 >= bytes.len() {
                return Err(format!("truncated percent-escape at offset {i}"));
            }
            let hi = hex_nibble(bytes[i + 1])
                .ok_or_else(|| format!("non-hex char in escape at offset {}", i + 1))?;
            let lo = hex_nibble(bytes[i + 2])
                .ok_or_else(|| format!("non-hex char in escape at offset {}", i + 2))?;
            out.push((hi << 4) | lo);
            i += 3;
        } else {
            out.push(b);
            i += 1;
        }
    }
    String::from_utf8(out).map_err(|e| format!("decoded bytes are not UTF-8: {e}"))
}

fn hex_nibble(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}
```

## Tests

The round-trip cases are drawn from the substrate's own table, one per
class, so the parity claim is checked against the same inputs
`EntityUri` is checked against. The rest pin the refusals, the one
asymmetry, and the two ways a fragment must behave: it survives a
round-trip as a *part*, and it never leaks into the identifier.

```rust {#tests}
#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip(input: &str) -> EntityId {
        let parsed: EntityId = input
            .parse()
            .unwrap_or_else(|e| panic!("parse `{input}`: {e}"));
        assert_eq!(parsed.to_string(), input, "round-trip mismatch for `{input}`");
        parsed
    }

    #[test]
    fn round_trips_one_id_per_corpus_class() {
        for input in [
            "x0k:commitment/no-shortcuts",
            "x0k:design/publish-a-region-as-a-repository",
            "x0k:architecture/publication-projection",
            "x0k:wiki/pattern-language-of-computing",
            "x0k:literate-spec/agent_messaging",
            "x0k:affordance/publish_region_as_repository",
            "x0k:software-module/x0k-folio",
            "x0k:publication/x0k-folio",
            "x0k:ontology-module/document",
            "x0k:seed/01HX9G7Z9C8M3J5K2P0R7T8V6W",
            "x0k:intent/ab1dd048-fa0b-47b4-9d65-af94da067049",
        ] {
            round_trip(input);
        }
    }

    #[test]
    fn a_fragment_is_a_part_and_never_a_tail_of_the_identifier() {
        // The two places the corpus already writes one: a publication
        // severing a crate's feature, and a publication (or a
        // transclusion) naming one section of a design.
        let feature = round_trip("x0k:software-module/x0k-folio#plugins");
        assert_eq!(feature.identifier, "x0k-folio");
        assert_eq!(feature.fragment.as_deref(), Some("plugins"));

        let section = round_trip(
            "x0k:design/publish-a-region-as-a-repository#read-an-affordance-out-of-a-document",
        );
        assert_eq!(section.identifier, "publish-a-region-as-a-repository");
        assert_eq!(
            section.fragment.as_deref(),
            Some("read-an-affordance-out-of-a-document")
        );

        // The whole entity a fragment names a part of, and back again.
        let whole = section.without_fragment();
        assert_eq!(whole.fragment, None);
        assert_eq!(whole.to_string(), "x0k:design/publish-a-region-as-a-repository");
        assert_eq!(
            whole.with_fragment("read-an-affordance-out-of-a-document"),
            section
        );

        // Two sections of one document are two ids, not one.
        assert_ne!(
            "x0k:design/a#one".parse::<EntityId>().unwrap(),
            "x0k:design/a#two".parse::<EntityId>().unwrap()
        );
    }

    #[test]
    fn a_literal_hash_in_an_identifier_encodes_so_the_separator_stays_one() {
        let id = EntityId::new("design", "has#hash");
        assert_eq!(id.to_string(), "x0k:design/has%23hash");
        let parsed: EntityId = id.to_string().parse().expect("round-trip");
        assert_eq!(parsed.identifier, "has#hash");
        assert_eq!(parsed.fragment, None);
    }

    #[test]
    fn splits_class_from_identifier() {
        let id = round_trip("x0k:design/publish-a-region-as-a-repository");
        assert_eq!(id.class, "design");
        assert_eq!(id.identifier, "publish-a-region-as-a-repository");
    }

    #[test]
    fn unknown_class_parses_and_round_trips() {
        let id = round_trip("x0k:hypothetical-future-class/some-id");
        assert_eq!(id.class, "hypothetical-future-class");
    }

    #[test]
    fn locator_suffix_is_refused_not_dropped() {
        let err = "x0k:design/messenger-tier-b@file-content:abcdef"
            .parse::<EntityId>()
            .expect_err("a content-state pin must not parse here");
        assert!(
            matches!(&err, EntityIdError::LocatorSuffix { locator, .. }
                     if locator == "file-content:abcdef"),
            "wrong error: {err:?}"
        );
    }

    #[test]
    fn percent_escapes_decode_and_re_encode() {
        let id: EntityId = "x0k:design/has%20space".parse().expect("parse");
        assert_eq!(id.identifier, "has space");
        assert_eq!(id.to_string(), "x0k:design/has%20space");

        let id: EntityId = "x0k:design/with%40at".parse().expect("parse");
        assert_eq!(id.identifier, "with@at");
        assert_eq!(id.to_string(), "x0k:design/with%40at");

        // Non-ASCII percent-encodes its UTF-8 bytes and survives the trip.
        let id = EntityId::new("design", "café");
        let parsed: EntityId = id.to_string().parse().expect("round-trip");
        assert_eq!(parsed.identifier, "café");

        // Punctuation the rule allows stays unencoded.
        round_trip("x0k:design/foo.bar_baz-qux");
    }

    #[test]
    fn identifier_may_hold_a_bare_slash_but_re_renders_encoded() {
        // The literate corpus ids itself this way, so the parse must
        // accept it; the renderer matches `EntityUri` and encodes. The
        // pair therefore parses a wider language than it renders, and
        // this test is where that is written down rather than discovered.
        let id: EntityId = "x0k:implementation/folio/identity".parse().expect("parse");
        assert_eq!(id.class, "implementation");
        assert_eq!(id.identifier, "folio/identity");
        assert_eq!(id.to_string(), "x0k:implementation/folio%2Fidentity");
    }

    #[test]
    fn malformed_inputs_return_typed_errors() {
        type Check = fn(&EntityIdError) -> bool;
        let cases: &[(&str, Check)] = &[
            ("design/foo", |e| {
                matches!(e, EntityIdError::MissingScheme(_))
            }),
            ("x0k:designfoo", |e| {
                matches!(e, EntityIdError::MissingSeparator(_))
            }),
            ("x0k:/foo", |e| matches!(e, EntityIdError::EmptyClass(_))),
            ("x0k:design/", |e| {
                matches!(e, EntityIdError::EmptyIdentifier(_))
            }),
            ("x0k:design/has space", |e| {
                matches!(e, EntityIdError::WhitespaceInIdentifier(_))
            }),
            ("x0k:design/foo%2", |e| {
                matches!(e, EntityIdError::InvalidEscape { .. })
            }),
            ("x0k:design/foo%ZZ", |e| {
                matches!(e, EntityIdError::InvalidEscape { .. })
            }),
            ("x0k:design/foo@", |e| {
                matches!(e, EntityIdError::LocatorSuffix { .. })
            }),
            // A pin hiding behind a fragment is still a pin.
            ("x0k:design/foo#bar@baz", |e| {
                matches!(e, EntityIdError::LocatorSuffix { .. })
            }),
            ("x0k:design/foo#", |e| {
                matches!(e, EntityIdError::EmptyFragment(_))
            }),
            ("x0k:design/#bar", |e| {
                matches!(e, EntityIdError::EmptyIdentifier(_))
            }),
            ("x0k:design/foo#has bar", |e| {
                matches!(e, EntityIdError::WhitespaceInIdentifier(_))
            }),
        ];
        for (input, check) in cases {
            let err = input.parse::<EntityId>().expect_err(input);
            assert!(check(&err), "wrong error for `{input}`: {err:?}");
        }
    }
}
```

## Composing the module

```rust {#root}
<<module-doc>>

<<entity-id>>

<<display>>

<<from-str>>

<<error>>

<<percent>>

<<tests>>
```

The module is small and will get smaller: when the substrate's side of
the split lands, `write_encoded`, `decode_percent`, and `must_encode`
stop being a second copy and become the only copy. Until then the tests
above are the join — they are the same cases `x0k-types` runs, so the
day the two disagree, both suites say so.
