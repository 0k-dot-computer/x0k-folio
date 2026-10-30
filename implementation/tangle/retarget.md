
# Moving a tangled output: retarget

```turtle folio:document
implementation:tangle\/retarget a x0k:Implementation ;
    x0k:status "draft" ;
    x0k:summary "Moving a tangled output to a new path by editing the documents that write it, not the file: their targets are rewritten, they are tangled again, and the new file must hold exactly the old one's bytes before the old one goes." ;
    x0k:concerns "tangle", "refactoring", "crates" ;
    x0k:cites implementation:tangle\/crate,
        implementation:tangle\/parsing,
        implementation:tangle\/identity-pipeline ;
    folio:tangleCrate "crates/x0k-tangle" ;
    folio:tangleRoot "src/retarget.rs" .
```

Splitting a crate moves source files, and most of this workspace's source
files are not written by hand: a chapter writes each one, and the file is a
projection that must equal a fresh tangle byte for byte. Moving such a file
is therefore an edit to the documents that write it. Done by hand it means
finding every chapter whose chunks land on the file, working out whether a
chunk's `file=`, the header's `folio:tangleRoot` or its `folio:tangleCrate`
put it there, spelling the new path relative to the right base, re-tangling,
and deleting the old file. Each step is easy to get slightly wrong, and a
slightly wrong move leaves an orphaned projection or a chapter that writes
somewhere nobody looks.

`x0k-tangle retarget FROM TO` does that move and proves it. The central
idea: **a move changes where bytes land, never what they are**, so the file
the documents write at `TO` must equal the file that was at `FROM`, and the
verb refuses to finish otherwise. A projection's `@generated` header names
the chapter it came from, not its own path, so a correct move leaves every
byte in place.

## The contract

`FROM` and `TO` are workspace-relative output paths. The verb finds the
documents whose tangle maps record `FROM`, rewrites in each the one place
that resolves to `FROM`, re-tangles them, checks that `TO` holds exactly the
bytes `FROM` held, and removes `FROM`. What it rewrites, most specific first,
is the order the identity pipeline resolves a target in:

- a chunk whose `file=` resolves to `FROM` has that attribute rewritten;
- otherwise a header whose `folio:tangleRoot` resolves to `FROM` has the
  root rewritten.

A path is spelled relative to the document's base: its `folio:tangleCrate`
directory, or the workspace root when it names none. `TO` outside that base
is a move to another crate, and only a whole document can make it: with
`--crate DIR` the header's `folio:tangleCrate` becomes `DIR`, and the verb
refuses when the document writes any output besides `FROM`, because those
would silently move too. A single chunk bound for another crate belongs in a
chapter of that crate; giving it one is writing, not retargeting.

Every refusal happens before a document is touched, and a byte mismatch after
re-tangling restores every document and its outputs as they were.

<a name="chunk-module-doc"></a><sub>[`src/retarget.rs`](../../crates/x0k-tangle/src/retarget.rs) · `#module-doc`</sub>

```rust {#module-doc}
//! `x0k-tangle retarget`: move a tangled output by rewriting the documents
//! that write it, re-tangle them, and prove the move kept every byte.
//! A target is rewritten where it resolves (a chunk's `file=`, else the
//! header's `folio:tangleRoot`), spelled relative to the document's
//! `folio:tangleCrate`; `--crate` moves a single-output document to another
//! crate. A refusal touches nothing; a byte mismatch restores everything.

use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};

use crate::parser::parse_document;
use crate::pipeline_runner::tangle_document;
use crate::PipelineRegistry;
```

## Paths compare after normalization

A chapter may spell a target `src/./x.rs` or reach it through `..`, and the
tangle map records the joined path. Comparison is lexical: `.` is dropped and
`..` pops a component, without touching the filesystem, so a path that does
not exist yet compares the same way as one that does. The dispatcher
already normalizes write targets this way, so retarget uses its helper.

<a name="chunk-normalize"></a><sub>[`src/retarget.rs`](../../crates/x0k-tangle/src/retarget.rs) · `#normalize`</sub>

```rust {#normalize}
use crate::pipeline_runner::lexically_normalize as normalize;
```

## Who writes FROM

The tangle maps are the record of which document wrote what, so the search
reads them rather than re-parsing every chapter: a `*.tangle-map.json` sits
beside each tangled document and lists its outputs. Build directories, hidden
directories and vendored trees hold none and are skipped, which keeps the walk
proportional to the documents, not to the build cache beside them.

