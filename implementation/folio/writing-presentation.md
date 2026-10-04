# Writing keeps its source

```turtle folio:document
implementation:folio\/writing-presentation a x0k:Implementation ;
    x0k:status "draft" ;
    x0k:summary "A Markdown document shown as editable text keeps its source bytes: each displayed run maps back to a source range, so a heading can hide its # and an edit still saves the exact source." ;
    x0k:cites implementation:folio\/html-canonical,
        implementation:document-editing\/source-projection ;
    x0k:implements design:programmable-text-editing ;
    x0k:motivatedBy intent:7d44fa4d-4474-4737-a69f-a8fd190fa580 ;
    folio:tangleCrate "crates/x0k-folio" ;
    folio:tangleRoot "src/writing_presentation.rs" .
```

A heading can lose its visible `# ` without losing those bytes in the document.
We give each displayed literal run a source range and carry its hidden prefix and
line ending in the presentation. The existing Markdown parser identifies ATX
headings; the HTML editing policy checks that input changes only run text. The
codec performs no I/O and holds no editing session, history or backend handle.

Alice writes `# café`, presses Return, then keeps typing before the new screen
arrives. Her submitted heading still contains a newline. Decoding that submitted
layout preserves the new raw source; a fresh projection splits it into a heading
and an empty paragraph. The two layouts have different run maps, but one source.

## Presentation is contextual

Literal mode never interprets markup. Markdown mode displays ATX headings and
keeps other syntax literal, including fences and inline markup. When there are no
ATX headings, the existing single paragraph remains the identity projection.
Setext headings and typeset math are outside this codec's contract. A rich layout
has one declared editable root, with independently addressed direct text runs:

<a name="chunk-declare-presentation"></a><sub>[`src/writing_presentation.rs`](../../crates/x0k-folio/src/writing_presentation.rs) · `#declare-presentation`</sub>

```rust {#declare-presentation}
use std::{collections::BTreeMap, ops::Range};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use pulldown_cmark::{Event, Parser, Tag};
use crate::html_canonical::HtmlTextPolicy;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WritingFormat { Literal, Markdown }
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Row { level: u8, prefix: String, ending: String }
#[derive(Clone, Debug)]
pub struct WritingPresentation {
    source: String,
    rows: Option<Vec<Row>>,
    pub runs: Vec<WritingRun>,
}
#[derive(Clone, Debug)]
pub struct WritingRun { pub element: String, pub source: Range<usize> }
const STYLE: &str = "white-space:pre-wrap;min-height:2em;margin:0";
```

The parser supplies heading locations, so a `#` inside a fenced code block is
ordinary text. Only the ATX prefix is hidden; spaces after its first delimiter
and optional closing hashes remain editable source. CRLF and LF endings remain
distinct. A trailing newline creates an empty writing run:

<a name="chunk-project-source"></a><sub>[`src/writing_presentation.rs`](../../crates/x0k-folio/src/writing_presentation.rs) · `#project-source`</sub>

