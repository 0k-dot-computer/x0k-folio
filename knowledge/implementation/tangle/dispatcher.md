---
x0k:
  format: folio/v1
  id: x0k:implementation/tangle/dispatcher
  type: implementation
  status: draft
  summary: The three entry points — one document, a directory, the whole workspace — and the loop between them that resolves inputs, runs each declared pipeline, writes outputs and records the sidecar.
  concerns: [tangle, literate, dispatch, sidecar, collision, freshness]
  tangle:
    crate: x0k-tangle
    root: src/pipeline_runner.rs
  edges:
    motivated_by:
      - x0k:intent/7bd8cc80-8d2e-4b26-b760-3445f5731beb
    cites:
      - x0k:implementation/tangle/protocol
      - x0k:implementation/tangle/pipeline
      - x0k:implementation/tangle/identity-pipeline
---
# The pipeline dispatcher

This module wires every other piece — the parser, the chunk resolver,
the registry, the identity plugin, the typed I/O — into the three
user-facing entry points:

- `tangle_document` — process one doc. Synthesizes a `PipelineDecl`
  for the `tangle:` block, runs every declared pipeline, writes
  outputs + a sidecar.
- `tangle_directory` — walk a directory, call `tangle_document` on
  each doc that opts into the protocol.
- `tangle_workspace` — walk every literate root the registry's
  plugins claim via `literate_roots()`, re-tangle dirty docs, surface
  output-path collisions and per-doc errors.

Everything else in the module is in service of these three.

## Module header

```rust {#module-header}
//! Pipeline dispatch + sidecar writing.
//!
//! This module wires the parser, the chunk resolver, and the
//! [`PipelineRegistry`] into the user-facing entry points:
//!
//! - [`tangle_document`] — process one `.md` doc by running every
//!   declared pipeline. A `tangle:` frontmatter block is synthesized
//!   into a virtual [`PipelineDecl`] with kind
//!   [`crate::IDENTITY_KIND`] before dispatch, so identity tangling
//!   rides the same code path as every other plugin.
//! - [`tangle_directory`] — walk a directory, run [`tangle_document`] on
//!   each folio/v1 file.
//! - [`tangle_workspace`] — walk every literate root contributed by the
//!   registry's plugins (via [`crate::TanglePipeline::literate_roots`])
//!   and re-tangle dirty docs.
//!
//! ## Sidecar shape
//!
//! One sidecar per source `.md` document, always written next to the
//! source as `<stem>.tangle-map.json`. The sidecar carries one
//! `pipelines: []` array with one entry per pipeline run (including
//! the synthetic identity-tangle one). Each entry records the
//! pipeline `kind`, its `input_hashes` (per declared input chunk),
//! the `config_hash`, and the produced `outputs` (path + hash).
//!
//! Wave 2 dropped the legacy top-level `output_hash` + `chunks` fields
//! and unified the sidecar location for identity-tangle docs and
//! pipeline-only docs.
//!
//! ## Output-path collisions
//!
//! [`tangle_workspace`] tracks the source doc for every output path it
//! writes during a single workspace pass. If a second doc declares the
//! same output, the second doc lands in [`WorkspaceTangleReport::errored`]
//! with an output-path collision diagnostic naming both sources. The
//! first writer keeps its outputs; processing continues.
//!
//! See `decisions/design/corpus/literate-pipelines.md` for the full spec.
```

## Imports

```rust {#imports}
use std::cell::Cell;
use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeSet, HashMap};
use std::fs::File;
use std::hash::{Hash, Hasher};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard};

use anyhow::{anyhow, Context, Result};
use tracing::warn;
use x0k_folio::colophon::{parse_envelope, DocType, PipelineDecl};
use serde::{Deserialize, Serialize};

use crate::identity_pipeline::IDENTITY_KIND;
use crate::parser::parse_document;
use crate::pipeline::{
    ChunkInput, ChunkVariant, CommentStyle, PipelineContext, PipelineError, PipelineOutput,
    PipelineRegistry,
};
use crate::resolve::expand_chunk;
```

## Sidecar shape

The sidecar is the on-disk record of "what tangle did to this doc."
It lives next to the source `.md`, contains the source hash plus one
entry per pipeline run. Each entry records what the plugin received
and produced; the dispatcher uses these hashes on the next run to
decide whether the doc is still up-to-date.

```rust {#sidecar-types}
/// One output written by a pipeline run, with its workspace-relative
/// path and content hash. Surfaced in the sidecar.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PipelineOutputRecord {
    pub path: String,
    pub hash: String,
}

/// Sidecar entry for one pipeline invocation: which kind ran, the
/// hashes of its inputs and config (so `tangle check` can detect
/// drift), and the outputs it produced.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PipelineSidecarEntry {
    pub kind: String,
    pub input_hashes: std::collections::BTreeMap<String, String>,
    pub config_hash: String,
    pub outputs: Vec<PipelineOutputRecord>,
}

/// Unified sidecar shape. One file per source `.md` doc, lives next
/// to the source. Carries one entry in `pipelines` for every pipeline
/// run, including the synthetic identity-tangle one.
#[derive(Debug, Serialize, Deserialize)]
pub struct TangleSidecar {
    /// Workspace-relative path to the source `.md`.
    pub source: String,
    pub source_hash: String,
    #[serde(default)]
    pub pipelines: Vec<PipelineSidecarEntry>,
}
```

`input_hashes` is a `BTreeMap` rather than `HashMap` so JSON output
has stable ordering. Without that, the sidecar would churn on every
run as HashMap insertion order shifted.

`source` is workspace-relative for the same reason `outputs[].path`
always was: a sidecar describes a *document*, not a machine. It used
to hold an absolute path, and that one field was enough to make the
file non-portable. Two consequences followed. A tangle run from a
second checkout of the same corpus (its VCS, jj, colocated with git,
keeps several working copies of one repository) rewrote every sidecar
it touched — same content, different prefix — so a checkout that
changed nothing still produced a dirty tree. And a checkout whose
`target/` was seeded from another inherited sidecars naming that other
tree, so anything reading them reasoned about the wrong one.

Nothing keys freshness on `source` — `source_hash` and the output
records do that work — so a sidecar still carrying an absolute path
is read exactly as before and stays correct. It converts the next
time its doc is tangled. The corpus therefore migrates gradually,
and mixed absolute/relative sidecars are a valid intermediate state
rather than a broken one. Converting all ~460 at once is a single
`x0k-tangle workspace` pass with the sidecars deleted first; it
touches every sidecar in the repo and should be landed as its own
described commit, not folded into unrelated work.

## Run-result types

What `tangle_document` returns. Distinct types from `PipelineOutput`
(the plugin's return) and `PipelineOutputRecord` (the sidecar's
serializable form) because each has different concerns: the plugin
returns content + path + style, the sidecar records path + hash, and
the run-result needs the composed-with-header bytes for callers that
want to inspect the writes.

```rust {#run-result-types}
/// Outputs surfaced from running one pipeline. Distinct from
/// [`PipelineOutput`] (the plugin's return type) — this carries the
/// already-composed final bytes (with @generated header).
#[derive(Debug)]
pub struct PipelineRunOutput {
    pub path: PathBuf,
    pub content: Vec<u8>,
    /// Pipeline kind that produced this output. The synthetic
    /// identity run carries [`crate::IDENTITY_KIND`].
    pub kind: String,
}

/// Final result of [`tangle_document`]. Identity-tangle outputs and
/// declared-pipeline outputs are both surfaced in
/// [`Self::pipeline_outputs`] — identity outputs are identifiable by
/// their `kind == crate::IDENTITY_KIND`. For ergonomic access to just
/// the identity bucket, [`Self::identity_outputs`] is a projection.
#[derive(Debug)]
pub struct TangleResult {
    /// Path to the source `.md` document.
    pub source_path: PathBuf,
    /// Convenience projection: pipeline outputs whose kind is
    /// `identity-tangle`. Computed from [`Self::pipeline_outputs`].
    pub identity_outputs: Vec<IdentityOutputRecord>,
    /// Outputs from every pipeline run (including the synthetic
    /// identity-tangle one), in declaration order.
    pub pipeline_outputs: Vec<PipelineRunOutput>,
    /// Sidecar files written. One per doc.
    pub sidecars_written: Vec<PathBuf>,
}

#[derive(Debug)]
pub struct IdentityOutputRecord {
    pub path: PathBuf,
    pub content: String,
}
```

## Where the sidecar lives

```rust {#sidecar-path}
/// Compute the canonical sidecar path for a source `.md` doc.
///
/// Always lives next to source as `<stem>.tangle-map.json`. No more
/// output-adjacent sidecars; one location, single source of truth.
fn sidecar_path(source_doc: &Path) -> PathBuf {
    let mut p = source_doc.to_path_buf();
    p.set_extension("tangle-map.json");
    p
}
```

The pre-Wave-2 protocol used to put identity-tangle sidecars next to
the output file and pipeline sidecars next to the source. That split
made the freshness check awkward (which sidecar do you read?). Wave 2
unified on "next to source, always" — readers always know where to
look, writers always know where to put it.

## One tangler at a time

Tangling is a whole-workspace mutation: ~460 generated files rewritten
from their sources. The workspace has six independent callers that can
each start one — the dev daemon's watcher, the `tangle_workspace` MCP
tool, the vite plugin and its binary fallback, a `package.json`
prebuild step, and `x0k-ui-widgets`' `build.rs`. Nothing coordinates
them. Two overlapping runs interleave writes to the same output paths,
and a `cargo build` reading those paths mid-run sees whatever state the
loser left behind.

The coordination has to be a *file* lock, not a `Mutex`: the six
callers live in at least three separate OS processes, so no in-process
primitive can see them all.

Where the lock file lives took a correction. It was
`<workspace_root>/target/.tangle.lock`, on the reasoning that `target/`
is already the workspace's scratch space, already `.gitignore`d, and
already created by anything that builds. All three are true of a cargo
workspace and none is true of the *other* place this tangler runs: an
arbitrary directory a reader tangles a single document in, following the
published README's sixty-second example. There, `target/` did not exist,
so the tangler fabricated one, holding nothing but a lock file, in
someone else's project — the README's own example littering, silently.

So the lock lives outside the workspace entirely, in the OS temp
directory, under a name derived from the workspace root. Nothing is
created in the tree the tangler was asked to write to but the files it
was asked to write.

```rust {#tangle-lock-state}
/// Serializes tanglers *within* this process. The file lock below
/// cannot: `flock` is held per open file description, so a second
/// thread opening the same path would sail past it.
static PROCESS_GATE: Mutex<()> = Mutex::new(());

thread_local! {
    /// Nesting depth for the current thread. `tangle_workspace` calls
    /// `tangle_document` per doc, and both acquire — without this the
    /// inner call would block on the lock the outer call is holding.
    static LOCK_DEPTH: Cell<usize> = const { Cell::new(0) };
}
```

The guard owns whatever the acquisition actually took. A nested
acquisition owns nothing and is pure bookkeeping; only the outermost
one holds the mutex and the locked file, and both release when it
drops.

```rust {#tangle-lock-guard}
/// Released by dropping. Fields are `Option` because a re-entrant
/// acquisition holds neither — see [`lock_workspace`].
struct TangleGuard {
    file: Option<File>,
    process: Option<MutexGuard<'static, ()>>,
}

impl Drop for TangleGuard {
    fn drop(&mut self) {
        LOCK_DEPTH.with(|d| d.set(d.get().saturating_sub(1)));
        // `file` and `process` drop after this body, releasing the
        // flock and the in-process gate in that order.
    }
}
```

