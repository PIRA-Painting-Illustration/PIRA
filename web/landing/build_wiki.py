#!/usr/bin/env python3
"""Builds the PIRA wiki into the landing site: docs/wiki/*.md -> web/landing/wiki/*.html.

GitHub wikis need a public repository on the free plan, so the wiki is published as pages of the
site instead. Home.md becomes wiki/index.html, _Sidebar.md the navigation on every page, and
links between pages (Page.md, Page.md#section) point at the built pages.

The converter covers the Markdown the wiki uses: # and ## headings, paragraphs, - and 1. lists
(indented lines continue an item), pipe tables, **bold**, *italic*, `code` and [links](url).
It uses only the standard library. Run it after editing the wiki:

    python3 web/landing/build_wiki.py
"""

from __future__ import annotations

import html
import re
import shutil
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
WIKI = ROOT / "docs" / "wiki"
OUT = ROOT / "web" / "landing" / "wiki"
LOGO = ROOT / "assets" / "app-icon" / "hicolor" / "64x64" / "apps" / "ai.storyteller.photocraft.png"

SKIP = {"README.md", "_Sidebar.md"}


def page_name(md: str) -> str:
    """The built file for a wiki page file name."""
    stem = md[:-3] if md.endswith(".md") else md
    return "index.html" if stem == "Home" else f"{stem}.html"


def slug(text: str) -> str:
    """GitHub's heading anchor: lower case, punctuation dropped, spaces to hyphens."""
    s = re.sub(r"<[^>]+>", "", text).lower()
    s = re.sub(r"[^\w\- ]", "", s)
    return s.replace(" ", "-")


def link_target(url: str) -> str:
    if re.match(r"^[a-z]+:", url) or url.startswith("#"):
        return url
    path, _, anchor = url.partition("#")
    if path.endswith(".md"):
        path = page_name(path)
    return path + (f"#{anchor}" if anchor else "")


def inline(text: str) -> str:
    """Escape `text` and apply code spans, links, bold and italic."""
    codes: list[str] = []

    def keep_code(m: re.Match) -> str:
        codes.append(f"<code>{html.escape(m.group(1))}</code>")
        return f"\x00{len(codes) - 1}\x00"

    text = re.sub(r"`([^`]+)`", keep_code, text)
    text = html.escape(text, quote=False)

    def link(m: re.Match) -> str:
        url = link_target(html.unescape(m.group(2)))
        external = re.match(r"^https?:", url) is not None
        extra = ' rel="noopener"' if external else ""
        return f'<a href="{html.escape(url)}"{extra}>{m.group(1)}</a>'

    text = re.sub(r"\[([^\]]+)\]\(([^)\s]+)\)", link, text)
    text = re.sub(r"\*\*(.+?)\*\*", r"<strong>\1</strong>", text)
    text = re.sub(r"(?<![\w*])\*(?!\s)(.+?)(?<!\s)\*(?![\w*])", r"<em>\1</em>", text)
    return re.sub(r"\x00(\d+)\x00", lambda m: codes[int(m.group(1))], text)


def convert(md: str) -> tuple[str, str]:
    """(title, body HTML) of a wiki page."""
    lines = md.splitlines()
    out: list[str] = []
    title = ""
    i = 0
    para: list[str] = []

    def flush_para() -> None:
        if para:
            out.append(f"<p>{inline(' '.join(para))}</p>")
            para.clear()

    while i < len(lines):
        line = lines[i]
        stripped = line.strip()
        if not stripped:
            flush_para()
            i += 1
            continue
        m = re.match(r"^(#{1,6})\s+(.*)$", line)
        if m:
            flush_para()
            level, text = len(m.group(1)), m.group(2).strip()
            if level == 1 and not title:
                title = re.sub(r"[*`]", "", text)
            out.append(f'<h{level} id="{slug(text)}">{inline(text)}</h{level}>')
            i += 1
            continue
        if stripped.startswith("|"):
            flush_para()
            rows = []
            while i < len(lines) and lines[i].strip().startswith("|"):
                rows.append([c.strip() for c in lines[i].strip().strip("|").split("|")])
                i += 1
            has_head = len(rows) > 1 and all(re.fullmatch(r":?-+:?", c) for c in rows[1])
            parts = ['<div class="table"><table>']
            if has_head:
                parts.append("<thead><tr>" + "".join(f"<th>{inline(c)}</th>" for c in rows[0]) + "</tr></thead>")
                rows = rows[2:]
            parts.append("<tbody>" + "".join("<tr>" + "".join(f"<td>{inline(c)}</td>" for c in r) + "</tr>" for r in rows) + "</tbody>")
            parts.append("</table></div>")
            out.append("".join(parts))
            continue
        lm = re.match(r"^(-|\*|\d+\.)\s+(.*)$", line)
        if lm:
            flush_para()
            ordered = lm.group(1)[0].isdigit()
            items: list[str] = []
            while i < len(lines):
                cur = lines[i]
                im = re.match(r"^(-|\*|\d+\.)\s+(.*)$", cur)
                if im and (im.group(1)[0].isdigit()) == ordered:
                    items.append(im.group(2).strip())
                elif cur.startswith((" ", "\t")) and cur.strip() and items:
                    items[-1] += " " + cur.strip()
                else:
                    break
                i += 1
            tag = "ol" if ordered else "ul"
            out.append(f"<{tag}>" + "".join(f"<li>{inline(it)}</li>" for it in items) + f"</{tag}>")
            continue
        if stripped.startswith(">"):
            flush_para()
            quote = []
            while i < len(lines) and lines[i].strip().startswith(">"):
                quote.append(lines[i].strip().lstrip(">").strip())
                i += 1
            out.append(f"<blockquote><p>{inline(' '.join(quote))}</p></blockquote>")
            continue
        para.append(stripped)
        i += 1
    flush_para()
    return title or "PIRA wiki", "\n".join(out)


