# Block identity: an id that stays with the words

```turtle folio:document
implementation:folio\/block-identity a x0k:Implementation ;
    x0k:status "draft" ;
    x0k:summary "How a block of a markdown or HTML body carries a stable id in the body itself, how edits, splits and merges carry that id by lineage, and how a selection, scroll position or proposal target names a block with a heading-path fallback." ;
    x0k:concerns "folio", "blocks", "identity", "markdown", "html", "anchors" ;
    x0k:cites implementation:folio\/segmentation,
        implementation:folio\/transclusion,
        implementation:document-editing\/session ;
    x0k:implements design:in-prose-authoring,
        design:transclusion ;
    x0k:motivatedBy intent:c84291d0-ccb4-2c71-e5e8-4489564e2578 ;
    folio:tangleCrate "crates/x0k-folio" ;
    folio:tangleRoot "src/block_identity.rs" .
```

A canvas placement, a transclusion and a public overlay each need to name
one block of a document and keep naming it while the document changes. A
byte range fails the moment someone types above it. A heading path fails
when the heading is renamed, and cannot name one paragraph of a section.
The segmentation key (`{path}/{kind}/{ordinal}`,
[`segmentation.md`](segmentation.md)) is section-scoped and positional:
insert a paragraph at the top of a section and every later paragraph's key
moves. The design asks for more (`in-prose-authoring` §"The one
document"): an id that is part of the document, keeps its address while the
block's words change, and survives restart and transclusion.

So the id lives in the document's own representation, where every copy,
save and sync carries it without a side table that could drift.

## The marker

A markdown block carries its id as a link reference definition on the line
that opens it. The label and the target are both the anchor:

```text
[block-7a3f09c2]: #block-7a3f09c2
A paragraph keeps this id while its words change.
```

A CommonMark reader consumes a definition and writes nothing for it, so a
marked body renders exactly as the unmarked one in the weave, the native
structural renderer and any other reader. The definition also does
something useful: within the document, `[as argued above][block-7a3f09c2]`
links to the block.

A list item is a block too. Its marker sits after the bullet, and the
item's text moves to the next line at the item's content column:

```text
- [block-0c41e9d2]: #block-0c41e9d2
  the item's text
```

An HTML body names a block with a `data-block-id` attribute on the block's
start tag, the attribute the web document editor and the study page already
read. Which elements are blocks, and how the attribute is read and written,
is §"HTML bodies" below; everything else in this chapter holds for both.

Some blocks cannot carry a marker without changing how they read, and are
never given one: an indented code block (the marker line would join it as
a paragraph), a task-list item (the definition would hide the checkbox),
an empty list item, and nested list items, which belong to their top-level
item's block. Such a block is still a block; an anchor inside it falls back
to its heading path.

The anchor grammar is `block-` and a lowercase alphanumeric tail. New ids
take eight hex digits.

<a name="chunk-block-id"></a><sub>[`src/block_identity.rs`](../../crates/x0k-folio/src/block_identity.rs) · `#block-id`</sub>

```rust {#block-id}
/// The anchor that names one block: `block-` and a lowercase alphanumeric
/// tail, as in `x0k:design/places#block-7a3f`.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BlockId(String);

impl BlockId {
    /// Read an anchor; anything outside the grammar is not a block id.
    pub fn parse(anchor: &str) -> Option<Self> {
        let tail = anchor.strip_prefix("block-")?;
        let valid = !tail.is_empty()
            && tail.len() <= 32
            && tail.bytes().all(|b| b.is_ascii_digit() || b.is_ascii_lowercase());
        valid.then(|| Self(anchor.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn marker(&self) -> String {
        format!("[{0}]: #{0}\n", self.0)
    }
}

impl std::fmt::Display for BlockId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockKind {
    Paragraph,
    Heading,
    Item,
    /// An HTML list, which the page renders as one block.
    List,
    Quote,
    Code,
    Html,
    Table,
    Rule,
    Other,
}

/// One block of a body: a top-level block, or one item of a top-level list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Block {
    pub id: Option<BlockId>,
    pub kind: BlockKind,
    /// The block's text as a reader sees it, without its marker.
    pub text: String,
    /// Where that text sits in the marker-free text ([`BlockText::visible`]).
    pub visible: Range<usize>,
}
```

## Reading a body

Reading separates the two things a stored body holds: the text a reader
sees, and the markers. Each marker is remembered with the place in the
marker-free text where it was removed. A block owns the marker removed
exactly at its opening: the start of its first line, or, for a list item,
the start of its content after the bullet. A marker that opens nothing
(followed by a blank line, or by a list) names nothing. When two markers
carry the same id, the first in reading order keeps it, so a pasted copy of
a marked block never steals its source's id.

Marker-shaped lines inside a fenced code block or an HTML block are text,
not markers.

<a name="chunk-read-body"></a><sub>[`src/block_identity.rs`](../../crates/x0k-folio/src/block_identity.rs) · `#read-body`</sub>

```rust {#read-body}
/// A body read for its blocks: the text without markers, where each marker
/// sat, and the blocks with the ids their markers give them.
#[derive(Clone, Debug)]
pub struct BlockText {
    visible: String,
    markers: Vec<Marker>,
    shapes: Vec<Shape>,
    format: Format,
}

#[derive(Clone, Debug)]
struct Marker {
    id: BlockId,
    stored: Range<usize>,
    at: usize,
}

/// One block as read: where its marker goes (`mark_at`), and the places a
/// marker read from the body may sit and still open it (`opening`: one
/// place in markdown, anywhere in the start tag in HTML). `spelling` is an
/// HTML marker exactly as the author wrote it.
#[derive(Clone, Debug)]
struct Shape {
    kind: BlockKind,
    range: Range<usize>,
    mark_at: usize,
    opening: Range<usize>,
    item_column: Option<usize>,
    markable: bool,
    id: Option<BlockId>,
    spelling: Option<String>,
}

impl BlockText {
    pub fn parse(text: &str) -> Self {
        Self::parse_as(text, Format::of(text))
    }

    fn parse_as(text: &str, format: Format) -> Self {
        let (visible, markers) = match format {
            Format::Markdown => strip_markers(text),
            Format::Html { from } => strip_html_markers(text, from),
        };
        let mut shapes = shapes_of(&visible, format);
        let mut named = HashSet::new();
        for marker in &markers {
            if named.contains(&marker.id) {
                continue;
            }
            let owner = shapes
                .iter_mut()
                .find(|shape| shape.opening.start <= marker.at && marker.at <= shape.opening.end);
            if let Some(shape) = owner.filter(|shape| shape.markable && shape.id.is_none()) {
                shape.id = Some(marker.id.clone());
                if let Format::Html { .. } = format {
                    shape.mark_at = marker.at;
                    shape.spelling = Some(text[marker.stored.clone()].to_string());
                }
                named.insert(marker.id.clone());
            }
        }
        Self { visible, markers, shapes, format }
    }

    /// The body as a reader sees it: every marker removed.
    pub fn visible(&self) -> &str {
        &self.visible
    }

    pub fn blocks(&self) -> Vec<Block> {
        self.shapes
            .iter()
            .map(|shape| Block {
                id: shape.id.clone(),
                kind: shape.kind,
                text: self.visible[shape.range.clone()].to_string(),
                visible: shape.range.clone(),
            })
            .collect()
    }

    pub fn find(&self, id: &BlockId) -> Option<Block> {
        self.blocks().into_iter().find(|block| block.id.as_ref() == Some(id))
    }

    /// The marker-free offset of a stored offset; inside a marker it is the
    /// marker's own place.
    pub fn visible_offset(&self, stored: usize) -> usize {
        let mut removed = 0;
        for marker in &self.markers {
            if stored <= marker.stored.start {
                break;
            }
            if stored < marker.stored.end {
                return marker.at;
            }
            removed += marker.stored.len();
        }
        stored - removed
    }

    /// The stored offset of a marker-free offset. At a marker's place,
    /// `past_markers` chooses after the marker (a block's first byte) or
    /// before it (the end of what precedes the block).
    pub fn stored_offset(&self, visible: usize, past_markers: bool) -> usize {
        let mut added = 0;
        for marker in &self.markers {
            if marker.at > visible || (marker.at == visible && !past_markers) {
                break;
            }
            added += marker.stored.len();
        }
        visible + added
    }
}

/// The blocks of a stored body, in reading order, with their ids.
pub fn blocks(text: &str) -> Vec<Block> {
    BlockText::parse(text).blocks()
}
```

Stripping walks the stored text line by line. A top-level marker is a whole
line. An item marker starts after the bullet and also takes the next line's
indentation up to the content column, so `- [id]: #id\n  text` reads as
`- text`.

<a name="chunk-strip-markers"></a><sub>[`src/block_identity.rs`](../../crates/x0k-folio/src/block_identity.rs) · `#strip-markers`</sub>

```rust {#strip-markers}
fn strip_markers(text: &str) -> (String, Vec<Marker>) {
    if !text.contains("[block-") {
        return (text.to_string(), Vec::new());
    }
    let literal = literal_ranges(text);
    let mut visible = String::with_capacity(text.len());
    let mut markers = Vec::new();
    let mut copied = 0;
    let mut line = 0;
    while line < text.len() {
        let end = text[line..].find('\n').map_or(text.len(), |at| line + at + 1);
        let in_literal = literal.iter().any(|range| range.contains(&line));
        if let Some((column, id)) = (!in_literal).then(|| marker_line(&text[line..end])).flatten() {
            let start = line + column;
            if start >= copied {
                let indent = text[end..].bytes().take(column).take_while(|b| *b == b' ').count();
                visible.push_str(&text[copied..start]);
                markers.push(Marker { id, stored: start..end + indent, at: visible.len() });
                copied = end + indent;
            }
        }
        line = end;
    }
    visible.push_str(&text[copied.min(text.len())..]);
    (visible, markers)
}

/// Fenced code and HTML blocks hold text that only looks like markers.
fn literal_ranges(text: &str) -> Vec<Range<usize>> {
    Parser::new_ext(text, options())
        .into_offset_iter()
        .filter_map(|(event, range)| match event {
            Event::Start(Tag::CodeBlock(_) | Tag::HtmlBlock) => Some(range),
            _ => None,
        })
        .collect()
}

fn marker_line(line: &str) -> Option<(usize, BlockId)> {
    let line = line.trim_end_matches(['\n', '\r', ' ', '\t']);
    let column = if line.starts_with('[') { 0 } else { bullet_width(line)? };
    let (label, target) = line[column..].strip_prefix('[')?.split_once("]: #")?;
    let id = BlockId::parse(label)?;
    (target == label).then_some((column, id))
}

/// The content column of a line that opens a list item at the margin.
fn bullet_width(line: &str) -> Option<usize> {
    let bytes = line.as_bytes();
    let bullet = match bytes.first()? {
        b'-' | b'*' | b'+' => 1,
        b'0'..=b'9' => {
            let digits = bytes.iter().take(9).take_while(|b| b.is_ascii_digit()).count();
            if !matches!(bytes.get(digits), Some(b'.' | b')')) {
                return None;
            }
            digits + 1
        }
        _ => return None,
    };
    let spaces = bytes[bullet..].iter().take_while(|b| **b == b' ').count();
    (1..=4).contains(&spaces).then_some(bullet + spaces)
}
```

The blocks come from the same CommonMark reader the renderers use, run over
the marker-free text: each top-level block, and each item of a top-level
list.

<a name="chunk-block-shapes"></a><sub>[`src/block_identity.rs`](../../crates/x0k-folio/src/block_identity.rs) · `#block-shapes`</sub>

```rust {#block-shapes}
fn options() -> Options {
    Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS
}

fn block_shapes(visible: &str) -> Vec<Shape> {
    let mut shapes = Vec::new();
    let mut depth = 0usize;
    let mut in_top_list = false;
    for (event, range) in Parser::new_ext(visible, options()).into_offset_iter() {
        match event {
            Event::Start(tag) => {
                if depth == 0 {
                    match &tag {
                        Tag::List(_) => in_top_list = true,
                        tag => shapes.push(top_shape(visible, range, kind_of(tag))),
                    }
                } else if depth == 1 && in_top_list && matches!(tag, Tag::Item) {
                    shapes.push(item_shape(visible, range));
                }
                depth += 1;
            }
            Event::End(end) => {
                depth -= 1;
                if depth == 0 && matches!(end, TagEnd::List(_)) {
                    in_top_list = false;
                }
            }
            Event::Rule if depth == 0 => {
                shapes.push(top_shape(visible, range, (BlockKind::Rule, true)));
            }
            _ => {}
        }
    }
    shapes
}

fn kind_of(tag: &Tag) -> (BlockKind, bool) {
    match tag {
        Tag::Paragraph => (BlockKind::Paragraph, true),
        Tag::Heading { .. } => (BlockKind::Heading, true),
        Tag::BlockQuote(_) => (BlockKind::Quote, true),
        Tag::CodeBlock(CodeBlockKind::Fenced(_)) => (BlockKind::Code, true),
        Tag::CodeBlock(CodeBlockKind::Indented) => (BlockKind::Code, false),
        Tag::HtmlBlock => (BlockKind::Html, true),
        Tag::Table(_) => (BlockKind::Table, true),
        _ => (BlockKind::Other, false),
    }
}

fn top_shape(visible: &str, range: Range<usize>, (kind, markable): (BlockKind, bool)) -> Shape {
    let range = trimmed(visible, range);
    let mark_at = line_start(visible, range.start);
    Shape {
        kind,
        range,
        mark_at,
        opening: mark_at..mark_at,
        item_column: None,
        markable,
        id: None,
        spelling: None,
    }
}

fn item_shape(visible: &str, range: Range<usize>) -> Shape {
    let range = trimmed(visible, range);
    let first_line = visible[range.clone()].lines().next().unwrap_or("");
    let at_margin = line_start(visible, range.start) == range.start;
    let column = bullet_width(first_line).filter(|_| at_margin);
    let markable = column.is_some_and(|column| {
        let content = &first_line[column..];
        !content.trim().is_empty() && !["[ ]", "[x]", "[X]"].iter().any(|task| content.starts_with(task))
    });
    let column = column.unwrap_or(0);
    let mark_at = range.start + column;
    Shape {
        kind: BlockKind::Item,
        mark_at,
        opening: mark_at..mark_at,
        range,
        item_column: Some(column),
        markable,
        id: None,
        spelling: None,
    }
}

fn trimmed(visible: &str, range: Range<usize>) -> Range<usize> {
    let kept = visible[range.clone()].trim_end().len();
    range.start..range.start + kept
}

fn line_start(text: &str, at: usize) -> usize {
    text[..at].rfind('\n').map_or(0, |newline| newline + 1)
}
```

Writing a body back puts each named block's marker at its opening. A
definition on the line after a line of text would continue that text as a
paragraph, so a named block written flush against text gets a blank line
before its marker; the blank line changes nothing a reader sees.

<a name="chunk-write-markers"></a><sub>[`src/block_identity.rs`](../../crates/x0k-folio/src/block_identity.rs) · `#write-markers`</sub>

```rust {#write-markers}
fn write_markers(visible: &str, shapes: &[Shape]) -> String {
    let mut stored = String::with_capacity(visible.len());
    let mut copied = 0;
    for shape in shapes {
        let Some(id) = &shape.id else { continue };
        stored.push_str(&visible[copied..shape.mark_at]);
        copied = shape.mark_at;
        match shape.item_column {
            Some(column) => {
                stored.push_str(&id.marker());
                stored.extend(std::iter::repeat_n(' ', column));
            }
            None => {
                if follows_text(&stored) {
                    stored.push('\n');
                }
                stored.push_str(&id.marker());
            }
        }
    }
    stored.push_str(&visible[copied..]);
    stored
}

fn follows_text(before: &str) -> bool {
    let Some(rest) = before.strip_suffix('\n') else { return !before.is_empty() };
    let previous = rest.rsplit('\n').next().unwrap_or(rest);
    !previous.trim().is_empty()
}
```

## HTML bodies

A folio document whose header declares `x0k:bodyFormat "html"` keeps an HTML
body after its header. The body starts after an optional title line and the
`turtle folio:document` header, where the document store's file projection
starts it. The title and header are
never blocks. Any other text is read as markdown.

<a name="chunk-body-format"></a><sub>[`src/block_identity.rs`](../../crates/x0k-folio/src/block_identity.rs) · `#body-format`</sub>

```rust {#body-format}
/// How a body's blocks are found: a markdown body, or the HTML body of a
/// folio document, which starts at `from`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Format {
    Markdown,
    Html { from: usize },
}

impl Format {
    fn of(text: &str) -> Self {
        html_body_start(text).map_or(Self::Markdown, |from| Self::Html { from })
    }
}

/// Where a folio document's HTML body starts: after an optional title line
/// and a `turtle folio:document` header declaring `x0k:bodyFormat "html"`.
fn html_body_start(text: &str) -> Option<usize> {
    const OPEN: &str = "```turtle folio:document\n";
    let mut at = 0;
    if text.starts_with('#') {
        at = text.find('\n')?;
        at += text[at..].bytes().take_while(|b| *b == b'\n').count();
    }
    if !text[at..].starts_with(OPEN) {
        return None;
    }
    let turtle = at + OPEN.len() - 1;
    let close = turtle + text[turtle..].find("\n```")? + 4;
    let declared = text[turtle..close]
        .split("bodyFormat")
        .nth(1)
        .is_some_and(|value| value.trim_start().starts_with("\"html\""));
    declared.then(|| close + usize::from(text[close..].starts_with('\n')))
}

/// Whether a text can hold block ids: an edit to a text that holds none,
/// inserting none, is plain text editing.
pub fn holds_ids(text: &str) -> bool {
    text.contains("[block-") || text.contains("data-block-id")
}

fn shapes_of(visible: &str, format: Format) -> Vec<Shape> {
    match format {
        Format::Markdown => block_shapes(visible),
        Format::Html { from } => html_shapes(visible, from),
    }
}

fn write(visible: &str, shapes: &[Shape], format: Format) -> String {
    match format {
        Format::Markdown => write_markers(visible, shapes),
        Format::Html { .. } => write_html_markers(visible, shapes),
    }
}
```

The blocks of an HTML body are the blocks the study page renders, edits and
matches one at a time, so an id this module writes is an id the page can
match by. The page's reader is a small parser that knows each node's source
span; this one is the same parser. It closes what the source leaves open
where a browser would: an `<li>` ends at the next `<li>` of its list, a `<p>`
at the next block-level start tag. `<script>`, `<style>`, `<textarea>` and
`<title>` hold raw text, and inside `<svg>` or `<math>` a `/>` closes its
element.

<a name="chunk-html-tree"></a><sub>[`src/block_identity.rs`](../../crates/x0k-folio/src/block_identity.rs) · `#html-tree`</sub>

```rust {#html-tree}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NodeKind {
    Element,
    Text,
    Comment,
}

/// A node of an HTML body with its source span; an element's start tag
/// ends at `open_end`.
#[derive(Debug)]
struct Node {
    kind: NodeKind,
    tag: String,
    start: usize,
    open_end: usize,
    end: usize,
    children: Vec<Node>,
}

impl Node {
    fn new(kind: NodeKind, tag: &str, start: usize, open_end: usize) -> Self {
        Self { kind, tag: tag.to_string(), start, open_end, end: open_end, children: Vec::new() }
    }
}

const VOID: &[&str] = &[
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source",
    "track", "wbr",
];
const RAW: &[&str] = &["script", "style", "textarea", "title"];
const FOREIGN: &[&str] = &["svg", "math"];
const CLOSES_P: &[&str] = &[
    "address", "article", "aside", "blockquote", "details", "div", "dl", "fieldset", "figcaption",
    "figure", "footer", "form", "h1", "h2", "h3", "h4", "h5", "h6", "header", "hr", "main", "nav",
    "ol", "p", "pre", "section", "table", "ul",
];
```

<a name="chunk-html-parse"></a><sub>[`src/block_identity.rs`](../../crates/x0k-folio/src/block_identity.rs) · `#html-parse`</sub>

```rust {#html-parse}
/// Parse `src[from..]` into elements, text and comments with their spans.
fn parse_html(src: &str, from: usize) -> Node {
    let to = src.len();
    let bytes = src.as_bytes();
    let mut stack = vec![Node::new(NodeKind::Element, "#root", from, from)];
    let mut i = from;
    while i < to {
        let Some(lt) = next_markup(src, i) else {
            push_text(&mut stack, i, to);
            break;
        };
        push_text(&mut stack, i, lt);
        if src[lt..].starts_with("<!--") {
            let end = src[lt + 4..].find("-->").map_or(to, |at| lt + 4 + at + 3);
            push(&mut stack, Node::new(NodeKind::Comment, "", lt, end));
            i = end;
            continue;
        }
        let gt = tag_end(bytes, lt + 1);
        let open_end = (gt + 1).min(to);
        if matches!(bytes[lt + 1], b'!' | b'?') {
            push(&mut stack, Node::new(NodeKind::Comment, "", lt, open_end));
            i = open_end;
            continue;
        }
        if bytes[lt + 1] == b'/' {
            let name = src[lt + 2..gt].trim().to_ascii_lowercase();
            if let Some(depth) = (1..stack.len()).rev().find(|k| stack[*k].tag == name) {
                close(&mut stack, depth, lt, open_end);
            }
            i = open_end;
            continue;
        }
        let name = src[lt + 1..].bytes().take_while(|b| b.is_ascii_alphanumeric() || *b == b'-').count();
        let tag = src[lt + 1..lt + 1 + name].to_ascii_lowercase();
        let foreign = stack.iter().any(|open| FOREIGN.contains(&open.tag.as_str()));
        if !foreign {
            implied_ends(&mut stack, &tag, lt);
        }
        let mut element = Node::new(NodeKind::Element, &tag, lt, open_end);
        if VOID.contains(&tag.as_str()) || (foreign && bytes[gt - 1] == b'/') {
            push(&mut stack, element);
            i = open_end;
        } else if RAW.contains(&tag.as_str()) {
            let closing = format!("</{tag}");
            let raw_end = src[open_end..].to_ascii_lowercase().find(&closing).map_or(to, |at| open_end + at);
            if raw_end > open_end {
                element.children.push(Node::new(NodeKind::Text, "", open_end, raw_end));
            }
            element.end = if raw_end == to { to } else { src[raw_end..].find('>').map_or(to, |at| raw_end + at + 1) };
            i = element.end;
            push(&mut stack, element);
        } else {
            stack.push(element);
            i = open_end;
        }
    }
    close(&mut stack, 1, to, to);
    stack.pop().expect("the root")
}

/// The next `<` that opens markup: a tag, a close tag, a comment or a
/// declaration.
fn next_markup(src: &str, from: usize) -> Option<usize> {
    let bytes = src.as_bytes();
    src[from..].match_indices('<').map(|(at, _)| from + at).find(|at| {
        bytes.get(at + 1).is_some_and(|b| b.is_ascii_alphabetic() || matches!(b, b'!' | b'/' | b'?'))
    })
}

/// The `>` that ends a tag begun before `from`, outside quoted values; the
/// end of the text when there is none.
fn tag_end(bytes: &[u8], from: usize) -> usize {
    let mut quote = None;
    for (at, b) in bytes.iter().enumerate().skip(from) {
        match quote {
            Some(open) if *b == open => quote = None,
            Some(_) => {}
            None if matches!(b, b'"' | b'\'') => quote = Some(*b),
            None if *b == b'>' => return at,
            None => {}
        }
    }
    bytes.len()
}

/// The ends a start tag implies: a new `<li>` closes the open item of its
/// list, and a block-level start tag closes an open `<p>`.
fn implied_ends(stack: &mut Vec<Node>, tag: &str, at: usize) {
    if tag == "li" {
        let mut depth = stack.len() - 1;
        while depth > 0 && !matches!(stack[depth].tag.as_str(), "ul" | "ol") {
            if stack[depth].tag == "li" {
                close(stack, depth, at, at);
                break;
            }
            depth -= 1;
        }
    }
    if CLOSES_P.contains(&tag) && stack.last().is_some_and(|open| open.tag == "p") {
        let depth = stack.len() - 1;
        close(stack, depth, at, at);
    }
}

/// Close every open element down to `depth`: the one at `depth` ends at
/// `end`, any left open inside it at `at`.
fn close(stack: &mut Vec<Node>, depth: usize, at: usize, end: usize) {
    while stack.len() > depth {
        let mut element = stack.pop().expect("an open element");
        element.end = if stack.len() == depth { end } else { at };
        push(stack, element);
    }
}

fn push(stack: &mut [Node], node: Node) {
    stack.last_mut().expect("the root").children.push(node);
}

fn push_text(stack: &mut [Node], start: usize, end: usize) {
    if end > start {
        push(stack, Node::new(NodeKind::Text, "", start, end));
    }
}
```

A start tag's attributes are read the way the page reads them: a name, then
optionally `=` and a quoted or bare value.

<a name="chunk-html-attributes"></a><sub>[`src/block_identity.rs`](../../crates/x0k-folio/src/block_identity.rs) · `#html-attributes`</sub>

```rust {#html-attributes}
/// One attribute of a start tag: its whole span, its name and its value.
struct Attribute {
    span: Range<usize>,
    name: Range<usize>,
    value: Range<usize>,
}

/// Where an element's attributes sit: after the tag name, before the `>`.
fn interior(src: &str, element: &Node) -> Range<usize> {
    let from = element.start + 1 + element.tag.len();
    let to = element.open_end - usize::from(src[..element.open_end].ends_with('>'));
    from..to.max(from)
}

fn attributes(src: &str, inside: Range<usize>) -> Vec<Attribute> {
    let bytes = src.as_bytes();
    let stop = |b: u8| b.is_ascii_whitespace() || matches!(b, b'=' | b'/' | b'>');
    let blank = |from: usize| from + bytes[from..inside.end].iter().take_while(|b| b.is_ascii_whitespace()).count();
    let mut found = Vec::new();
    let mut at = inside.start;
    while at < inside.end {
        if stop(bytes[at]) {
            at += 1;
            continue;
        }
        let name = at..at + bytes[at..inside.end].iter().take_while(|b| !stop(**b)).count();
        let (mut end, mut value) = (name.end, name.end..name.end);
        let equals = blank(name.end);
        if equals < inside.end && bytes[equals] == b'=' {
            let start = blank(equals + 1);
            match bytes.get(start).filter(|_| start < inside.end) {
                Some(quote @ (b'"' | b'\'')) => {
                    let close = bytes[start + 1..inside.end]
                        .iter()
                        .position(|b| b == quote)
                        .map_or(inside.end, |at| start + 1 + at);
                    value = start + 1..close;
                    end = (close + 1).min(inside.end);
                }
                _ => {
                    let len = bytes[start..inside.end]
                        .iter()
                        .take_while(|b| !b.is_ascii_whitespace() && **b != b'>')
                        .count();
                    value = start..start + len;
                    end = start + len;
                }
            }
        }
        found.push(Attribute { span: name.start..end, name, value });
        at = end;
    }
    found
}

fn class_of<'a>(src: &'a str, element: &Node) -> &'a str {
    attributes(src, interior(src, element))
        .into_iter()
        .rev()
        .find(|attribute| src[attribute.name.clone()].eq_ignore_ascii_case("class"))
        .map_or("", |attribute| &src[attribute.value])
}
```

The blocks are the leaves of the body. A sectioning element (`section`,
`article`, `main`, `header`, `footer`, `aside`, `nav`) or a plain `div`
whose children include a block-level element is a container: its children
are blocks, and it is not one. Every other element is one block, whatever
it holds: a paragraph, a heading, a whole list, a `<figure>` with its
`<svg>` and caption, a `<details>` with its summary. A container whose class
marks it as one rendered thing (`media-embed`, `transclusion`) is one block
too. A run of bare text between elements is a block that cannot carry an
attribute, and so can be anchored only by heading path, like markdown's
unmarkable blocks; so is a `<script>`, `<style>` or `<template>`. Comments
are not blocks.

<a name="chunk-html-blocks"></a><sub>[`src/block_identity.rs`](../../crates/x0k-folio/src/block_identity.rs) · `#html-blocks`</sub>

```rust {#html-blocks}
const PHRASING: &[&str] = &[
    "a", "abbr", "b", "bdi", "bdo", "br", "cite", "code", "data", "dfn", "em", "i", "img", "kbd",
    "mark", "q", "s", "samp", "small", "span", "strong", "sub", "sup", "time", "u", "var", "wbr",
];
const CONTAINERS: &[&str] = &["section", "article", "main", "header", "footer", "aside", "nav", "div"];
const ATOMIC_CLASSES: &[&str] = &["media-embed", "transclusion"];

fn leaves<'a>(src: &str, node: &'a Node, out: &mut Vec<&'a Node>) {
    for child in &node.children {
        match child.kind {
            NodeKind::Comment => {}
            NodeKind::Text if src[child.start..child.end].trim().is_empty() => {}
            NodeKind::Element if is_container(src, child) => leaves(src, child, out),
            NodeKind::Text | NodeKind::Element => out.push(child),
        }
    }
}

fn is_container(src: &str, element: &Node) -> bool {
    CONTAINERS.contains(&element.tag.as_str())
        && element
            .children
            .iter()
            .any(|child| child.kind == NodeKind::Element && !PHRASING.contains(&child.tag.as_str()))
        && !class_of(src, element).split_ascii_whitespace().any(|class| ATOMIC_CLASSES.contains(&class))
}

fn html_shapes(visible: &str, from: usize) -> Vec<Shape> {
    let root = parse_html(visible, from);
    let mut nodes = Vec::new();
    leaves(visible, &root, &mut nodes);
    nodes.into_iter().map(|node| html_shape(visible, node)).collect()
}

fn html_shape(visible: &str, node: &Node) -> Shape {
    let range = trimmed(visible, node.start..node.end);
    if node.kind == NodeKind::Text {
        let start = range.end - visible[range.clone()].trim_start().len();
        return Shape {
            kind: BlockKind::Other,
            range: start..range.end,
            mark_at: start,
            opening: start..start,
            item_column: None,
            markable: false,
            id: None,
            spelling: None,
        };
    }
    let opening = interior(visible, node);
    Shape {
        kind: html_kind(&node.tag),
        range,
        mark_at: opening.start,
        opening,
        item_column: None,
        markable: !matches!(node.tag.as_str(), "script" | "style" | "template"),
        id: None,
        spelling: None,
    }
}

fn html_kind(tag: &str) -> BlockKind {
    match tag {
        "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => BlockKind::Heading,
        "p" => BlockKind::Paragraph,
        "ul" | "ol" | "dl" | "menu" => BlockKind::List,
        "li" => BlockKind::Item,
        "blockquote" => BlockKind::Quote,
        "pre" => BlockKind::Code,
        "table" => BlockKind::Table,
        "hr" => BlockKind::Rule,
        _ => BlockKind::Other,
    }
}
```

A block's id is a `data-block-id` attribute on its start tag whose value is a
block id. Reading removes it together with the one whitespace character
before it, wherever in the tag it sits, so the text a reader sees is the
body without ids. An attribute anywhere else (on a container, inside a
block, in a script's text) is the author's text, not an id. A new id is
written straight after the tag name, `<p data-block-id="block-7a3f09c2">`;
an id read from the body is written back where it was and as it was
spelled, as long as its block's start tag is unchanged.

<a name="chunk-html-markers"></a><sub>[`src/block_identity.rs`](../../crates/x0k-folio/src/block_identity.rs) · `#html-markers`</sub>

```rust {#html-markers}
/// The `data-block-id` attributes on the blocks' start tags, each with the
/// whitespace before it, removed.
fn strip_html_markers(text: &str, from: usize) -> (String, Vec<Marker>) {
    if !text[from..].contains("data-block-id") {
        return (text.to_string(), Vec::new());
    }
    let root = parse_html(text, from);
    let mut nodes = Vec::new();
    leaves(text, &root, &mut nodes);
    let found = nodes
        .into_iter()
        .filter(|node| node.kind == NodeKind::Element)
        .flat_map(|node| id_attributes(text, node))
        .collect();
    remove_ids(text, found)
}

fn id_attributes(text: &str, element: &Node) -> Vec<(Range<usize>, BlockId)> {
    attributes(text, interior(text, element))
        .into_iter()
        .filter(|attribute| text[attribute.name.clone()].eq_ignore_ascii_case("data-block-id"))
        .filter_map(|attribute| {
            let id = BlockId::parse(&text[attribute.value])?;
            let space = usize::from(text[..attribute.span.start].ends_with(|c: char| c.is_ascii_whitespace()));
            Some((attribute.span.start - space..attribute.span.end, id))
        })
        .collect()
}

fn remove_ids(text: &str, found: Vec<(Range<usize>, BlockId)>) -> (String, Vec<Marker>) {
    let mut visible = String::with_capacity(text.len());
    let mut markers = Vec::new();
    let mut copied = 0;
    for (stored, id) in found {
        visible.push_str(&text[copied..stored.start]);
        copied = stored.end;
        markers.push(Marker { id, stored, at: visible.len() });
    }
    visible.push_str(&text[copied..]);
    (visible, markers)
}

fn write_html_markers(visible: &str, shapes: &[Shape]) -> String {
    let mut stored = String::with_capacity(visible.len() + 28 * shapes.len());
    let mut copied = 0;
    for shape in shapes {
        let Some(id) = &shape.id else { continue };
        stored.push_str(&visible[copied..shape.mark_at]);
        copied = shape.mark_at;
        match &shape.spelling {
            Some(spelling) => stored.push_str(spelling),
            None => stored.push_str(&format!(" data-block-id=\"{id}\"")),
        }
    }
    stored.push_str(&visible[copied..]);
    stored
}

/// An heir whose start tag reads as its ancestor's did keeps the id where and
/// as the author wrote it.
fn keep_spelling(before: &str, old: &Shape, after: &str, heir: &mut Shape) {
    let Some(spelling) = &old.spelling else { return };
    let unchanged = heir.spelling.is_none()
        && heir.opening.end >= heir.range.start
        && before[old.range.start..old.opening.end] == after[heir.range.start..heir.opening.end];
    if unchanged {
        heir.mark_at = heir.range.start + (old.mark_at - old.range.start);
        heir.spelling = Some(spelling.clone());
    }
}

/// Remove the `data-block-id` attributes written by the text an edit
/// inserted at `span`; answers the text and how many bytes went.
fn drop_inserted_ids(visible: &str, from: usize, span: Range<usize>) -> (String, usize) {
    if !visible[span.clone()].contains("data-block-id") {
        return (visible.to_string(), 0);
    }
    fn elements<'a>(node: &'a Node, out: &mut Vec<&'a Node>) {
        for child in &node.children {
            if child.kind == NodeKind::Element {
                out.push(child);
                elements(child, out);
            }
        }
    }
    let root = parse_html(visible, from);
    let mut all = Vec::new();
    elements(&root, &mut all);
    let mut found: Vec<_> = all
        .into_iter()
        .flat_map(|element| id_attributes(visible, element))
        .filter(|(range, _)| span.start <= range.start && range.end <= span.end)
        .collect();
    found.sort_by_key(|(range, _)| range.start);
    let removed = found.iter().map(|(range, _)| range.len()).sum();
    (remove_ids(visible, found).0, removed)
}
```

## Minting on first need

A document gains markers only when something needs to name one of its
blocks. Minting names every unnamed block that can carry a marker and never
changes an existing id. A body whose blocks are all named, or that cannot
carry markers, comes back byte for byte.

A new id is a hash of a seed (the caller passes the document and field),
the block's text and an attempt count. Two replicas minting the same block
of the same document therefore write the same marker, and a concurrent
merge of the two collapses to one id. An id the body already holds,
including an orphaned marker's, moves to the next attempt, so ids are
unique within the body.

<a name="chunk-assign-ids"></a><sub>[`src/block_identity.rs`](../../crates/x0k-folio/src/block_identity.rs) · `#assign-ids`</sub>

```rust {#assign-ids}
/// Name every unnamed block that can carry a marker; existing ids stay.
pub fn assign_ids(text: &str, seed: &str) -> String {
    let body = BlockText::parse(text);
    if body.shapes.iter().all(|shape| shape.id.is_some() || !shape.markable) {
        return text.to_string();
    }
    let mut taken: HashSet<BlockId> = body.markers.iter().map(|marker| marker.id.clone()).collect();
    let mut shapes = body.shapes.clone();
    for shape in shapes.iter_mut().filter(|shape| shape.markable && shape.id.is_none()) {
        let content = &body.visible[shape.range.clone()];
        let id = (0u32..)
            .map(|attempt| minted(seed, content, attempt))
            .find(|id| !taken.contains(id))
            .expect("an unused block id");
        taken.insert(id.clone());
        shape.id = Some(id);
    }
    write(&body.visible, &shapes, body.format)
}

fn minted(seed: &str, content: &str, attempt: u32) -> BlockId {
    let mut hasher = blake3::Hasher::new();
    hasher.update(seed.as_bytes());
    hasher.update(&[0]);
    hasher.update(content.as_bytes());
    hasher.update(&attempt.to_le_bytes());
    BlockId(format!("block-{}", &hasher.finalize().to_hex()[..8]))
}
```

## Carrying ids through an edit

An edit is a byte range and its replacement. Carrying applies it to the
marker-free text, reads the blocks of the result, and gives each old id to
the block its text went to. The rule, which the property test below
checks against an independent model:

- **Lineage.** An old block's text after the edit is what survives before
  the edit range and what survives after it. When an edit made from inside
  the block replaced all of its text, the inserted text is its lineage.
- **Keep.** An old id goes to the first new block, in reading order, that
  overlaps its lineage. Typing inside a block keeps its id; so does
  changing its kind (a paragraph that becomes a heading).
- **Split.** The first half keeps the id; the second half is new and is
  named on its next need. A split at the very start of a block moves no
  text into the first half, so the block's text, which is all in the
  second half, keeps its id.
- **Merge.** Two blocks joined into one keep the first one's id. The
  second id retires: it is never reused for another block in this body.
- **Delete.** A block whose text is gone loses its id; a transclusion of it
  shows a broken reference, never a different block.
- **Paste.** Markers in inserted text are removed; in an HTML body, so is
  any `data-block-id` attribute the inserted text writes. Pasted text is
  new material until something names it.

A body with no markers, given an edit that inserts none, is edited exactly
as plain text, so every document written before block identity is
untouched by it.

<a name="chunk-carry"></a><sub>[`src/block_identity.rs`](../../crates/x0k-folio/src/block_identity.rs) · `#carry`</sub>

```rust {#carry}
/// Apply one edit to a stored body and carry each id to the block its text
/// went to.
pub fn carry(text: &str, range: Range<usize>, insert: &str) -> String {
    carry_as(text, range, insert, None)
}

/// [`carry`], reading the body as `fixed` when the caller knows its format,
/// else as its header declares.
fn carry_as(text: &str, range: Range<usize>, insert: &str, fixed: Option<Format>) -> String {
    if !holds_ids(text) && !holds_ids(insert) {
        let mut edited = text.to_string();
        edited.replace_range(range, insert);
        return edited;
    }
    let body = BlockText::parse_as(text, fixed.unwrap_or_else(|| Format::of(text)));
    let edit = body.visible_offset(range.start)..body.visible_offset(range.end);
    let mut visible = body.visible.clone();
    visible.replace_range(edit.clone(), insert);
    let format = fixed.unwrap_or_else(|| Format::of(&visible));
    let inserted = match format {
        Format::Markdown => {
            let (inserted, _) = strip_markers(insert);
            visible.replace_range(edit.start..edit.start + insert.len(), &inserted);
            inserted.len()
        }
        Format::Html { from } => {
            let (kept, removed) = drop_inserted_ids(&visible, from, edit.start..edit.start + insert.len());
            visible = kept;
            insert.len() - removed
        }
    };
    let mut shapes = shapes_of(&visible, format);
    for old in &body.shapes {
        let Some(id) = &old.id else { continue };
        let lineage = lineage(&old.range, &edit, inserted);
        let heir = shapes
            .iter_mut()
            .find(|new| lineage.iter().any(|piece| overlaps(piece, &new.range)));
        if let Some(heir) = heir.filter(|heir| heir.markable && heir.id.is_none()) {
            heir.id = Some(id.clone());
            keep_spelling(&body.visible, old, &visible, heir);
        }
    }
    write(&visible, &shapes, format)
}

fn lineage(block: &Range<usize>, edit: &Range<usize>, inserted: usize) -> Vec<Range<usize>> {
    let shift = |at: usize| at - edit.end + edit.start + inserted;
    let mut pieces = Vec::new();
    let head = block.start..block.end.min(edit.start);
    if !head.is_empty() {
        pieces.push(head);
    }
    let tail = block.start.max(edit.end);
    if tail < block.end {
        pieces.push(shift(tail)..shift(block.end));
    }
    let inside = block.start <= edit.start && edit.end <= block.end;
    if pieces.is_empty() && inside && inserted > 0 {
        pieces.push(edit.start..edit.start + inserted);
    }
    pieces
}

fn overlaps(a: &Range<usize>, b: &Range<usize>) -> bool {
    a.start < b.end && b.start < a.end
}
```

A transclusion reads one block by id, and an edit made through it writes
back over exactly that block, keeping its id. A transclusion is handed the
body with its header lifted out, so an HTML body arrives looking like
markdown; an id the markdown reading does not find is looked for again in
the HTML reading of the same text.

<a name="chunk-block-access"></a><sub>[`src/block_identity.rs`](../../crates/x0k-folio/src/block_identity.rs) · `#block-access`</sub>

```rust {#block-access}
/// The reader's text of the block named `id`.
pub fn block_text(text: &str, id: &BlockId) -> Option<String> {
    reading(text, id).map(|(_, block)| block.text)
}

/// Replace the named block's text, keeping its id (edit-through).
pub fn replace_block(text: &str, id: &BlockId, replacement: &str) -> Option<String> {
    let (body, block) = reading(text, id)?;
    let start = body.stored_offset(block.visible.start, block.kind != BlockKind::Item);
    let end = body.stored_offset(block.visible.end, false);
    Some(carry_as(text, start..end, replacement, Some(body.format)))
}

/// The reading of `text` in which `id` names a block: as its header declares,
/// or, for an HTML body lifted out of its header, as HTML.
fn reading(text: &str, id: &BlockId) -> Option<(BlockText, Block)> {
    let declared = BlockText::parse(text);
    if let Some(block) = declared.find(id) {
        return Some((declared, block));
    }
    if declared.format != Format::Markdown || !text.contains("data-block-id") {
        return None;
    }
    let html = BlockText::parse_as(text, Format::Html { from: 0 });
    let block = html.find(id)?;
    Some((html, block))
}
```

## Anchors

Positional state belongs to an occurrence, not to the document
(`in-prose-authoring` §"Anchor strategies"). A selection, a scroll position
and a proposal target all name a place the same way: a block id and a byte
range within the block's text, which survives every edit outside the block
and moves with the block. A block without an id falls back to its heading
path: the titles of the enclosing headings and a range measured from the
end of the nearest heading. Both measure marker-free text, so minting ids
never moves an anchor taken before them. A scroll position is an anchor
with an empty range.

Restoring clamps a range to what remains of its block or section, and
returns `None` when the block or the heading path is gone.

<a name="chunk-anchors"></a><sub>[`src/block_identity.rs`](../../crates/x0k-folio/src/block_identity.rs) · `#anchors`</sub>

```rust {#anchors}
/// Where a selection, scroll position or proposal target sits, named so it
/// survives edits: by block id, or by heading path when the block has none.
/// Ranges are byte offsets into the block's text, or into the section after
/// the nearest heading.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BlockAnchor {
    Block { id: BlockId, range: Range<usize> },
    HeadingPath { headings: Vec<String>, range: Range<usize> },
}

impl BlockText {
    /// Anchor a stored range.
    pub fn anchor(&self, stored: Range<usize>) -> BlockAnchor {
        let start = self.visible_offset(stored.start);
        let end = self.visible_offset(stored.end).max(start);
        let owner = self
            .shapes
            .iter()
            .find(|shape| shape.range.start <= start && start <= shape.range.end);
        if let Some(shape) = owner.filter(|shape| shape.id.is_some()) {
            let id = shape.id.clone().expect("a named block");
            let end = end.min(shape.range.end);
            return BlockAnchor::Block {
                id,
                range: start - shape.range.start..end - shape.range.start,
            };
        }
        let (headings, section) = self.section_at(start);
        BlockAnchor::HeadingPath { headings, range: start - section..end - section }
    }

    /// The stored range an anchor names now.
    pub fn restore(&self, anchor: &BlockAnchor) -> Option<Range<usize>> {
        let (within, range) = match anchor {
            BlockAnchor::Block { id, range } => {
                let shape = self.shapes.iter().find(|shape| shape.id.as_ref() == Some(id))?;
                (shape.range.clone(), range)
            }
            BlockAnchor::HeadingPath { headings, range } => (self.section(headings)?, range),
        };
        let place = |at: usize| {
            let mut at = (within.start + at).min(within.end);
            while !self.visible.is_char_boundary(at) {
                at -= 1;
            }
            at
        };
        let start = self.stored_offset(place(range.start), true);
        let end = self.stored_offset(place(range.end), false).max(start);
        Some(start..end)
    }

    fn headings(&self) -> impl Iterator<Item = (usize, String, &Range<usize>)> + '_ {
        self.shapes
            .iter()
            .filter(|shape| shape.kind == BlockKind::Heading)
            .map(|shape| {
                let (level, title) = heading_title(&self.visible[shape.range.clone()]);
                (level, title, &shape.range)
            })
    }

    fn section_at(&self, at: usize) -> (Vec<String>, usize) {
        let mut path: Vec<(usize, String)> = Vec::new();
        let mut start = 0;
        for (level, title, range) in self.headings().filter(|(_, _, range)| range.end <= at) {
            while path.last().is_some_and(|(open, _)| *open >= level) {
                path.pop();
            }
            path.push((level, title));
            start = range.end;
        }
        (path.into_iter().map(|(_, title)| title).collect(), start)
    }

    fn section(&self, headings: &[String]) -> Option<Range<usize>> {
        let mut path: Vec<(usize, String)> = Vec::new();
        let mut open: Option<usize> = headings.is_empty().then_some(0);
        for (level, title, range) in self.headings() {
            if let Some(start) = open {
                return Some(start..range.start);
            }
            while path.last().is_some_and(|(held, _)| *held >= level) {
                path.pop();
            }
            path.push((level, title));
            if path.iter().map(|(_, title)| title).eq(headings.iter()) {
                open = Some(range.end);
            }
        }
        open.map(|start| start..self.visible.len())
    }
}

fn heading_title(text: &str) -> (usize, String) {
    let html_level = text.get(..2).filter(|open| open.eq_ignore_ascii_case("<h")).and_then(|_| {
        text.as_bytes().get(2).filter(|digit| digit.is_ascii_digit())
    });
    if let Some(digit) = html_level {
        let mut title = String::new();
        let mut in_tag = false;
        for c in text.chars() {
            match c {
                '<' => in_tag = true,
                '>' => in_tag = false,
                c if !in_tag => title.push(c),
                _ => {}
            }
        }
        return (usize::from(digit - b'0'), title.split_whitespace().collect::<Vec<_>>().join(" "));
    }
    let hashes = text.bytes().take_while(|b| *b == b'#').count();
    if hashes > 0 {
        let title = text[hashes..].trim().trim_end_matches('#').trim();
        return (hashes, title.to_string());
    }
    let mut lines = text.lines();
    let title = lines.next().unwrap_or("").trim().to_string();
    let level = if lines.next().is_some_and(|line| line.trim_start().starts_with('=')) { 1 } else { 2 };
    (level, title)
}
```

## What must hold

The invariant this module exists for, checked by property over random
documents and random sequences of edits, splits, merges, insertions,
deletions and mints: **every block whose text survives an operation keeps
its id under the rule above, no id names two blocks, and the text a reader
sees is exactly the text the operations produce.** The model in the test
derives the expected ids from the operation alone (a split's first half
keeps, a merge keeps the first), never from this module's lineage
computation, so a wrong rule shows as a disagreement. The property runs
through the editing session in `x0k-document`
([`session.md`](../document-editing/session.md) §"Blocks keep their ids"),
which is where edits reach a stored body.

The tests here pin what the property cannot see: that marking a body never
changes what it renders, that reading and writing a marked body is the
identity, and the anchor fallbacks.

<a name="chunk-tests"></a><sub>[`src/block_identity.rs`](../../crates/x0k-folio/src/block_identity.rs) · `#tests`</sub>

```rust {#tests}
#[cfg(test)]
mod tests {
    use super::*;

    const BODY: &str = "# Places\n\nIntro paragraph.\n\n## Brief\n\nFirst point\ncontinues here.\n\n- one\n- two\n  - nested\n- [ ] a task\n\n1. first\n2. second\n\n> quoted\n> words\n\n```rust\n[block-fake]: #block-fake\nfn x() {}\n```\n\n| a | b |\n| - | - |\n| 1 | 2 |\n\n---\n\n<div>raw</div>\n\nLast words.\n";

    /// What a reader renders: a definition yields no event.
    fn events(text: &str) -> Vec<Event<'_>> {
        Parser::new_ext(text, options()).collect()
    }

    #[test]
    fn marking_never_changes_what_a_body_renders() {
        let marked = assign_ids(BODY, "doc/body");
        assert_ne!(marked, BODY);
        assert_eq!(events(&marked), events(BODY));
        assert_eq!(BlockText::parse(&marked).visible(), BODY);
        let ids: Vec<_> = blocks(&marked).into_iter().filter_map(|block| block.id).collect();
        let unique: HashSet<_> = ids.iter().collect();
        assert_eq!(unique.len(), ids.len());
        let unnamed: Vec<_> = blocks(&marked)
            .into_iter()
            .filter(|block| block.id.is_none())
            .map(|block| block.text)
            .collect();
        assert_eq!(unnamed, vec!["- [ ] a task".to_string()]);
    }

    #[test]
    fn reading_and_writing_a_marked_body_is_the_identity() {
        let marked = assign_ids(BODY, "doc/body");
        assert_eq!(assign_ids(&marked, "doc/body"), marked);
        assert_eq!(assign_ids(&marked, "another/seed"), marked);
        let body = BlockText::parse(&marked);
        assert_eq!(write_markers(&body.visible, &body.shapes), marked);
        assert_eq!(carry(&marked, 0..0, ""), marked);
        assert!(marked.contains("```rust\n[block-fake]: #block-fake\n"), "a fence keeps its text");
    }

    #[test]
    fn a_marker_flush_against_text_gets_a_blank_line() {
        let marked = assign_ids("Text\n# Heading\n", "doc/body");
        assert_eq!(events(&marked), events("Text\n# Heading\n"));
        assert_eq!(blocks(&marked).iter().filter(|block| block.id.is_some()).count(), 2);
    }

    #[test]
    fn a_duplicate_marker_names_only_the_first_block() {
        let text = "[block-aa]: #block-aa\nfirst\n\n[block-aa]: #block-aa\nsecond\n";
        let ids: Vec<_> = blocks(text).into_iter().map(|block| block.id).collect();
        assert_eq!(ids, vec![BlockId::parse("block-aa"), None]);
        let minted = assign_ids(text, "doc/body");
        let ids: Vec<_> = blocks(&minted).into_iter().filter_map(|block| block.id).collect();
        assert_eq!(ids.len(), 2);
        assert_ne!(ids[0], ids[1]);
    }

    #[test]
    fn a_block_id_anchor_follows_its_block_through_edits_above() {
        let marked = assign_ids("Alpha beta.\n\nGamma delta.\n", "doc/body");
        let body = BlockText::parse(&marked);
        let gamma = body.blocks()[1].clone();
        let start = body.stored_offset(gamma.visible.start + 6, true);
        let anchor = body.anchor(start..start + 5);
        assert!(matches!(anchor, BlockAnchor::Block { .. }));
        let edited = carry(&marked, 0..0, "New opening.\n\n");
        let range = BlockText::parse(&edited).restore(&anchor).unwrap();
        assert_eq!(&edited[range], "delta");
    }

    #[test]
    fn a_heading_path_anchor_restores_across_minting() {
        let text = "# Top\n\n## Brief\n\nFirst point here.\n\n## Other\n\nElsewhere.\n";
        let start = text.find("point").unwrap();
        let anchor = BlockText::parse(text).anchor(start..start + 5);
        assert_eq!(
            anchor,
            BlockAnchor::HeadingPath {
                headings: vec!["Top".into(), "Brief".into()],
                range: 8..13,
            }
        );
        let marked = assign_ids(text, "doc/body");
        let range = BlockText::parse(&marked).restore(&anchor).unwrap();
        assert_eq!(&marked[range], "point");
        let renamed = marked.replace("## Brief", "## Summary");
        assert_eq!(BlockText::parse(&renamed).restore(&anchor), None);
    }

    #[test]
    fn replace_block_edits_through_and_keeps_the_id() {
        let marked = assign_ids("One.\n\n- item\n\nThree.\n", "doc/body");
        let item = blocks(&marked)[1].clone();
        let id = item.id.clone().unwrap();
        let edited = replace_block(&marked, &id, "- rewritten item").unwrap();
        assert_eq!(block_text(&edited, &id).as_deref(), Some("- rewritten item"));
        assert_eq!(BlockText::parse(&edited).visible(), "One.\n\n- rewritten item\n\nThree.\n");
    }
}
```

An HTML body adds what is particular to it: which elements are blocks and
which are not, that the title and header are never touched, that an id
written by an author keeps its place and spelling, that a pasted id names
nothing, and that a body lifted out of its header still finds its blocks.

<a name="chunk-html-tests"></a><sub>[`src/block_identity.rs`](../../crates/x0k-folio/src/block_identity.rs) · `#html-tests`</sub>

```rust {#html-tests}
#[cfg(test)]
mod html_tests {
    use super::*;

    const HEAD: &str = "# Title\n\n```turtle folio:document\nmanuscript:a\\/b a x0k:Manuscript ;\n    x0k:bodyFormat \"html\" .\n```\n\n";
    const BODY: &str = "<h1>Every rebellion</h1>\n\n<p class=\"lede\">Opening words.</p>\n\nbare text\n\n<!-- a comment -->\n<section id=\"machine\">\n<h2>A machine</h2>\n<p>One <em>value</em> in.</p>\n<figure class=\"study-figure\"><svg viewBox=\"0 0 8 8\"><rect width=\"8\"/></svg><figcaption>The chip.</figcaption></figure>\n<div class=\"media-embed\" data-ref=\"x0k:a/b\"><p>embedded</p></div>\n<details><summary>Source</summary><pre>fn x() {}</pre></details>\n<ul>\n<li>one\n<li>two\n</ul>\n</section>\n<script>let a = '<p data-block-id=\"block-fake\">';</script>\n<img src=\"x.png\"/>\n";

    fn page() -> String {
        format!("{HEAD}{BODY}")
    }

    fn ids(text: &str) -> Vec<Option<BlockId>> {
        blocks(text).into_iter().map(|block| block.id).collect()
    }

    #[test]
    fn marking_an_html_body_names_its_blocks_and_changes_nothing_else() {
        let marked = assign_ids(&page(), "doc/body");
        assert_eq!(BlockText::parse(&marked).visible(), page());
        assert!(marked.starts_with(HEAD), "the title and header are not blocks");
        assert!(!marked.contains("[block-"));
        let read = blocks(&marked);
        let kinds: Vec<_> = read.iter().map(|block| block.kind).collect();
        use BlockKind::*;
        assert_eq!(kinds, [Heading, Paragraph, Other, Heading, Paragraph, Other, Other, Other, List, Other, Other]);
        let unnamed: Vec<_> = read.iter().filter(|block| block.id.is_none()).map(|block| block.text.as_str()).collect();
        assert_eq!(unnamed, ["bare text", "<script>let a = '<p data-block-id=\"block-fake\">';</script>"]);
        let named: HashSet<_> = read.iter().filter_map(|block| block.id.clone()).collect();
        assert_eq!(named.len(), 9, "ids are unique");
        assert!(marked.contains("<section id=\"machine\">\n"), "a container is not a block");
        assert!(marked.contains("<p>embedded</p>"), "an embed is one block");
        assert!(marked.contains("<img data-block-id=\"") && marked.contains("<p data-block-id=\"block-"));
    }

    #[test]
    fn reading_and_writing_a_marked_html_body_is_the_identity() {
        let marked = assign_ids(&page(), "doc/body");
        assert_eq!(assign_ids(&marked, "doc/body"), marked);
        assert_eq!(assign_ids(&marked, "another/seed"), marked);
        assert_eq!(carry(&marked, 0..0, ""), marked);

        let spelled = "<p class=\"lede\"\n   data-block-id='block-aa'>";
        let authored = page().replace("<p class=\"lede\">", spelled);
        assert_eq!(ids(&authored)[1], BlockId::parse("block-aa"));
        let at = authored.find("Opening").unwrap();
        let edited = carry(&authored, at..at + 7, "Closing");
        let edited = carry(&edited, edited.find("rebellion").unwrap()..edited.find("rebellion").unwrap(), "small ");
        assert!(edited.contains(&format!("{spelled}Closing words.</p>")), "{edited}");
        assert_eq!(ids(&edited)[1], BlockId::parse("block-aa"));
        assert_eq!(ids(&edited).iter().flatten().count(), 1, "carrying an id names nothing new");
    }

    #[test]
    fn an_html_split_keeps_the_first_half_and_a_merge_keeps_the_first() {
        let marked = assign_ids(&format!("{HEAD}<p>Alpha beta.</p>\n\n<p>Gamma.</p>\n"), "doc");
        let (alpha, gamma) = (ids(&marked)[0].clone(), ids(&marked)[1].clone());
        let cut = marked.find(" beta").unwrap();
        let split = carry(&marked, cut..cut + 1, "</p>\n\n<p>");
        assert_eq!(ids(&split), [alpha.clone(), None, gamma.clone()]);
        let close = split.find("Alpha</p>").unwrap() + 5;
        let open = close + split[close..].find("<p>").unwrap() + 3;
        let joined = carry(&split, close..open, " ");
        assert_eq!(ids(&joined), [alpha.clone(), gamma.clone()]);
        assert_eq!(BlockText::parse(&joined).visible(), format!("{HEAD}<p>Alpha beta.</p>\n\n<p>Gamma.</p>\n"));
        let close = joined.find("beta.</p>").unwrap() + 5;
        let open = joined.find("Gamma").unwrap();
        let merged = carry(&joined, close..open, " ");
        assert_eq!(ids(&merged), [alpha]);
        assert!(!merged.contains(gamma.unwrap().as_str()), "the second id retires");
    }

    #[test]
    fn a_pasted_or_duplicated_id_names_nothing() {
        let marked = assign_ids(&format!("{HEAD}<p>Source.</p>\n"), "doc");
        let id = ids(&marked)[0].clone().unwrap();
        let end = marked.len();
        let pasted = carry(&marked, end..end, &format!("\n<p data-block-id=\"{id}\">Copy.</p>\n"));
        assert_eq!(ids(&pasted), [Some(id.clone()), None]);
        assert_eq!(pasted.matches(id.as_str()).count(), 1);

        let twice = format!("{HEAD}<p data-block-id=\"block-aa\">first</p>\n<p data-block-id=\"block-aa\">second</p>\n");
        assert_eq!(ids(&twice), [BlockId::parse("block-aa"), None]);
        let renamed = ids(&assign_ids(&twice, "doc"));
        assert!(renamed.iter().all(Option::is_some) && renamed[0] != renamed[1]);
    }

    #[test]
    fn a_body_lifted_out_of_its_header_still_finds_its_blocks() {
        let marked = assign_ids(&page(), "doc/body");
        let body = &marked[HEAD.len()..];
        let id = ids(&marked)[4].clone().unwrap();
        assert_eq!(block_text(body, &id).as_deref(), Some("<p>One <em>value</em> in.</p>"));
        let edited = replace_block(body, &id, "<p>Two values in.</p>").unwrap();
        assert_eq!(block_text(&edited, &id).as_deref(), Some("<p>Two values in.</p>"));
        let rest: Vec<_> = ids(&marked).into_iter().flatten().collect();
        assert!(rest.iter().all(|kept| edited.contains(kept.as_str())), "no other id moved");
    }

    #[test]
    fn an_html_heading_path_anchor_restores_across_minting() {
        let text = page();
        let start = text.find("value").unwrap();
        let anchor = BlockText::parse(&text).anchor(start..start + 5);
        let BlockAnchor::HeadingPath { headings, .. } = &anchor else { panic!("{anchor:?}") };
        assert_eq!(headings, &["Every rebellion".to_string(), "A machine".to_string()]);
        let marked = assign_ids(&text, "doc/body");
        let range = BlockText::parse(&marked).restore(&anchor).unwrap();
        assert_eq!(&marked[range], "value");
    }
}
```

## Composing the module

<a name="chunk-module-doc"></a><sub>[`src/block_identity.rs`](../../crates/x0k-folio/src/block_identity.rs) · `#module-doc`</sub>

