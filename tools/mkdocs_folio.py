"""MkDocs hook: show a folio document's fenced blocks as code.

Python-Markdown accepts a fence opening of a language and, optionally, an
attribute list in braces. folio writes two other forms, and both fall through
to inline code that runs on into the prose after the block:

- a typed block, ``turtle folio:document`` (the header) or
  ``turtle folio:graph``: a language and a marker word;
- a chunk, ``python {#backoff from="pkg/retry.py" symbol="Backoff.next"}``:
  a language and then the attributes.

This hook rewrites those openings before Markdown runs, and nothing else:
a typed block becomes a plain ``turtle`` fence, and a chunk becomes
``{.python #backoff}``, which pymdownx.superfences and fenced_code both read
as Python code with the chunk's id. The other attributes name where the
code came from and are not shown. Fences inside another fence, and indented
code, are left alone.

Copy this file into your site and name it in ``mkdocs.yml``, by its path
relative to that file::

    hooks:
      - mkdocs_folio.py

To drop the header instead of showing it, set ``folio_header: hide`` in a
page's front matter, or under ``extra:`` for the whole site; a page's own
setting wins. Only the page's first fenced block is dropped, and only when its
info string is exactly ``turtle folio:document``.
"""

import re

_OPENING = re.compile(r"^( {0,3})(`{3,}|~{3,})(.*?)\s*$")
_TYPED = re.compile(r"^([\w+.-]+)[ \t]+[A-Za-z][\w-]*:[\w/-]+$")
_CHUNK = re.compile(r"^([\w+.-]+)[ \t]*\{[ \t]*#([^\s}\"']+)[^}]*\}$")
_HEADER = "turtle folio:document"


def _closes(line, fence):
    match = _OPENING.match(line.rstrip("\r\n"))
    return (
        match is not None
        and match.group(2)[0] == fence[0]
        and len(match.group(2)) >= len(fence)
        and match.group(3) == ""
    )


def _opening(indent, fence, info):
    typed = _TYPED.match(info)
    if typed:
        return f"{indent}{fence}{typed.group(1)}"
    chunk = _CHUNK.match(info)
    if chunk:
        return f"{indent}{fence}{{.{chunk.group(1)} #{chunk.group(2)}}}"
    return None


def rewrite(markdown, drop_header=False):
    """Return ``markdown`` with folio's fence openings in a form Markdown reads."""
    out = []
    fence = None  # the opening run of the fence we are inside, if any
    dropping = False
    seen_fence = False
    for line in markdown.splitlines(keepends=True):
        if fence is not None:
            if _closes(line, fence):
                fence = None
                if dropping:
                    dropping = False
                    continue
            if not dropping:
                out.append(line)
            continue
        match = _OPENING.match(line.rstrip("\r\n"))
        if match is None or (match.group(2)[0] == "`" and "`" in match.group(3)):
            out.append(line)
            continue
        indent, fence, info = match.groups()
        first, seen_fence = not seen_fence, True
        if drop_header and first and info == _HEADER:
            dropping = True
            continue
        opening = _opening(indent, fence, info)
        if opening is None:
            out.append(line)
        else:
            ending = line[len(line.rstrip("\r\n")):]
            out.append(opening + ending)
    return "".join(out)


def on_page_markdown(markdown, page, config, files):
    setting = page.meta.get("folio_header", (config.get("extra") or {}).get("folio_header"))
    return rewrite(markdown, drop_header=setting == "hide")