```rust {#project-source}
impl WritingPresentation {
    pub fn project(source: &str, format: WritingFormat) -> Self {
        let mut headings = BTreeMap::new();
        if format == WritingFormat::Markdown {
            for (event, range) in Parser::new(source).into_offset_iter() {
                if let Event::Start(Tag::Heading { level, .. }) = event {
                    let start=source[..range.start].rfind('\n').map_or(0,|newline| newline+1);
                    if let Some((prefix, found)) = atx_prefix(&source[start..]) {
                        let indent=prefix.bytes().take_while(|b| *b==b' ').count();
                        if found == level as u8 && range.start==start+indent { headings.insert(start, (prefix, found)); }
                    }
                }
            }
        }
        if headings.is_empty() { return Self::literal(source); }
        let mut rows = Vec::new(); let mut lengths = Vec::new(); let mut offset = 0;
        for line in source.split_inclusive('\n') {
            let (text, ending) = if let Some(text)=line.strip_suffix("\r\n") {(text,"crlf")}
                else if let Some(text)=line.strip_suffix('\n') {(text,"lf")} else {(line,"none")};
            let (prefix,level)=headings.get(&offset).cloned().unwrap_or_default();
            lengths.push(text.len()-prefix.len());
            rows.push(Row {level,prefix,ending:ending.into()}); offset += line.len();
        }
        if source.ends_with('\n') { rows.push(Row {level:0,prefix:String::new(),ending:"none".into()}); lengths.push(0); }
        let runs = runs_for(&rows, &lengths);
        Self {source:source.into(),rows:Some(rows),runs}
    }
    fn literal(source: &str) -> Self {
        Self {source:source.into(),rows:None,runs:vec![WritingRun {element:"/p[1]".into(),source:0..source.len()}]}
    }
    pub fn source(&self) -> &str { &self.source }
    pub fn region(&self) -> &str { if self.rows.is_some() { "/div[1]" } else { "/p[1]" } }
    pub fn rich(&self) -> bool { self.rows.is_some() }
}
```

The same parser identifies code spans and blocks at a source cursor, so writing
commands can apply their explicit code-context restrictions. The row descriptors
describe hidden bytes, not edit authority. A caller must
still check the decoded source against its original document reading. Prefixes
must agree with their heading level; endings use a closed vocabulary. Registered
paths use per-tag ordinals, just like the canonical HTML policy:

<a name="chunk-address-runs"></a><sub>[`src/writing_presentation.rs`](../../crates/x0k-folio/src/writing_presentation.rs) · `#address-runs`</sub>

```rust {#address-runs}
fn atx_prefix(line: &str) -> Option<(String,u8)> {
    let indent = line.bytes().take_while(|b| *b==b' ').count();
    if indent>3 { return None; }
    let level = line[indent..].bytes().take_while(|b| *b==b'#').count();
    if !(1..=6).contains(&level) || !matches!(line.as_bytes().get(indent+level),Some(b' '|b'\t')) { return None; }
    Some((line[..indent+level+1].into(),level as u8))
}
fn tag(row:&Row) -> String { if row.level==0 { "p".into() } else { format!("h{}",row.level) } }
fn ending(row:&Row) -> &str { match row.ending.as_str() { "lf"=>"\n", "crlf"=>"\r\n", _=>"" } }
fn runs_for(rows:&[Row],lengths:&[usize]) -> Vec<WritingRun> {
    let mut offset=0; let mut ordinals=BTreeMap::<String,usize>::new();
    rows.iter().zip(lengths).map(|(row,len)| {
        let tag=tag(row); let ordinal=ordinals.entry(tag.clone()).or_default(); *ordinal+=1;
        offset+=row.prefix.len(); let start=offset; offset+=len;
        let run=WritingRun {element:format!("/div[1]/{tag}[{ordinal}]"),source:start..offset};
        offset+=ending(row).len(); run
    }).collect()
}
/// Markdown code spans and blocks supply contextual command constraints.
pub fn code_at(source:&str,offset:usize) -> bool {
    Parser::new(source).into_offset_iter().any(|(event,range)|
        matches!(event,Event::Code(_)|Event::Start(Tag::CodeBlock(_))) && range.start<=offset && offset<=range.end)
}
pub fn escape(value:&str) -> String {
    value.replace('&',"&amp;").replace('<',"&lt;").replace('>',"&gt;").replace('"',"&quot;").replace('\r',"&#13;")
}
```

The renderer accepts caller-owned focus attributes on the root. They remain
outside the hidden-byte descriptor. Each empty row has a positive writing box;
no zero-width character enters the document. Ordinary paragraphs retain their
existing style, while heading sizes inherit the surrounding font:

<a name="chunk-render-presentation"></a><sub>[`src/writing_presentation.rs`](../../crates/x0k-folio/src/writing_presentation.rs) · `#render-presentation`</sub>

