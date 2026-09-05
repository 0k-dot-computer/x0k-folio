---
x0k:
  format: folio/v1
  id: x0k:implementation/tangle/region-gfm
  type: implementation
  status: draft
  summary: "A chapter woven for a forge's renderer — a caption over every named fence, x0k: links rewritten to shipped paths — under two tested invariants, the same tangle and line-for-line inversion; and an affordance's section woven with the evidence its record holds."
  concerns: [tangle, publishing, weave, markdown, github, affordances]
  tangle:
    crate: x0k-tangle
    root: src/region_gfm.rs
  edges:
    implements:
      - x0k:design/publish-a-region-as-a-repository
    cites:
      - x0k:implementation/tangle/region-repo
      - x0k:implementation/tangle/receiving
      - x0k:implementation/tangle/weave
---
# Weaving a chapter for a forge

A [literate program](../../wiki/literate-programming.md "x0k:wiki/literate-programming") has two projections:
the tangle, which is the code, and the weave, which is the document as
something to read. The [HTML weave](weave.md) is one of the second kind,
and it controls its renderer. A public repository does not: the chapter a
reader opens on GitHub is rendered by GitHub, which keeps one word of a
fence's info-string — so `{#name file="…" proves="…"}` is invisible and a
reader cannot see which file a block lands in or what it proves — leaves
`<<name>>` as literal text inside the code, and has no idea what an
`x0k:` link is. Shipping the corpus bytes and calling them readable was
the state of things until 2026-09-05, when the operator asked whether
what you read on GitHub is *a woven projection of the actual source*. It
is now, and this chapter is that weave: the third projection, markdown
to markdown, for a renderer we do not own.

Its two invariants are what make it a projection rather than a fork.
**The woven chapter tangles to the bytes the source does.** Fences and
their info-strings are never edited; everything the weave adds is prose
around them, so the tangler — which reads only the fences — sees the same
document, and the projected repository's own gate (re-tangle every
chapter, require a clean tree) stays exact. **The weave inverts line for
line.** Every line it adds is marked, every link it rewrites keeps its
original as the link's title, so `unweave_chapter` recovers the source
byte for byte and a contribution made against the woven chapter can be
routed back to the corpus document ([`receiving.md`](receiving.md)
unweaves both sides before it diffs). A source that already holds a woven
line is refused rather than woven twice: the inverse must be exact, and
an ambiguous line would make it a guess.

<a name="chunk-module-doc"></a><sub>[`src/region_gfm.rs`](../../../x0k-tangle/src/region_gfm.rs) · `#module-doc`</sub>

```rust {#module-doc}
//! Weaving a literate chapter for a forge's renderer, and an affordance's
//! section with its evidence.
//!
//! GitHub renders the corpus bytes badly: it keeps one word of a fence's
//! info-string, so `{#name file= proves=}` is invisible, `<<name>>` is
//! literal, and an `x0k:` link is dead. [`weave_chapter`] adds a caption
//! above every named fence and rewrites `x0k:` links to the shipped path,
//! touching nothing else, under two invariants both pinned by tests here:
//! the woven chapter tangles to the same bytes, and [`unweave_chapter`]
//! recovers the source line for line. [`weave_affordance_section`] does
//! the second job: under an affordance's declaration it writes what the
//! projector's record knows — who it is for, the cues that reach it, the
//! chapters that realize it, and each proof test with its outcome and its
//! body under `<details>`.
```

<a name="chunk-imports"></a><sub>[`src/region_gfm.rs`](../../../x0k-tangle/src/region_gfm.rs) · `#imports`</sub>

```rust {#imports}
use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{bail, Result};
use x0k_folio::colophon::{parse_envelope, split_frontmatter};

use crate::chunk_refs::find_chunk_refs_aware;
use crate::parser::parse_info_string;
```

## The segments of a body

Both weaves walk the body the same way: as a run of prose lines and
fenced blocks, where a fence opens on a run of three or more backticks
or tildes and closes on a run of the same character at least as long.
That is CommonMark's rule, and it is also the tangler's, which is why
the weave can promise to leave every fence exactly where the tangler
finds it. An indented or nested fence — a chapter quoting markdown
inside a four-backtick block — is a fence all the same; what is inside
one is not prose and is not read.

<a name="chunk-segments"></a><sub>[`src/region_gfm.rs`](../../../x0k-tangle/src/region_gfm.rs) · `#segments`</sub>

```rust {#segments}
/// One piece of a body: a line of prose, or a whole fenced block with
/// its opening line, its info-string, its body lines and its closing
/// line (absent when the document ends inside the fence).
enum Segment<'a> {
    Line(&'a str),
    Fence {
        open: &'a str,
        info: String,
        body: Vec<&'a str>,
        close: Option<&'a str>,
    },
}

/// The fence run a line opens or closes: its character and length, when
/// the line starts (after indentation) with three or more of one.
fn fence_run(line: &str) -> Option<(char, usize)> {
    let t = line.trim_start();
    let c = t.chars().next().filter(|c| *c == '`' || *c == '~')?;
    let n = t.chars().take_while(|x| *x == c).count();
    (n >= 3).then_some((c, n))
}

