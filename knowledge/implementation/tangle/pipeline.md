---
x0k:
  format: folio/v1
  id: x0k:implementation/tangle/pipeline
  type: implementation
  status: draft
  summary: The plugin contract — the trait, the typed input and output surfaces, the error shape, the registry — pure data and no I/O, which is what lets identity tangling be one plugin among others.
  concerns: [tangle, literate, pipelines, plugin, protocol]
  tangle:
    crate: x0k-tangle
    root: src/pipeline.rs
  edges:
    cites:
      - x0k:implementation/tangle/protocol
      - x0k:implementation/tangle/identity-pipeline
      - x0k:implementation/tangle/dispatcher
      - x0k:implementation/tangle/chunk
    realizes:
      - x0k:design/literate-pipelines
    presupposes:
      - x0k:wiki/literate-programming
---
# The pipeline protocol

This module defines the *contract* between tangle and its plugins —
the `TanglePipeline` trait, the typed I/O surfaces
(`PipelineContext`, `PipelineOutput`), the typed error shape, and the
`PipelineRegistry` that holds the plugin set. Everything in here is
pure data + trait; the dispatch loop that runs plugins lives in
[`pipeline_runner`](dispatcher.md).

Two principles shape the module:

- **The plugin is pure.** It takes inputs + config in, returns
  artifacts out. It never reads files, never writes files, never holds
  global state. Tangle owns I/O.
- **Identity tangling is just another plugin.** The `IdentityPipeline`
  defined in [`identity-pipeline`](identity-pipeline.md) implements
  the same trait and lives in `PipelineRegistry::default()`. There's
  no special-case code path for the `tangle:` frontmatter block — it
  becomes a synthetic `PipelineDecl` before dispatch.

## Module header

The module-level doc-comment summarizes the protocol and points at
the upstream design contract. The frontmatter example doubles as a
spec for human readers — anyone debugging pipeline declarations can
grep `pipelines:` in a literate doc and read this comment to know the
shape.