```rust {#render-presentation}
impl WritingPresentation {
    pub fn html(&self, root_attributes:&str) -> String {
        let Some(rows)=&self.rows else {
            return format!("<p{root_attributes} contenteditable=\"plaintext-only\" style=\"{STYLE}\">{}</p>",escape(&self.source));
        };
        let mut html=format!("<div{root_attributes} contenteditable=\"plaintext-only\" data-x0k-writing=\"{}\" style=\"{STYLE}\">",escape(&serde_json::to_string(rows).expect("rows serialize")));
        for (row,run) in rows.iter().zip(&self.runs) {
            let tag=tag(row);
            let size=if row.level==0 {"1em"} else {match row.level {1=>"1.35em",2=>"1.25em",_=>"1.15em"}};
            html.push_str(&format!("<{tag} style=\"white-space:pre-wrap;min-height:1.4em;margin:0;font-size:{size};font-weight:{}\">{}</{tag}>",if row.level==0 {"inherit"} else {"600"},escape(&self.source[run.source.clone()])));
        }
        html.push_str("</div>"); html
    }
}
```

Input is decoded in its submitted layout. We read only the named literal child
runs and reconstruct the expected entire fragment; extra elements, attributes or
nested text fail that comparison. A heading containing a just-typed newline is
valid submitted input even though its next projection has a different shape:

<a name="chunk-decode-submitted-layout"></a><sub>[`src/writing_presentation.rs`](../../crates/x0k-folio/src/writing_presentation.rs) · `#decode-submitted-layout`</sub>

```rust {#decode-submitted-layout}
impl WritingPresentation {
    pub fn decode(html:&str, root_attributes:&str) -> Result<Self> {
        let policy=HtmlTextPolicy::Editing;
        if let Ok(attributes)=policy.element_attributes(html,"/div[1]") {
            if let Some(encoded)=attributes.get("data-x0k-writing") {
                let rows:Vec<Row>=serde_json::from_str(encoded)?;
                ensure!(!rows.is_empty() && rows.len()<=10000,"invalid writing row count");
                for (index,row) in rows.iter().enumerate() {
                    ensure!(matches!(row.ending.as_str(),"none"|"lf"|"crlf"),"invalid line ending");
                    ensure!(index+1==rows.len() || row.ending!="none","missing row boundary");
                    ensure!(row.level<=6 && if row.level==0 {row.prefix.is_empty()} else {atx_prefix(&row.prefix).is_some_and(|(prefix,level)| prefix==row.prefix && level==row.level)},"invalid hidden heading prefix");
                }
                let addresses=runs_for(&rows,&vec![0;rows.len()]);
                let texts=addresses.iter().map(|run| policy.literal_element_text(html,&run.element)).collect::<std::result::Result<Vec<_>,_>>()?;
                let mut source=String::new();
                for (row,text) in rows.iter().zip(&texts) { source.push_str(&row.prefix); source.push_str(text); source.push_str(ending(row)); }
                let runs=runs_for(&rows,&texts.iter().map(String::len).collect::<Vec<_>>());
                let decoded=Self {source,rows:Some(rows),runs};
                ensure!(policy.normalize_html(html)==policy.normalize_html(&decoded.html(root_attributes)),"the writing projection changed structure");
                return Ok(decoded);
            }
        }
        let decoded=Self::literal(&policy.literal_element_text(html,"/p[1]")?);
        ensure!(policy.normalize_html(html)==policy.normalize_html(&decoded.html(root_attributes)),"the literal writing projection changed structure");
        Ok(decoded)
    }
}
```

The source/display map ends at this boundary. A change across hidden separators
needs an explicit structural operation from the owner; flattening child text does
not authorize it.

## The same field can live inside a page

A journal gives each root a canonical page address. We decode only that declared
root; literal fields keep their existing page styles, while rich fields still
regenerate their complete codec fragment. Extra child markup cannot be flattened
into source. The entry owner independently checks structure, identity and reading
before accepting these bytes. Scoped run addresses and focus attributes are
presentation coordinates, not document authority.