Acquisition claims the depth slot and builds the guard *before* the
first fallible call, so an error on the way in still unwinds the
depth counter. The wait is unconditional: a tangle run is seconds at
worst, so blocking is kinder than failing and leaving the caller to
decide what a contended workspace means.

```rust {#tangle-lock-acquire}
/// Take the workspace-wide tangle lock, blocking until it's free.
/// Re-entrant per thread: a nested call returns an inert guard.
fn lock_workspace(workspace_root: &Path) -> Result<TangleGuard> {
    let outermost = LOCK_DEPTH.with(|d| {
        let n = d.get();
        d.set(n + 1);
        n == 0
    });
    let mut guard = TangleGuard { file: None, process: None };
    if !outermost {
        return Ok(guard);
    }
    // A poisoned gate means some other tangler panicked mid-run. The
    // files it half-wrote are this run's problem to overwrite, not a
    // reason to refuse to start.
    guard.process = Some(PROCESS_GATE.lock().unwrap_or_else(|e| e.into_inner()));

    let lock_path = lock_path_for(workspace_root);
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&lock_path)
        .with_context(|| format!("opening {}", lock_path.display()))?;
    file.lock()
        .with_context(|| format!("locking {}", lock_path.display()))?;
    guard.file = Some(file);
    Ok(guard)
}
```

The name has to be a *function of the workspace root* and nothing else:
every process tangling one workspace must land on the same file, and two
workspaces must never share one. Two roots can share a tail
(`~/a/main` and `~/b/main`), so the whole path is folded into a hash and
the readable tail is kept only so a human can tell the lock files apart.
The hash is hand-rolled FNV-1a rather than `DefaultHasher` because
`DefaultHasher`'s output is explicitly unspecified across builds, and two
`x0k-tangle` binaries built by different compilers must still agree on
the name or they do not coordinate at all.

```rust {#tangle-lock-path}
/// The workspace-wide tangle lock's path: a file in the OS temp directory
/// named after `workspace_root`, which is already absolutized by the
/// callers. Deliberately outside the workspace — see the prose above.
fn lock_path_for(workspace_root: &Path) -> PathBuf {
    let key = workspace_root.to_string_lossy();
    // FNV-1a over the whole path: the disambiguator, so two roots sharing a
    // tail cannot collide. Not `DefaultHasher` — its output is unspecified
    // across builds, and every x0k-tangle binary must compute this name
    // identically to coordinate.
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in key.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    // Every char of the slug is ASCII after the map, so the byte slice below
    // cannot split one.
    let mut slug: String = key
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    if slug.len() > 48 {
        slug = slug[slug.len() - 48..].to_string();
    }
    std::env::temp_dir().join(format!("x0k-tangle{slug}-{hash:016x}.lock"))
}
```

## Containment: the workspace root is a boundary

A tangle run is told which tree it owns. Until this section existed,
nothing held it to that. The workspace root arrived from the CLI
un-resolved — `--workspace .` stayed the literal `.` all the way to
the write — so the tree that actually received the generated files
was whichever directory the process happened to be sitting in. That
is not a boundary, it is a coincidence, and it failed the way
coincidences fail: a run started in one checkout wrote a generated
file into a different one, and the only thing that caught it was a
person noticing.

Two moves close it. First, **the root is resolved once, at the
entry points**, so every path downstream is absolute and the
run can say which tree it is writing to. Second, **every write is
checked against that root** — output files and sidecars alike. A
path that resolves outside is not clamped or silently relocated; it
is an error, because a tangler that wanted to write there has
already lost track of where it is.

`absolutize` handles a root that may not exist yet (a fresh temp
dir in a test, a not-yet-created output tree), so it canonicalizes
when it can and falls back to `cwd`-joining plus lexical
normalization when it cannot.

```rust {#containment-fns}
/// Resolve `path` to an absolute, lexically-normalized form.
///
/// `canonicalize` is preferred — it resolves symlinks, so two names
/// for one tree compare equal — but it fails on a path that does not
/// exist yet, which is the common case for an output file. The
/// fallback joins the cwd and normalizes `.` / `..` textually.
fn absolutize(path: &Path) -> PathBuf {
    if let Ok(c) = std::fs::canonicalize(path) {
        return c;
    }
    let joined = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("/"))
            .join(path)
    };
    lexically_normalize(&joined)
}

/// Collapse `.` and `..` textually. Purely syntactic: it never
/// touches the filesystem, so it is safe on paths that don't exist.
fn lexically_normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for comp in path.components() {
        match comp {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// Resolve a write target against the workspace root, refusing
/// anything that lands outside it.
///
/// `root` must already be absolutized. Relative targets are joined
/// onto the root; absolute ones are taken as given. Either way the
/// result must sit under the root, or the run errors rather than
/// writing into a tree it was not given.
fn contain_within_root(root: &Path, target: &Path) -> Result<PathBuf> {
    let joined = if target.is_absolute() {
        target.to_path_buf()
    } else {
        root.join(target)
    };
    let resolved = absolutize(&joined);
    if !resolved.starts_with(root) {
        return Err(anyhow!(
            "tangle refused to write outside its workspace root\n  \
             workspace root: {}\n  \
             attempted write: {}\n\
             The path resolves outside the tree this run was given. \
             Re-run with the workspace root you actually mean.",
            root.display(),
            resolved.display(),
        ));
    }
    Ok(resolved)
}
```

The guard is deliberately blunt. It does not try to decide whether
the escape was benign — a sidecar seeded from another checkout, a
`root:` that climbed too far, a wrapper script invoked from the
wrong directory all look the same from here, and all three are the
same mistake wearing different clothes.

## tangle_document

The single-doc entry point. Steps:

1. Read + parse the source `.md`.
2. Decide whether there's anything to do: a doc with neither
   `tangle:` nor `pipelines:` returns an empty result and writes
   nothing.
3. Build the unified pipeline list: synthesize an identity-tangle
   `PipelineDecl` if `tangle:` is present, then append the
   doc-declared pipelines.
4. For each pipeline in that list:
   - Look up the plugin in the registry; error if unknown kind.
   - Resolve declared `inputs:` chunks into typed `ChunkInput`s;
     expand `<<refs>>` so the plugin sees fully-resolved content.
   - Hash the inputs + config for sidecar drift detection.
   - Call `plugin.transform()`; promote `PipelineError` into anyhow.
   - For each returned output: compose with `@generated` header,
     contain against the root, atomic write, record path + hash in
     the sidecar entry.
5. Write the sidecar next to the source.

Step 0, before any of that, is resolving both the root and the doc
path to absolute form. Everything downstream — the joins, the
`strip_prefix` that makes sidecar paths portable, the containment
check — is comparing paths, and comparing a relative path against an
absolute one is how the tree you write to becomes an accident of cwd.