```rust {#module-header}
//! Pipeline protocol: codegen plugins that consume named chunks from a
//! literate document and produce transformed outputs.
//!
//! See `decisions/design/corpus/literate-pipelines.md` for the design contract.
//!
//! ## Plugin contract
//!
//! A pipeline plugin implements [`TanglePipeline`]. It is pure: same
//! inputs + config produce the same outputs. The plugin never touches the
//! filesystem — it returns [`PipelineOutput`]s describing what should be
//! written, and tangle is responsible for atomic write, hash recording,
//! and the `@generated` header.
//!
//! ## Registry
//!
//! Plugins register against a [`PipelineRegistry`]. The library has no
//! plugin dependencies; the `x0k-tangle` binary wires the registry by
//! calling each first-party plugin's `register()` function. External
//! consumers (build scripts) build their own registry with only the
//! plugins they need.
//!
//! ## Frontmatter shape
//!
//! See [`x0k_folio::colophon::PipelineDecl`] for the
//! frontmatter wire format. Pipelines are declared inside the `x0k:`
//! envelope under `pipelines:`.
//!
//! ```yaml
//! x0k:
//!   pipelines:
//!     - kind: theme-codegen
//!       input: tokens                   # shorthand; long form is `inputs: { ... }`
//!       config:
//!         name: pansophia
//!         scheme: single
//! ```
```

## Imports

```rust {#imports}
use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
```

## The TanglePipeline trait

The trait surface is deliberately small: one identifier (`kind`), one
transformer (`transform`), and one discovery hook (`literate_roots`).
Three methods, no associated types, no lifetimes beyond the borrow on
`PipelineContext`. Plugins can be added without coordinating shape
changes across the substrate.

```rust {#trait-def}
/// A pipeline transformer. The plugin takes resolved chunks + config
/// and returns the artifacts to write. The plugin must be pure (no
/// filesystem access, no global state); tangle owns I/O.
pub trait TanglePipeline: Send + Sync {
    /// Unique identifier — matches the `kind:` field in
    /// frontmatter pipeline declarations.
    fn kind(&self) -> &str;

    /// Transform the inputs to outputs. Called with the resolved
    /// chunk content (already passed through `<<refs>>` expansion).
    fn transform(&self, ctx: &PipelineContext) -> Result<Vec<PipelineOutput>, PipelineError>;

    /// Conventional literate-source roots this plugin expects, expressed as
    /// workspace-relative path strings. Aggregated by
    /// [`crate::pipeline_runner::tangle_workspace`] into the discovery set
    /// it walks. Pipelines that don't claim roots (e.g., a pipeline only
    /// invoked programmatically) return an empty vec — the default.
    fn literate_roots(&self) -> Vec<&'static str> {
        Vec::new()
    }
}
```

The `literate_roots` default is empty because not every plugin owns a
directory in the workspace — a build script could register a plugin
just to drive `tangle_document` on specific paths, never participating
in workspace discovery. First-party plugins like
[`IdentityPipeline`](identity-pipeline.md) and `ThemeCodegenPlugin`
override it to claim `knowledge/implementation` and `decisions/design/themes`.

## PipelineContext

The plugin reads everything it needs from `PipelineContext`. The
fields are organized by what the plugin uses them for:

- **`source_path`, `workspace_root`** — for diagnostics and joining
  relative output paths.
- **`config`** — untyped JSON; the plugin deserializes into its own
  typed shape. Untyped on the boundary so the trait stays
  config-shape-agnostic.
- **`inputs`** — already-resolved chunk content keyed by the
  parameter name from the frontmatter `inputs:` map.
- **`all_chunks`, `chunk_order`** — for plugins that need the full
  doc graph (identity tangling walks the whole graph to find
  top-level chunks; theme-codegen ignores these).

```rust {#pipeline-context}
/// Context passed to [`TanglePipeline::transform`]. Carries the
/// source-doc location (for diagnostics), workspace root (for
/// resolving outputs), the plugin's config payload, and the
/// resolved input chunks.
pub struct PipelineContext<'a> {
    /// Absolute path to the source `.md` document the pipeline runs
    /// against.
    pub source_path: &'a Path,
    /// Workspace root. Plugins emit workspace-relative paths in
    /// [`PipelineOutput::path`]; tangle joins them against this when
    /// writing.
    pub workspace_root: &'a Path,
    /// The pipeline's `config:` block from the frontmatter, captured
    /// as untyped JSON. Plugins deserialize this into their own typed
    /// shape via `serde_json`.
    pub config: &'a serde_json::Value,
    /// Resolved chunk inputs, keyed by the plugin-parameter name
    /// declared in the frontmatter `inputs:` map. For single-input
    /// plugins, the shorthand `input: <name>` normalizes to the
    /// canonical `"default"` key.
    pub inputs: &'a HashMap<String, ChunkInput>,
    /// The document's full parsed chunk graph, keyed by chunk name
    /// with one entry per language variant. Identity-tangle and any
    /// other pipeline that needs to walk the doc beyond the declared
    /// `inputs:` (e.g., to resolve `<<refs>>` across the whole graph)
    /// reads through this. Plugins that work off `inputs` alone may
    /// ignore it.
    pub all_chunks: &'a HashMap<String, Vec<crate::chunk::Chunk>>,
    /// Chunk names in declaration order — the order they appear in
    /// the source `.md`. Identity tangling consumes this so the
    /// emitted output's chunk concatenation matches the doc's
    /// authoring order (not alphabetical). Plugins that don't care
    /// about order may ignore it.
    pub chunk_order: &'a [String],
}
```

## ChunkInput and ChunkVariant

A resolved input is a vec of language variants — the same chunk
`#name` can have a Rust fence and a TypeScript fence, and a plugin
that targets one language picks the right variant. For single-language
chunks (the common case), `variants` has length 1.

```rust {#chunk-input}
/// One resolved input. A chunk may have multiple language variants
/// (e.g., one rust + one typescript fence with the same `#name`); the
/// pipeline picks which variants it consumes.
#[derive(Debug, Clone)]
pub struct ChunkInput {
    /// All variants of this chunk by source-document fence-language.
    pub variants: Vec<ChunkVariant>,
}

/// One language variant of a chunk. Content has already been passed
/// through `<<chunk-ref>>` expansion, so the pipeline gets the same
/// final string identity-tangle would write.
#[derive(Debug, Clone)]
pub struct ChunkVariant {
    /// Fence info-string language token, e.g. `"toml"`, `"rust"`.
    /// Empty string when the source fence had no language.
    pub lang: String,
    /// Final expanded content. Multi-body chunks (multiple fences
    /// with the same `#name`) are already concatenated.
    pub content: String,
}
```

The "already expanded" guarantee matters: plugins don't need to know
about `<<refs>>`. By the time tangle hands them a `ChunkVariant`, the
content is what would land in the output file if identity tangling
were running. Plugins transform that string further (theme-codegen
parses it as TOML, service-codegen as something else); the
literate-substrate concerns are factored out cleanly.

## Comment styles for the generated header

Plugins declare what comment style fits their output file — line
comments for `.rs`, `.ts`, `.py`; block comments for `.css`, `.html`;
none for binary or content-defining files. Tangle uses this to format
the `@generated by x0k-tangle ...` header it prepends.

```rust {#comment-style}
/// Style of `@generated` comment header tangle prepends to the
/// output. `None` means the plugin's content is written verbatim —
/// useful for binary outputs, JSON manifests, or files whose syntax
/// can't host comments.
#[derive(Debug, Clone, Copy)]
pub enum CommentStyle {
    /// Line-comment marker (`"//"`, `"#"`, `";"`).
    Line(&'static str),
    /// Block-comment open/close pair (`("/*", "*/")`).
    Block(&'static str, &'static str),
}
```

The `&'static str` slots mean comment markers are compile-time
constants — no allocation, no possibility of a plugin accidentally
constructing an invalid comment shape at runtime.

## PipelineOutput

What the plugin returns. One per file the dispatcher should write. The
plugin gives a workspace-relative path (or absolute, when the plugin
has a strong opinion); the content bytes; and the requested header
style.

```rust {#pipeline-output}
/// One output file the pipeline wants tangle to write.
pub struct PipelineOutput {
    /// Workspace-relative (or absolute) path. Relative paths are
    /// joined against [`PipelineContext::workspace_root`].
    pub path: PathBuf,
    /// File body. Tangle prepends the `@generated` header when
    /// `header_comment_style` is `Some`, then writes verbatim.
    pub content: Vec<u8>,
    /// How to format the `@generated` header. `None` skips it
    /// (binary outputs, content-defining files).
    pub header_comment_style: Option<CommentStyle>,
}
```

## Typed errors

`PipelineError` carries a kind discriminator + a human message. The
kinds map onto distinct operator concerns:

- **`MissingInput`** — the doc didn't declare a chunk the plugin
  expected; surface this as "your `inputs:` map is incomplete."
- **`InvalidConfig`** — the `config:` block didn't deserialize;
  surface as "your `config:` shape doesn't match what this plugin
  expects."
- **`TransformFailed`** — generic plugin-side failure (bad TOML, bad
  symbol, etc.); the message carries the detail.

The display impl prepends the kind so operators see the category
without parsing message text.

```rust {#error-types}
/// Pipeline-side failure. Tangle promotes this into the top-level
/// `TangleError` so the operator sees one error stream regardless of
/// which side failed.
#[derive(Debug)]
pub struct PipelineError {
    pub kind: PipelineErrorKind,
    pub message: String,
}

#[derive(Debug)]
pub enum PipelineErrorKind {
    /// The `inputs:` map referenced a chunk by name, but the
    /// document didn't define one (and tangle reported it as
    /// missing).
    MissingInput { name: String },
    /// The `config:` payload didn't deserialize into the plugin's
    /// expected shape.
    InvalidConfig,
    /// Any other plugin-side failure — bad TOML, unknown role,
    /// etc. The `message` carries the human-readable detail.
    TransformFailed,
}

impl fmt::Display for PipelineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            PipelineErrorKind::MissingInput { name } => {
                write!(f, "missing input chunk `{name}`: {}", self.message)
            }
            PipelineErrorKind::InvalidConfig => {
                write!(f, "invalid pipeline config: {}", self.message)
            }
            PipelineErrorKind::TransformFailed => {
                write!(f, "pipeline transform failed: {}", self.message)
            }
        }
    }
}

impl std::error::Error for PipelineError {}

impl PipelineError {
    pub fn missing_input(name: impl Into<String>) -> Self {
        let n = name.into();
        Self {
            message: format!("expected `{n}` to be declared in `inputs:`"),
            kind: PipelineErrorKind::MissingInput { name: n },
        }
    }

    pub fn invalid_config(message: impl Into<String>) -> Self {
        Self {
            kind: PipelineErrorKind::InvalidConfig,
            message: message.into(),
        }
    }

    pub fn transform_failed(message: impl Into<String>) -> Self {
        Self {
            kind: PipelineErrorKind::TransformFailed,
            message: message.into(),
        }
    }
}
```

The three constructor functions reduce ceremony: plugins call
`PipelineError::missing_input("tokens")` rather than building the
struct literal. The dispatcher promotes whatever the plugin returns
into anyhow's error chain.

## The registry

`PipelineRegistry` is a `HashMap<String, Arc<dyn TanglePipeline>>`
behind a tiny API. The Arc lets the dispatcher clone plugin handles
into worker contexts cheaply; the trait object lets the registry hold
heterogeneous plugins under one type.

The `Default` impl is the load-bearing convenience: a fresh
`PipelineRegistry::default()` already has identity tangling
registered. That guarantee means *every* registry can handle
identity-tangle docs; downstream bundles (`x0k-tangle-bundle`) add
plugins on top.

```rust {#registry}
/// Registry of pipeline plugins.
///
/// The library ships a default registry that already contains
/// [`crate::IdentityPipeline`] — every registry has identity tangling
/// for free, because identity-tangle is itself a plugin in the unified
/// dispatch model. First-party plugins (theme-codegen, future
/// service-codegen, ontology vocab) are registered on top by the
/// `x0k-tangle-bundle` crate's `default_registry()` factory. Build
/// scripts that want only a subset construct their own registry via
/// [`Self::empty`] or [`Self::new`] + selective `register()` calls.
#[derive(Clone)]
pub struct PipelineRegistry {
    pipelines: HashMap<String, Arc<dyn TanglePipeline>>,
}

impl Default for PipelineRegistry {
    fn default() -> Self {
        let mut registry = Self::empty();
        registry.register(crate::identity_pipeline::IdentityPipeline);
        registry
    }
}

impl PipelineRegistry {
    /// Alias for [`Self::default`] — every default registry already
    /// has identity tangling registered.
    pub fn new() -> Self {
        Self::default()
    }

    /// Build an empty registry with no plugins registered, not even
    /// identity. Reserved for tests and tooling that explicitly want a
    /// plugin-less surface.
    pub fn empty() -> Self {
        Self {
            pipelines: HashMap::new(),
        }
    }

    /// Register a plugin instance. Later registrations under the
    /// same `kind()` overwrite earlier ones (consumers wiring two
    /// crates that both export the same kind get the second).
    pub fn register<P: TanglePipeline + 'static>(&mut self, plugin: P) {
        let kind = plugin.kind().to_string();
        self.pipelines.insert(kind, Arc::new(plugin));
    }

    /// Look up a registered plugin by its `kind` string.
    pub fn get(&self, kind: &str) -> Option<&Arc<dyn TanglePipeline>> {
        self.pipelines.get(kind)
    }

    /// Iterate all registered kinds — useful for diagnostics and
    /// CLI help.
    pub fn kinds(&self) -> impl Iterator<Item = &str> {
        self.pipelines.keys().map(|s| s.as_str())
    }

    /// Iterate the registered plugin instances. Used by the workspace
    /// dispatcher to aggregate [`TanglePipeline::literate_roots`] into
    /// the directory-discovery set.
    pub fn iter(&self) -> impl Iterator<Item = &Arc<dyn TanglePipeline>> {
        self.pipelines.values()
    }

    /// Number of registered plugins.
    pub fn len(&self) -> usize {
        self.pipelines.len()
    }

    /// Whether the registry has any plugins.
    pub fn is_empty(&self) -> bool {
        self.pipelines.is_empty()
    }
}
```

`register` overwriting on duplicate `kind` is intentional: if two
plugin crates accidentally claim the same `kind` (or if a consumer
deliberately wants to swap an implementation), the second
registration wins. Tests cover that the count stays at 1.

## Tests

The pipeline tests exercise the registry surface itself. The fixture
is a tiny in-test `MirrorPipeline` that exists solely to drive
`transform` end-to-end without pulling in the real codegens.
`MirrorPipeline` exists alongside [`IdentityPipeline`](identity-pipeline.md);
they have distinct `kind()` values so the default registry's identity
slot stays untouched in tests that register the mirror.

`````rust {#tests}
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// A pipeline that mirrors its inputs back as outputs, named
    /// after the input. Useful as a fixture for end-to-end
    /// dispatcher tests. Named `MirrorPipeline` to avoid clashing
    /// with the real [`crate::IdentityPipeline`] now that identity
    /// tangling is itself a plugin.
    struct MirrorPipeline;

    impl TanglePipeline for MirrorPipeline {
        fn kind(&self) -> &str {
            "mirror"
        }

        fn transform(&self, ctx: &PipelineContext) -> Result<Vec<PipelineOutput>, PipelineError> {
            let mut outputs = Vec::new();
            let mut keys: Vec<&String> = ctx.inputs.keys().collect();
            keys.sort();
            for key in keys {
                let input = ctx
                    .inputs
                    .get(key)
                    .ok_or_else(|| PipelineError::missing_input(key.clone()))?;
                let body = input
                    .variants
                    .first()
                    .map(|v| v.content.clone())
                    .unwrap_or_default();
                outputs.push(PipelineOutput {
                    path: PathBuf::from(format!("out/{key}.txt")),
                    content: body.into_bytes(),
                    header_comment_style: Some(CommentStyle::Line("#")),
                });
            }
            Ok(outputs)
        }
    }

    #[test]
    fn register_and_get_round_trips() {
        // `empty()` (NOT `default()`) so this test sees a blank
        // registry — `default()` ships with `IdentityPipeline`
        // pre-registered.
        let mut registry = PipelineRegistry::empty();
        assert!(registry.is_empty());
        registry.register(MirrorPipeline);
        assert_eq!(registry.len(), 1);
        let p = registry.get("mirror").expect("mirror present");
        assert_eq!(p.kind(), "mirror");
        assert!(registry.get("missing-kind").is_none());
    }

    #[test]
    fn default_registry_has_identity_pipeline() {
        let registry = PipelineRegistry::default();
        assert!(!registry.is_empty(), "default registry should have identity");
        let p = registry
            .get(crate::IDENTITY_KIND)
            .expect("identity-tangle present in default registry");
        assert_eq!(p.kind(), crate::IDENTITY_KIND);
    }

    #[test]
    fn mirror_pipeline_returns_inputs_as_outputs() {
        let pipeline = MirrorPipeline;
        let mut inputs: HashMap<String, ChunkInput> = HashMap::new();
        inputs.insert(
            "default".to_string(),
            ChunkInput {
                variants: vec![ChunkVariant {
                    lang: "toml".to_string(),
                    content: "[theme]\nid = \"test\"\n".to_string(),
                }],
            },
        );
        let workspace = PathBuf::from("/tmp/ws");
        let source = workspace.join("doc.md");
        let cfg = serde_json::json!({});
        let empty_chunks: HashMap<String, Vec<crate::chunk::Chunk>> = HashMap::new();
        let empty_order: Vec<String> = Vec::new();
        let ctx = PipelineContext {
            source_path: &source,
            workspace_root: &workspace,
            config: &cfg,
            inputs: &inputs,
            all_chunks: &empty_chunks,
            chunk_order: &empty_order,
        };
        let outs = pipeline.transform(&ctx).expect("transform");
        assert_eq!(outs.len(), 1);
        assert_eq!(outs[0].path, PathBuf::from("out/default.txt"));
        let body = std::str::from_utf8(&outs[0].content).unwrap();
        assert!(body.contains("[theme]"));
        assert!(matches!(
            outs[0].header_comment_style,
            Some(CommentStyle::Line("#"))
        ));
    }

    #[test]
    fn registry_overwrites_on_duplicate_kind() {
        struct OtherMirror;
        impl TanglePipeline for OtherMirror {
            fn kind(&self) -> &str {
                "mirror"
            }
            fn transform(
                &self,
                _ctx: &PipelineContext,
            ) -> Result<Vec<PipelineOutput>, PipelineError> {
                Ok(vec![])
            }
        }
        let mut registry = PipelineRegistry::empty();
        registry.register(MirrorPipeline);
        registry.register(OtherMirror);
        // Still one entry; the second registration overwrote the first.
        assert_eq!(registry.len(), 1);
    }
}
`````

## Composing the module

```rust {#root}
<<module-header>>

<<imports>>

<<trait-def>>

<<pipeline-context>>

<<chunk-input>>

<<comment-style>>

<<pipeline-output>>

<<error-types>>

<<registry>>

<<tests>>
```
