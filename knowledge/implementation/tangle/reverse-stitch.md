---
x0k:
  format: folio/v1
  id: x0k:implementation/tangle/reverse-stitch
  type: implementation
  status: draft
  summary: "Lifting an edited generated file back through the sidecar's line ranges into a patch against its document. Exported and tested but unwired: nothing in the tree calls it today."
  concerns: [tangle, stitch, reverse-tangle, sidecar, chunks]
  tangle:
    crate: x0k-tangle
    root: src/stitch.rs
  edges:
    implements:
      - x0k:design/literate-programming
    cites:
      - x0k:implementation/tangle/source-sync
      - x0k:implementation/tangle/identity-pipeline
---

# Reading an edited output back through the sidecar

When someone edits a generated `.rs` file directly — against the rule, but
it happens — the tangle map sidecar still knows which output lines came from
which chunk. Stitching uses that map to lift each chunk's current output
lines back into a patch against the document's source lines, so the edit can
be carried into the [literate source](../../wiki/literate-programming.md "x0k:wiki/literate-programming") instead
of being overwritten by the
next tangle. It is the sidecar-driven complement to
`x0k:implementation/tangle/source-sync`, which pulls bodies from *named
symbols* rather than from line ranges.

Nothing in the tree calls `stitch` today; the module is exported and tested
but unwired. The edit-generated-files rule is enforced by convention and by
the drift gate rather than by this path, and the sidecar's line ranges are
the only place the reverse mapping exists.

<a name="chunk-module-doc"></a><sub>[`src/stitch.rs`](../../../x0k-tangle/src/stitch.rs) · `#module-doc`</sub>

```rust {#module-doc}
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
```

## The map this reads

The sidecar shape stitching reads is the line-range map: per chunk, the
output lines it produced and the document lines it came from, under the
source path and the two content hashes. This is the only reader of that
shape, so the types live here with it.

<a name="chunk-tangle-map"></a><sub>[`src/stitch.rs`](../../../x0k-tangle/src/stitch.rs) · `#tangle-map`</sub>

```rust {#tangle-map}
#[derive(Debug, Serialize, Deserialize)]
pub struct TangleMap {
    pub source: String,
    pub source_hash: String,
    pub output_hash: String,
    pub chunks: Vec<TangleMapEntry>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TangleMapEntry {
    pub name: String,
    pub output_lines: [usize; 2],
    pub source_lines: [usize; 2],
}
```

## A patch

A patch names a chunk, carries the body the output currently holds, and the
document line range the sidecar recorded for that chunk.

<a name="chunk-stitch-patch"></a><sub>[`src/stitch.rs`](../../../x0k-tangle/src/stitch.rs) · `#stitch-patch`</sub>

```rust {#stitch-patch}
pub struct StitchPatch {
    pub chunk_name: String,
    pub new_body: String,
    pub source_line_start: usize,
    pub source_line_end: usize,
}
```

## Lifting output lines into patches

For every chunk in the map whose recorded output range still fits the file,
the current lines of that range become the patch body. Every chunk produces
a patch; the caller decides which differ from the document.

The lift is not quite verbatim, because the tangle is not: a document line
`<<!name>>` — the verbatim escape ([`resolution.md`](resolution.md)) —
tangles to the literal `<<name>>`, and a lifted `<<name>>` fed back into the
document would be a *reference* on the next tangle, expanding a chunk where
the author wanted text. So a lifted line that reads as a ref is re-escaped
to `<<!name>>` — unless the document's recorded body for that chunk already
holds that exact bare line, which is the one way a bare `<<name>>` reaches
an output: sitting inside a string literal or comment the language-aware
scanner leaves alone ([`chunk-refs.md`](chunk-refs.md)). The markdown
content is read for that check and nothing else; the sidecar's
`source_lines` are the fence line (1-indexed) and the line after the last
body line, so the body is the half-open range between them.

<a name="chunk-stitch"></a><sub>[`src/stitch.rs`](../../../x0k-tangle/src/stitch.rs) · `#stitch`</sub>

```rust {#stitch}
pub fn stitch(
    sidecar_path: &Path,
    rs_content: &str,
    md_content: &str,
) -> Result<Vec<StitchPatch>> {
    let sidecar_str = std::fs::read_to_string(sidecar_path)?;
    let map: TangleMap = serde_json::from_str(&sidecar_str)?;

    let rs_lines: Vec<&str> = rs_content.lines().collect();
    let md_lines: Vec<&str> = md_content.lines().collect();
    let mut patches = Vec::new();

    for entry in &map.chunks {
        let [out_start, out_end] = entry.output_lines;

        if out_start >= rs_lines.len() || out_end > rs_lines.len() {
            continue;
        }

        // The document body the sidecar recorded for this chunk: the lines
        // strictly between the opening fence and the recorded end.
        let [src_start, src_end] = entry.source_lines;
        let recorded_body: &[&str] = if src_start < src_end && src_end <= md_lines.len() + 1 {
            &md_lines[src_start..src_end - 1]
        } else {
            &[]
        };

        let current_body: String = rs_lines[out_start..out_end]
            .iter()
            .map(|line| lift_line(line, recorded_body))
            .collect::<Vec<String>>()
            .join("\n");

        // Always produce a patch — the caller diffs against the md content
        patches.push(StitchPatch {
            chunk_name: entry.name.clone(),
            new_body: current_body,
            source_line_start: entry.source_lines[0],
            source_line_end: entry.source_lines[1],
        });
    }

    Ok(patches)
}

/// An output line, as the document should hold it. A line the scanner
/// would read as a ref (`<<name>>`, indentation allowed) is re-escaped to
/// `<<!name>>` unless the recorded body already holds it bare.
fn lift_line(line: &str, recorded_body: &[&str]) -> String {
    let trimmed = line.trim_start();
    let is_ref = trimmed
        .strip_prefix("<<")
        .and_then(|rest| rest.strip_suffix(">>"))
        .map(|inner| {
            let inner = inner.trim();
            !inner.is_empty() && !inner.contains(' ')
        })
        .unwrap_or(false);
    if is_ref && !recorded_body.contains(&line) {
        let indent = &line[..line.len() - trimmed.len()];
        format!("{indent}<<!{}>>", &trimmed[2..trimmed.len() - 2].trim())
    } else {
        line.to_string()
    }
}
```