<a name="chunk-map-outputs"></a><sub>[`src/retarget.rs`](../../crates/x0k-tangle/src/retarget.rs) · `#map-outputs`</sub>

```rust {#map-outputs}
/// A tangle map's source document and output paths, all workspace-relative.
fn map_outputs(map: &Path) -> Result<(PathBuf, Vec<PathBuf>)> {
    let json: serde_json::Value = serde_json::from_slice(&std::fs::read(map)?)
        .with_context(|| format!("{} is not a tangle map", map.display()))?;
    let source = json["source"].as_str().context("a tangle map without a source")?;
    let outputs = json["pipelines"].as_array().into_iter().flatten()
        .flat_map(|p| p["outputs"].as_array().into_iter().flatten())
        .filter_map(|o| o["path"].as_str().map(|p| normalize(Path::new(p))))
        .collect();
    Ok((PathBuf::from(source), outputs))
}
```

<a name="chunk-writers"></a><sub>[`src/retarget.rs`](../../crates/x0k-tangle/src/retarget.rs) · `#writers`</sub>

```rust {#writers}
const SKIPPED: &[&str] = &["target", "target.noindex", "node_modules", "vendor"];

/// Every document whose tangle map records `from`, with all of its outputs.
fn writers_of(workspace: &Path, from: &Path) -> Result<Vec<(PathBuf, Vec<PathBuf>)>> {
    let mut found = Vec::new();
    let walk = walkdir::WalkDir::new(workspace).into_iter().filter_entry(|e| {
        let name = e.file_name().to_string_lossy();
        e.depth() == 0 || !(name.starts_with('.') || SKIPPED.contains(&name.as_ref()))
    });
    for entry in walk.filter_map(|e| e.ok()) {
        if entry.file_name().to_string_lossy().ends_with(".tangle-map.json") {
            let (source, outputs) = map_outputs(entry.path())?;
            if outputs.iter().any(|o| o == from) {
                found.push((source, outputs));
            }
        }
    }
    Ok(found)
}
```

## The rewrite, decided before anything is written

For each writer the plan is computed whole first: its base, the new
spelling, and the exact text edits. Only when every writer has a plan does
any file change, so a refusal for the third document leaves the first two
alone. The base is the header's crate, or the empty path for the workspace
root; with `--crate` the new base is the given directory, and the document
must write nothing but `FROM`.

<a name="chunk-plan-type"></a><sub>[`src/retarget.rs`](../../crates/x0k-tangle/src/retarget.rs) · `#plan-type`</sub>

```rust {#plan-type}
/// One document's rewrite: its path and its text before and after.
struct Plan {
    document: PathBuf,
    before: String,
    after: String,
    rewrites: Vec<String>,
}
```

<a name="chunk-plan-base"></a><sub>[`src/retarget.rs`](../../crates/x0k-tangle/src/retarget.rs) · `#plan-base`</sub>

```rust {#plan-base}
/// The document's base and `to` spelled relative to the base it will have.
fn new_spelling(doc: &Path, base: &Path, outputs: &[PathBuf], from: &Path, to: &Path,
    new_crate: Option<&Path>) -> Result<(PathBuf, PathBuf)> {
    let target_base = new_crate.map(normalize).unwrap_or_else(|| base.to_path_buf());
    if new_crate.is_some() {
        let others: Vec<_> = outputs.iter().filter(|o| *o != from).collect();
        if !others.is_empty() {
            bail!("{} also writes {:?}; --crate would move them too. Give the chunk for {} a \
                chapter of the target crate instead", doc.display(), others, from.display());
        }
    }
    let Ok(relative) = to.strip_prefix(&target_base) else {
        bail!("{} is outside {}'s crate {}; pass --crate DIR to move the whole document",
            to.display(), doc.display(), target_base.display());
    };
    Ok((target_base, relative.to_path_buf()))
}
```

The text edits touch two kinds of line. A fence line carries a chunk's
attributes, and its `file="…"` is rewritten when it resolves to `FROM`. The
header carries `folio:tangleRoot "…"` and `folio:tangleCrate "…"`; the root
is rewritten only when no chunk's `file=` did, since a chunk's own target
takes precedence over the root when the pipeline resolves it.

<a name="chunk-rewrite-fences"></a><sub>[`src/retarget.rs`](../../crates/x0k-tangle/src/retarget.rs) · `#rewrite-fences`</sub>