Between parsing and dispatch sits one genus check. A **publication**
document (`type: publication`) may carry a `tangle:` block, but its
output — the projected repository's `README.md`, per
[`region-repo.md`](region-repo.md) — is expressed relative to a
*projection* root, never the corpus root the document lives in.
Tangled from the corpus root, `root: README.md` would resolve to the
monorepo's own tracked `README.md` and overwrite it. No sweep walks
`decisions/publications/` (the registry's literate roots are
`knowledge/implementation` and `decisions/design/themes`, and the
daemon's watcher and `tools/tangle-current` copy the same list), so
the only way there is an explicit `tangle <publication-doc>` from the
wrong root — the same mis-invocation shape the containment section
above was bought by. The discriminant is the target, not the caller:
a projection root is one that already carries `PROVENANCE.json`, which
the projector writes before it tangles the README. Anywhere else, a
publication document is refused with the command that does tangle it.

```rust {#tangle-document}
/// Run every pipeline declared (or synthesized) for one document.
///
/// - `doc_path` — path to the source `.md` file; resolved to
///   absolute form on entry.
/// - `workspace_root` — workspace root, also resolved on entry. Every
///   write this call makes must land under it.
/// - `registry` — registry of pipeline plugins. Should already have
///   [`crate::IdentityPipeline`] registered (the default registry
///   does).
///
/// This function performs the filesystem writes (output files +
/// sidecar). On error before write, no files are touched. Holds the
/// workspace tangle lock for its whole body, so a concurrent tangler
/// in another process waits rather than interleaving writes.
pub fn tangle_document(
    doc_path: &Path,
    workspace_root: &Path,
    registry: &PipelineRegistry,
) -> Result<TangleResult> {
    let workspace_root = &absolutize(workspace_root);
    let doc_path = &absolutize(doc_path);
    let _lock = lock_workspace(workspace_root)?;

    let content = std::fs::read_to_string(doc_path)
        .with_context(|| format!("reading {}", doc_path.display()))?;
    let parsed = parse_document(&content)
        .with_context(|| format!("parsing {}", doc_path.display()))?;

    // A publication's `tangle:` block targets a projection root (its
    // README), never the corpus root it lives in. Only a root that
    // already carries `PROVENANCE.json` — a projection — may receive it.
    let is_publication = parse_envelope(&content)
        .map(|(env, _)| env.doc_type == DocType::Publication)
        .unwrap_or(false);
    if is_publication && !workspace_root.join("PROVENANCE.json").is_file() {
        return Err(anyhow!(
            "refusing to tangle publication document {} into {}: a publication's \
             tangle output is its projected repository's README, relative to a \
             projection root (one carrying PROVENANCE.json), never a corpus root. \
             Use `x0k-tangle project-repo` instead.",
            doc_path.display(),
            workspace_root.display()
        ));
    }

    let has_identity = parsed.tangle_crate.is_some()
        || parsed.tangle_root.is_some()
        || !parsed.tangle_roots.is_empty();
    let has_pipelines = !parsed.pipelines.is_empty();

    let mut result = TangleResult {
        source_path: doc_path.to_path_buf(),
        identity_outputs: Vec::new(),
        pipeline_outputs: Vec::new(),
        sidecars_written: Vec::new(),
    };

    if !has_identity && !has_pipelines {
        return Ok(result);
    }

    // Build the unified pipeline list. Identity tangling is a
    // synthetic prepended entry — same code path as theme-codegen
    // etc.
    let mut all_pipelines: Vec<PipelineDecl> = Vec::with_capacity(parsed.pipelines.len() + 1);
    if has_identity {
        let mut identity_config = serde_json::json!({
            "crate": parsed.tangle_crate.clone(),
            "root": parsed
                .tangle_root
                .as_ref()
                .map(|p| p.to_string_lossy().to_string()),
        });
        // The `roots` key appears only when declared: an absent map and
        // an empty map are the same config, and keying the JSON on the
        // distinction would churn every existing sidecar's config_hash.
        if !parsed.tangle_roots.is_empty() {
            let roots_json: serde_json::Map<String, serde_json::Value> = parsed
                .tangle_roots
                .iter()
                .map(|(lang, path)| {
                    (
                        lang.clone(),
                        serde_json::Value::String(path.to_string_lossy().to_string()),
                    )
                })
                .collect();
            identity_config["roots"] = serde_json::Value::Object(roots_json);
        }
        all_pipelines.push(PipelineDecl {
            kind: IDENTITY_KIND.to_string(),
            inputs: HashMap::new(),
            config: identity_config,
        });
    }
    all_pipelines.extend(parsed.pipelines.iter().cloned());

    let source_hash_str = content_hash(&content);

    let mut pipeline_sidecar_entries: Vec<PipelineSidecarEntry> = Vec::new();

    for decl in &all_pipelines {
        let plugin = registry
            .get(&decl.kind)
            .ok_or_else(|| {
                anyhow!(
                    "unknown pipeline kind `{}` in {}",
                    decl.kind,
                    doc_path.display()
                )
            })?
            .clone();

        // Resolve inputs. The identity pipeline has no declared
        // inputs (it walks `all_chunks` itself), so this loop is a
        // no-op for it.
        let mut inputs: HashMap<String, ChunkInput> = Default::default();
        let mut input_hashes = std::collections::BTreeMap::new();
        for (param_name, chunk_name) in &decl.inputs {
            let variants = parsed.chunks.get(chunk_name).ok_or_else(|| {
                anyhow!(
                    "pipeline `{}` references undefined chunk `{}` in {}",
                    decl.kind,
                    chunk_name,
                    doc_path.display()
                )
            })?;
            let mut resolved_variants = Vec::new();
            let mut concat_for_hash = String::new();
            for v in variants {
                let lang = v.lang.clone().unwrap_or_default();
                // Expand <<refs>>; falls back to raw body when no refs.
                let content = if v.is_from_ref() || v.is_media {
                    v.combined_body()
                } else {
                    expand_chunk(&parsed, chunk_name)
                        .with_context(|| format!("expanding chunk `{chunk_name}`"))?
                };
                concat_for_hash.push_str(&content);
                concat_for_hash.push('\n');
                resolved_variants.push(ChunkVariant { lang, content });
            }
            input_hashes.insert(param_name.clone(), content_hash(&concat_for_hash));
            inputs.insert(
                param_name.clone(),
                ChunkInput {
                    variants: resolved_variants,
                },
            );
        }

        let config_hash = content_hash(&decl.config.to_string());

        let ctx = PipelineContext {
            source_path: doc_path,
            workspace_root,
            config: &decl.config,
            inputs: &inputs,
            all_chunks: &parsed.chunks,
            chunk_order: &parsed.chunk_order,
        };

        let outputs = plugin.transform(&ctx).map_err(|e: PipelineError| {
            anyhow!(
                "pipeline `{}` in {}: {}",
                decl.kind,
                doc_path.display(),
                e
            )
        })?;

        let mut sidecar_outputs = Vec::new();
        for out in outputs {
            let abs_path = contain_within_root(workspace_root, &out.path)
                .with_context(|| {
                    format!("pipeline `{}` in {}", decl.kind, doc_path.display())
                })?;
            let final_bytes =
                compose_with_header(&out, doc_path, workspace_root, &decl.kind);
            write_atomic(&abs_path, &final_bytes)?;
            let hash = content_hash_bytes(&final_bytes);
            sidecar_outputs.push(PipelineOutputRecord {
                path: abs_path
                    .strip_prefix(workspace_root)
                    .unwrap_or(&abs_path)
                    .to_string_lossy()
                    .to_string(),
                hash: hash.clone(),
            });

            // Back-compat projection: surface identity-tangle outputs
            // in a typed bucket on the result for callers that want
            // them separately. Pipeline outputs are also recorded
            // below so a single iteration sees the full picture.
            if decl.kind == IDENTITY_KIND {
                let content_str = std::str::from_utf8(&final_bytes)
                    .map(|s| s.to_string())
                    .unwrap_or_default();
                result.identity_outputs.push(IdentityOutputRecord {
                    path: abs_path.clone(),
                    content: content_str,
                });
            }

            result.pipeline_outputs.push(PipelineRunOutput {
                path: abs_path,
                content: final_bytes,
                kind: decl.kind.clone(),
            });
        }

        pipeline_sidecar_entries.push(PipelineSidecarEntry {
            kind: decl.kind.clone(),
            input_hashes,
            config_hash,
            outputs: sidecar_outputs,
        });
    }

    // Sidecar emission. Always next to the source — one sidecar per
    // doc, even when the doc mixes identity + extra pipelines.
    if !pipeline_sidecar_entries.is_empty() {
        let sidecar_path = contain_within_root(workspace_root, &sidecar_path(doc_path))?;

        let sidecar = TangleSidecar {
            // Workspace-relative, like `outputs`. An absolute path
            // here would name this machine's checkout, and the
            // sidecar travels with the doc.
            source: doc_path
                .strip_prefix(workspace_root)
                .unwrap_or(doc_path)
                .to_string_lossy()
                .to_string(),
            source_hash: source_hash_str.clone(),
            pipelines: pipeline_sidecar_entries,
        };

        let json = serde_json::to_string_pretty(&sidecar)?;
        write_atomic(&sidecar_path, json.as_bytes())?;
        result.sidecars_written.push(sidecar_path);
    }

    Ok(result)
}
```

The `identity_outputs` projection is a back-compat affordance: code
written before identity-tangle became a plugin still iterates over
that field. New code can iterate `pipeline_outputs` and filter by
`kind == IDENTITY_KIND` if it specifically wants the identity bucket.

## tangle_directory

A thin wrapper around `tangle_document` that walks a directory. The
cheap content-gate (`!contains("tangle:") && !contains("pipelines:")`)
avoids parsing every `AGENTS.md` in the tree.

```rust {#tangle-directory}
/// Walk `dir` and run [`tangle_document`] on each `.md` folio/v1
/// file that declares `tangle:` or `pipelines:`. Skipped files (no
/// frontmatter, no relevant blocks) yield no result entry.
pub fn tangle_directory(
    dir: &Path,
    workspace_root: &Path,
    registry: &PipelineRegistry,
) -> Result<Vec<TangleResult>> {
    let workspace_root = &absolutize(workspace_root);
    let mut results = Vec::new();
    for entry in walkdir::WalkDir::new(dir).into_iter().filter_map(|e| e.ok()) {
        let p = entry.path();
        if !p.is_file() || p.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let content = match std::fs::read_to_string(p) {
            Ok(c) => c,
            Err(_) => continue,
        };
        // Cheap gate: skip files with neither block. Otherwise we'd
        // parse every AGENTS.md in the tree.
        if !content.contains("tangle:") && !content.contains("pipelines:") {
            continue;
        }
        let result = tangle_document(p, workspace_root, registry)?;
        if !result.identity_outputs.is_empty() || !result.pipeline_outputs.is_empty() {
            results.push(result);
        }
    }
    Ok(results)
}
```

## Freshness checking

`doc_freshness` decides whether a doc + its outputs are still in
sync with the sidecar. It powers `tangle check` (which never
writes) and `tangle workspace` (which skips up-to-date docs).

The freshness states make the operator's mental model explicit:

- **`Skip`** — doc has neither block. Nothing to check.
- **`UpToDate`** — sidecar matches source AND every recorded output
  still exists with the recorded hash.
- **`Dirty`** — one of: no sidecar, source changed, an output is
  missing, an output drifted, or an output resolves outside the
  workspace root.

That last one is the read-side half of containment, and it is the
one that bites in practice. A checkout seeded from another inherits
that checkout's sidecars. When those sidecars
recorded absolute output paths, freshness checked the *other* tree's
files, found them present and matching, and pronounced the doc
up-to-date — so the workspace's own generated file was never
produced, and the tangler reported success. An output path that
escapes the root tells us nothing about this tree, so the honest
answer is `Dirty`: re-tangle locally and record paths we can trust.

```rust {#freshness-types}
/// Per-doc result of the up-to-date check used by [`tangle_workspace`]
/// and by the `tangle check` CLI subcommand.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DocFreshness {
    /// Doc has no `tangle:` and no `pipelines:` — skip.
    Skip,
    /// Source hash matches sidecar AND every declared output still
    /// exists on disk with the recorded hash.
    UpToDate,
    /// Either no sidecar exists, source has changed, or one of the
    /// declared outputs is missing / hash-mismatched.
    Dirty(DirtyReason),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DirtyReason {
    /// No sidecar file next to the source doc.
    NoSidecar,
    /// Source hash differs from the recorded sidecar source_hash.
    SourceChanged,
    /// One of the declared output files is missing on disk.
    OutputMissing(PathBuf),
    /// An output file exists but its hash differs from the recorded one.
    OutputDrifted(PathBuf),
    /// A recorded output path resolves outside the workspace root —
    /// the sidecar describes some other tree (typically inherited by
    /// a workspace seeded from another checkout).
    OutputOutsideRoot(PathBuf),
}
```

```rust {#doc-freshness-fn}
/// Determine whether a doc is up to date with respect to its sidecar.
///
/// The sidecar lives next to the source as `<stem>.tangle-map.json`.
/// Returns [`DocFreshness::Skip`] if the doc has neither block,
/// [`DocFreshness::UpToDate`] if everything matches, or
/// [`DocFreshness::Dirty`] with a reason otherwise.
pub fn doc_freshness(doc_path: &Path, workspace_root: &Path) -> Result<DocFreshness> {
    let workspace_root = &absolutize(workspace_root);
    let doc_path = &absolutize(doc_path);
    let content = std::fs::read_to_string(doc_path)
        .with_context(|| format!("reading {}", doc_path.display()))?;
    let parsed = parse_document(&content)
        .with_context(|| format!("parsing {}", doc_path.display()))?;

    let has_identity = parsed.tangle_crate.is_some()
        || parsed.tangle_root.is_some()
        || !parsed.tangle_roots.is_empty();
    let has_pipelines = !parsed.pipelines.is_empty();
    if !has_identity && !has_pipelines {
        return Ok(DocFreshness::Skip);
    }

    let sidecar_path = sidecar_path(doc_path);
    if !sidecar_path.exists() {
        return Ok(DocFreshness::Dirty(DirtyReason::NoSidecar));
    }

    let sidecar_text = std::fs::read_to_string(&sidecar_path)
        .with_context(|| format!("reading sidecar {}", sidecar_path.display()))?;
    let sidecar: TangleSidecar = match serde_json::from_str(&sidecar_text) {
        Ok(s) => s,
        Err(_) => return Ok(DocFreshness::Dirty(DirtyReason::NoSidecar)),
    };

    let current_source_hash = content_hash(&content);
    if current_source_hash != sidecar.source_hash {
        return Ok(DocFreshness::Dirty(DirtyReason::SourceChanged));
    }

    // Check every recorded pipeline output (identity included).
    for pipeline in &sidecar.pipelines {
        for out_rec in &pipeline.outputs {
            let abs = match contain_within_root(
                workspace_root,
                Path::new(&out_rec.path),
            ) {
                Ok(p) => p,
                Err(_) => {
                    return Ok(DocFreshness::Dirty(DirtyReason::OutputOutsideRoot(
                        PathBuf::from(&out_rec.path),
                    )))
                }
            };
            if !abs.exists() {
                return Ok(DocFreshness::Dirty(DirtyReason::OutputMissing(abs)));
            }
            let bytes = std::fs::read(&abs)
                .with_context(|| format!("reading output {}", abs.display()))?;
            if content_hash_bytes(&bytes) != out_rec.hash {
                return Ok(DocFreshness::Dirty(DirtyReason::OutputDrifted(abs)));
            }
        }
    }

    Ok(DocFreshness::UpToDate)
}
```

## Workspace tangling

`tangle_workspace` is the top-level entry point that `x0k-tangle
workspace` invokes. It aggregates roots from every plugin's
`literate_roots`, walks them, and:

- Skips up-to-date docs (just claims their outputs for collision
  tracking).
- Re-tangles dirty docs.
- Collects per-doc errors without aborting.
- Detects output-path collisions and surfaces them with both source
  paths in the diagnostic.

Output drift is different from a source edit: it means bytes owned by
the doc changed behind the tangler's back. Before restoring that
projection, the workspace pass emits `obs.tangle_output_drifted` with
both source and output paths. That event is the hand-edit detector;
regeneration remains the repair.

```rust {#workspace-types}
/// Report from one invocation of [`tangle_workspace`].
#[derive(Debug, Default)]
pub struct WorkspaceTangleReport {
    /// Docs that were re-tangled this invocation.
    pub tangled: Vec<TangleResult>,
    /// Docs skipped because their sidecar + outputs are up-to-date.
    pub up_to_date: Vec<PathBuf>,
    /// Docs that errored. The workspace tangle continues on error and
    /// collects them here.
    pub errored: Vec<(PathBuf, anyhow::Error)>,
}
```

```rust {#tangle-workspace-fn}
/// Walk every literate root contributed by the registry's plugins
/// (via [`crate::TanglePipeline::literate_roots`]) and re-tangle any
/// doc whose source / outputs have drifted from the recorded sidecar.
/// Continues on per-doc error and collects errors in the report
/// rather than aborting.
///
/// Roots are deduplicated; non-existent roots are silently skipped.
///
/// ## Output-path collision detection
///
/// As docs are tangled, every written output path is recorded with
/// its source `.md`. If a second doc declares the same output path
/// during this workspace pass, that doc is added to
/// [`WorkspaceTangleReport::errored`] with a diagnostic naming both
/// sources. The first writer keeps its outputs; the colliding doc's
/// sidecar is NOT written (because the dispatcher already wrote it as
/// part of `tangle_document` — see follow-up below). We then mark the
/// colliding doc as errored to surface it.
///
/// Detection runs in two passes per doc:
/// 1. **Pre-flight planning** — call [`tangle_document`] in a "dry
///    enough" mode by reusing its return value, then check each
///    output path against the global map. If any clash, error.
///
/// Because plugins may decide output paths at transform time, we
/// detect collisions *after* `tangle_document` returns — i.e., the
/// colliding doc's file writes have already happened. That's OK for
/// the use case (the user gets a loud diagnostic; deciding which doc
/// owns the path is a manual fix). A future refinement could add a
/// dry-run mode, but the simpler post-hoc check is sufficient for
/// surfacing the bug.
pub fn tangle_workspace(
    workspace_root: &Path,
    registry: &PipelineRegistry,
) -> Result<WorkspaceTangleReport> {
    let workspace_root = &absolutize(workspace_root);
    // Held for the whole pass, not per doc: the collision bookkeeping
    // below only means anything if no other tangler writes into these
    // paths between docs.
    let _lock = lock_workspace(workspace_root)?;

    let mut report = WorkspaceTangleReport::default();

    let roots: BTreeSet<PathBuf> = registry
        .iter()
        .flat_map(|plugin| {
            plugin
                .literate_roots()
                .into_iter()
                .map(|s| workspace_root.join(s))
        })
        .collect();

    // Track which doc first declared each output path during this
    // workspace pass. Subsequent docs declaring the same path are
    // collision errors.
    let mut output_claims: HashMap<PathBuf, PathBuf> = HashMap::new();
    // Remember the winning doc's content so we can restore it after
    // a later colliding doc overwrites the file (the dispatcher
    // writes in `tangle_document` before we get a chance to check).
    let mut claimed_content: HashMap<PathBuf, Vec<u8>> = HashMap::new();

    for root in roots {
        if !root.exists() {
            continue;
        }
        for entry in walkdir::WalkDir::new(&root).into_iter().filter_map(|e| e.ok())
        {
            let p = entry.path();
            if !p.is_file() || p.extension().and_then(|e| e.to_str()) != Some("md") {
                continue;
            }
            // Cheap content gate before parsing.
            let content = match std::fs::read_to_string(p) {
                Ok(c) => c,
                Err(_) => continue,
            };
            if !content.contains("tangle:") && !content.contains("pipelines:") {
                continue;
            }

            match doc_freshness(p, workspace_root) {
                Ok(DocFreshness::Skip) => continue,
                Ok(DocFreshness::UpToDate) => {
                    // Even up-to-date docs claim their outputs so a
                    // dirty later doc colliding with them is caught.
                    // Use `entry().or_insert` so the FIRST claimant
                    // sticks across multiple collisions on the same
                    // path (otherwise the chain of error messages
                    // shifts as each new colliding doc overwrites
                    // the recorded "first declarer").
                    let mut had_collision = false;
                    if let Some(claims) = sidecar_output_claims(p, workspace_root) {
                        for out_abs in claims {
                            let first = output_claims
                                .entry(out_abs.clone())
                                .or_insert_with(|| p.to_path_buf())
                                .clone();
                            if first != p {
                                report.errored.push((
                                    p.to_path_buf(),
                                    collision_error(&out_abs, &first, p),
                                ));
                                had_collision = true;
                                break;
                            }
                            // Record bytes from disk so a later
                            // colliding doc's overwrite can be
                            // reverted.
                            if let Ok(bytes) = std::fs::read(&out_abs) {
                                claimed_content
                                    .entry(out_abs.clone())
                                    .or_insert(bytes);
                            }
                        }
                    }
                    if !had_collision {
                        report.up_to_date.push(p.to_path_buf());
                    }
                    continue;
                }
                Ok(DocFreshness::Dirty(reason)) => {
                    if let DirtyReason::OutputDrifted(output) = &reason {
                        warn!(
                            activity.type = "obs.tangle_output_drifted",
                            source = %p.display(),
                            output = %output.display(),
                            "tangle output drifted from its recorded projection"
                        );
                    }
                    // Fall through to tangle.
                }
                Err(e) => {
                    report.errored.push((p.to_path_buf(), e));
                    continue;
                }
            }

            match tangle_document(p, workspace_root, registry) {
                Ok(result) => {
                    // Check every output written by this doc against
                    // the global claim map. Use `entry().or_insert`
                    // so the FIRST claimant sticks; chained
                    // collisions all point back to the same original
                    // doc instead of leapfrogging.
                    //
                    // Because plugins write through `tangle_document`
                    // before we get here, a colliding doc has already
                    // overwritten the file on disk. We restore the
                    // first claimant's content (preserved in
                    // `claimed_content`) so the winning doc stays
                    // up-to-date next pass.
                    let mut collision: Option<anyhow::Error> = None;
                    for out in &result.pipeline_outputs {
                        let first = output_claims
                            .entry(out.path.clone())
                            .or_insert_with(|| p.to_path_buf())
                            .clone();
                        if first != p {
                            collision =
                                Some(collision_error(&out.path, &first, p));
                            // Restore the original winner's bytes.
                            if let Some(bytes) = claimed_content.get(&out.path) {
                                let _ = write_atomic(&out.path, bytes);
                            }
                            break;
                        }
                        // First write of this path — remember the
                        // content so a later collision can be reverted.
                        claimed_content
                            .entry(out.path.clone())
                            .or_insert_with(|| out.content.clone());
                    }
                    if let Some(err) = collision {
                        report.errored.push((p.to_path_buf(), err));
                    } else if !result.identity_outputs.is_empty()
                        || !result.pipeline_outputs.is_empty()
                    {
                        report.tangled.push(result);
                    } else {
                        // Doc had a tangle/pipelines block but produced
                        // no outputs (e.g. empty). Treat as up-to-date.
                        report.up_to_date.push(p.to_path_buf());
                    }
                }
                Err(e) => report.errored.push((p.to_path_buf(), e)),
            }
        }
    }

    Ok(report)
}
```

The collision detection deserves a careful read. The protocol commits
to a small-but-real guarantee: when two docs declare the same output,
the first one to write keeps its content, and the operator gets a
diagnostic naming both source docs. The "claimed_content" map is what
makes that work after-the-fact — the colliding doc's
`tangle_document` already overwrote the file, so we restore it before
the operator's next workspace pass sees an inconsistent state.

## Helpers

```rust {#collision-error-fn}
/// Build a collision error for a doubly-declared output path.
fn collision_error(path: &Path, first: &Path, second: &Path) -> anyhow::Error {
    anyhow!(
        "output path collision: {}\n  first declared by: {}\n  also declared by:  {}\nboth documents would tangle to the same file. Pick one as the source of truth.",
        path.display(),
        first.display(),
        second.display(),
    )
}
```

```rust {#sidecar-output-claims-fn}
/// Read the sidecar next to `doc_path` (if any) and return absolute
/// paths for every recorded output. Used when reusing an up-to-date
/// doc's claims in the workspace collision tracker — we want a doc
/// whose sidecar is fresh to still register its outputs so a later
/// dirty doc colliding with them is caught.
///
/// Returns `None` if the sidecar is missing or unreadable (the doc
/// would have been Dirty in that case anyway).
fn sidecar_output_claims(
    doc_path: &Path,
    workspace_root: &Path,
) -> Option<Vec<PathBuf>> {
    let sidecar_path = sidecar_path(doc_path);
    let text = std::fs::read_to_string(&sidecar_path).ok()?;
    let sidecar: TangleSidecar = serde_json::from_str(&text).ok()?;
    let workspace_root = &absolutize(workspace_root);
    let mut claims = Vec::new();
    for pipeline in &sidecar.pipelines {
        for out in &pipeline.outputs {
            // A claim outside the root describes another tree. Drop
            // it rather than let it stand in for a local file — the
            // collision tracker restores claimed content on conflict,
            // and restoring into a foreign checkout is the exact
            // cross-workspace write containment exists to stop.
            let Ok(abs) = contain_within_root(workspace_root, Path::new(&out.path))
            else {
                continue;
            };
            claims.push(abs);
        }
    }
    Some(claims)
}
```

```rust {#compose-with-header-fn}
/// Prepend the `@generated by x0k-tangle (pipeline: <kind>) — DO NOT
/// EDIT.` header to the plugin's output, using the requested comment
/// style. When the plugin sets `header_comment_style = None`, the
/// content is returned verbatim. The `source` path is rendered
/// workspace-relative (matching the identity-tangle convention) so
/// header text doesn't drift with the operator's checkout location.
fn compose_with_header(
    out: &PipelineOutput,
    source: &Path,
    workspace_root: &Path,
    kind: &str,
) -> Vec<u8> {
    let Some(style) = out.header_comment_style else {
        return out.content.clone();
    };
    let body_text = std::str::from_utf8(&out.content);
    // Only prepend a textual header when the plugin's payload is utf-8.
    // Otherwise (binary) fall through to verbatim.
    let body = match body_text {
        Ok(t) => t,
        Err(_) => return out.content.clone(),
    };

    let rel_source = source
        .strip_prefix(workspace_root)
        .unwrap_or(source)
        .display()
        .to_string();
    let line1 = format!(
        "@generated by x0k-tangle (pipeline: {kind}) from {rel_source} — DO NOT EDIT."
    );
    let header = match style {
        CommentStyle::Line(prefix) => format!("{prefix} {line1}\n"),
        CommentStyle::Block(open, close) => format!("{open} {line1} {close}\n"),
    };
    let mut out_bytes = Vec::with_capacity(header.len() + body.len() + 1);
    out_bytes.extend_from_slice(header.as_bytes());
    out_bytes.extend_from_slice(body.as_bytes());
    out_bytes
}
```

Every generated file lands through `write_atomic`, and the atomicity
is load-bearing rather than decorative. A generated `.rs` file is read
by `cargo build` at unpredictable moments; `std::fs::write` opens with
`O_TRUNC`, so between the truncate and the last byte there is a window
where the file on disk is empty or half a module. A build that reads
in that window doesn't see a stale file — it sees a broken one.

So we write the bytes to a sibling temp file and `rename` over the
target. `rename(2)` within a directory is atomic: a reader holds
either the whole old file or the whole new one, never a mixture. The
temp file must be a *sibling* — `rename` across filesystems fails, and
`target/` or `/tmp` may well be a different mount.

```rust {#temp-sibling-fn}
/// Counter making concurrent temp names distinct within a process;
/// the pid separates processes.
static TEMP_SEQ: AtomicU64 = AtomicU64::new(0);

/// A hidden sibling of `path` to stage content in. Dot-prefixed and
/// `.tmp`-suffixed so a tangler crash leaves litter that no tool
/// (cargo, walkdir's `.md` filter, the doc index) will pick up.
fn temp_sibling(path: &Path) -> PathBuf {
    let seq = TEMP_SEQ.fetch_add(1, Ordering::Relaxed);
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("output");
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    dir.join(format!(".{name}.tangle-{}-{seq}.tmp", std::process::id()))
}
```

The staging write is `sync_all`'d before the rename. That costs a
flush per generated file, which is the price of the crash story being
"either the old file or the new one" rather than "a correctly-named
file full of zeroes."

```rust {#write-atomic-fn}
/// Write `content` to `path`, creating parent directories. Readers
/// observe the old content or the new, never a truncated prefix.
fn write_atomic(path: &Path, content: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating {}", parent.display()))?;
    }
    let tmp = temp_sibling(path);
    if let Err(e) = stage(&tmp, content) {
        // Don't leave the partial staging file behind.
        let _ = std::fs::remove_file(&tmp);
        return Err(e).with_context(|| format!("writing {}", path.display()));
    }
    std::fs::rename(&tmp, path)
        .inspect_err(|_| {
            let _ = std::fs::remove_file(&tmp);
        })
        .with_context(|| format!("publishing {}", path.display()))?;
    Ok(())
}

/// Fill the staging file and flush it to disk.
fn stage(tmp: &Path, content: &[u8]) -> std::io::Result<()> {
    let mut f = File::create(tmp)?;
    f.write_all(content)?;
    f.sync_all()
}
```

```rust {#hash-fns}
fn content_hash(s: &str) -> String {
    let mut h = DefaultHasher::new();
    s.hash(&mut h);
    format!("{:016x}", h.finish())
}

fn content_hash_bytes(b: &[u8]) -> String {
    let mut h = DefaultHasher::new();
    b.hash(&mut h);
    format!("{:016x}", h.finish())
}
```

`DefaultHasher` is fast and good enough — the sidecar isn't a
cryptographic boundary, just a "did this content change?" check.

## Tests

The test fixtures use an `EchoPipeline` to drive `tangle_document`
end-to-end without dragging in real plugins. The cases cover: the
happy path (echo pipeline runs, sidecar lands next to source);
identity-tangle synthesis (a doc with only `tangle:` routes through
the identity plugin); the unified sidecar (a doc with both `tangle:`
and `pipelines:` produces a single sidecar with entries for both);
collision detection; freshness skipping.

Three more cover the write path rather than the dispatch: a reader
thread racing `write_atomic` and never seeing a partial file, the
staging file not surviving the call, and the lock's re-entrancy.

`````rust {#tests}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::{
        ChunkVariant, CommentStyle, PipelineContext, PipelineError, PipelineOutput, TanglePipeline,
    };
    use std::sync::atomic::AtomicBool;
    use std::sync::Arc;
    use tempfile::TempDir;

    /// A pipeline that writes its first input back, prefixed with
    /// a marker. Useful to drive `tangle_document` end-to-end.
    struct EchoPipeline;
    impl TanglePipeline for EchoPipeline {
        fn kind(&self) -> &str {
            "echo"
        }
        fn literate_roots(&self) -> Vec<&'static str> {
            vec!["knowledge/implementation"]
        }
        fn transform(&self, ctx: &PipelineContext) -> Result<Vec<PipelineOutput>, PipelineError> {
            let default = ctx
                .inputs
                .get("default")
                .ok_or_else(|| PipelineError::missing_input("default"))?;
            let v: &ChunkVariant = default
                .variants
                .first()
                .ok_or_else(|| PipelineError::transform_failed("no variants"))?;
            let suffix = ctx
                .config
                .get("suffix")
                .and_then(|s| s.as_str())
                .unwrap_or("out");
            Ok(vec![PipelineOutput {
                path: PathBuf::from(format!("artifacts/{suffix}.txt")),
                content: format!("ECHO:\n{}", v.content).into_bytes(),
                header_comment_style: Some(CommentStyle::Line("#")),
            }])
        }
    }

    /// A pipeline that deliberately aims outside the workspace root.
    /// `config.target` is the path it hands back — relative with
    /// `..`, or absolute — so one fixture covers both escapes.
    struct EscapePipeline;
    impl TanglePipeline for EscapePipeline {
        fn kind(&self) -> &str {
            "escape"
        }
        fn literate_roots(&self) -> Vec<&'static str> {
            vec!["knowledge/implementation"]
        }
        fn transform(&self, ctx: &PipelineContext) -> Result<Vec<PipelineOutput>, PipelineError> {
            let target = ctx
                .config
                .get("target")
                .and_then(|s| s.as_str())
                .ok_or_else(|| PipelineError::missing_input("target"))?;
            Ok(vec![PipelineOutput {
                path: PathBuf::from(target),
                content: b"ESCAPED".to_vec(),
                header_comment_style: Some(CommentStyle::Line("#")),
            }])
        }
    }

    #[test]
    fn tangle_document_runs_registered_pipeline() {
        let tmp = TempDir::new().unwrap();
        let workspace = tmp.path().to_path_buf();
        let doc = r#"---
x0k:
  format: folio/v1
  id: x0k:design/test
  type: design
  status: proposed
  pipelines:
    - kind: echo
      input: tokens
      config:
        suffix: hello
---

# Test

```toml {#tokens}
foo = "bar"
```
"#;
        let doc_path = workspace.join("test.md");
        std::fs::write(&doc_path, doc).unwrap();

        let mut registry = PipelineRegistry::default();
        registry.register(EchoPipeline);

        let result = tangle_document(&doc_path, &workspace, &registry).expect("tangle ok");
        assert_eq!(result.pipeline_outputs.len(), 1);

        let output_path = workspace.join("artifacts/hello.txt");
        assert!(output_path.exists(), "pipeline output written");
        let written = std::fs::read_to_string(&output_path).unwrap();
        assert!(written.starts_with("# @generated by x0k-tangle (pipeline: echo)"));
        assert!(written.contains("ECHO:"));
        assert!(written.contains("foo = \"bar\""));

        // Sidecar lives next to the source for pipeline-only docs.
        let sidecar_path = doc_path.with_extension("tangle-map.json");
        assert!(sidecar_path.exists(), "pipeline sidecar next to source");
        let sidecar_json = std::fs::read_to_string(&sidecar_path).unwrap();
        let sidecar: TangleSidecar = serde_json::from_str(&sidecar_json).unwrap();
        assert_eq!(sidecar.pipelines.len(), 1);
        assert_eq!(sidecar.pipelines[0].kind, "echo");
        assert_eq!(sidecar.pipelines[0].outputs.len(), 1);
    }

    #[test]
    fn tangle_document_errors_on_unknown_kind() {
        let tmp = TempDir::new().unwrap();
        let workspace = tmp.path().to_path_buf();
        let doc = r#"---
x0k:
  format: folio/v1
  id: x0k:design/test
  type: design
  status: proposed
  pipelines:
    - kind: nope
      input: tokens
---

```toml {#tokens}
x = 1
```
"#;
        let doc_path = workspace.join("test.md");
        std::fs::write(&doc_path, doc).unwrap();
        let registry = PipelineRegistry::default();
        let err = tangle_document(&doc_path, &workspace, &registry).unwrap_err();
        assert!(err.to_string().contains("unknown pipeline kind"));
    }

    #[test]
    fn tangle_document_passes_through_when_no_blocks() {
        let tmp = TempDir::new().unwrap();
        let workspace = tmp.path().to_path_buf();
        let doc_path = workspace.join("plain.md");
        std::fs::write(&doc_path, "---\nc0k:\n  format: folio/v1\n  id: x0k:design/x\n  type: design\n  status: proposed\n---\nbody\n").unwrap();
        let registry = PipelineRegistry::default();
        let result = tangle_document(&doc_path, &workspace, &registry).unwrap();
        assert!(result.identity_outputs.is_empty());
        assert!(result.pipeline_outputs.is_empty());
        assert!(result.sidecars_written.is_empty());
    }

    /// Identity-tangle is itself a plugin; the default registry has
    /// it pre-registered.
    #[test]
    fn identity_pipeline_registered_in_default_registry() {
        let registry = PipelineRegistry::default();
        assert!(
            registry.get(IDENTITY_KIND).is_some(),
            "default registry should have identity-tangle"
        );
    }

    /// When a doc has only `tangle:` (no `pipelines:`), the
    /// dispatcher synthesizes an identity-tangle pipeline run and
    /// emits the result through the same code path.
    #[test]
    fn tangle_document_routes_tangle_block_through_identity_plugin() {
        let tmp = TempDir::new().unwrap();
        let workspace = tmp.path().to_path_buf();
        let doc = r#"---
x0k:
  format: folio/v1
  id: x0k:wiki/code/sample
  type: wiki
  status: proposed
  tangle:
    root: out/sample.txt
---

# Sample

```text {#body}
hello world
```
"#;
        let doc_path = workspace.join("sample.md");
        std::fs::write(&doc_path, doc).unwrap();

        let registry = PipelineRegistry::default();
        let result =
            tangle_document(&doc_path, &workspace, &registry).expect("identity tangle ok");

        // The identity-tangle plugin emitted one output.
        assert_eq!(result.pipeline_outputs.len(), 1);
        assert_eq!(result.identity_outputs.len(), 1);
        assert_eq!(result.pipeline_outputs[0].kind, IDENTITY_KIND);

        let out_path = workspace.join("out/sample.txt");
        assert!(out_path.exists(), "identity output written");
        let written = std::fs::read_to_string(&out_path).unwrap();
        // Identity outputs land in unknown-extension territory
        // (`.txt`), so the dispatcher chose the default `//` line
        // comment style.
        assert!(written.starts_with("// @generated by x0k-tangle (pipeline: identity-tangle)"));
        assert!(written.contains("hello world"));

        // Wave 2: sidecar lives next to the SOURCE, not next to the
        // identity output. One location, single source of truth.
        let sidecar_path = doc_path.with_extension("tangle-map.json");
        assert!(sidecar_path.exists(), "identity sidecar next to source");
        assert!(
            !out_path.with_extension("tangle-map.json").exists(),
            "no sidecar next to output anymore"
        );
        let sidecar_json = std::fs::read_to_string(&sidecar_path).unwrap();
        let sidecar: TangleSidecar = serde_json::from_str(&sidecar_json).unwrap();
        assert_eq!(sidecar.pipelines.len(), 1);
        assert_eq!(sidecar.pipelines[0].kind, IDENTITY_KIND);
        assert_eq!(sidecar.pipelines[0].outputs.len(), 1);
        assert_eq!(sidecar.pipelines[0].outputs[0].path, "out/sample.txt");
    }

    /// A bilingual doc — per-language `tangle.roots:`, shared chunk
    /// names in two fence languages — tangles one output per language,
    /// each with lang-pinned `<<ref>>` resolution and the comment
    /// style of its own extension (`--` for `.gls`).
    #[test]
    fn tangle_document_routes_per_language_roots() {
        let tmp = TempDir::new().unwrap();
        let workspace = tmp.path().to_path_buf();
        let doc = r#"---
x0k:
  format: folio/v1
  id: x0k:wiki/code/bilingual
  type: wiki
  status: proposed
  tangle:
    roots:
      rust: out/demo.rs
      gallowglass: out/demo.gls
---

# Bilingual

```rust {#body}
fn body() {}
```

```gallowglass {#body}
let body = 0
```

```rust {#main}
<<body>>
fn main() {}
```

```gallowglass {#main}
<<body>>
let main = body
```
"#;
        let doc_path = workspace.join("bilingual.md");
        std::fs::write(&doc_path, doc).unwrap();

        let registry = PipelineRegistry::default();
        let result =
            tangle_document(&doc_path, &workspace, &registry).expect("bilingual tangle ok");
        assert_eq!(result.pipeline_outputs.len(), 2, "one output per language");

        let rs = std::fs::read_to_string(workspace.join("out/demo.rs")).unwrap();
        assert!(rs.starts_with("// @generated by x0k-tangle"));
        assert!(rs.contains("fn body() {}"), "rust ref resolved to rust variant: {rs}");
        assert!(!rs.contains("let body"), "no gallowglass leaked into rust: {rs}");

        let gls = std::fs::read_to_string(workspace.join("out/demo.gls")).unwrap();
        assert!(gls.starts_with("-- @generated by x0k-tangle"), "gls header uses --: {gls}");
        assert!(gls.contains("let body = 0"), "gls ref resolved to gls variant: {gls}");
        assert!(!gls.contains("fn body"), "no rust leaked into gallowglass: {gls}");
    }

    /// `absolutize` collapses `.` and `..` and lands on an absolute
    /// path whether or not the target exists yet.
    /// A publication document's `tangle:` block names its projected
    /// repository's README, so it tangles only into a projection root
    /// (one carrying `PROVENANCE.json`). From any other root — the
    /// corpus, where `root: README.md` would clobber the monorepo's
    /// own README — it is refused, naming `project-repo`.
    #[test]
    fn publication_document_tangles_only_into_a_projection_root() {
        let tmp = TempDir::new().unwrap();
        let workspace = tmp.path().to_path_buf();
        let doc = "---\nx0k:\n  format: folio/v1\n  id: x0k:publication/demo\n  type: publication\n  status: proposed\n  tangle:\n    root: README.md\n  edges:\n    publishes:\n      - x0k:software-module/demo\n---\n\n# Demo\n\n```markdown {#readme}\n# Demo\n\nA readme.\n```\n";
        let doc_path = workspace.join("decisions/publications/example.md");
        std::fs::create_dir_all(doc_path.parent().unwrap()).unwrap();
        std::fs::write(&doc_path, doc).unwrap();
        // The corpus root's own README must survive the refusal.
        std::fs::write(workspace.join("README.md"), "# The corpus\n").unwrap();
        let registry = PipelineRegistry::default();

        let err = tangle_document(&doc_path, &workspace, &registry)
            .expect_err("a corpus root refuses a publication document");
        assert!(
            err.to_string().contains("project-repo"),
            "refusal names the command that does tangle it: {err}"
        );
        assert_eq!(
            std::fs::read_to_string(workspace.join("README.md")).unwrap(),
            "# The corpus\n",
            "the corpus README is untouched by the refusal"
        );
        assert!(
            !doc_path.with_extension("tangle-map.json").exists(),
            "no sidecar is written on refusal"
        );

        // A projection root carries PROVENANCE.json; the same call writes.
        std::fs::write(workspace.join("PROVENANCE.json"), "{}").unwrap();
        let result = tangle_document(&doc_path, &workspace, &registry).unwrap();
        assert_eq!(result.identity_outputs.len(), 1);
        assert_eq!(result.identity_outputs[0].path, workspace.join("README.md"));
        let readme = std::fs::read_to_string(workspace.join("README.md")).unwrap();
        assert!(
            readme.starts_with(
                "<!-- @generated by x0k-tangle (pipeline: identity-tangle) from decisions/publications/example.md — DO NOT EDIT. -->\n"
            ),
            "the README carries the block-comment header GitHub renders invisibly: {readme}"
        );
        assert!(readme.ends_with("A readme.\n"));
    }

    #[test]
    fn absolutize_resolves_relative_and_dotted_paths() {
        let tmp = TempDir::new().unwrap();
        let root = std::fs::canonicalize(tmp.path()).unwrap();
        std::fs::create_dir_all(root.join("a/b")).unwrap();

        // Existing path with `..` in the middle.
        assert_eq!(absolutize(&root.join("a/b/../b")), root.join("a/b"));
        // Non-existent path — canonicalize fails, lexical fallback runs.
        assert_eq!(
            absolutize(&root.join("a/./nope/../made-up.txt")),
            root.join("a/made-up.txt")
        );
        // A relative path resolves against the process cwd, and is
        // absolute afterwards. (Read-only: no cwd change.)
        let here = absolutize(Path::new("."));
        assert!(here.is_absolute(), "relative root becomes absolute: {here:?}");
        assert_eq!(here, std::fs::canonicalize(std::env::current_dir().unwrap()).unwrap());
    }

    /// The containment predicate itself: inside is resolved, outside
    /// is refused, and both escape shapes — `..` and an absolute path
    /// in another tree — are refused the same way.
    #[test]
    fn contain_within_root_refuses_escapes() {
        let tmp = TempDir::new().unwrap();
        let root = std::fs::canonicalize(tmp.path()).unwrap();
        let outside = root.parent().unwrap().join("elsewhere.txt");

        assert_eq!(
            contain_within_root(&root, Path::new("src/gen.rs")).unwrap(),
            root.join("src/gen.rs")
        );
        assert_eq!(
            contain_within_root(&root, &root.join("src/gen.rs")).unwrap(),
            root.join("src/gen.rs")
        );
        assert!(contain_within_root(&root, Path::new("../escape.txt")).is_err());
        assert!(contain_within_root(&root, &outside).is_err());

        let err = contain_within_root(&root, &outside).unwrap_err().to_string();
        assert!(err.contains("outside its workspace root"), "names the property: {err}");
        assert!(err.contains(&root.display().to_string()), "names the root: {err}");
    }

    /// A tangle run must not write outside the root it was given, by
    /// either escape shape. Nothing is created on the far side — the
    /// run errors instead.
    #[test]
    fn tangle_refuses_to_write_outside_workspace_root() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join("ws");
        let neighbour = tmp.path().join("neighbour");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir_all(&neighbour).unwrap();

        let mut registry = PipelineRegistry::default();
        registry.register(EscapePipeline);

        for target in [
            "../neighbour/stolen.txt".to_string(),
            neighbour.join("stolen.txt").display().to_string(),
        ] {
            let doc = format!(
                r#"---
x0k:
  format: folio/v1
  id: x0k:design/escape
  type: design
  status: proposed
  pipelines:
    - kind: escape
      input: tokens
      config:
        target: "{target}"
---

# Escape

```toml {{#tokens}}
foo = "bar"
```
"#
            );
            let doc_path = root.join("escape.md");
            std::fs::write(&doc_path, doc).unwrap();

            let err = tangle_document(&doc_path, &root, &registry)
                .expect_err(&format!("must refuse target {target}"));
            assert!(
                err.to_string().contains("outside its workspace root")
                    || err.chain().any(|c| c.to_string().contains("outside its workspace root")),
                "refusal names the property for {target}: {err:#}"
            );
            assert!(
                !neighbour.join("stolen.txt").exists(),
                "nothing written outside the root for {target}"
            );
        }
    }

    /// The regression this whole section exists for. A workspace
    /// seeded from another checkout inherits its sidecars, which
    /// named that checkout's files by absolute path. Freshness used
    /// to validate *those* files and call the doc up-to-date, so the
    /// local tree never got its generated file. It is Dirty now.
    #[test]
    fn sidecar_from_another_tree_is_dirty_not_up_to_date() {
        let tmp = TempDir::new().unwrap();
        let other = tmp.path().join("other-checkout");
        let ws = tmp.path().join("ws");
        std::fs::create_dir_all(other.join("out")).unwrap();
        std::fs::create_dir_all(&ws).unwrap();

        let doc = r#"---
x0k:
  format: folio/v1
  id: x0k:wiki/code/seeded
  type: wiki
  status: proposed
  tangle:
    root: out/seeded.txt
---

```text {#body}
hi
```
"#;
        // Tangle in the other checkout, the way a seeded workspace's
        // sidecars were produced.
        let other_doc = other.join("seeded.md");
        std::fs::write(&other_doc, doc).unwrap();
        let registry = PipelineRegistry::default();
        tangle_document(&other_doc, &other, &registry).expect("other tangle ok");

        // Copy doc + sidecar across, then absolutize the recorded
        // output path — exactly the shape sidecars carried before.
        let ws_doc = ws.join("seeded.md");
        std::fs::write(&ws_doc, doc).unwrap();
        let sidecar_text =
            std::fs::read_to_string(other_doc.with_extension("tangle-map.json")).unwrap();
        let mut sidecar: TangleSidecar = serde_json::from_str(&sidecar_text).unwrap();
        for p in &mut sidecar.pipelines {
            for o in &mut p.outputs {
                o.path = other.join(&o.path).display().to_string();
            }
        }
        std::fs::write(
            ws_doc.with_extension("tangle-map.json"),
            serde_json::to_string_pretty(&sidecar).unwrap(),
        )
        .unwrap();

        // The other tree's output exists and matches its hash — the
        // exact condition that used to read as up-to-date.
        assert!(other.join("out/seeded.txt").exists());
        assert!(matches!(
            doc_freshness(&ws_doc, &ws).unwrap(),
            DocFreshness::Dirty(DirtyReason::OutputOutsideRoot(_))
        ));

        // And re-tangling produces the local file, leaving the other
        // tree untouched.
        let before = std::fs::read(other.join("out/seeded.txt")).unwrap();
        tangle_document(&ws_doc, &ws, &registry).expect("local tangle ok");
        assert!(ws.join("out/seeded.txt").exists(), "local output produced");
        assert_eq!(
            std::fs::read(other.join("out/seeded.txt")).unwrap(),
            before,
            "the other checkout was not written to"
        );
    }

    /// A root given with `..` in it resolves to the same tree as the
    /// clean form, and the sidecar it writes records workspace-
    /// relative paths for both `source` and every output — so the
    /// file describes a document, not a machine.
    #[test]
    fn sidecar_paths_are_workspace_relative() {
        let tmp = TempDir::new().unwrap();
        let workspace = std::fs::canonicalize(tmp.path()).unwrap();
        let docs = workspace.join("docs");
        std::fs::create_dir_all(&docs).unwrap();
        let doc = r#"---
x0k:
  format: folio/v1
  id: x0k:wiki/code/rel
  type: wiki
  status: proposed
  tangle:
    root: out/rel.txt
---

```text {#body}
hi
```
"#;
        let doc_path = docs.join("rel.md");
        std::fs::write(&doc_path, doc).unwrap();

        let registry = PipelineRegistry::default();
        // Dotted, un-normalized root — same tree, spelled awkwardly.
        let dotted = workspace.join("docs/../.");
        let result = tangle_document(&doc_path, &dotted, &registry).expect("tangle ok");
        assert_eq!(result.pipeline_outputs.len(), 1);
        assert_eq!(result.pipeline_outputs[0].path, workspace.join("out/rel.txt"));

        let sidecar: TangleSidecar = serde_json::from_str(
            &std::fs::read_to_string(doc_path.with_extension("tangle-map.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(sidecar.source, "docs/rel.md", "source is workspace-relative");
        assert_eq!(sidecar.pipelines[0].outputs[0].path, "out/rel.txt");
        assert!(
            !sidecar.source.starts_with('/'),
            "no absolute path leaks into the sidecar: {}",
            sidecar.source
        );

        // Same doc, root spelled cleanly: identical sidecar.
        let again = std::fs::read_to_string(doc_path.with_extension("tangle-map.json")).unwrap();
        tangle_document(&doc_path, &workspace, &registry).expect("tangle ok");
        assert_eq!(
            std::fs::read_to_string(doc_path.with_extension("tangle-map.json")).unwrap(),
            again,
            "how the root is spelled does not churn the sidecar"
        );
    }

    /// The oracle for checkout-independence: the same document, tangled
    /// from two checkouts at different absolute paths, produces
    /// byte-identical outputs — generated file *and* sidecar. Anything
    /// that leaks the tangling machine's path into either artifact shows
    /// up here as a byte difference, and shows up in a shared repository
    /// as two workspaces rewriting each other's files forever.
    #[test]
    fn two_checkouts_tangle_byte_identically() {
        let doc = r#"---
x0k:
  format: folio/v1
  id: x0k:wiki/code/portable
  type: wiki
  status: proposed
  tangle:
    crate: demo
    root: src/lib.rs
---

```rust {#body}
pub fn hello() {}
```
"#;

        // Two checkouts whose absolute paths differ in depth as well as
        // in spelling, so a leaked prefix cannot coincidentally match.
        let tmp_a = TempDir::new().unwrap();
        let tmp_b = TempDir::new().unwrap();
        let a = std::fs::canonicalize(tmp_a.path()).unwrap().join("co");
        let b = std::fs::canonicalize(tmp_b.path())
            .unwrap()
            .join("a-considerably-longer-checkout-name/nested/deeper");

        let registry = PipelineRegistry::default();
        let mut artifacts = Vec::new();
        for root in [&a, &b] {
            let docs = root.join("knowledge/implementation/demo");
            std::fs::create_dir_all(&docs).unwrap();
            let doc_path = docs.join("portable.md");
            std::fs::write(&doc_path, doc).unwrap();
            tangle_document(&doc_path, root, &registry).expect("tangle ok");
            artifacts.push((
                std::fs::read_to_string(root.join("demo/src/lib.rs")).unwrap(),
                std::fs::read_to_string(doc_path.with_extension("tangle-map.json")).unwrap(),
            ));
        }

        let (gen_a, map_a) = &artifacts[0];
        let (gen_b, map_b) = &artifacts[1];
        assert_eq!(gen_a, gen_b, "generated file differs between checkouts");
        assert_eq!(map_a, map_b, "sidecar differs between checkouts");
        assert!(
            gen_a.contains("from knowledge/implementation/demo/portable.md"),
            "header names the doc workspace-relatively: {gen_a}"
        );
        for artifact in [gen_a, map_a] {
            assert!(
                !artifact.contains(a.to_str().unwrap())
                    && !artifact.contains(b.to_str().unwrap()),
                "a checkout's absolute path leaked into: {artifact}"
            );
        }
    }

    /// Wave-2 verification: sidecar location is always next to source,
    /// even when the identity output lands in a deep nested dir.
    #[test]
    fn sidecar_lives_next_to_source() {
        let tmp = TempDir::new().unwrap();
        let workspace = tmp.path().to_path_buf();
        let src_dir = workspace.join("docs");
        std::fs::create_dir_all(&src_dir).unwrap();
        let doc = r#"---
x0k:
  format: folio/v1
  id: x0k:wiki/code/foo
  type: wiki
  status: proposed
  tangle:
    root: sub/foo.txt
---

```text {#body}
hi
```
"#;
        let doc_path = src_dir.join("foo.md");
        std::fs::write(&doc_path, doc).unwrap();

        let registry = PipelineRegistry::default();
        tangle_document(&doc_path, &workspace, &registry).expect("identity tangle ok");

        // Output lands at workspace/sub/foo.txt
        let out_path = workspace.join("sub/foo.txt");
        assert!(out_path.exists(), "output written at sub/foo.txt");
        // Sidecar lives at docs/foo.tangle-map.json (next to source)
        let sidecar_next_to_src = src_dir.join("foo.tangle-map.json");
        assert!(
            sidecar_next_to_src.exists(),
            "sidecar next to source at {}",
            sidecar_next_to_src.display()
        );
        // No output-adjacent sidecar
        let sidecar_next_to_out = workspace.join("sub/foo.tangle-map.json");
        assert!(
            !sidecar_next_to_out.exists(),
            "no sidecar next to output at {}",
            sidecar_next_to_out.display()
        );
    }

    /// Wave-2 verification: when a doc declares both `tangle:` and
    /// `pipelines:`, the unified sidecar carries entries for both —
    /// the synthetic identity-tangle run plus each declared pipeline.
    #[test]
    fn unified_sidecar_carries_identity_and_pipeline_entries() {
        let tmp = TempDir::new().unwrap();
        let workspace = tmp.path().to_path_buf();
        let src_dir = workspace.join("docs");
        std::fs::create_dir_all(&src_dir).unwrap();
        let doc = r#"---
x0k:
  format: folio/v1
  id: x0k:wiki/code/mixed
  type: wiki
  status: proposed
  tangle:
    root: out/mixed.txt
  pipelines:
    - kind: echo
      input: tokens
      config: { suffix: extra }
---

# Mixed

```text {#body}
identity content
```

```toml {#tokens}
foo = "bar"
```
"#;
        let doc_path = src_dir.join("mixed.md");
        std::fs::write(&doc_path, doc).unwrap();

        let mut registry = PipelineRegistry::default();
        registry.register(EchoPipeline);
        let result = tangle_document(&doc_path, &workspace, &registry)
            .expect("mixed identity + pipeline tangle ok");

        // Both pipelines produced outputs.
        assert_eq!(result.pipeline_outputs.len(), 2);
        let kinds: Vec<&str> = result
            .pipeline_outputs
            .iter()
            .map(|o| o.kind.as_str())
            .collect();
        assert!(kinds.contains(&IDENTITY_KIND));
        assert!(kinds.contains(&"echo"));

        // Single sidecar next to source, with both entries.
        let sidecar_path = doc_path.with_extension("tangle-map.json");
        assert!(sidecar_path.exists(), "single sidecar next to source");
        let sidecar_json = std::fs::read_to_string(&sidecar_path).unwrap();
        let sidecar: TangleSidecar = serde_json::from_str(&sidecar_json).unwrap();
        assert_eq!(
            sidecar.pipelines.len(),
            2,
            "sidecar carries identity + echo entries"
        );
        let sidecar_kinds: Vec<&str> =
            sidecar.pipelines.iter().map(|p| p.kind.as_str()).collect();
        assert!(sidecar_kinds.contains(&IDENTITY_KIND));
        assert!(sidecar_kinds.contains(&"echo"));
    }

    /// `tangle_workspace` aggregates `literate_roots()` from each
    /// registered plugin. Adding a plugin with a new root is enough
    /// to make `tangle_workspace` walk that directory.
    #[test]
    fn tangle_workspace_aggregates_roots_from_registry() {
        let tmp = TempDir::new().unwrap();
        let workspace = tmp.path().to_path_buf();

        // Two roots, two docs — one under each plugin's claimed root.
        std::fs::create_dir_all(workspace.join("knowledge/implementation/x")).unwrap();
        std::fs::create_dir_all(workspace.join("decisions/design/themes")).unwrap();

        let doc1 = r#"---
x0k:
  format: folio/v1
  id: x0k:design/test1
  type: design
  status: proposed
  pipelines:
    - kind: echo
      input: tokens
      config: { suffix: one }
---

```toml {#tokens}
one = 1
```
"#;
        std::fs::write(workspace.join("knowledge/implementation/x/d1.md"), doc1).unwrap();

        // Plugin two — claims `decisions/design/themes`.
        struct ThemesEchoPipeline;
        impl TanglePipeline for ThemesEchoPipeline {
            fn kind(&self) -> &str {
                "themes-echo"
            }
            fn literate_roots(&self) -> Vec<&'static str> {
                vec!["decisions/design/themes"]
            }
            fn transform(
                &self,
                ctx: &PipelineContext,
            ) -> Result<Vec<PipelineOutput>, PipelineError> {
                let default = ctx
                    .inputs
                    .get("default")
                    .ok_or_else(|| PipelineError::missing_input("default"))?;
                let v = default.variants.first().unwrap();
                Ok(vec![PipelineOutput {
                    path: PathBuf::from("artifacts/themes.txt"),
                    content: v.content.clone().into_bytes(),
                    header_comment_style: Some(CommentStyle::Line("#")),
                }])
            }
        }
        let doc2 = r#"---
x0k:
  format: folio/v1
  id: x0k:design/test2
  type: design
  status: proposed
  pipelines:
    - kind: themes-echo
      input: tokens
---

```toml {#tokens}
two = 2
```
"#;
        std::fs::write(workspace.join("decisions/design/themes/d2.md"), doc2).unwrap();

        let mut registry = PipelineRegistry::default();
        registry.register(EchoPipeline);
        registry.register(ThemesEchoPipeline);

        // Both docs should be picked up — one via EchoPipeline's
        // `knowledge/implementation` root, the other via ThemesEcho's
        // `decisions/design/themes` root.
        let report = tangle_workspace(&workspace, &registry).expect("workspace tangle ok");
        assert_eq!(
            report.tangled.len(),
            2,
            "two docs tangled (one per plugin root): {:?}",
            report.errored
        );
        assert!(report.errored.is_empty(), "no errors: {:?}", report.errored);
    }

    /// Verify the @generated header is line-comment-styled when the
    /// plugin requested `CommentStyle::Line`.
    #[test]
    fn header_uses_line_comment_style() {
        let out = PipelineOutput {
            path: PathBuf::from("a.txt"),
            content: b"body".to_vec(),
            header_comment_style: Some(CommentStyle::Line("//")),
        };
        let composed =
            compose_with_header(&out, Path::new("src.md"), Path::new("/ws"), "demo");
        let s = std::str::from_utf8(&composed).unwrap();
        assert!(s.starts_with("// @generated by x0k-tangle (pipeline: demo) from src.md"));
    }

    /// Verify the @generated header switches to block comments when
    /// the plugin asks for `CommentStyle::Block`.
    #[test]
    fn header_uses_block_comment_style() {
        let out = PipelineOutput {
            path: PathBuf::from("a.css"),
            content: b"body".to_vec(),
            header_comment_style: Some(CommentStyle::Block("/*", "*/")),
        };
        let composed =
            compose_with_header(&out, Path::new("src.md"), Path::new("/ws"), "demo");
        let s = std::str::from_utf8(&composed).unwrap();
        assert!(s.starts_with("/* @generated by x0k-tangle (pipeline: demo) from src.md "));
        assert!(s.contains("*/"));
    }

    /// Multi-input pipelines see all declared inputs.
    #[test]
    fn multi_input_pipeline_resolves_all() {
        struct MultiInput;
        impl TanglePipeline for MultiInput {
            fn kind(&self) -> &str {
                "multi"
            }
            fn transform(
                &self,
                ctx: &PipelineContext,
            ) -> Result<Vec<PipelineOutput>, PipelineError> {
                assert!(ctx.inputs.contains_key("a"));
                assert!(ctx.inputs.contains_key("b"));
                Ok(vec![])
            }
        }
        let tmp = TempDir::new().unwrap();
        let workspace = tmp.path().to_path_buf();
        let doc = r#"---
x0k:
  format: folio/v1
  id: x0k:design/test
  type: design
  status: proposed
  pipelines:
    - kind: multi
      inputs:
        a: chunk-a
        b: chunk-b
---

```toml {#chunk-a}
a = 1
```

```toml {#chunk-b}
b = 2
```
"#;
        let doc_path = workspace.join("test.md");
        std::fs::write(&doc_path, doc).unwrap();
        let mut registry = PipelineRegistry::default();
        registry.register(MultiInput);
        tangle_document(&doc_path, &workspace, &registry).expect("multi-input runs");
    }

    // ---- tangle_workspace tests ----

    /// Set up a fake workspace with one literate root (`knowledge/implementation/x`)
    /// containing one doc that uses the `echo` pipeline. Returns the
    /// workspace root, doc path, and a registry with the EchoPipeline
    /// registered.
    fn fake_workspace_with_pipeline_doc(tmp: &TempDir) -> (PathBuf, PathBuf, PipelineRegistry) {
        let workspace = tmp.path().to_path_buf();
        let dir = workspace.join("knowledge/implementation/x");
        std::fs::create_dir_all(&dir).unwrap();
        let doc = r#"---
x0k:
  format: folio/v1
  id: x0k:design/test
  type: design
  status: proposed
  pipelines:
    - kind: echo
      input: tokens
      config:
        suffix: ws
---

# Test

```toml {#tokens}
foo = "bar"
```
"#;
        let doc_path = dir.join("doc.md");
        std::fs::write(&doc_path, doc).unwrap();

        let mut registry = PipelineRegistry::default();
        registry.register(EchoPipeline);
        (workspace, doc_path, registry)
    }

    #[test]
    fn tangle_workspace_finds_known_roots() {
        let tmp = TempDir::new().unwrap();
        let (workspace, doc_path, registry) = fake_workspace_with_pipeline_doc(&tmp);

        let report = tangle_workspace(&workspace, &registry).expect("workspace tangle ok");
        assert_eq!(report.tangled.len(), 1, "exactly one doc tangled");
        assert_eq!(report.up_to_date.len(), 0);
        assert!(report.errored.is_empty(), "no errors: {:?}", report.errored);

        let tangled = &report.tangled[0];
        assert_eq!(tangled.source_path, doc_path);
        assert_eq!(tangled.pipeline_outputs.len(), 1);
    }

    #[test]
    fn tangle_workspace_skips_up_to_date() {
        let tmp = TempDir::new().unwrap();
        let (workspace, doc_path, registry) = fake_workspace_with_pipeline_doc(&tmp);

        // First invocation: tangles.
        let first = tangle_workspace(&workspace, &registry).unwrap();
        assert_eq!(first.tangled.len(), 1);

        // Second invocation: source + outputs unchanged ⇒ up-to-date.
        let second = tangle_workspace(&workspace, &registry).unwrap();
        assert_eq!(
            second.tangled.len(),
            0,
            "second run should re-use sidecar"
        );
        assert_eq!(second.up_to_date.len(), 1);
        assert_eq!(second.up_to_date[0], doc_path);
        assert!(second.errored.is_empty());
    }

    #[test]
    fn tangle_workspace_continues_on_error() {
        let tmp = TempDir::new().unwrap();
        let workspace = tmp.path().to_path_buf();
        let dir = workspace.join("knowledge/implementation/x");
        std::fs::create_dir_all(&dir).unwrap();

        // Doc 1: valid pipeline doc.
        let good_doc = r#"---
x0k:
  format: folio/v1
  id: x0k:design/good
  type: design
  status: proposed
  pipelines:
    - kind: echo
      input: tokens
      config:
        suffix: ok
---

```toml {#tokens}
ok = true
```
"#;
        std::fs::write(dir.join("good.md"), good_doc).unwrap();

        // Doc 2: references an unknown pipeline kind ⇒ errors.
        let bad_doc = r#"---
x0k:
  format: folio/v1
  id: x0k:design/bad
  type: design
  status: proposed
  pipelines:
    - kind: does-not-exist
      input: tokens
---

```toml {#tokens}
broken = true
```
"#;
        std::fs::write(dir.join("bad.md"), bad_doc).unwrap();

        let mut registry = PipelineRegistry::default();
        registry.register(EchoPipeline);

        let report = tangle_workspace(&workspace, &registry).unwrap();
        // Good doc tangled, bad doc collected as error, workflow continued.
        assert_eq!(report.tangled.len(), 1, "good doc tangled");
        assert_eq!(report.errored.len(), 1, "bad doc errored");
        assert!(report.errored[0]
            .1
            .to_string()
            .contains("unknown pipeline kind"));
    }

    /// Wave-2 verification: two docs declaring the same output path
    /// produce a collision error in `WorkspaceTangleReport.errored`,
    /// naming both source docs. Processing continues for both docs.
    #[test]
    fn tangle_workspace_errors_on_output_collision() {
        let tmp = TempDir::new().unwrap();
        let workspace = tmp.path().to_path_buf();
        let dir = workspace.join("knowledge/implementation/x");
        std::fs::create_dir_all(&dir).unwrap();

        // Two identity-tangle docs declaring the same `root:` output.
        let doc_a = r#"---
x0k:
  format: folio/v1
  id: x0k:wiki/code/a
  type: wiki
  status: proposed
  tangle:
    root: shared/output.txt
---

```text {#body}
from a
```
"#;
        // Use distinct names so the WalkDir order is deterministic and
        // the "first" claimant is predictable on most filesystems —
        // but the test doesn't assume which one wins.
        std::fs::write(dir.join("a.md"), doc_a).unwrap();

        let doc_b = r#"---
x0k:
  format: folio/v1
  id: x0k:wiki/code/b
  type: wiki
  status: proposed
  tangle:
    root: shared/output.txt
---

```text {#body}
from b
```
"#;
        std::fs::write(dir.join("b.md"), doc_b).unwrap();

        let registry = PipelineRegistry::default();
        let report = tangle_workspace(&workspace, &registry).expect("workspace tangle ok");

        // One doc tangled successfully, one errored with collision.
        assert_eq!(
            report.errored.len(),
            1,
            "exactly one collision error: errored={:?}",
            report
                .errored
                .iter()
                .map(|(p, e)| (p.display().to_string(), e.to_string()))
                .collect::<Vec<_>>()
        );
        let (errored_path, errored_err) = &report.errored[0];
        let err_str = errored_err.to_string();
        assert!(
            err_str.contains("output path collision"),
            "error mentions collision: {err_str}"
        );
        assert!(
            err_str.contains("shared/output.txt"),
            "error names colliding path: {err_str}"
        );
        assert!(
            err_str.contains("a.md") && err_str.contains("b.md"),
            "error names both source docs: {err_str}"
        );
        // The errored doc is one of the two source docs.
        let errored_name = errored_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");
        assert!(
            errored_name == "a.md" || errored_name == "b.md",
            "errored path is one of the two source docs: {errored_name}"
        );

        // The other doc tangled successfully (its output was written).
        assert!(
            !report.tangled.is_empty(),
            "the first writer's tangle succeeded"
        );
    }

    /// A reader hammering a file while `write_atomic` rewrites it must
    /// never observe a truncated prefix. With `fs::write` this fails
    /// within a handful of iterations; with rename-over it cannot fail.
    #[test]
    fn write_atomic_is_never_observed_partial() {
        let tmp = TempDir::new().unwrap();
        let target = tmp.path().join("generated.rs");

        // Two payloads big enough that a truncating write can't land in
        // one page, and distinguishable by content as well as length.
        let a = vec![b'a'; 512 * 1024];
        let b = vec![b'b'; 768 * 1024];
        write_atomic(&target, &a).unwrap();

        let done = Arc::new(AtomicBool::new(false));
        let reader = {
            let (target, done) = (target.clone(), Arc::clone(&done));
            std::thread::spawn(move || {
                let mut reads = 0u64;
                while !done.load(Ordering::Relaxed) {
                    // A failed open is fine (rename is not instantaneous
                    // from the opener's side on every platform); a
                    // successful open must yield a whole payload.
                    if let Ok(bytes) = std::fs::read(&target) {
                        let uniform = |c: u8| bytes.iter().all(|&x| x == c);
                        assert!(
                            (bytes.len() == 512 * 1024 && uniform(b'a'))
                                || (bytes.len() == 768 * 1024 && uniform(b'b')),
                            "reader saw a partial file of {} bytes",
                            bytes.len()
                        );
                        reads += 1;
                    }
                }
                reads
            })
        };

        for i in 0..100 {
            write_atomic(&target, if i % 2 == 0 { &b } else { &a }).unwrap();
        }
        done.store(true, Ordering::Relaxed);
        let reads = reader.join().unwrap();
        assert!(reads > 0, "the reader actually read the file");
    }

    /// The staging file is transient: once `write_atomic` returns, the
    /// output directory holds the target and nothing else.
    #[test]
    fn write_atomic_leaves_no_staging_file() {
        let tmp = TempDir::new().unwrap();
        let target = tmp.path().join("nested/dir/out.rs");
        write_atomic(&target, b"content").unwrap();

        let siblings: Vec<String> = std::fs::read_dir(target.parent().unwrap())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(siblings, vec!["out.rs".to_string()]);
        assert_eq!(std::fs::read(&target).unwrap(), b"content");
    }

    /// `tangle_workspace` takes the lock and then calls
    /// `tangle_document`, which takes it again — re-entrancy is the
    /// only reason that isn't a deadlock.
    #[test]
    fn workspace_lock_is_reentrant_and_file_backed() {
        let tmp = TempDir::new().unwrap();
        let workspace = tmp.path();

        let outer = lock_workspace(workspace).unwrap();
        let inner = lock_workspace(workspace).unwrap();
        assert!(outer.file.is_some(), "outer acquisition owns the lock file");
        assert!(inner.file.is_none(), "nested acquisition owns nothing");
        // The lock lives in the OS temp dir, NOT in the tree the tangler was
        // pointed at: a tangle run creates nothing in the caller's directory
        // but the files it was asked to write.
        assert!(lock_path_for(workspace).exists());
        assert!(
            !workspace.join("target").exists(),
            "the tangler must not fabricate a target/ in the caller's tree"
        );

        drop(inner);
        drop(outer);
        // Depth unwound — the next acquisition is outermost again.
        assert!(lock_workspace(workspace).unwrap().file.is_some());
    }
}
`````

## Composing the module

```rust {#root}
<<module-header>>

<<imports>>

<<sidecar-types>>

<<run-result-types>>

<<sidecar-path>>

<<tangle-lock-state>>

<<tangle-lock-guard>>

<<tangle-lock-acquire>>

<<tangle-lock-path>>

<<containment-fns>>

<<tangle-document>>

<<tangle-directory>>

<<freshness-types>>

<<doc-freshness-fn>>

<<workspace-types>>

<<tangle-workspace-fn>>

<<collision-error-fn>>

<<sidecar-output-claims-fn>>

<<compose-with-header-fn>>

<<temp-sibling-fn>>

<<write-atomic-fn>>

<<hash-fns>>

<<tests>>
```