def sidebar(current: str) -> str:
    """The navigation from _Sidebar.md, the current page marked."""
    body = convert((WIKI / "_Sidebar.md").read_text(encoding="utf-8"))[1]
    # The first link (the wiki's home) is the heading of the navigation.
    body = body.replace(f'href="{current}"', f'href="{current}" aria-current="page"')
    return body


STYLE = """
:root{color-scheme:dark;--bg:#0d141d;--panel:#14212c;--line:#2a3b4c;--text:#f1f5f7;--muted:#a7b7c1;--a:#b4f0d7;--b:#89bdff}
*{box-sizing:border-box}html{scroll-behavior:smooth}
body{margin:0;background:radial-gradient(ellipse 70% 35% at 85% -10%,#234b5a 0%,transparent 75%),var(--bg);color:var(--text);font:16px/1.65 Montserrat,system-ui,-apple-system,Segoe UI,sans-serif;overflow-x:clip}
a{color:var(--a)}
.wrap{max-width:1180px;margin:auto;padding:0 26px}
header{border-bottom:1px solid var(--line)}
nav.top{display:flex;align-items:center;justify-content:space-between;min-height:72px;gap:20px}
.brand{display:flex;align-items:center;gap:12px;text-decoration:none;color:var(--text)}
.brand img{width:36px;height:36px}
.brand span{font:700 22px/1 Philosopher,Georgia,serif}
.brand small{font:500 13px/1 Montserrat,system-ui,sans-serif;color:var(--muted);margin-left:4px}
.toplinks{display:flex;gap:22px}.toplinks a{text-decoration:none;color:var(--muted);font-size:14px}.toplinks a:hover{color:var(--text)}
.layout{display:grid;grid-template-columns:250px minmax(0,1fr);gap:46px;padding-top:40px;padding-bottom:70px}
aside{position:sticky;top:20px;align-self:start;max-height:calc(100vh - 40px);overflow:auto;font-size:14px}
aside p{margin:18px 0 6px;color:var(--text)}aside p:first-child{margin-top:0}
aside ul{list-style:none;margin:0;padding:0}
aside li a{display:block;padding:5px 10px;border-radius:7px;color:var(--muted);text-decoration:none}
aside li a:hover{color:var(--text);background:#152634}
aside a[aria-current]{color:var(--a);background:#163133}
aside summary{display:none}
article{max-width:760px;min-width:0}
article h1{font:700 clamp(34px,5vw,48px)/1.1 Philosopher,Georgia,serif;margin:0 0 18px;letter-spacing:-.01em}
article h2{font:700 28px/1.2 Philosopher,Georgia,serif;margin:42px 0 10px;padding-top:6px;border-top:1px solid var(--line)}
article h1+p{font-size:18px;color:var(--muted)}
article p,article li{color:#d6e0e5}
article strong{color:var(--text)}
article ul,article ol{padding-left:24px}article li{margin:5px 0}
code{background:#1b303c;padding:2px 6px;border-radius:5px;font-size:.88em;overflow-wrap:anywhere}
.table{overflow-x:auto;margin:18px 0;border:1px solid var(--line);border-radius:12px}
table{border-collapse:collapse;width:100%;font-size:14px}
th,td{text-align:left;vertical-align:top;padding:10px 14px;border-bottom:1px solid var(--line)}
th{background:var(--panel);color:var(--text)}tr:last-child td{border-bottom:0}
blockquote{margin:18px 0;border-left:3px solid var(--a);padding:6px 18px;background:#11262a;border-radius:0 10px 10px 0}
.pager{display:flex;justify-content:space-between;gap:16px;margin-top:52px;padding-top:20px;border-top:1px solid var(--line);font-size:14px}
.pager a{text-decoration:none;color:var(--muted)}.pager a:hover{color:var(--a)}.pager b{display:block;color:var(--text);font-size:16px}
footer{padding:26px 0 50px;color:var(--muted);font-size:13px;border-top:1px solid var(--line)}
@media(max-width:860px){
 .layout{grid-template-columns:1fr;gap:18px;padding-top:22px}
 aside{position:static;max-height:none;border:1px solid var(--line);border-radius:12px;padding:4px 14px;background:var(--panel)}
 aside summary{display:block;cursor:pointer;padding:10px 0;font-weight:600}
 aside details:not([open]) .navbody{display:none}
 .toplinks a.hide-sm{display:none}
}
"""