/// Cut a body into segments. Lines keep their terminators, so the
/// segments concatenate back to the body byte for byte.
fn segments(body: &str) -> Vec<Segment<'_>> {
    let mut out = Vec::new();
    let mut open: Option<(char, usize, &str, String, Vec<&str>)> = None;
    for line in body.split_inclusive('\n') {
        let run = fence_run(line);
        if let Some((c, n, _, _, _)) = &open {
            if run.is_some_and(|(c2, n2)| c2 == *c && n2 >= *n) {
                let (_, _, open_line, info, body) = open.take().unwrap();
                out.push(Segment::Fence { open: open_line, info, body, close: Some(line) });
                continue;
            }
        }
        if let Some((_, _, _, _, body)) = &mut open {
            body.push(line);
            continue;
        }
        match run {
            Some((c, n)) => {
                let info = line.trim_start()[n..].trim().to_string();
                open = Some((c, n, line, info, Vec::new()));
            }
            None => out.push(Segment::Line(line)),
        }
    }
    if let Some((_, _, open_line, info, body)) = open {
        out.push(Segment::Fence { open: open_line, info, body, close: None });
    }
    out
}
```

## The chapter weave

What the weave needs to know beyond the chapter is where things landed:
which `x0k:` ids the projection ships and at what path, and the title
and page of each affordance a fence may say it proves. Both are the
projector's to supply; the weave resolves and never guesses, so an id
the projection does not carry is left as the author wrote it — an
`x0k:` link out of the region dangles on the forge exactly as the edge
dangles in the graph, which is the ordinary case for a published region.

<a name="chunk-chapter-links"></a><sub>[`src/region_gfm.rs`](../../../x0k-tangle/src/region_gfm.rs) · `#chapter-links`</sub>

```rust {#chapter-links}
/// Where the projection put things, for the weave to link to.
pub struct ChapterLinks<'a> {
    /// `x0k:` id → projection-relative path of the document that carries
    /// it: every literate chapter by its `id:`, every named document by
    /// the reference the publication wrote (a section under its
    /// `<id>#<anchor>` reference).
    pub uri_to_rel: &'a BTreeMap<String, String>,
    /// Affordance id → (title, projection-relative page) for what a
    /// fence may say it proves.
    pub affordances: &'a BTreeMap<String, (String, String)>,
}

/// The mark every caption line starts with; what [`unweave_chapter`]
/// drops, and what a source may not already hold.
pub const CAPTION_MARK: &str = "<a name=\"chunk-";
```

The caption sits above the fence, small, as the tangler's own reading of
it: the file the chunk lands in (linked, when the crate ships beside the
chapter), the chunk's name under an anchor, what it proves (linked to
the affordance's page), and what it assembles — each `<<name>>` in the
body as a link to that chunk's anchor, so the literal reference inside
the code has a live twin above it. A chunk that appears twice is one
chunk continued, and its second caption says so under a second anchor.

<a name="chunk-weave-chapter"></a><sub>[`src/region_gfm.rs`](../../../x0k-tangle/src/region_gfm.rs) · `#weave-chapter`</sub>