## Applying patches to the document

Application walks the document's lines. At a line that a patch names as its
chunk start, the line is kept as the fence, the new body follows, and the
old body is skipped through to the recorded end. The oddly indented comment
inside the loop is a formatter artifact carried verbatim: this module is
projected byte-for-byte, so the chunk layout is the canonical formatting.

<a name="chunk-apply-patches"></a><sub>[`src/stitch.rs`](../../../x0k-tangle/src/stitch.rs) · `#apply-patches`</sub>

```rust {#apply-patches}
pub fn apply_patches(md_content: &str, patches: &[StitchPatch]) -> Result<String> {
    let md_lines: Vec<&str> = md_content.lines().collect();
    let mut result = Vec::new();
    let mut skip_until: Option<usize> = None;

    for (i, line) in md_lines.iter().enumerate() {
        let line_num = i + 1; // 1-indexed

        if let Some(end) = skip_until {
            if line_num < end {
                continue;
            }
            skip_until = None;
        }

        // Check if this line is the start of a chunk that has a patch
        if let Some(patch) = patches.iter().find(|p| p.source_line_start == line_num) {
            // Find the code fence opening — we're at the line the sidecar
            // recorded as the chunk start. Walk backwards to find the ``` line.
            // Actually, the source_line points at the code block content start,
            // so we emit the fence, then the new body, then skip to the old end.
            result.push(*line); // keep the ``` fence line
                                // Push new body lines
            for body_line in patch.new_body.lines() {
                result.push(body_line);
            }
            // Skip old body lines until source_line_end
            skip_until = Some(patch.source_line_end);
            continue;
        }

        result.push(*line);
    }

    Ok(result.join("\n"))
}
```

## Tests

The round-trip case pins the escape: a document whose chunk holds
`<<!greet>>` tangles to an output holding `<<greet>>`; lifting that output
back yields `<<!greet>>` again, and applying the patch reproduces the
document. A bare `<<greet>>` that the document already holds (inside a
string literal, say) lifts as itself.

<a name="chunk-tests"></a><sub>[`src/stitch.rs`](../../../x0k-tangle/src/stitch.rs) · `#tests`</sub>

```rust {#tests}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stitch_patch_creation() {
        let patch = StitchPatch {
            chunk_name: "test".to_string(),
            new_body: "new code".to_string(),
            source_line_start: 5,
            source_line_end: 8,
        };
        assert_eq!(patch.chunk_name, "test");
        assert_eq!(patch.source_line_start, 5);
    }

    #[test]
    fn a_lifted_literal_ref_line_is_re_escaped_and_round_trips() {
        // The chunk holds a bare `<<greet>>` inside a raw string (left
        // alone by the language-aware scanner) and an escaped one outside.
        let md = "# Doc\n\n```rust {#demo}\nlet s = r#\"\n<<greet>>\n\"#;\n    <<!greet>>\n```\n";
        let rs = "let s = r#\"\n<<greet>>\n\"#;\n    <<greet>>\n";
        let map = TangleMap {
            source: "doc.md".to_string(),
            source_hash: String::new(),
            output_hash: String::new(),
            chunks: vec![TangleMapEntry {
                name: "demo".to_string(),
                output_lines: [0, 4],
                // Fence on line 3 (1-indexed); the body is lines 4–7.
                source_lines: [3, 8],
            }],
        };
        let dir = tempfile::tempdir().unwrap();
        let sidecar = dir.path().join("doc.tangle-map.json");
        std::fs::write(&sidecar, serde_json::to_string(&map).unwrap()).unwrap();

        let patches = stitch(&sidecar, rs, md).unwrap();
        assert_eq!(patches.len(), 1);
        assert_eq!(
            patches[0].new_body,
            "let s = r#\"\n<<greet>>\n\"#;\n    <<!greet>>"
        );
        assert_eq!(apply_patches(md, &patches).unwrap(), md.trim_end_matches('\n'));
    }
}
```

## The file

<a name="chunk-root"></a><sub>[`src/stitch.rs`](../../../x0k-tangle/src/stitch.rs) · `#root` · assembles [module-doc](#chunk-module-doc) · [tangle-map](#chunk-tangle-map) · [stitch-patch](#chunk-stitch-patch) · [stitch](#chunk-stitch) · [apply-patches](#chunk-apply-patches) · [tests](#chunk-tests)</sub>

```rust {#root}
<<module-doc>>

<<tangle-map>>

<<stitch-patch>>

<<stitch>>

<<apply-patches>>

<<tests>>
```

The sidecar records line ranges, and line ranges are exactly what an edit
invalidates: a chunk that grew by two lines shifts every chunk after it.
Stitching is sound only for the first edited chunk in a file, which is why
a symbol-addressed sync is the direction that got wired.