PAGE = """<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<meta name="description" content="{description}">
<title>{title}</title>
<link rel="icon" type="image/png" href="../media/logo-64.png">
<link rel="preconnect" href="https://fonts.googleapis.com"><link rel="preconnect" href="https://fonts.gstatic.com" crossorigin><link href="https://fonts.googleapis.com/css2?family=Montserrat:wght@400;500;600;700&family=Philosopher:ital,wght@0,400;0,700;1,400&display=swap" rel="stylesheet">
<style>{style}</style>
</head>
<body>
<header><div class="wrap"><nav class="top"><a class="brand" href="../index.html"><img src="../media/logo-64.png" alt=""><span>PIRA</span><small>wiki</small></a><div class="toplinks"><a href="../index.html">Home</a><a class="hide-sm" href="index.html">Wiki contents</a><a href="Troubleshooting-and-FAQ.html">Help</a></div></nav></div></header>
<div class="wrap layout">
<aside><details open><summary>All wiki pages</summary><div class="navbody">{sidebar}</div></details></aside>
<main><article>
{body}
{pager}
</article></main>
</div>
<footer class="wrap">PIRA is in early testing. Built from the wiki in the PIRA repository.</footer>
<script>
// On a phone the page list starts folded so the article comes first; wider, it is always open.
(function () {{
  var narrow = matchMedia('(max-width:860px)'), list = document.querySelector('aside details');
  var fit = function () {{ list.open = !narrow.matches; }};
  fit();
  narrow.addEventListener('change', fit);
}})();
</script>
</body>
</html>
"""


def order() -> list[str]:
    """Pages in the sidebar's order, then any it leaves out."""
    side = (WIKI / "_Sidebar.md").read_text(encoding="utf-8")
    names = []
    for m in re.finditer(r"\(([A-Za-z0-9-]+\.md)\)", side):
        if m.group(1) not in names:
            names.append(m.group(1))
    rest = sorted(p.name for p in WIKI.glob("*.md") if p.name not in SKIP and p.name not in names)
    return names + rest


def main() -> None:
    if OUT.exists():
        shutil.rmtree(OUT)
    OUT.mkdir(parents=True)
    media = OUT.parent / "media"
    media.mkdir(exist_ok=True)
    shutil.copyfile(LOGO, media / "logo-64.png")
    pages = order()
    titles = {}
    bodies = {}
    for name in pages:
        titles[name], bodies[name] = convert((WIKI / name).read_text(encoding="utf-8"))
    for n, name in enumerate(pages):
        built = page_name(name)
        prev_ = pages[n - 1] if n > 0 else None
        next_ = pages[n + 1] if n + 1 < len(pages) else None
        pager = '<nav class="pager">'
        pager += f'<a href="{page_name(prev_)}">Previous<b>{html.escape(titles[prev_])}</b></a>' if prev_ else "<span></span>"
        pager += f'<a href="{page_name(next_)}" style="text-align:right">Next<b>{html.escape(titles[next_])}</b></a>' if next_ else "<span></span>"
        pager += "</nav>"
        first_para = re.search(r"<p>(.*?)</p>", bodies[name], re.S)
        description = re.sub(r"<[^>]+>", "", first_para.group(1)) if first_para else titles[name]
        (OUT / built).write_text(
            PAGE.format(
                title=html.escape(titles[name] if name == "Home.md" else f"{titles[name]} · PIRA wiki"),
                description=html.escape(description[:200], quote=True),
                style=STYLE.strip(),
                sidebar=sidebar(built),
                body=bodies[name],
                pager=pager,
            ),
            encoding="utf-8",
            newline="\n",
        )
    print(f"built {len(pages)} pages into {OUT}")


if __name__ == "__main__":
    main()