<a name="chunk-page-field"></a><sub>[`src/writing_presentation.rs`](../../crates/x0k-folio/src/writing_presentation.rs) · `#page-field`</sub>

```rust {#page-field}
impl WritingPresentation {
    /// The single typed delimiter that gives this heading its ATX meaning.
    pub fn heading_delimiter(&self,offset:usize) -> Option<Range<usize>> {
        let index=self.runs.iter().position(|run| run.source.start==offset)?;
        let row=self.rows.as_ref()?.get(index)?;
        (row.level>0 && matches!(row.prefix.as_bytes().last(),Some(b' '|b'\t'))).then(|| offset-1..offset)
    }
    pub fn scoped_runs(&self, region:&str) -> Vec<WritingRun> {
        self.runs.iter().map(|run| WritingRun {element:run.element.replacen(self.region(),region,1),source:run.source.clone()}).collect()
    }
    pub fn decode_region(html:&str, region:&str) -> Result<Self> {
        let policy=HtmlTextPolicy::Editing; let attributes=policy.element_attributes(html,region)?;
        ensure!(attributes.get("contenteditable").map(String::as_str)==Some("plaintext-only"),"the field is not declared editable");
        let tag=region.rsplit('/').next().and_then(|leaf| leaf.split('[').next()).unwrap_or_default();
        if !attributes.contains_key("data-x0k-writing") {
            return Ok(Self::literal(&policy.literal_element_text(html,region)?));
        }
        ensure!(tag=="div","rich writing requires its declared root");
        let render=|key:&str,value:&str| format!(" {key}=\"{}\"",escape(value));
        let extra=attributes.iter().filter(|(key,_)| !matches!(key.as_str(),"style"|"contenteditable"|"data-x0k-writing"))
            .map(|(key,value)| render(key,value)).collect::<String>();
        let all=attributes.iter().map(|(key,value)| render(key,value)).collect::<String>();
        let fragment=format!("<div{all}>{}</div>",policy.element_inner_html(html,region)?);
        Self::decode(&fragment,&extra)
    }
    pub fn selection_attributes(&self,token:&str,anchor:usize,focus:usize) -> Option<String> {
        self.selection_attributes_in(self.region(),token,anchor,focus)
    }
    pub fn selection_attributes_in(&self,region:&str,token:&str,anchor:usize,focus:usize) -> Option<String> {
        let scoped=self.scoped_runs(region);
        let endpoint=|name,offset| -> Option<String> {
            if !self.source.is_char_boundary(offset) {return None;}
            let run=scoped.iter().rev().find(|run| run.source.start<=offset && offset<=run.source.end)?;
            Some(format!(" data-x0k-selection-{name}-node=\"{}\" data-x0k-selection-{name}-run=\"0\" data-x0k-selection-{name}-text=\"{}\" data-x0k-selection-{name}=\"{}\"",
                escape(&run.element),escape(&self.source[run.source.clone()]),offset-run.source.start))
        };
        if !self.rich() {
            ensure_selection(&self.source,anchor,focus)?;
            return Some(format!(" data-x0k-focus-request=\"{}\" data-x0k-selection-text=\"{}\" data-x0k-selection-anchor=\"{anchor}\" data-x0k-selection-focus=\"{focus}\"",escape(token),escape(&self.source)));
        }
        Some(format!(" data-x0k-focus-request=\"{}\"{}{}",escape(token),endpoint("anchor",anchor)?,endpoint("focus",focus)?))
    }
}
fn ensure_selection(source:&str,anchor:usize,focus:usize) -> Option<()> {
    (source.is_char_boundary(anchor) && source.is_char_boundary(focus)).then_some(())
}
```

<a name="chunk-test-source-fidelity"></a><sub>[`src/writing_presentation.rs`](../../crates/x0k-folio/src/writing_presentation.rs) · `#test-source-fidelity`</sub>