```rust {#rewrite-fences}
/// Rewrite every fence's `file="…"` that resolves under `base` to `from`.
fn rewrite_fences(text: &str, base: &Path, from: &Path, new: &Path, notes: &mut Vec<String>) -> String {
    let mut out = String::with_capacity(text.len());
    for line in text.split_inclusive('\n') {
        let fence = line.trim_start().starts_with("```");
        let Some(start) = line.find("file=\"").filter(|_| fence) else { out.push_str(line); continue };
        let value_start = start + "file=\"".len();
        let Some(len) = line[value_start..].find('"') else { out.push_str(line); continue };
        let value = &line[value_start..value_start + len];
        if normalize(&base.join(value)) == from {
            notes.push(format!("file=\"{value}\" -> \"{}\"", new.display()));
            out.push_str(&line[..value_start]);
            out.push_str(&new.display().to_string());
            out.push_str(&line[value_start + len..]);
        } else {
            out.push_str(line);
        }
    }
    out
}
```

<a name="chunk-rewrite-header"></a><sub>[`src/retarget.rs`](../../crates/x0k-tangle/src/retarget.rs) · `#rewrite-header`</sub>

```rust {#rewrite-header}
/// Replace the one `predicate "old"` in the header with `predicate "new"`.
fn rewrite_header(text: &str, predicate: &str, old: &str, new: &str) -> Result<String> {
    let needle = format!("{predicate} \"{old}\"");
    match text.matches(&needle).count() {
        1 => Ok(text.replacen(&needle, &format!("{predicate} \"{new}\""), 1)),
        n => bail!("expected one `{needle}`, found {n}"),
    }
}
```

<a name="chunk-plan-document"></a><sub>[`src/retarget.rs`](../../crates/x0k-tangle/src/retarget.rs) · `#plan-document`</sub>

```rust {#plan-document}
fn plan_document(workspace: &Path, doc: &Path, outputs: &[PathBuf], from: &Path, to: &Path,
    new_crate: Option<&Path>) -> Result<Plan> {
    let before = std::fs::read_to_string(workspace.join(doc))
        .with_context(|| format!("reading {}", doc.display()))?;
    let parsed = parse_document(&before)?;
    let base = parsed.tangle_crate.as_deref().map(|c| normalize(Path::new(c))).unwrap_or_default();
    if parsed.tangle_roots.iter().any(|(_, r)| normalize(&base.join(r)) == from) {
        bail!("{} writes {} through folio:tangleRoots, which retarget does not rewrite",
            doc.display(), from.display());
    }
    let (target_base, new) = new_spelling(doc, &base, outputs, from, to, new_crate)?;
    let mut rewrites = Vec::new();
    let mut after = rewrite_fences(&before, &base, from, &new, &mut rewrites);
    if rewrites.is_empty() {
        let root = parsed.tangle_root.as_deref()
            .filter(|r| normalize(&base.join(r)) == from)
            .with_context(|| format!("{}'s map names {} but no chunk or root resolves to it; \
                re-tangle it first", doc.display(), from.display()))?;
        after = rewrite_header(&after, "folio:tangleRoot", &root.display().to_string(),
            &new.display().to_string())?;
        rewrites.push(format!("folio:tangleRoot -> \"{}\"", new.display()));
    }
    match (new_crate, parsed.tangle_crate.as_deref()) {
        (Some(dir), Some(old)) => {
            after = rewrite_header(&after, "folio:tangleCrate", old, &target_base.display().to_string())?;
            rewrites.push(format!("folio:tangleCrate -> \"{}\"", dir.display()));
        }
        (Some(_), None) => bail!("{} names no folio:tangleCrate to move; it writes relative to the \
            workspace root, so retarget it without --crate", doc.display()),
        (None, _) => {}
    }
    Ok(Plan { document: doc.to_path_buf(), before, after, rewrites })
}
```

## Apply, re-tangle, prove

With every plan in hand, the documents are written and re-tangled through the
host's registry, the same registry `tangle` uses, so a document needing a
plugin is tangled by the binary that carries it. Then the proof: the bytes at
`TO` against the bytes `FROM` held. On a match `FROM` is removed and the moves
are reported. On a mismatch every document is written back and re-tangled,
`TO` is removed if nothing wrote it before, and the verb fails naming the
mismatch, so a failed move leaves the tree as it found it.