```rust {#weave-chapter}
/// Weave one chapter for a forge: a caption above every named fence,
/// `x0k:` links rewritten to the shipped path (the id kept as the link's
/// title), nothing else touched. `rel` is the chapter's projection-relative
/// path, from which every link is made relative; `crate_name` the crate
/// its code lands in, for the file captions. Refuses a chapter that
/// already holds a woven line, so the inverse stays exact.
pub fn weave_chapter(
    text: &str,
    rel: &str,
    crate_name: Option<&str>,
    links: &ChapterLinks,
) -> Result<String> {
    let Some((_, body)) = split_frontmatter(text) else {
        bail!("{rel} carries no folio/v1 envelope");
    };
    let head = &text[..text.len() - body.len()];
    let root = parse_envelope(text)
        .ok()
        .and_then(|(env, _)| env.tangle.and_then(|t| t.root));
    let mut out = String::with_capacity(text.len() + text.len() / 4);
    out.push_str(head);
    let mut seen: BTreeMap<String, usize> = BTreeMap::new();
    for segment in segments(body) {
        match segment {
            Segment::Line(line) => {
                refuse_woven_line(line, rel)?;
                out.push_str(&rewrite_links(line, |target| resolve_link(target, rel, links)));
            }
            Segment::Fence { open, info, body, close } => {
                for line in &body {
                    refuse_woven_line(line, rel)?;
                }
                let attrs = parse_info_string(&info);
                if let Some(name) = &attrs.name {
                    let times = seen.entry(name.clone()).or_insert(0);
                    *times += 1;
                    let file = attrs.file.as_ref().map(|f| f.to_string_lossy().to_string()).or_else(|| root.clone());
                    let body_text: String = body.concat();
                    let refs = find_chunk_refs_aware(&body_text, attrs.lang.as_deref());
                    let mut parts: Vec<String> = Vec::new();
                    if let Some(file) = &file {
                        parts.push(match crate_name {
                            Some(krate) => format!("[`{file}`]({})", relative_link(rel, &format!("{krate}/{file}"))),
                            None => format!("`{file}`"),
                        });
                    }
                    let anchor = if *times == 1 {
                        format!("chunk-{name}")
                    } else {
                        format!("chunk-{name}-{times}")
                    };
                    parts.push(if *times == 1 {
                        format!("`#{name}`")
                    } else {
                        format!("`#{name}` · continues")
                    });
                    for id in &attrs.proves {
                        parts.push(match links.affordances.get(id) {
                            Some((title, page)) => format!("proves [{title}]({})", relative_link(rel, page)),
                            None => format!("proves `{id}`"),
                        });
                    }
                    let mut assembled: Vec<String> = Vec::new();
                    for r in refs.iter().filter(|r| !r.escaped) {
                        let item = match &r.doc_uri {
                            Some(uri) => format!("`{uri}::{}`", r.name),
                            None => format!("[{}](#chunk-{})", r.name, r.name),
                        };
                        if !assembled.contains(&item) {
                            assembled.push(item);
                        }
                    }
                    if !assembled.is_empty() {
                        parts.push(format!("assembles {}", assembled.join(" · ")));
                    }
                    out.push_str(&format!("{CAPTION_MARK}{}\"></a><sub>{}</sub>\n\n", &anchor["chunk-".len()..], parts.join(" · ")));
                }
                out.push_str(open);
                for line in body {
                    out.push_str(line);
                }
                if let Some(close) = close {
                    out.push_str(close);
                }
            }
        }
    }
    Ok(out)
}

/// A line the weave would have written, found in a source: refused, so
/// that the inverse of the weave is never a guess. The two shapes are
/// exactly what [`unweave_chapter`] acts on — a caption line, and a link
/// rewritten to `](<path> "x0k:…")` — so a bare `"x0k:…"` in a code
/// fence (a test's string literal) is not one.
fn refuse_woven_line(line: &str, rel: &str) -> Result<()> {
    if line.starts_with(CAPTION_MARK) || rewritten_link(line).is_some() {
        bail!(
            "{rel} already holds a woven line and cannot be woven again — \
             `{}`",
            line.trim_end()
        );
    }
    Ok(())
}