```rust {#module-doc}
//! Block identity: the stable id each block of a body carries, written into
//! the body itself (a link reference definition in markdown, a
//! `data-block-id` attribute in HTML), carried through edits by lineage, and
//! used to anchor selections, scroll positions and proposal targets.
use std::collections::HashSet;
use std::ops::Range;

use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd};
```

<a name="chunk-root"></a><sub>[`src/block_identity.rs`](../../crates/x0k-folio/src/block_identity.rs) · `#root` · assembles [module-doc](#chunk-module-doc) · [block-id](#chunk-block-id) · [read-body](#chunk-read-body) · [strip-markers](#chunk-strip-markers) · [block-shapes](#chunk-block-shapes) · [write-markers](#chunk-write-markers) · [body-format](#chunk-body-format) · [html-tree](#chunk-html-tree) · [html-parse](#chunk-html-parse) · [html-attributes](#chunk-html-attributes) · [html-blocks](#chunk-html-blocks) · [html-markers](#chunk-html-markers) · [assign-ids](#chunk-assign-ids) · [carry](#chunk-carry) · [block-access](#chunk-block-access) · [anchors](#chunk-anchors) · [tests](#chunk-tests) · [html-tests](#chunk-html-tests)</sub>

```rust {#root}
<<module-doc>>

<<block-id>>

<<read-body>>

<<strip-markers>>

<<block-shapes>>

<<write-markers>>

<<body-format>>

<<html-tree>>

<<html-parse>>

<<html-attributes>>

<<html-blocks>>

<<html-markers>>

<<assign-ids>>

<<carry>>

<<block-access>>

<<anchors>>

<<tests>>

<<html-tests>>
```

What remains open: lineage that crosses fields (a block dragged from one Today entry to
another is, today, a deletion and an insertion).