<a name="chunk-retarget-fn"></a><sub>[`src/retarget.rs`](../../crates/x0k-tangle/src/retarget.rs) · `#retarget-fn` · assembles [apply-and-prove](#chunk-apply-and-prove)</sub>

```rust {#retarget-fn}
/// One moved document and the edits made to it.
pub struct Retargeted {
    pub document: PathBuf,
    pub rewrites: Vec<String>,
}

/// Move the tangled output `from` to `to` (both workspace-relative).
pub fn retarget_output(workspace: &Path, from: &Path, to: &Path, new_crate: Option<&Path>,
    registry: &PipelineRegistry) -> Result<Vec<Retargeted>> {
    let (from, to) = (normalize(from), normalize(to));
    let old_bytes = std::fs::read(workspace.join(&from))
        .with_context(|| format!("{} does not exist", from.display()))?;
    if workspace.join(&to).exists() {
        bail!("{} already exists", to.display());
    }
    let writers = writers_of(workspace, &from)?;
    if writers.is_empty() {
        bail!("no document's tangle map records {}", from.display());
    }
    let plans = writers.iter()
        .map(|(doc, outputs)| plan_document(workspace, doc, outputs, &from, &to, new_crate))
        .collect::<Result<Vec<_>>>()?;
    <<apply-and-prove>>
}
```

<a name="chunk-apply-and-prove"></a><sub>[`src/retarget.rs`](../../crates/x0k-tangle/src/retarget.rs) · `#apply-and-prove`</sub>

```rust {#apply-and-prove}
let write_all = |pick: fn(&Plan) -> &String| -> Result<()> {
    for p in &plans {
        std::fs::write(workspace.join(&p.document), pick(p))?;
        tangle_document(&workspace.join(&p.document), workspace, registry)?;
    }
    Ok(())
};
write_all(|p| &p.after)?;
let new_bytes = std::fs::read(workspace.join(&to)).ok();
if new_bytes.as_deref() != Some(old_bytes.as_slice()) {
    let _ = std::fs::remove_file(workspace.join(&to));
    write_all(|p| &p.before)?;
    bail!("re-tangling did not reproduce {} at {}; every document is restored",
        from.display(), to.display());
}
std::fs::remove_file(workspace.join(&from))?;
Ok(plans.into_iter().map(|p| Retargeted { document: p.document, rewrites: p.rewrites }).collect())
```

## What the tests hold it to

Each test builds a scratch workspace with one or two chapters, tangles them,
retargets, and checks the chapter text, the bytes at the new path, the old
path's absence, and the tangle map. The refusals check that nothing changed.

<a name="chunk-test-support"></a><sub>[`src/retarget.rs`](../../crates/x0k-tangle/src/retarget.rs) · `#test-support` · assembles [tests](#chunk-tests)</sub>

```rust {#test-support}
#[cfg(test)]
mod tests {
    use super::*;

    fn workspace() -> tempfile::TempDir { tempfile::tempdir().unwrap() }

    fn chapter(ws: &Path, name: &str, header: &str, body: &str) -> PathBuf {
        let rel = PathBuf::from(format!("docs/{name}.md"));
        std::fs::create_dir_all(ws.join("docs")).unwrap();
        std::fs::write(ws.join(&rel), format!(
            "# {name}\n\n```turtle folio:document\nimplementation:{name} a x0k:Implementation ;\n{header} .\n```\n\n{body}")).unwrap();
        tangle_document(&ws.join(&rel), ws, &PipelineRegistry::default()).unwrap();
        rel
    }

    fn read(ws: &Path, p: &str) -> String { std::fs::read_to_string(ws.join(p)).unwrap() }

    fn run(ws: &Path, from: &str, to: &str, krate: Option<&str>) -> Result<Vec<Retargeted>> {
        retarget_output(ws, Path::new(from), Path::new(to), krate.map(Path::new), &PipelineRegistry::default())
    }

    <<tests>>
}
```

<a name="chunk-tests"></a><sub>[`src/retarget.rs`](../../crates/x0k-tangle/src/retarget.rs) · `#tests`</sub>

```rust {#tests}
#[test]
fn a_chunk_file_moves_within_its_crate_and_keeps_every_byte() {
    let ws = workspace();
    let doc = chapter(ws.path(), "a", "    folio:tangleCrate \"crates/c\" ;\n    folio:tangleRoot \"src/lib.rs\"",
        "```rust {#lib}\npub mod b;\n```\n\n```rust {#b file=\"src/b.rs\"}\npub fn b() {}\n```\n");
    let before = read(ws.path(), "crates/c/src/b.rs");
    let moved = run(ws.path(), "crates/c/src/b.rs", "crates/c/src/inner/b.rs", None).unwrap();
    assert_eq!(moved.len(), 1);
    assert!(read(ws.path(), doc.to_str().unwrap()).contains("file=\"src/inner/b.rs\""));
    assert_eq!(read(ws.path(), "crates/c/src/inner/b.rs"), before);
    assert!(!ws.path().join("crates/c/src/b.rs").exists());
    assert!(read(ws.path(), "crates/c/src/lib.rs").contains("pub mod b;"), "the other output stays");
    assert!(read(ws.path(), "docs/a.tangle-map.json").contains("crates/c/src/inner/b.rs"));
}

#[test]
fn a_root_moves_when_no_chunk_names_the_file() {
    let ws = workspace();
    let doc = chapter(ws.path(), "r", "    folio:tangleCrate \"crates/c\" ;\n    folio:tangleRoot \"src/x.rs\"",
        "```rust {#x}\npub fn x() {}\n```\n");
    let before = read(ws.path(), "crates/c/src/x.rs");
    run(ws.path(), "crates/c/src/x.rs", "crates/c/src/y.rs", None).unwrap();
    assert!(read(ws.path(), doc.to_str().unwrap()).contains("folio:tangleRoot \"src/y.rs\""));
    assert_eq!(read(ws.path(), "crates/c/src/y.rs"), before);
    assert!(!ws.path().join("crates/c/src/x.rs").exists());
}

#[test]
fn a_whole_document_moves_to_another_crate() {
    let ws = workspace();
    let doc = chapter(ws.path(), "e", "    folio:tangleCrate \"crates/ops\" ;\n    folio:tangleRoot \"src/edge.rs\"",
        "```rust {#edge}\npub struct Edge;\n```\n");
    let before = read(ws.path(), "crates/ops/src/edge.rs");
    run(ws.path(), "crates/ops/src/edge.rs", "edges/edge/src/lib.rs", Some("edges/edge")).unwrap();
    let text = read(ws.path(), doc.to_str().unwrap());
    assert!(text.contains("folio:tangleCrate \"edges/edge\"") && text.contains("folio:tangleRoot \"src/lib.rs\""), "{text}");
    assert_eq!(read(ws.path(), "edges/edge/src/lib.rs"), before);
    assert!(!ws.path().join("crates/ops/src/edge.rs").exists());
}

#[test]
fn a_crate_move_of_a_document_with_other_outputs_is_refused_untouched() {
    let ws = workspace();
    let doc = chapter(ws.path(), "m", "    folio:tangleCrate \"crates/ops\" ;\n    folio:tangleRoot \"src/lib.rs\"",
        "```rust {#lib}\npub mod b;\n```\n\n```rust {#b file=\"src/b.rs\"}\npub fn b() {}\n```\n");
    let text = read(ws.path(), doc.to_str().unwrap());
    let err = run(ws.path(), "crates/ops/src/b.rs", "edges/b/src/lib.rs", Some("edges/b")).err().unwrap();
    assert!(err.to_string().contains("also writes"), "{err}");
    assert_eq!(read(ws.path(), doc.to_str().unwrap()), text);
    assert!(ws.path().join("crates/ops/src/b.rs").exists() && !ws.path().join("edges").exists());
}

#[test]
fn a_target_outside_the_crate_needs_crate_and_nothing_is_touched() {
    let ws = workspace();
    let doc = chapter(ws.path(), "o", "    folio:tangleCrate \"crates/c\" ;\n    folio:tangleRoot \"src/x.rs\"",
        "```rust {#x}\npub fn x() {}\n```\n");
    let text = read(ws.path(), doc.to_str().unwrap());
    let err = run(ws.path(), "crates/c/src/x.rs", "crates/d/src/x.rs", None).err().unwrap();
    assert!(err.to_string().contains("--crate"), "{err}");
    assert_eq!(read(ws.path(), doc.to_str().unwrap()), text);
}

#[test]
fn an_output_no_map_records_is_refused() {
    let ws = workspace();
    std::fs::create_dir_all(ws.path().join("src")).unwrap();
    std::fs::write(ws.path().join("src/hand.rs"), "fn main() {}\n").unwrap();
    let err = run(ws.path(), "src/hand.rs", "src/moved.rs", None).err().unwrap();
    assert!(err.to_string().contains("no document"), "{err}");
    assert!(ws.path().join("src/hand.rs").exists());
}
```