/// Where an `x0k:` link target lands, relative to the chapter at `rel`:
/// the exact id when the projection carries it, else its document with
/// the fragment carried along, else nothing.
fn resolve_link(target: &str, rel: &str, links: &ChapterLinks) -> Option<String> {
    if let Some(dest) = links.uri_to_rel.get(target) {
        return Some(relative_link(rel, dest));
    }
    let (base, fragment) = target.split_once('#')?;
    let dest = links.uri_to_rel.get(base)?;
    Some(format!("{}#{fragment}", relative_link(rel, dest)))
}
```

A link is rewritten only in prose. Inline code is skipped the way a
fence is: a chapter that shows the link syntax in a code span is not
linking, and rewriting it would change what the reader is shown as an
example. The scanner is the one the format library uses to read prose
edges, restated here for the rewrite rather than the read.

<a name="chunk-rewrite-links"></a><sub>[`src/region_gfm.rs`](../../../x0k-tangle/src/region_gfm.rs) · `#rewrite-links`</sub>

```rust {#rewrite-links}
/// Rewrite every `[…](x0k:…)` on a prose line whose target `resolve`
/// answers, to `[…](<path> "x0k:…")`; inline code spans are skipped and
/// every other byte is kept.
fn rewrite_links(line: &str, resolve: impl Fn(&str) -> Option<String>) -> String {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while !rest.is_empty() {
        let next_code = rest.find('`');
        let next_link = rest.find("](x0k:");
        if let Some(c) = next_code.filter(|c| next_link.is_none_or(|l| l >= *c)) {
            let (span, after) = code_span(&rest[c..]);
            out.push_str(&rest[..c]);
            out.push_str(span);
            rest = after;
        } else if let Some(l) = next_link {
            let after = &rest[l + 2..];
            let end = after.find(|ch: char| ch == ')' || ch.is_whitespace()).unwrap_or(after.len());
            let target = &after[..end];
            out.push_str(&rest[..l + 2]);
            match resolve(target) {
                Some(path) if after[end..].starts_with(')') => {
                    out.push_str(&format!("{path} \"{target}\""));
                }
                _ => out.push_str(target),
            }
            rest = &after[end..];
        } else {
            out.push_str(rest);
            break;
        }
    }
    out
}

/// `s` starts with a backtick run: the code span it opens (through the
/// matching closing run, or to the end of the line when it never
/// closes), and what follows it.
fn code_span(s: &str) -> (&str, &str) {
    let n = s.chars().take_while(|c| *c == '`').count();
    let body = &s[n..];
    let mut i = 0;
    while i < body.len() {
        if body[i..].starts_with('`') {
            let m = body[i..].chars().take_while(|c| *c == '`').count();
            if m == n {
                return s.split_at(n + i + m);
            }
            i += m;
        } else {
            i += body[i..].chars().next().map(char::len_utf8).unwrap_or(1);
        }
    }
    (s, "")
}
```

The inverse drops what the weave added and restores what it rewrote,
and nothing else: a caption line and the blank line under it; a link's
path in favour of the id in its title. It reads only the body — the
envelope was never woven — and it needs no knowledge of the projection,
which is what lets the receiver apply it to a clone it did not make.

<a name="chunk-unweave-chapter"></a><sub>[`src/region_gfm.rs`](../../../x0k-tangle/src/region_gfm.rs) · `#unweave-chapter`</sub>

