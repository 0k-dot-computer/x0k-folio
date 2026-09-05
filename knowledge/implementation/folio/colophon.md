---
x0k:
  format: folio/v1
  id: x0k:implementation/folio/colophon
  type: implementation
  status: draft
  summary: The envelope's single parser and renderer, permissive about keys it does not own and closed about the keywords it does, consumed by every crate that touches a folio file.
  concerns: [folio, envelope, frontmatter, parsing, yaml]
  tangle:
    crate: x0k-folio
    root: src/colophon.rs
  edges:
    implements:
      - x0k:design/knowledge-kinds-and-citations
    cites:
      - x0k:architecture/filesystem-graph-materialization
      - x0k:implementation/folio/format
      - x0k:implementation/tangle/parsing
---
# The colophon: one envelope, one parser, one renderer

Every folio/v1 document in the corpus — a wiki page, a design decision, a
publication manifest, this very file — opens with the same `--- ... ---`
YAML block. That block is the document's **colophon**: its identity
(`id:`), its genus (`type:`), its lifecycle (`status:`), and its place in
the graph (`edges:`). Before this module existed the envelope had grown
several parsers — the wiki crate, the daemon, the tangler each read the
YAML their own way — and "what is a valid folio file" had no single
answer. This module is that answer: exactly one parser
([`parse_envelope`](#parse-envelope)) and one renderer
([`render_envelope`](#render-envelope)) for the shape, consumed by every
crate that touches a folio file.

Our carried example throughout the folio chapters is a real envelope, the
publication manifest at `decisions/publications/x0k-folio.md`
— the document that publishes this crate to the public internet:

```yaml
x0k:
  format: folio/v1
  type: publication
  id: x0k:publication/x0k-folio
  status: proposed
  edges:
    publishes:
      - x0k:software-module/x0k-folio
      - x0k:software-module/x0k-tangle
  license: MIT OR Apache-2.0
```

Parsing it yields a typed [`Colophon`](#colophon-type): `doc_type` is
`DocType::Publication`, `status` is `Some(Status::Proposed)`, and
`edges["publishes"]` carries the module URIs. The `license:` key is worth
pausing on: it is not a field this parser knows. The envelope tolerates
unknown keys (serde ignores wire fields with no target), so consumers with
domain-specific needs — the repository projector reads `license:` with its
own scan — can extend the envelope without a lockstep change here. That
tolerance is a deliberate one-way valve: unknown *keys* pass silently, but
unknown *values* for the keys we do own (`type:`, `status:`) fail loudly.

## The contract

The parser's scope is deliberately narrow:

- **In scope:** splitting a file into YAML + body, deserializing the
  `x0k:` block to the typed envelope, and rendering an envelope back to
  canonical YAML.
- **Out of scope:** edge-predicate validation (the daemon's job),
  `EntityUri` parsing of edge targets, and wiki-specific body extraction.

The out-of-scope list is why edge targets and the document `id` stay
`String` here. Promoting them to `EntityUri` would drag URI validation
rules into every consumer of the shared parser, and each consumer wants a
different strictness — the daemon's typed wrapper enforces predicate
vocabularies, the tangler just needs `tangle.crate`. Keep the shared layer
permissive; promote in the consumer.

The module performs no IO. It takes strings and returns strings and typed
values; where a file comes from and what happens to the rendered YAML is
entirely the caller's business.

```rust {#module-doc}
//! Canonical folio/v1 frontmatter envelope: shared parser + renderer.
//!
//! This module owns the YAML envelope shape that wiki pages and decision
//! documents share. Daemon and wiki crates consume from here so the
//! `--- ... ---` block has exactly one parser and one renderer in the
//! workspace. Edge targets and ids stay `String` — each consumer decides
//! how strict to be; the daemon's typed wrapper promotes to `EntityUri`.
//!
//! The split between file authority and database authority that this
//! envelope serves is set out in an internal architecture decision
//! (`filesystem-graph-materialization`); this module needs none of it
//! to parse or render.

use serde::Deserialize;
use std::collections::BTreeMap;
use std::collections::HashMap;
```

## Format tokens

The wire token `folio/v1` is what makes a frontmatter block a colophon
rather than arbitrary YAML — [`is_colophon`](#is-colophon) gates on it and
[`parse_envelope`](#parse-envelope) rejects anything else. Alongside it
live the recognized body formats. A folio body is markdown unless the
envelope says otherwise; HTML is the one opt-in alternative:

```rust {#format-tokens}
/// Frontmatter format token authoritative for this parser.
pub const FORMAT_FOLIO_V1: &str = "folio/v1";

/// Default `body_format` when the envelope omits the field. Every legacy
/// decision body is markdown.
pub const BODY_FORMAT_MARKDOWN: &str = "markdown";

/// HTML body format. Authored as opt-in; renderer dispatches on this value.
pub const BODY_FORMAT_HTML: &str = "html";

/// Recognized `body_format` values. Unknown values warn and fall back to
/// `markdown` per the `unknown_edges` forward-compat pattern.
pub const KNOWN_BODY_FORMATS: &[&str] = &[BODY_FORMAT_MARKDOWN, BODY_FORMAT_HTML];
```

An unknown `body_format` is the forward-compat case: a newer peer may have
written a format this build doesn't know. Erroring would make old software
unable to *open* new documents, which is worse than rendering them as
markdown; so the policy is warn-and-fall-back, centralized here so callers
don't each reinvent it:

```rust {#normalize-body-format}
/// Normalize the parsed-or-omitted body format string. Returns the canonical
/// value (`"markdown"` or `"html"`); for unrecognized values it emits a
/// `tracing::warn` and returns `"markdown"`. Centralizes the warn-and-fall-back
/// behaviour so callers don't reimplement the policy.
pub fn normalize_body_format(raw: Option<&str>) -> String {
    match raw {
        None => BODY_FORMAT_MARKDOWN.to_string(),
        Some(v) if v == BODY_FORMAT_MARKDOWN => BODY_FORMAT_MARKDOWN.to_string(),
        Some(v) if v == BODY_FORMAT_HTML => BODY_FORMAT_HTML.to_string(),
        Some(other) => {
            tracing::warn!(
                body_format = %other,
                "unknown `x0k.body_format` value; falling back to `markdown` (forward-compat)"
            );
            BODY_FORMAT_MARKDOWN.to_string()
        }
    }
}
```

## The genus: DocType

`type:` names the document's genus per `ontology/`. The variants are a
closed set — an unknown genus is a parse error, not a tolerated extension,
because everything downstream (review workflow, storage authority, URI
namespace) dispatches on it. Our carried publication manifest parses to
`DocType::Publication`; this literate page parses to
`DocType::Implementation`.

```rust {#doc-type}
/// Genus types per `ontology/`. Decision subtypes (commitment / design /
/// architecture / publication) live here directly — for those, `DocType` IS
/// the decision subtype because each has its own review workflow. Knowledge
/// genus types (Wiki today) carry their page-kind in the optional
/// `subtype` field on the parsed envelope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocType {
    Commitment,
    Design,
    Architecture,
    Publication,
    /// Authored long-form content — an author's *corpus* of composed works,
    /// independent of whether any Publication makes them public. File-canonical
    /// and reviewed through version control (authorship and editorial control
    /// matter), distinct from the database-canonical, dialog-produced Wiki
    /// (reference knowledge) and from Decisions (choices). A Publication is a thin manifest that *selects*
    /// manuscripts to publish; the manuscript exists in the corpus regardless.
    /// Lives under `manuscripts/<work>/<part>.md`; addressed as
    /// `x0k:manuscript/<work>/<part>`.
    Manuscript,
    Wiki,
    /// Literate-implementation documents under `knowledge/implementation/`.
    /// Tangle into real crates; implement one or more upstream designs or
    /// affordances. Addressed as `x0k:implementation/<dir>/<slug>`.
    Implementation,
    /// The curated document half of a Seed — a captured idea in the
    /// planning graph. Each planning entity is split in two: an authored,
    /// file-canonical half (title, description prose, icon recipe, curated
    /// frontmatter edges) materialized under `seeds/<id>.md`, and a
    /// dynamic half (status, timestamps, queue position) that lives only
    /// as database facts and never enters the file. The two are joined by
    /// URI (`x0k:seed/<id>`); the daemon that materializes documents owns
    /// the per-field split.
    Seed,
    /// An Intent's curated document half. `intents/<id>.md`, addressed
    /// `x0k:intent/<id>`. Curated DAG edges (`depends_on`,
    /// `refined_from`, `child_of`) live in frontmatter; execution
    /// status/config stay FACT-only.
    Intent,
    /// An Affordance's curated document half. `affordances/<id>.md`,
    /// addressed `x0k:affordance/<id>`. Title + description are the
    /// curated body; per-context status claims stay FACT-only.
    Affordance,
}
```

The string round-trip is hand-written rather than derived from serde
because the same names appear in contexts serde never sees (URI segments,
query filters), and a match statement the compiler exhaustiveness-checks
is the cheapest way to keep the two directions in lockstep.

The inward half is an inherent `from_str` returning `Option`, not
`FromStr`. Clippy flags the name for exactly that reason, and the allow
is a decision rather than a silencing: an unknown genus is *absence*,
not a failure with a story to tell, so there is no error type worth
minting, and every caller in the corpus writes
`.and_then(DocType::from_str)` or `.ok_or_else(…)` over the `Option`.
`FromStr` would force `Result<Self, E>` on all of them for a `()`-shaped
error. `Status::from_str` below is the same call:

```rust {#doc-type-strings}
impl DocType {
    pub fn as_str(self) -> &'static str {
        match self {
            DocType::Commitment => "commitment",
            DocType::Design => "design",
            DocType::Architecture => "architecture",
            DocType::Publication => "publication",
            DocType::Manuscript => "manuscript",
            DocType::Wiki => "wiki",
            DocType::Implementation => "implementation",
            DocType::Seed => "seed",
            DocType::Intent => "intent",
            DocType::Affordance => "affordance",
        }
    }

    /// Parse a genus name. `None` for an unrecognized one: an unknown
    /// genus is absence, not an error with anything to say, so this is
    /// deliberately not `FromStr` — see the prose above.
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        Some(match s {
            "commitment" => DocType::Commitment,
            "design" => DocType::Design,
            "architecture" => DocType::Architecture,
            "publication" => DocType::Publication,
            "manuscript" => DocType::Manuscript,
            "wiki" => DocType::Wiki,
            "implementation" => DocType::Implementation,
            "seed" => DocType::Seed,
            "intent" => DocType::Intent,
            "affordance" => DocType::Affordance,
            _ => return None,
        })
    }
}
```

## Lifecycle: Status

One enum serves two lifecycles — decisions move `proposed → accepted →
superseded`, knowledge pages move `draft → stable → stale`. Merging them
into one type means the shared parser doesn't need to know the genus
before it can read the status; which values are *legal* for a given genus
is (like predicate vocabularies) the consumer's rule to enforce.

```rust {#status}
/// Lifecycle status. Decisions use `proposed | accepted | superseded`;
/// wiki pages use `draft | stable | stale`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Proposed,
    Accepted,
    Superseded,
    Draft,
    Stable,
    Stale,
}

impl Status {
    pub fn as_str(self) -> &'static str {
        match self {
            Status::Proposed => "proposed",
            Status::Accepted => "accepted",
            Status::Superseded => "superseded",
            Status::Draft => "draft",
            Status::Stable => "stable",
            Status::Stale => "stale",
        }
    }

    /// Parse a status name. `None` for an unrecognized one; not `FromStr`
    /// for the same reason [`DocType::from_str`] is not.
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        Some(match s {
            "proposed" => Status::Proposed,
            "accepted" => Status::Accepted,
            "superseded" => Status::Superseded,
            "draft" => Status::Draft,
            "stable" => Status::Stable,
            "stale" => Status::Stale,
            _ => return None,
        })
    }
}
```

## Optional blocks: materialization, tangle, pipelines

Three optional sub-blocks ride the envelope, each the keyhole for one
subsystem. `materialization:` points a projected file back at the Loro
document it is a window onto; `tangle:` marks a literate document whose
code chunks project into a crate (this page carries one); `pipelines:`
declares codegen passes over named chunks.

```rust {#materialization}
/// Optional materialization metadata block — pointers to the Loro doc and
/// last revision the file was projected from. Absent on file-authority docs
/// that have not yet been ingested.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Materialization {
    /// Identifier of the Loro document this file is a projection of.
    pub loro_doc_id: Option<String>,
    /// Revision of that document the file was last projected from.
    pub document_revision_id: Option<String>,
    /// Hash of the body as projected, for detecting edits made to the file.
    pub content_hash: Option<String>,
}
```

`TangleConfig` mirrors what the tangler's own frontmatter walk extracts
(see [`tangle/parsing.md`](../tangle/parsing.md)) — the crate, the default
output file, and the per-language `roots:` map for bilingual documents:

```rust {#tangle-config}
/// Optional tangle configuration — when present, the document participates
/// in literate programming: named code chunks in the markdown body can be
/// tangled into compilable source files.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TangleConfig {
    /// Workspace crate this document tangles into (e.g. `"x0k-vcs"`).
    pub crate_name: Option<String>,
    /// Default output file relative to the crate directory.
    pub root: Option<String>,
    /// Per-language output roots (fence language → path) for bilingual
    /// docs that tangle one prose body to multiple substrates (e.g.
    /// `rust:` and `gallowglass:`). Sorted map; empty when absent.
    pub roots: BTreeMap<String, String>,
}
```

A pipeline declaration names a transformer plugin, the chunks it consumes,
and an untyped config payload the plugin deserializes itself. The wire
format has a shorthand (`input: tokens`) and a long form (`inputs: {name:
chunk}`); the shorthand normalizes at parse time to a single entry under
the `"default"` key, so plugins see one shape:

````rust {#pipeline-decl}
/// Declaration of one pipeline pass for a literate document. A pipeline
/// names a transformer (`kind`) registered against a `PipelineRegistry`,
/// the input chunks it consumes (keyed by the plugin's parameter name →
/// chunk name in this document), and the plugin-deserialized `config`
/// payload.
///
/// Wire format supports two shorthand forms (normalized at parse time):
///
/// ```yaml
/// # shorthand: single input plugin
/// pipelines:
///   - kind: theme-codegen
///     input: tokens                          # bare chunk name
///     config: { name: pansophia, scheme: single }
///
/// # long form: explicit name → chunk map
/// pipelines:
///   - kind: theme-codegen
///     inputs: { tokens: tokens-chunk }
///     config: { ... }
/// ```
///
/// The shorthand `input: tokens` is normalized to
/// `inputs: { "default": "tokens" }`. Plugins that take a single input
/// look up `inputs["default"]`; multi-input plugins always use the long
/// form and pick their own keys.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PipelineDecl {
    /// Identifier of the transformer plugin (`"theme-codegen"`, etc).
    pub kind: String,
    /// Map from plugin-param name → chunk name in this document.
    /// Shorthand `input: <name>` normalizes to a single entry under the
    /// `"default"` key.
    pub inputs: HashMap<String, String>,
    /// Plugin-specific configuration payload, passed through as a typed
    /// JSON value (deserialized into the plugin's own typed config in
    /// `transform`).
    pub config: serde_json::Value,
}
````

## Errors a caller can act on

The error enum is typed at the level of the caller's decision, not the
parser's internals. The distinction that matters most is the first two
variants: `NoFrontmatter` and `NotColophon` mean "this is not a folio/v1
file — skip it", while everything after means "this file *claims* to be
folio/v1 and is broken — surface it". A directory walk over a mixed tree
leans on exactly that split.

```rust {#folio-error}
/// Errors typed at the level a caller can act on: missing envelope means
/// "not a folio/v1 file"; malformed envelope means "claims to be
/// folio/v1 but isn't, surface it".
#[derive(Debug)]
pub enum FolioError {
    NoFrontmatter,
    NotColophon,
    InvalidYaml(String),
    MissingField { field: &'static str },
    WrongFormat { got: String },
    InvalidType { got: String },
    InvalidStatus { got: String },
}

impl std::fmt::Display for FolioError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoFrontmatter => f.write_str("file has no frontmatter block"),
            Self::NotColophon => {
                f.write_str("frontmatter is not folio/v1 (no `x0k.format: folio/v1`)")
            }
            Self::InvalidYaml(msg) => write!(f, "frontmatter YAML is malformed: {msg}"),
            Self::MissingField { field } => write!(f, "required field `x0k.{field}` is missing"),
            Self::WrongFormat { got } => {
                write!(f, "`x0k.format` must be `folio/v1`, got `{got}`")
            }
            Self::InvalidType { got } => write!(
                f,
                "`x0k.type` must be one of commitment|design|architecture|publication|manuscript|wiki|implementation|seed|intent|affordance, got `{got}`"
            ),
            Self::InvalidStatus { got } => write!(
                f,
                "`x0k.status` must be one of proposed|accepted|superseded|draft|stable|stale, got `{got}`"
            ),
        }
    }
}

impl std::error::Error for FolioError {}
```

## The parsed envelope

`Colophon` is the typed result. Two shape decisions deserve their
sentence. `status` is `Option` even though decisions require it — the
*shared* envelope stays permissive and each consumer enforces its own
requirements, the same division of labor as the string-shaped URIs.
And `Eq` is deliberately not derived: `pipelines[].config` is a
`serde_json::Value`, which can carry `f64`s, and floats only have partial
equality.

```rust {#colophon-type}
/// Parsed canonical envelope. Edge targets and the document `id` are kept as
/// strings here so the shared parser doesn't have to know about
/// `EntityUri` validation rules — promote in the consumer.
///
/// `summary` and `updated_by` live on the shared envelope so both wiki and
/// decision-domain docs can carry them when useful.
///
/// `Eq` is intentionally not derived: `pipelines[].config` is a
/// `serde_json::Value`, which carries `f64` numbers that have only
/// partial equality. Tests assert equality via `assert_eq!` (which only
/// requires `PartialEq`); consumers that need hash/key behavior pin off
/// the `id` field.
#[derive(Debug, Clone, PartialEq)]
pub struct Colophon {
    /// The document's identity: an `x0k:<genus>/<stem>` URI, kept as a
    /// string (see the type-level note).
    pub id: String,
    /// The document's genus, from the closed [`DocType`] set.
    pub doc_type: DocType,
    /// Optional CURIE-form subtype for genera with agent-curated page-kinds
    /// (e.g. `wiki:Methodology`). For decision subtypes the type IS the
    /// subtype, so this stays `None`.
    pub subtype: Option<String>,
    /// Optional in the shared envelope. Decision docs require it; wiki
    /// pages also carry it. The daemon's typed wrapper enforces presence.
    pub status: Option<Status>,
    /// Free-form topic tags; empty when the envelope omits `concerns:`.
    pub concerns: Vec<String>,
    /// Optional one-line summary of the document.
    pub summary: Option<String>,
    /// Optional identity of the last editor.
    pub updated_by: Option<String>,
    /// ISO-8601 UTC timestamp the document was first created. Carried on
    /// the envelope (not the Loro op-log) so a daemon restart re-hydrates
    /// timestamps from the file alone.
    pub created_at: Option<String>,
    /// ISO-8601 UTC timestamp of the most recent update. See `created_at`.
    pub updated_at: Option<String>,
    /// Graph edges, predicate → target URIs, in sorted predicate order.
    /// Predicate vocabularies are validated by the consumer, not here.
    pub edges: BTreeMap<String, Vec<String>>,
    /// Present on files projected from a Loro document; absent on
    /// file-authority documents that have not been ingested.
    pub materialization: Option<Materialization>,
    /// Literate programming tangle configuration. When present, the
    /// document's named code chunks can be tangled into source files.
    pub tangle: Option<TangleConfig>,
    /// Pipeline declarations — codegen plugins that consume named chunks
    /// from this document's body and produce transformed outputs. See
    /// [`PipelineDecl`] for the wire format; the registry that resolves
    /// plugin names is `PipelineRegistry` in `x0k-tangle`'s `pipeline`
    /// module (this crate does not depend on the tangler).
    pub pipelines: Vec<PipelineDecl>,
    /// Body format dispatch flag — `"markdown"` (default) or `"html"`.
    /// Unknown values are coerced to `"markdown"` at parse time with a
    /// tracing warning (see `normalize_body_format`).
    pub body_format: String,
}
```

## The wire shapes

Deserialization goes through private `Wire*` structs that mirror the YAML
exactly, every field `Option` and `default`. This is the standard
two-layer move: serde gets a shape it can fill mechanically, and the
promotion from wire to `Colophon` — where required fields are demanded and
keywords validated — happens in one readable pass inside
[`parse_envelope`](#parse-envelope) instead of scattered across serde
attributes.

```rust {#wire-shapes}
/// Inner serde shape mirrors the YAML wire format. `edges` is captured as
/// `BTreeMap<String, Vec<String>>` because predicate names are open-ended.
#[derive(Debug, Deserialize)]
struct WireRoot {
    x0k: Option<WireC0k>,
}

#[derive(Debug, Deserialize)]
struct WireC0k {
    format: Option<String>,
    id: Option<String>,
    #[serde(rename = "type")]
    doc_type: Option<String>,
    #[serde(default)]
    subtype: Option<String>,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    concerns: Option<Vec<String>>,
    #[serde(default)]
    summary: Option<String>,
    #[serde(default)]
    updated_by: Option<String>,
    #[serde(default)]
    created_at: Option<String>,
    #[serde(default)]
    updated_at: Option<String>,
    #[serde(default)]
    edges: Option<BTreeMap<String, Vec<String>>>,
    #[serde(default)]
    materialization: Option<WireMaterialization>,
    #[serde(default)]
    tangle: Option<WireTangle>,
    #[serde(default)]
    pipelines: Option<Vec<WirePipelineDecl>>,
    #[serde(default)]
    body_format: Option<String>,
}

#[derive(Debug, Deserialize)]
struct WireMaterialization {
    #[serde(default)]
    loro_doc_id: Option<String>,
    #[serde(default)]
    document_revision_id: Option<String>,
    #[serde(default)]
    content_hash: Option<String>,
}

#[derive(Debug, Deserialize)]
struct WireTangle {
    #[serde(default, rename = "crate")]
    crate_name: Option<String>,
    #[serde(default)]
    root: Option<String>,
    #[serde(default)]
    roots: Option<BTreeMap<String, String>>,
}

#[derive(Debug, Deserialize)]
struct WirePipelineDecl {
    kind: String,
    /// Long form — explicit name → chunk map.
    #[serde(default)]
    inputs: Option<HashMap<String, String>>,
    /// Shorthand — bare chunk name. Normalized to
    /// `inputs: { "default": <name> }`.
    #[serde(default)]
    input: Option<String>,
    /// Plugin config payload, captured as untyped JSON for the plugin to
    /// deserialize into its own typed shape.
    #[serde(default)]
    config: serde_norway::Value,
}
```

## Splitting the file

Before any YAML is parsed the file must split into envelope and body. The
split recognizes two closers: `\n---\n` (a body follows) and `\n---` at
end-of-file (an envelope-only document, legal for thin manifests). Note
the return type — both halves borrow from the input, so a caller that only
wants the body pays for no allocation:

```rust {#split-frontmatter}
/// Parse the frontmatter block out of `content`, leaving the body untouched.
/// Returns `(yaml_block, body)`. Both wiki and daemon callers split on the
/// same shape so the parser surface is uniform.
pub fn split_frontmatter(content: &str) -> Option<(&str, &str)> {
    let body_start_marker = "\n---\n";
    let eof_marker = "\n---";
    let after_open = content.strip_prefix("---\n")?;
    if let Some(idx) = after_open.find(body_start_marker) {
        let yaml = &after_open[..idx];
        let body = &after_open[idx + body_start_marker.len()..];
        return Some((yaml, body));
    }
    if let Some(idx) = after_open.find(eof_marker) {
        let yaml = &after_open[..idx];
        let body = &after_open[idx + eof_marker.len()..];
        return Some((yaml, body));
    }
    None
}
```

Directory walks meet thousands of files that are not folio documents, so
there is a cheap gate that avoids the full YAML parse — a substring check,
knowingly imprecise (a stray `folio/v1` in a comment would pass), because
the full parser runs right behind it for anything that matters:

```rust {#is-colophon}
/// Quick check: does this file claim folio/v1? Cheap so callers can gate
/// without paying full parse cost on legacy files.
pub fn is_colophon(content: &str) -> bool {
    let Some((yaml, _)) = split_frontmatter(content) else {
        return false;
    };
    yaml.contains(FORMAT_FOLIO_V1)
}
```

## parse_envelope

The public entry point, and the one place wire shapes are promoted to the
typed envelope. Reading it top to bottom is reading the envelope's rules:
`format` must be present and exact; `id` and `type` must be present;
`type` and `status` must be known keywords; everything else defaults. This
is where our carried manifest's `type: publication` either becomes
`DocType::Publication` or the file is rejected as loudly as possible.

The YAML parser is `serde_norway`. It is a fork of `serde_yaml` with the
same API — the swap was an identifier rename and nothing else — chosen
because `serde_yaml` is archived upstream and its C backend
(`unsafe-libyaml`) is unmaintained, and `serde_norway` is the only
drop-in that replaces the backend too. There is no advisory against
either, so this is hygiene rather than a fix; it is worth doing because
this crate is published, and the first dependency a reader of a fresh
repository inspects should not be a deprecated one. `serde_yml`, the
other name in that neighbourhood, is a trap: it carries RUSTSEC-2025-0067
and RUSTSEC-2025-0068, so adopting it would introduce two advisories
where there are none.

```rust {#parse-envelope}
/// Parse a folio/v1 envelope from full file contents. Returns the typed
/// envelope plus the body string (the markdown after the closing `---`).
///
/// The shared envelope is permissive: missing `status` is allowed at this
/// layer (each consumer enforces what they require). Type keywords (`type`,
/// `status`) ARE validated against the known closed sets, so unknown values
/// fail loudly.
pub fn parse_envelope(content: &str) -> Result<(Colophon, String), FolioError> {
    let (yaml_block, body) = split_frontmatter(content).ok_or(FolioError::NoFrontmatter)?;
    let root: WireRoot =
        serde_norway::from_str(yaml_block).map_err(|e| FolioError::InvalidYaml(e.to_string()))?;
    let block = root.x0k.ok_or(FolioError::NotColophon)?;

    let format = block
        .format
        .ok_or(FolioError::MissingField { field: "format" })?;
    if format != FORMAT_FOLIO_V1 {
        return Err(FolioError::WrongFormat { got: format });
    }

    let id = block.id.ok_or(FolioError::MissingField { field: "id" })?;

    let type_str = block
        .doc_type
        .ok_or(FolioError::MissingField { field: "type" })?;
    let doc_type = DocType::from_str(&type_str).ok_or(FolioError::InvalidType { got: type_str })?;

    let status = match block.status {
        Some(s) => Some(Status::from_str(&s).ok_or(FolioError::InvalidStatus { got: s })?),
        None => None,
    };

    let materialization = block.materialization.map(|m| Materialization {
        loro_doc_id: m.loro_doc_id,
        document_revision_id: m.document_revision_id,
        content_hash: m.content_hash,
    });

    let tangle = block.tangle.map(|t| TangleConfig {
        crate_name: t.crate_name,
        root: t.root,
        roots: t.roots.unwrap_or_default(),
    });

    let pipelines = block
        .pipelines
        .unwrap_or_default()
        .into_iter()
        .map(|p| {
            let mut inputs = p.inputs.unwrap_or_default();
            if let Some(short) = p.input {
                inputs.entry("default".to_string()).or_insert(short);
            }
            // YAML value → JSON value via serde_norway::Value -> serde_json
            // round-trip through serialize.
            let config = yaml_to_json(p.config);
            PipelineDecl {
                kind: p.kind,
                inputs,
                config,
            }
        })
        .collect::<Vec<_>>();

    let body_format = normalize_body_format(block.body_format.as_deref());

    Ok((
        Colophon {
            id,
            doc_type,
            subtype: block.subtype,
            status,
            concerns: block.concerns.unwrap_or_default(),
            summary: block.summary,
            updated_by: block.updated_by,
            created_at: block.created_at,
            updated_at: block.updated_at,
            edges: block.edges.unwrap_or_default(),
            materialization,
            tangle,
            pipelines,
            body_format,
        },
        body.to_string(),
    ))
}
```

Pipeline configs arrive as `serde_norway::Value` but plugins consume
`serde_json::Value` — JSON is the lingua franca of the plugin boundary.
The conversion is a straightforward structural walk; the one judgment call
is collapsing non-string YAML map keys via `to_string`, which matches the
loose frontmatter convention that structured config keys are always
strings anyway:

```rust {#yaml-to-json}
/// Convert a `serde_norway::Value` to a `serde_json::Value`. YAML maps with
/// non-string keys collapse their keys via `to_string` (matches the loose
/// frontmatter convention where structured config keys are always
/// strings).
fn yaml_to_json(v: serde_norway::Value) -> serde_json::Value {
    match v {
        serde_norway::Value::Null => serde_json::Value::Null,
        serde_norway::Value::Bool(b) => serde_json::Value::Bool(b),
        serde_norway::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                serde_json::Value::Number(serde_json::Number::from(i))
            } else if let Some(u) = n.as_u64() {
                serde_json::Value::Number(serde_json::Number::from(u))
            } else if let Some(f) = n.as_f64() {
                serde_json::Number::from_f64(f)
                    .map(serde_json::Value::Number)
                    .unwrap_or(serde_json::Value::Null)
            } else {
                serde_json::Value::Null
            }
        }
        serde_norway::Value::String(s) => serde_json::Value::String(s),
        serde_norway::Value::Sequence(seq) => {
            serde_json::Value::Array(seq.into_iter().map(yaml_to_json).collect())
        }
        serde_norway::Value::Mapping(map) => {
            let mut out = serde_json::Map::with_capacity(map.len());
            for (k, v) in map {
                let key = match k {
                    serde_norway::Value::String(s) => s,
                    other => serde_norway::to_string(&other)
                        .unwrap_or_default()
                        .trim()
                        .to_string(),
                };
                out.insert(key, yaml_to_json(v));
            }
            serde_json::Value::Object(out)
        }
        serde_norway::Value::Tagged(t) => yaml_to_json(t.value),
    }
}
```

## render_envelope

The inverse direction. The obvious move would be `serde_norway::to_string`
on a `Serialize` derive — and it would be wrong, because serde does not
promise field order or formatting stability across versions, and this
output lands in version-controlled files where every spurious byte is a
diff. So the renderer is a hand-rolled string builder with a fixed field
order, and *omission is load-bearing*: default values (`body_format:
markdown`, empty `concerns`, empty sub-blocks) are not written at all, so
legacy documents round-trip without picking up stray fields.

```rust {#render-envelope}
/// Render a `Colophon` as the canonical `--- ... ---` YAML
/// frontmatter block (including the leading and trailing `---` lines and
/// the trailing newline).
///
/// Field order is fixed: `format`, `id`, `type`, `subtype`, `status`,
/// `summary`, `updated_by`, `concerns`, `edges`, `materialization`.
pub fn render_envelope(env: &Colophon) -> String {
    let mut out = String::new();
    out.push_str("---\n");
    out.push_str("x0k:\n");
    out.push_str("  format: ");
    out.push_str(FORMAT_FOLIO_V1);
    out.push('\n');
    out.push_str(&format!("  id: {}\n", env.id));
    out.push_str(&format!("  type: {}\n", env.doc_type.as_str()));
    if let Some(sub) = &env.subtype {
        out.push_str(&format!("  subtype: {}\n", sub));
    }
    if let Some(status) = env.status {
        out.push_str(&format!("  status: {}\n", status.as_str()));
    }
    // Only emit `body_format` when the value is non-default. Every legacy
    // markdown-bodied doc continues to roundtrip without picking up a
    // stray field; HTML bodies (and any future format) write the flag.
    if env.body_format != BODY_FORMAT_MARKDOWN {
        out.push_str(&format!("  body_format: {}\n", env.body_format));
    }
    if let Some(summary) = &env.summary {
        if !summary.is_empty() {
            out.push_str(&format!("  summary: {}\n", yaml_scalar(summary)));
        }
    }
    if let Some(updated_by) = &env.updated_by {
        if !updated_by.is_empty() {
            out.push_str(&format!("  updated_by: {}\n", yaml_scalar(updated_by)));
        }
    }
    if let Some(created_at) = &env.created_at {
        if !created_at.is_empty() {
            out.push_str(&format!("  created_at: {}\n", yaml_scalar(created_at)));
        }
    }
    if let Some(updated_at) = &env.updated_at {
        if !updated_at.is_empty() {
            out.push_str(&format!("  updated_at: {}\n", yaml_scalar(updated_at)));
        }
    }
    if !env.concerns.is_empty() {
        out.push_str("  concerns:\n");
        for tag in &env.concerns {
            out.push_str(&format!("    - {}\n", tag));
        }
    }
    if !env.edges.is_empty() {
        out.push_str("  edges:\n");
        for (predicate, targets) in &env.edges {
            out.push_str(&format!("    {}:\n", predicate));
            for target in targets {
                out.push_str(&format!("      - {}\n", target));
            }
        }
    }
    if let Some(m) = &env.materialization {
        let any =
            m.loro_doc_id.is_some() || m.document_revision_id.is_some() || m.content_hash.is_some();
        if any {
            out.push_str("  materialization:\n");
            if let Some(v) = &m.loro_doc_id {
                out.push_str(&format!("    loro_doc_id: {}\n", v));
            }
            if let Some(v) = &m.document_revision_id {
                out.push_str(&format!("    document_revision_id: {}\n", v));
            }
            if let Some(v) = &m.content_hash {
                out.push_str(&format!("    content_hash: {}\n", v));
            }
        }
    }
    if let Some(t) = &env.tangle {
        let any = t.crate_name.is_some() || t.root.is_some() || !t.roots.is_empty();
        if any {
            out.push_str("  tangle:\n");
            if let Some(v) = &t.crate_name {
                out.push_str(&format!("    crate: {}\n", v));
            }
            if let Some(v) = &t.root {
                out.push_str(&format!("    root: {}\n", v));
            }
            if !t.roots.is_empty() {
                out.push_str("    roots:\n");
                for (lang, path) in &t.roots {
                    out.push_str(&format!("      {}: {}\n", lang, path));
                }
            }
        }
    }
    if !env.pipelines.is_empty() {
        out.push_str("  pipelines:\n");
        for p in &env.pipelines {
            out.push_str(&format!("    - kind: {}\n", p.kind));
            // Emit `input:` shorthand when the decl carries exactly one
            // entry under the canonical `default` key; otherwise long
            // form `inputs:` map.
            if p.inputs.len() == 1 {
                if let Some(only) = p.inputs.get("default") {
                    out.push_str(&format!("      input: {}\n", only));
                } else {
                    out.push_str("      inputs:\n");
                    let mut keys: Vec<&String> = p.inputs.keys().collect();
                    keys.sort();
                    for k in keys {
                        out.push_str(&format!("        {}: {}\n", k, p.inputs[k]));
                    }
                }
            } else if !p.inputs.is_empty() {
                out.push_str("      inputs:\n");
                let mut keys: Vec<&String> = p.inputs.keys().collect();
                keys.sort();
                for k in keys {
                    out.push_str(&format!("        {}: {}\n", k, p.inputs[k]));
                }
            }
            if !p.config.is_null() {
                let cfg = serde_norway::to_string(&p.config).unwrap_or_default();
                let cfg_trimmed = cfg.trim();
                if !cfg_trimmed.is_empty() && cfg_trimmed != "null" {
                    out.push_str("      config:\n");
                    for line in cfg_trimmed.lines() {
                        out.push_str(&format!("        {}\n", line));
                    }
                }
            }
        }
    }
    out.push_str("---\n");
    out
}
```

The renderer needs to write string values that YAML will read back
unchanged. Rather than pull in a YAML emitter for a handful of scalar
fields, a small function decides between the plain form and the
double-quoted-with-escapes form. The `needs_quote` predicate is a
blocklist of the characters that make YAML re-interpret a plain scalar —
grungy, but the mirror of what a full YAML emitter would write for these
fields:

```rust {#yaml-scalar}
/// Serialize a string as a YAML scalar. Plain unquoted form when the value
/// is YAML-safe; double-quoted with escapes otherwise. Mirrors what a full
/// YAML emitter would produce for these fields, kept inline so the renderer
/// stays a small `format!`-style string builder.
pub fn yaml_scalar(s: &str) -> String {
    let needs_quote = s.is_empty()
        || s.starts_with(' ')
        || s.starts_with('-')
        || s.starts_with('?')
        || s.starts_with(':')
        || s.starts_with('!')
        || s.starts_with('&')
        || s.starts_with('*')
        || s.starts_with('[')
        || s.starts_with(']')
        || s.starts_with('{')
        || s.starts_with('}')
        || s.starts_with('|')
        || s.starts_with('>')
        || s.starts_with('@')
        || s.starts_with('`')
        || s.starts_with('\'')
        || s.starts_with('"')
        || s.starts_with('#')
        || s.contains('\n')
        || s.contains(": ")
        || s.contains(" #");
    if !needs_quote {
        return s.to_string();
    }
    let mut q = String::with_capacity(s.len() + 2);
    q.push('"');
    for c in s.chars() {
        match c {
            '\\' => q.push_str("\\\\"),
            '"' => q.push_str("\\\""),
            '\n' => q.push_str("\\n"),
            '\t' => q.push_str("\\t"),
            '\r' => q.push_str("\\r"),
            c => q.push(c),
        }
    }
    q.push('"');
    q
}
```

Note the asymmetry the two halves leave open: `parse_envelope` accepts
files the renderer would never produce (extra keys, human comments,
arbitrary field order), and `render_envelope` canonicalizes. A
parse-then-render round trip therefore *normalizes* a file — which is
exactly why the in-place save path in [`segmentation.md`](segmentation.md)
goes out of its way never to re-render the frontmatter of a file a human
authored.

## Tests

The tests pin the envelope's rules from both directions: what parses
(minimal, full, missing-optional, unknown-key tolerance), what rejects
(unknown type, missing format, malformed YAML, legacy flat frontmatter),
and that render-then-parse round-trips including the omit-when-default
behaviors.

```rust {#tests}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_minimal_envelope() {
        let content = r#"---
x0k:
  format: folio/v1
  id: x0k:design/example
  type: design
  status: proposed
---
Body here.
"#;
        let (env, body) = parse_envelope(content).expect("parse");
        assert_eq!(env.id, "x0k:design/example");
        assert_eq!(env.doc_type, DocType::Design);
        assert_eq!(env.status, Some(Status::Proposed));
        assert_eq!(env.subtype, None);
        assert!(env.edges.is_empty());
        assert!(env.materialization.is_none());
        assert_eq!(body.trim(), "Body here.");
    }

    #[test]
    fn parses_full_wiki_envelope() {
        let content = r#"---
x0k:
  format: folio/v1
  id: x0k:wiki/oracle
  type: wiki
  subtype: wiki:Methodology
  status: stable
  summary: A short summary.
  updated_by: agent
  concerns: [oracle, shaping]
  edges:
    cites:
      - x0k:wiki/related
    refines:
      - x0k:wiki/agent-pattern
---
# Oracle

Body prose.
"#;
        let (env, _body) = parse_envelope(content).expect("parse wiki envelope");
        assert_eq!(env.doc_type, DocType::Wiki);
        assert_eq!(env.subtype.as_deref(), Some("wiki:Methodology"));
        assert_eq!(env.status, Some(Status::Stable));
        assert_eq!(env.summary.as_deref(), Some("A short summary."));
        assert_eq!(env.updated_by.as_deref(), Some("agent"));
        assert_eq!(env.concerns, vec!["oracle", "shaping"]);
        assert_eq!(env.edges["cites"], vec!["x0k:wiki/related"]);
        assert_eq!(env.edges["refines"], vec!["x0k:wiki/agent-pattern"]);
    }

    #[test]
    fn missing_optional_status() {
        let content = r#"---
x0k:
  format: folio/v1
  id: x0k:wiki/no-status
  type: wiki
  subtype: wiki:Concept
---
# Title

body.
"#;
        let (env, _body) = parse_envelope(content).expect("parse without status");
        assert!(env.status.is_none());
    }

    #[test]
    fn ignores_unknown_authority_field() {
        // Legacy files may still carry a stray `authority:` key. The
        // parser silently drops it (serde-default ignores unknown wire
        // fields when there's no matching deserialize target).
        let content = r#"---
x0k:
  format: folio/v1
  id: x0k:design/legacy
  type: design
  status: proposed
  authority: file
---
body
"#;
        let (env, _) = parse_envelope(content).expect("legacy authority field tolerated");
        assert_eq!(env.id, "x0k:design/legacy");
        assert_eq!(env.status, Some(Status::Proposed));
    }

    #[test]
    fn rejects_unknown_type() {
        let content = r#"---
x0k:
  format: folio/v1
  id: x0k:design/example
  type: pamphlet
  status: proposed
---
"#;
        let err = parse_envelope(content).expect_err("unknown type must reject");
        assert!(matches!(err, FolioError::InvalidType { .. }));
    }

    #[test]
    fn rejects_missing_format() {
        let content = r#"---
x0k:
  id: x0k:design/example
  type: design
  status: proposed
---
"#;
        let err = parse_envelope(content).expect_err("missing format must reject");
        assert!(matches!(err, FolioError::MissingField { field: "format" }));
    }

    #[test]
    fn rejects_malformed_yaml() {
        let content = "---\nx0k:\n  format: folio/v1\n  id: : :\n---\nbody\n";
        let err = parse_envelope(content).expect_err("bad YAML must reject");
        assert!(matches!(err, FolioError::InvalidYaml(_)));
    }

    #[test]
    fn legacy_flat_frontmatter_is_not_colophon() {
        let content = "---\nstatus: Proposed\n---\nBody\n";
        assert!(!is_colophon(content));
        let err = parse_envelope(content).expect_err("legacy must not parse as v1");
        assert!(matches!(err, FolioError::NotColophon));
    }

    #[test]
    fn render_then_parse_roundtrips() {
        let mut edges = BTreeMap::new();
        edges.insert(
            "cites".to_string(),
            vec![
                "x0k:wiki/related".to_string(),
                "x0k:design/oracle".to_string(),
            ],
        );
        let env = Colophon {
            id: "x0k:wiki/sample".to_string(),
            doc_type: DocType::Wiki,
            subtype: Some("wiki:Methodology".to_string()),
            status: Some(Status::Stable),
            concerns: vec!["alpha".to_string(), "beta".to_string()],
            summary: Some("Concise summary.".to_string()),
            updated_by: Some("agent".to_string()),
            created_at: Some("2026-01-15T10:30:00.000000Z".to_string()),
            updated_at: Some("2026-05-06T12:00:00.000000Z".to_string()),
            edges,
            materialization: None,
            tangle: None,
            pipelines: vec![],
            body_format: BODY_FORMAT_MARKDOWN.to_string(),
        };
        let yaml = render_envelope(&env);
        let full = format!("{yaml}\nbody\n");
        let (parsed, _body) = parse_envelope(&full).expect("roundtrip parse");
        assert_eq!(parsed, env);
    }

    #[test]
    fn render_omits_empty_summary_and_updated_by() {
        let env = Colophon {
            id: "x0k:wiki/x".to_string(),
            doc_type: DocType::Wiki,
            subtype: Some("wiki:Concept".to_string()),
            status: Some(Status::Stable),
            concerns: vec![],
            summary: Some(String::new()),
            updated_by: Some(String::new()),
            created_at: None,
            updated_at: None,
            edges: BTreeMap::new(),
            materialization: None,
            tangle: None,
            pipelines: vec![],
            body_format: BODY_FORMAT_MARKDOWN.to_string(),
        };
        let yaml = render_envelope(&env);
        assert!(!yaml.contains("summary:"));
        assert!(!yaml.contains("updated_by:"));
        assert!(!yaml.contains("created_at:"));
        assert!(!yaml.contains("updated_at:"));
    }

    #[test]
    fn renders_and_parses_timestamps() {
        let env = Colophon {
            id: "x0k:wiki/timestamped".to_string(),
            doc_type: DocType::Wiki,
            subtype: Some("wiki:Concept".to_string()),
            status: Some(Status::Stable),
            concerns: vec![],
            summary: None,
            updated_by: None,
            created_at: Some("2026-01-15T10:30:00.000000Z".to_string()),
            updated_at: Some("2026-05-06T12:00:00.000000Z".to_string()),
            edges: BTreeMap::new(),
            materialization: None,
            tangle: None,
            pipelines: vec![],
            body_format: BODY_FORMAT_MARKDOWN.to_string(),
        };
        let yaml = render_envelope(&env);
        assert!(yaml.contains("created_at: 2026-01-15T10:30:00.000000Z"));
        assert!(yaml.contains("updated_at: 2026-05-06T12:00:00.000000Z"));
        let full = format!("{yaml}\nbody\n");
        let (parsed, _body) = parse_envelope(&full).expect("parse with timestamps");
        assert_eq!(
            parsed.created_at.as_deref(),
            Some("2026-01-15T10:30:00.000000Z")
        );
        assert_eq!(
            parsed.updated_at.as_deref(),
            Some("2026-05-06T12:00:00.000000Z")
        );
    }

    #[test]
    fn yaml_scalar_quotes_when_needed() {
        assert_eq!(yaml_scalar("simple"), "simple");
        assert_eq!(yaml_scalar(""), "\"\"");
        assert_eq!(
            yaml_scalar("Has: a colon, a # hash, and \"quotes\"."),
            "\"Has: a colon, a # hash, and \\\"quotes\\\".\""
        );
    }

    #[test]
    fn default_body_format_is_markdown_when_field_absent() {
        let content = r#"---
x0k:
  format: folio/v1
  id: x0k:design/legacy
  type: design
  status: proposed
---
body
"#;
        let (env, _) = parse_envelope(content).expect("parse legacy markdown");
        assert_eq!(env.body_format, BODY_FORMAT_MARKDOWN);
    }

    #[test]
    fn explicit_body_format_html_parses() {
        let content = r#"---
x0k:
  format: folio/v1
  id: x0k:design/html-decision
  type: design
  status: proposed
  body_format: html
---
<p>Hello.</p>
"#;
        let (env, body) = parse_envelope(content).expect("parse html-bodied");
        assert_eq!(env.body_format, BODY_FORMAT_HTML);
        assert!(body.contains("<p>Hello.</p>"));
    }

    #[test]
    fn unknown_body_format_warns_and_falls_back_to_markdown() {
        // Unknown values must not error — forward-compat per the
        // `unknown_edges` pattern. The fallback is markdown.
        let content = r#"---
x0k:
  format: folio/v1
  id: x0k:design/forward-compat
  type: design
  status: proposed
  body_format: yaml
---
body
"#;
        let (env, _) = parse_envelope(content).expect("unknown body_format must not error");
        assert_eq!(env.body_format, BODY_FORMAT_MARKDOWN);
    }

    #[test]
    fn render_emits_body_format_only_when_non_default() {
        let mut env = Colophon {
            id: "x0k:design/x".to_string(),
            doc_type: DocType::Design,
            subtype: None,
            status: Some(Status::Proposed),
            concerns: vec![],
            summary: None,
            updated_by: None,
            created_at: None,
            updated_at: None,
            edges: BTreeMap::new(),
            materialization: None,
            tangle: None,
            pipelines: vec![],
            body_format: BODY_FORMAT_MARKDOWN.to_string(),
        };
        let md_yaml = render_envelope(&env);
        assert!(
            !md_yaml.contains("body_format"),
            "default markdown body_format must be omitted from render, got:\n{md_yaml}"
        );

        env.body_format = BODY_FORMAT_HTML.to_string();
        let html_yaml = render_envelope(&env);
        assert!(
            html_yaml.contains("body_format: html"),
            "non-default html body_format must be rendered, got:\n{html_yaml}"
        );
    }

    #[test]
    fn render_then_parse_roundtrip_preserves_html_body_format() {
        let env = Colophon {
            id: "x0k:design/html-roundtrip".to_string(),
            doc_type: DocType::Design,
            subtype: None,
            status: Some(Status::Proposed),
            concerns: vec![],
            summary: None,
            updated_by: None,
            created_at: None,
            updated_at: None,
            edges: BTreeMap::new(),
            materialization: None,
            tangle: None,
            pipelines: vec![],
            body_format: BODY_FORMAT_HTML.to_string(),
        };
        let yaml = render_envelope(&env);
        let full = format!("{yaml}\n<p>hi</p>\n");
        let (parsed, _body) = parse_envelope(&full).expect("roundtrip parse");
        assert_eq!(parsed.body_format, BODY_FORMAT_HTML);
        assert_eq!(parsed, env);
    }

    #[test]
    fn parses_materialization_block() {
        let content = r#"---
x0k:
  format: folio/v1
  id: x0k:design/x
  type: design
  status: proposed
  materialization:
    loro_doc_id: doc-123
    document_revision_id: rev-456
    content_hash: hash-789
---
body
"#;
        let (env, _) = parse_envelope(content).expect("parse mat");
        let m = env.materialization.expect("materialization present");
        assert_eq!(m.loro_doc_id.as_deref(), Some("doc-123"));
        assert_eq!(m.document_revision_id.as_deref(), Some("rev-456"));
        assert_eq!(m.content_hash.as_deref(), Some("hash-789"));
    }

    #[test]
    fn parses_pipeline_shorthand_input() {
        let content = r#"---
x0k:
  format: folio/v1
  id: x0k:design/themes/pansophia
  type: design
  status: proposed
  pipelines:
    - kind: theme-codegen
      input: tokens
      config:
        name: pansophia
        scheme: single
---
body
"#;
        let (env, _) = parse_envelope(content).expect("parse pipeline shorthand");
        assert_eq!(env.pipelines.len(), 1);
        let p = &env.pipelines[0];
        assert_eq!(p.kind, "theme-codegen");
        // Shorthand `input:` normalizes to `inputs.default`.
        assert_eq!(p.inputs.get("default").map(|s| s.as_str()), Some("tokens"));
        assert_eq!(
            p.config.get("name").and_then(|v| v.as_str()),
            Some("pansophia")
        );
        assert_eq!(
            p.config.get("scheme").and_then(|v| v.as_str()),
            Some("single")
        );
    }

    #[test]
    fn parses_pipeline_long_form_inputs_map() {
        let content = r#"---
x0k:
  format: folio/v1
  id: x0k:design/multi
  type: design
  status: proposed
  pipelines:
    - kind: theme-codegen
      inputs:
        tokens: tokens-chunk
      config:
        name: business
        scheme: dual
---
body
"#;
        let (env, _) = parse_envelope(content).expect("parse pipeline long form");
        let p = &env.pipelines[0];
        assert_eq!(
            p.inputs.get("tokens").map(|s| s.as_str()),
            Some("tokens-chunk")
        );
    }

    #[test]
    fn omits_pipelines_when_envelope_has_none() {
        let env = Colophon {
            id: "x0k:design/x".to_string(),
            doc_type: DocType::Design,
            subtype: None,
            status: Some(Status::Proposed),
            concerns: vec![],
            summary: None,
            updated_by: None,
            created_at: None,
            updated_at: None,
            edges: BTreeMap::new(),
            materialization: None,
            tangle: None,
            pipelines: vec![],
            body_format: BODY_FORMAT_MARKDOWN.to_string(),
        };
        let yaml = render_envelope(&env);
        assert!(!yaml.contains("pipelines:"));
    }
}
```

## Composing the module

```rust {#root}
<<module-doc>>

<<format-tokens>>

<<normalize-body-format>>

<<doc-type>>

<<doc-type-strings>>

<<status>>

<<materialization>>

<<tangle-config>>

<<pipeline-decl>>

<<folio-error>>

<<colophon-type>>

<<wire-shapes>>

<<split-frontmatter>>

<<is-colophon>>

<<parse-envelope>>

<<yaml-to-json>>

<<render-envelope>>

<<yaml-scalar>>

<<tests>>
```

What this module leaves genuinely open: the renderer and parser are not
inverses over the space of *files*, only over the space of *envelopes* —
a fact every caller that holds a human-authored file must respect by
splicing bodies rather than re-rendering (the
`body_swap_preserves_frontmatter_verbatim` test in
[`segmentation.md`](segmentation.md) pins the technique). And the closed
`DocType` set means adding a genus is a code change in a public library;
that is the point — a genus is an ontology commitment, not a string.