```rust {#test-source-fidelity}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn page_fields_keep_raw_source_scoped_runs_and_checked_focus() {
        let p=WritingPresentation::project("# café\r\nbody 界",WritingFormat::Markdown);
        let field=p.html(" id=\"entry-1\" data-journal-reading=\"reading\"");
        let page=format!("<main><section><div>{field}</div></section></main>");
        let region="/main[1]/section[1]/div[1]/div[1]";
        let decoded=WritingPresentation::decode_region(&page,region).unwrap(); assert_eq!(decoded.source(),p.source());
        assert_eq!(decoded.scoped_runs(region)[1].element,format!("{region}/p[1]"));
        let focus=p.selection_attributes("focus",p.source().len(),p.source().len()).unwrap();
        assert!(focus.contains("data-x0k-selection-focus-node=\"/div[1]/p[1]\""));
        assert!(p.selection_attributes("hidden",1,1).is_none());
        assert!(p.selection_attributes("unicode",6,6).is_none());
        assert!(WritingPresentation::decode_region(&page.replace("body 界","<em>body 界</em>"),region).is_err());
        let old="<main><p contenteditable=\"plaintext-only\" style=\"white-space:pre-wrap;min-height:2em\">literal # café</p></main>";
        assert_eq!(WritingPresentation::decode_region(old,"/main[1]/p[1]").unwrap().source(),"literal # café");
    }
    #[test]
    fn headings_preserve_exact_source_and_exclude_fenced_hashes() {
        let source="# café\r\nbody\n```\n# literal\n```\n## 界\n";
        let p=WritingPresentation::project(source,WritingFormat::Markdown);
        assert!(p.rich()); assert_eq!(p.runs[0].source,2..7);
        let html=p.html(""); assert!(html.contains("<h1 ")); assert!(html.contains("<h2 "));
        assert_eq!(html.matches("<h1 ").count(),1);
        let d=WritingPresentation::decode(&html,"").unwrap(); assert_eq!(d.source(),source);
        assert_eq!(d.runs.last().unwrap().source,source.len()..source.len());
        assert!(!WritingPresentation::project(source,WritingFormat::Literal).rich());
    }
    #[test]
    fn submitted_newline_keeps_its_layout_before_fresh_projection() {
        let p=WritingPresentation::project("# café",WritingFormat::Markdown);
        let html=p.html("").replace(">café</h1>",">café\nbody</h1>");
        let d=WritingPresentation::decode(&html,"").unwrap(); assert_eq!(d.source(),"# café\nbody");
        assert_eq!(d.runs.len(),1);
        let fresh=WritingPresentation::project(d.source(),WritingFormat::Markdown);
        assert_eq!(fresh.runs.len(),2);
        assert_eq!(&fresh.source()[fresh.runs[1].source.clone()],"body");
    }
    #[test]
    fn empty_headings_and_terminal_carriage_return_keep_source_bytes() {
        for source in ["# ","# café\nend\r","  ## café\n"] {
            let p=WritingPresentation::project(source,WritingFormat::Markdown);
            assert!(p.rich(),"{source:?}");
            assert!(p.html("").contains(if source.starts_with("  ") {"<h2 "} else {"<h1 "}));
            assert_eq!(WritingPresentation::decode(&p.html(""),"").unwrap().source(),source);
        }
    }
    #[test]
    fn extra_markup_and_metadata_are_not_text() {
        let p=WritingPresentation::project("# café",WritingFormat::Markdown); let html=p.html("");
        assert!(WritingPresentation::decode(&html.replace(">café</h1>",">café<em>lost</em></h1>"),"").is_err());
        assert!(WritingPresentation::decode(&html.replace("font-weight:600","font-weight:700"),"").is_err());
        assert!(WritingPresentation::decode(&(html+"<p>lost</p>"),"").is_err());
    }
}
```

The hard part is the next caret, not the larger glyphs. A submitted DOM and its
fresh projection can both describe the same source while assigning different
run identities. Keeping both maps explicit lets an attachment acknowledge the
input it received and focus the page it actually displays.