```rust {#unweave-chapter}
/// Recover the source chapter from its woven form: caption lines (and
/// the blank line under each) dropped, rewritten links restored from
/// the id kept as their title. The envelope is not read.
pub fn unweave_chapter(text: &str) -> String {
    let Some((_, body)) = split_frontmatter(text) else {
        return text.to_string();
    };
    let head = &text[..text.len() - body.len()];
    let mut out = String::with_capacity(text.len());
    out.push_str(head);
    let mut lines = body.split_inclusive('\n').peekable();
    while let Some(line) = lines.next() {
        if line.starts_with(CAPTION_MARK) {
            if lines.peek().is_some_and(|l| l.trim().is_empty()) {
                lines.next();
            }
            continue;
        }
        out.push_str(&restore_links(line));
    }
    out
}

/// The first rewritten link on a line, as `(start of the target, start
/// of the title, end of the link)` — the byte after `](`, the space
/// before `"x0k:`, and the byte after `")`. `None` when the line holds
/// no link of that shape.
fn rewritten_link(line: &str) -> Option<(usize, usize, usize)> {
    let mut from = 0;
    while let Some(t) = line[from..].find(" \"x0k:").map(|i| i + from) {
        let close = line[t..].find("\")").map(|i| i + t);
        // The title has to be an id — a class and a stem — and the target
        // a path with no whitespace; prose *about* the shape, with an
        // ellipsis for the id, is not the shape.
        let is_id = |id: &str| id.contains('/') && !id.contains(char::is_whitespace) && !id.contains('…');
        match (line[..t].rfind("]("), close) {
            (Some(open), Some(close))
                if !line[open + 2..t].contains(char::is_whitespace) && is_id(&line[t + 2..close]) =>
            {
                return Some((open + 2, t, close + 2));
            }
            _ => from = t + 1,
        }
    }
    None
}

/// `[…](<path> "x0k:…")` back to `[…](x0k:…)`, every occurrence on the
/// line.
fn restore_links(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while let Some((target, title, end)) = rewritten_link(rest) {
        out.push_str(&rest[..target]);
        out.push_str(&rest[title + 2..end - 2]);
        out.push(')');
        rest = &rest[end..];
    }
    out.push_str(rest);
    out
}
```

## The affordance's page

An affordance crosses as the section of the design that declares it,
and that section is its page. The declaration says what the affordance
is; the projector's record knows what the declaration cannot — who it
is claimed for, which cues reach it, which chapters realize or present
it, and what each proof test did when the projector ran it. The weave
writes that under the declaration's block, and puts each test's body
under `<details>`, so the page reads as a claim with the evidence one
click down. The record arrives already rendered where rendering is the
projector's business (the glyphs, the actor phrase) and as data where it
is not.

<a name="chunk-affordance-evidence"></a><sub>[`src/region_gfm.rs`](../../../x0k-tangle/src/region_gfm.rs) · `#affordance-evidence`</sub>

```rust {#affordance-evidence}
/// What the projector knows about one affordance, for its page. Links
/// are already relative to the page.
pub struct AffordanceEvidence {
    /// The declared id, matched against the `id:` line of the block.
    pub id: String,
    /// The actor and status glyphs, rendered.
    pub glyphs: String,
    /// `proven`, `declared` or `claimed`.
    pub status: String,
    /// "a person, an agent" — empty when the declaration claims no actor.
    pub actors: String,
    /// `(surface, cue)` per signifier that reaches it.
    pub cues: Vec<(String, String)>,
    /// `(title, href)` of each chapter that realizes or presents it.
    pub chapters: Vec<(String, String)>,
    /// Each proof test, in proof order.
    pub proofs: Vec<ProofEvidence>,
}

/// One proof test: its name, what it did at projection (`None` when the
/// proofs did not run), the chapter and chunk that tangle it, and its
/// body as the chapter tangles it.
pub struct ProofEvidence {
    pub test: String,
    pub outcome: Option<String>,
    pub chapter: (String, String),
    pub chunk: String,
    pub source: String,
}

/// Weave the evidence under each `yaml x0k:affordance` block of `text`
/// whose `id:` an entry of `evidence` names. Blocks without evidence,
/// and everything else, are left as written.
pub fn weave_affordance_section(text: &str, evidence: &[AffordanceEvidence]) -> String {
    let Some((_, body)) = split_frontmatter(text) else {
        return text.to_string();
    };
    let head = &text[..text.len() - body.len()];
    let mut out = String::with_capacity(text.len() * 2);
    out.push_str(head);
    for segment in segments(body) {
        match segment {
            Segment::Line(line) => out.push_str(line),
            Segment::Fence { open, info, body, close } => {
                out.push_str(open);
                for line in &body {
                    out.push_str(line);
                }
                if let Some(close) = close {
                    out.push_str(close);
                }
                if info != "yaml x0k:affordance" {
                    continue;
                }
                let id = body
                    .iter()
                    .find_map(|l| l.trim().strip_prefix("id:"))
                    .map(|v| v.trim().to_string());
                if let Some(ev) = id.and_then(|id| evidence.iter().find(|e| e.id == id)) {
                    out.push('\n');
                    out.push_str(&render_evidence(ev));
                }
            }
        }
    }
    out
}

/// The evidence, as the page shows it.
fn render_evidence(ev: &AffordanceEvidence) -> String {
    let mut out = String::new();
    let mut line = format!("{} *{}*", ev.glyphs, ev.status);
    if !ev.actors.is_empty() {
        line.push_str(&format!(" · for {}", ev.actors));
    }
    if !ev.cues.is_empty() {
        let cues: Vec<String> = ev.cues.iter().map(|(s, c)| format!("`{s}` `{c}`")).collect();
        line.push_str(&format!(" · reachable through {}", cues.join(", ")));
    }
    out.push_str(&line);
    out.push_str("\n\n");
    if !ev.chapters.is_empty() {
        let chapters: Vec<String> =
            ev.chapters.iter().map(|(title, href)| format!("[{title}]({href})")).collect();
        out.push_str(&format!("*realized in* {}\n\n", chapters.join(" · ")));
    }
    if !ev.proofs.is_empty() {
        out.push_str("*proven by* each test below, as its chapter tangles it and as it ran at projection.\n\n");
        for p in &ev.proofs {
            let outcome = p.outcome.as_deref().unwrap_or("not run");
            out.push_str(&format!(
                "<details><summary><code>{}</code> · {outcome} · <a href=\"{}#chunk-{}\">#{}</a> in {}</summary>\n\n```rust\n{}\n```\n\n</details>\n\n",
                p.test,
                p.chapter.1,
                p.chunk,
                p.chunk,
                p.chapter.0,
                p.source.trim_end()
            ));
        }
    }
    out
}
```

A page links from where it sits. The projection's paths are all relative
to its root, and a page under `decisions/design/corpus/<stem>/` has to
climb out before it can reach `knowledge/implementation/`; the helper is
the one place that arithmetic is done.

<a name="chunk-relative-link"></a><sub>[`src/region_gfm.rs`](../../../x0k-tangle/src/region_gfm.rs) · `#relative-link`</sub>

```rust {#relative-link}
/// The path from the file at `from` to the file at `to`, both
/// projection-relative: `../` per directory `from` sits in below their
/// common prefix, then the rest of `to`.
pub fn relative_link(from: &str, to: &str) -> String {
    let from_dir: Vec<&str> = Path::new(from)
        .parent()
        .map(|p| p.to_str().unwrap_or("").split('/').filter(|s| !s.is_empty()).collect())
        .unwrap_or_default();
    let to_parts: Vec<&str> = to.split('/').filter(|s| !s.is_empty()).collect();
    let common = from_dir.iter().zip(to_parts.iter()).take_while(|(a, b)| a == b).count();
    let mut parts: Vec<&str> = vec![".."; from_dir.len() - common];
    parts.extend_from_slice(&to_parts[common..]);
    parts.join("/")
}
```

## Signifier

The forge weave is the same affordance the HTML weave presents, on a
second surface: a document read as the woven artifact, here by a
renderer that is not ours.

```yaml x0k:signifier
id: x0k:signifier/x0k-tangle-weave-chapter
cue: weave_chapter
edges:
  signifies:
    - x0k:affordance/weave_a_document
  presentedOn:
    - x0k:surface/sdk
```

## Tests

The two invariants first, on a chapter that exercises every rule: a
rooted chunk and a filed one, a chunk that proves and assembles, a
repeated chunk, a link the projection carries and one it does not, a
link in a code span, and a nested fence showing the syntax.

<a name="chunk-tests"></a><sub>[`src/region_gfm.rs`](../../../x0k-tangle/src/region_gfm.rs) · `#tests` · proves [Read a document as the woven artifact](../../../decisions/design/corpus/literate-programming/read-a-document-as-the-woven-artifact.md)</sub>

```rust {#tests proves="x0k:affordance/weave_a_document"}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_document;

    const CHAPTER: &str = "---\nx0k:\n  format: folio/v1\n  id: x0k:implementation/demo/lines\n  type: implementation\n  status: draft\n  summary: A demo.\n  tangle:\n    crate: demo-crate\n    root: src/lib.rs\n---\n# Lines\n\nReads [first lines](x0k:wiki/first-lines) and [elsewhere](x0k:wiki/elsewhere);\nsee `[not a link](x0k:wiki/first-lines)` and [the design](x0k:design/demo-design#read-a-line).\n\n````markdown\n```rust {#shown}\n<<not-assembled>>\n```\n````\n\n```rust {#parse}\npub fn parse_line(s: &str) -> &str { s.lines().next().unwrap_or(\"\").trim() }\n```\n\n```rust {#root}\n<<parse>>\n<<parse>>\n<<!parse>>\n```\n\n```rust {#parse}\n// continued\n```\n\n```rust {#tests file=\"tests/proof.rs\" proves=\"x0k:affordance/read_a_line\"}\n#[test]\nfn a_line_is_read() { assert_eq!(demo_crate::parse_line(\" a \\nb\"), \"a\"); }\n```\n";

    fn links() -> (BTreeMap<String, String>, BTreeMap<String, (String, String)>) {
        let mut uri = BTreeMap::new();
        uri.insert("x0k:wiki/first-lines".to_string(), "knowledge/wiki/first-lines.md".to_string());
        uri.insert("x0k:design/demo-design".to_string(), "decisions/design/demo-design.md".to_string());
        let mut aff = BTreeMap::new();
        aff.insert(
            "x0k:affordance/read_a_line".to_string(),
            ("Read a line".to_string(), "decisions/design/demo-design/read-a-line.md".to_string()),
        );
        (uri, aff)
    }

    fn woven() -> String {
        let (uri, aff) = links();
        let links = ChapterLinks { uri_to_rel: &uri, affordances: &aff };
        weave_chapter(CHAPTER, "knowledge/implementation/demo/lines.md", Some("demo-crate"), &links).unwrap()
    }

    #[test]
    fn the_weave_inverts_line_for_line() {
        assert_eq!(unweave_chapter(&woven()), CHAPTER);
    }

    #[test]
    fn the_woven_chapter_tangles_to_the_same_chunks() {
        let a = parse_document(CHAPTER).unwrap();
        let b = parse_document(&woven()).unwrap();
        assert_eq!(a.chunk_order, b.chunk_order);
        for name in &a.chunk_order {
            let (ca, cb) = (a.chunk_variants(name).unwrap(), b.chunk_variants(name).unwrap());
            assert_eq!(ca.len(), cb.len(), "{name}");
            for (x, y) in ca.iter().zip(cb.iter()) {
                assert_eq!(x.file_target, y.file_target, "{name}");
                assert_eq!(x.proves, y.proves, "{name}");
                let (bx, by): (Vec<&str>, Vec<&str>) =
                    (x.bodies.iter().map(|b| b.text.as_str()).collect(), y.bodies.iter().map(|b| b.text.as_str()).collect());
                assert_eq!(bx, by, "{name}");
            }
        }
    }

    #[test]
    fn captions_say_file_name_proof_and_assembly_and_links_land() {
        let w = woven();
        assert!(w.contains("<a name=\"chunk-parse\"></a><sub>[`src/lib.rs`](../../../demo-crate/src/lib.rs) · `#parse`</sub>\n\n```rust {#parse}\n"), "{w}");
        assert!(w.contains("<a name=\"chunk-root\"></a><sub>[`src/lib.rs`](../../../demo-crate/src/lib.rs) · `#root` · assembles [parse](#chunk-parse)</sub>\n\n```rust {#root}\n"), "once, and never the escaped ref: {w}");
        assert!(w.contains("<a name=\"chunk-parse-2\"></a><sub>[`src/lib.rs`](../../../demo-crate/src/lib.rs) · `#parse` · continues</sub>\n\n"), "{w}");
        assert!(w.contains("<a name=\"chunk-tests\"></a><sub>[`tests/proof.rs`](../../../demo-crate/tests/proof.rs) · `#tests` · proves [Read a line](../../../decisions/design/demo-design/read-a-line.md)</sub>\n\n"), "{w}");
        assert!(w.contains("Reads [first lines](../../wiki/first-lines.md \"x0k:wiki/first-lines\") and [elsewhere](x0k:wiki/elsewhere);\n"), "a carried link lands, an uncarried one is left: {w}");
        assert!(w.contains("see `[not a link](x0k:wiki/first-lines)` and [the design](../../../decisions/design/demo-design.md#read-a-line \"x0k:design/demo-design#read-a-line\").\n"), "{w}");
        assert!(!w.contains("chunk-shown"), "a fence inside a fence is not a chunk: {w}");
    }

    #[test]
    fn a_string_literal_naming_an_id_is_not_a_woven_line() {
        let (uri, aff) = links();
        let links = ChapterLinks { uri_to_rel: &uri, affordances: &aff };
        let chapter = CHAPTER.replace(
            "```rust {#parse}\n",
            "```rust {#parse}\nconst ID: &str = \"x0k:design/example\";\n",
        );
        let woven = weave_chapter(&chapter, "knowledge/implementation/demo/lines.md", Some("demo-crate"), &links)
            .expect("a literal is not a rewritten link");
        assert_eq!(unweave_chapter(&woven), chapter);
    }

    #[test]
    fn a_source_holding_a_woven_line_is_refused() {
        let (uri, aff) = links();
        let links = ChapterLinks { uri_to_rel: &uri, affordances: &aff };
        let err = weave_chapter(&woven(), "knowledge/implementation/demo/lines.md", Some("demo-crate"), &links)
            .expect_err("woven twice");
        assert!(err.to_string().contains("already holds a woven line"), "{err}");
    }

    #[test]
    fn the_affordance_section_carries_the_evidence_under_its_block() {
        let page = "---\nx0k:\n  format: folio/v1\n  id: x0k:design/demo-design#read-a-line\n  type: design\n---\n\n### Read a line\n\nI read a line.\n\n```yaml x0k:affordance\nid: x0k:affordance/read_a_line\nactors: [human]\n```\n\nAfter.\n";
        let ev = AffordanceEvidence {
            id: "x0k:affordance/read_a_line".to_string(),
            glyphs: "<img alt=\"proven\">".to_string(),
            status: "proven".to_string(),
            actors: "a person".to_string(),
            cues: vec![("cli".to_string(), "demo read".to_string())],
            chapters: vec![("Lines".to_string(), "../../../knowledge/implementation/demo/lines.md".to_string())],
            proofs: vec![ProofEvidence {
                test: "a_line_is_read".to_string(),
                outcome: Some("passed".to_string()),
                chapter: ("Lines".to_string(), "../../../knowledge/implementation/demo/lines.md".to_string()),
                chunk: "tests".to_string(),
                source: "#[test]\nfn a_line_is_read() {}\n".to_string(),
            }],
        };
        let out = weave_affordance_section(page, &[ev]);
        let expected = "```yaml x0k:affordance\nid: x0k:affordance/read_a_line\nactors: [human]\n```\n\n<img alt=\"proven\"> *proven* · for a person · reachable through `cli` `demo read`\n\n*realized in* [Lines](../../../knowledge/implementation/demo/lines.md)\n\n*proven by* each test below, as its chapter tangles it and as it ran at projection.\n\n<details><summary><code>a_line_is_read</code> · passed · <a href=\"../../../knowledge/implementation/demo/lines.md#chunk-tests\">#tests</a> in Lines</summary>\n\n```rust\n#[test]\nfn a_line_is_read() {}\n```\n\n</details>\n\n\nAfter.\n";
        assert!(out.ends_with(expected), "{out}");
        assert_eq!(weave_affordance_section(page, &[]), page, "no evidence, no change");
    }

    #[test]
    fn relative_links_climb_out_and_descend() {
        assert_eq!(relative_link("README.md", "knowledge/wiki/a.md"), "knowledge/wiki/a.md");
        assert_eq!(relative_link("knowledge/implementation/demo/lines.md", "knowledge/wiki/a.md"), "../../wiki/a.md");
        assert_eq!(relative_link("decisions/design/d/read.md", "demo-crate/src/lib.rs"), "../../../demo-crate/src/lib.rs");
        assert_eq!(relative_link("a/b.md", "a/c.md"), "c.md");
    }
}
```

## Composing the module

<a name="chunk-root"></a><sub>[`src/region_gfm.rs`](../../../x0k-tangle/src/region_gfm.rs) · `#root` · assembles [module-doc](#chunk-module-doc) · [imports](#chunk-imports) · [segments](#chunk-segments) · [chapter-links](#chunk-chapter-links) · [weave-chapter](#chunk-weave-chapter) · [rewrite-links](#chunk-rewrite-links) · [unweave-chapter](#chunk-unweave-chapter) · [affordance-evidence](#chunk-affordance-evidence) · [relative-link](#chunk-relative-link) · [tests](#chunk-tests)</sub>

```rust {#root}
<<module-doc>>

<<imports>>

<<segments>>

<<chapter-links>>

<<weave-chapter>>

<<rewrite-links>>

<<unweave-chapter>>

<<affordance-evidence>>

<<relative-link>>

<<tests>>
```
