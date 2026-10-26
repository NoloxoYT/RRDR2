#!/usr/bin/env python3
"""Generate a docs.rs-style static site for the RRDR natives.

Source: lib/NativeDecl.cs (ScriptHookRDR2DotNet V2 declaration file).
Output: site/natives/  (open site/index.html in a browser)

Features:
- docs.rs / rustdoc look: top bar with search, left sidebar, fn blocks
- search over native names, descriptions, (rust) names and hashes
- natives grouped into thematic sections (Ped related, Player related, ...)
- one page per native namespace (module)
- one dedicated detail page per native (n/<module>/<rust_name>.html)

Usage:
    python3 tools/generate_natives_site.py [--source lib/NativeDecl.cs] [--out site/natives]
"""

from __future__ import annotations

import argparse
import html
import json
import os
import re
import shutil
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from generate_natives import (  # noqa: E402
    CLASS_RE,
    DECL_RE,
    PARAM_RE,
    PARAM_TYPES,
    RETURN_TYPES,
    choose_names,
    sanitize_param,
)

BUCKETS = {
    "Ped related": [
        "ped", "perschar", "companion", "population", "brain", "ik",
        "attribute", "task", "animscene", "flock", "interaction",
    ],
    "Player related": [
        "player", "stats", "law", "money", "bounty", "posse", "crew",
    ],
    "World related": [
        "entity", "object", "misc", "fire", "water", "zone", "map",
        "pathfind", "interior", "volume", "physics", "graphics",
        "clock", "streaming", "shapetest", "event", "collection",
    ],
    "Vehicles": ["vehicle", "aitransport", "aicoverpoint"],
    "Camera": ["cam", "txd"],
    "Audio": ["audio", "voice"],
    "UI & Input": [
        "hud", "pad", "minigame", "localization", "uiapps", "uidebug",
        "uievents", "uifeed", "uilog", "uipinning", "uitutorial",
        "uistatemachine", "uistickyfeed", "compapp",
    ],
    "AI & Scripts": [
        "missiondata", "scripts", "spactionproxy", "recording", "replay",
    ],
    "Data & Storage": [
        "datafile", "decorator", "databinding", "inventory",
        "itemdatabase", "itemset", "dlc", "save", "persistence",
        "netshopping", "unlock",
    ],
    "Online & Telemetry": [
        "network", "socialclub", "socialclubfeed", "telemetry",
        "google_analytics",
    ],
    "System & Utility": [
        "builtin", "debug", "crashlog", "queue", "_namespace4", "compendium",
    ],
}


# ---------------------------------------------------------------------------
# Parsing with descriptions
# ---------------------------------------------------------------------------

def clean_docs(raw_lines):
    old_name = None
    stripped = [line.strip() for line in raw_lines]
    stripped = [ln for ln in stripped if ln not in ("<summary>", "</summary>")]
    while stripped and stripped[0] == "":
        stripped.pop(0)
    while stripped and stripped[-1] == "":
        stripped.pop()
    for candidate in stripped:
        m = re.match(r"Old name:\s*(\S+)", candidate)
        if m:
            old_name = m.group(1)
            break
    desc = "\n".join(stripped)
    short = stripped[0] if stripped else ""
    if len(short) > 150:
        short = short[:149] + " ..."
    return {"desc": desc, "short": short, "old": old_name}


def parse_with_docs(source):
    declarations = []
    skipped_variadic = []
    current_class = None
    unparsed = []
    pending_docs = []

    with open(source, encoding="utf-8-sig") as handle:
        lines = handle.read().splitlines()

    for line_number, line in enumerate(lines, 1):
        match = CLASS_RE.match(line)
        if match:
            current_class = match.group(1)
            pending_docs = []
            continue

        stripped = line.strip()

        if stripped.startswith("///"):
            pending_docs.append(stripped[3:].lstrip())
            continue

        if "public static" not in line:
            pending_docs = []
            continue

        match = DECL_RE.match(line)
        if not match:
            unparsed.append((line_number, line.strip()))
            continue

        params = []
        variadic = False
        raw_params = match.group("params").strip()

        if raw_params:
            for raw_param in raw_params.split(","):
                raw_param = raw_param.strip()
                if not raw_param:
                    continue
                if raw_param.startswith("params "):
                    variadic = True
                    continue
                param_match = PARAM_RE.match(raw_param)
                if not param_match:
                    unparsed.append((line_number, "PARAM: " + raw_param))
                    continue
                param_type = param_match.group("type").replace(" ", "")
                param_name = param_match.group("name")
                if param_match.group("variadic"):
                    variadic = True
                params.append((param_type, param_name, False))

        if variadic:
            skipped_variadic.append(current_class + "::" + match.group("name"))
            pending_docs = []
            continue

        ret = match.group("ret").replace(" ", "")
        if ret not in RETURN_TYPES:
            unparsed.append((line_number, "RETURN: " + ret))
            pending_docs = []
            continue

        ok = True
        for param_type, _, _ in params:
            if param_type not in PARAM_TYPES:
                ok = False
                unparsed.append((line_number, "PARAM TYPE: " + param_type))
                break
        if not ok:
            pending_docs = []
            continue

        declarations.append({
            "class": current_class,
            "name": match.group("name"),
            "ret": ret,
            "params": params,
            "hash": match.group("hash"),
            "docs": clean_docs(pending_docs),
        })
        pending_docs = []

    if unparsed:
        print("Unparsed declarations:", file=sys.stderr)
        for item in unparsed[:20]:
            print("  ", item, file=sys.stderr)
        raise SystemExit(1)

    if any(decl["class"] is None for decl in declarations):
        raise SystemExit("Found a declaration before the first class")

    return declarations, skipped_variadic


# ---------------------------------------------------------------------------
# Site rendering
# ---------------------------------------------------------------------------

HEADER_SEARCH = """
<div class="live-search">
  <label for="search-input" class="sr-only">Search natives</label>
  <span class="hs-icon">&#8902;</span>
  <input id="search-input" type="text" autocomplete="off" spellcheck="false"
         placeholder="Search natives - name or description" tabindex="-1" />
  <span class="hs-hint" title="Press / to focus">/</span>
  <div class="live-results" id="live-results" hidden></div>
</div>
"""

TOPBAR = """
<header class="topbar">
  <a class="brand" href="{brand_href}">
    <img class="brand-mark" src="{logo_src}" alt="RRDR" width="24" height="24" />
    <span class="brand-text">rrdr<span class="brand-dim">.natives</span></span>
  </a>
{search}
</header>
"""


def root_join(root, path):
    """Join a site-root prefix with a site-root-relative path (pure relative URLs)."""
    return root + "/" + path if root else path


def topbar_html(root=""):
    return TOPBAR.format(search=HEADER_SEARCH,
                         brand_href=esc(root_join(root, "index.html")),
                         logo_src=esc(root_join(root, "static/logo.svg")))


def esc(text):
    return html.escape(text, quote=True)


# Where the generator writes its output, relative to the repository root. The
# GitHub Pages site sits next to the Rust doc, so "natives/" stays stable no
# matter which domain (github.io or crates.io) is rendering the pages.
SITE_DIR = "site"


def sidebar_html(active_module, bucket_of, module_counts, active_fns=None, root=""):
    parts = ['<nav class="sidebar">',
             '<div class="side-crate"><a href="%s">rrdr_natives</a></div>'
             % esc(root_join(root, "index.html"))]
    current_bucket = bucket_of.get(active_module)
    for bucket, names in BUCKETS.items():
        parts.append('<h3 class="side-bucket%s">%s</h3>' % (
            "" if bucket == current_bucket else " dim", esc(bucket)))
        for name in names:
            if name not in module_counts:
                continue
            cls = " active" if name == active_module else ""
            parts.append('<a class="side-mod%s" href="%s.html">%s<span class="count">%d</span></a>' % (
                cls, esc(root_join(root, name)), esc(name), module_counts[name]))
    if active_fns:
        parts.append('<h3 class="side-bucket">Functions</h3>')
        for rust_name, decl in active_fns:
            parts.append('<a class="side-fn" href="%s">%s</a>' % (
                esc(root_join(root, "n/%s/%s.html" % (active_module, rust_name))),
                esc(decl["name"])))
    parts.append("</nav>")
    return "\n".join(parts)


def page_head(title, root=""):
    return (
        "<!DOCTYPE html>\n"
        '<html lang="en"><head>\n'
        '<meta charset="utf-8">\n'
        '<meta name="viewport" content="width=device-width, initial-scale=1">\n'
        "<title>%s</title>\n"
        '<link rel="stylesheet" href="%s">\n'
        '<link rel="stylesheet" href="%s">\n'
        '<link rel="icon" href="%s">\n'
        "</head><body>\n" % (esc(title),
                             esc(root_join(root, "static/normalize.css")),
                             esc(root_join(root, "static/style.css")),
                             esc(root_join(root, "static/favicon.svg")))
    )


INDEX_VERSION = ""


def page_foot(search_page=False, root=""):
    flag = ' data-searchpage="1"' if search_page else ""
    suffix = "?v=" + INDEX_VERSION if INDEX_VERSION else ""
    return (
        '<script src="%s"></script>\n'
        '<script src="%s" data-root="%s"%s></script>\n'
        "</body></html>\n" % (esc(root_join(root, "search-index.js" + suffix)),
                              esc(root_join(root, "static/search.js" + suffix)),
                              esc(root), flag)
    )


DOCTYPE_HEAD = (
    "<!DOCTYPE html>\n"
    '<html lang="en"><head>\n'
    '<meta charset="utf-8">\n'
    '<meta name="viewport" content="width=device-width, initial-scale=1">\n'
)


def render_sig(params, ret):
    pieces = []
    for t, n, _ in params:
        pieces.append('<span class="p-name">%s</span><span class="p-sep">:</span> '
                      '<span class="p-type">%s</span>' % (esc(n), esc(t)))
    inner = ", ".join(pieces)
    ret_span = ""
    if ret != "void":
        ret_span = ' <span class="arrow">&#8594;</span> <span class="ret-type">%s</span>' % esc(ret)
    return '<span class="paren">(</span>%s<span class="paren">)</span>%s' % (inner, ret_span)


def render_module_page(module, bucket, rust_fns, bucket_of, module_counts):
    parts = [page_head("rrdr_natives::%s - RRDR natives" % module)]
    parts.append(topbar_html())
    parts.append(sidebar_html(module, bucket_of, module_counts, rust_fns))
    parts.append('<main class="content">')
    parts.append('<div class="doc-page-head">')
    parts.append('<h1 class="module-title"><span class="ns">rrdr_natives::</span>%s</h1>' % esc(module))
    parts.append('<div class="crumb"><a href="index.html">%s</a> <span class="sep">&#8250;</span> '
                 '<a href="%s.html">%s</a></div>' % (esc(bucket), module, esc(module)))
    count = len(rust_fns)
    unique_hashes = len({d["hash"].lower() for _, d in rust_fns})
    parts.append('<div class="mod-stats">%d function%s &middot; %d unique native hash%s</div>' % (
        count, "" if count == 1 else "s", unique_hashes, "" if unique_hashes == 1 else "es"))
    parts.append("</div>")
    parts.append('<section class="item-list">')
    for rust_name, decl in rust_fns:
        cname = decl["name"]
        desc = decl["docs"]["desc"]
        old = decl["docs"]["old"]
        params = decl["params"]
        ret = decl["ret"]
        hash_hex = decl["hash"]
        anchor = esc(rust_name)
        parts.append('<section class="fn-block" id="%s">' % anchor)
        header = ('<h3 class="code-header"><span class="fn-mod">%s::</span>'
                  '<a class="fn-name" href="%s">%s</a>' % (
                      esc(decl["class"]),
                      esc("n/%s/%s.html" % (module, rust_name)), esc(cname)))
        if rust_name.endswith(("_VEC3", "_VEC3_2", "_VEC3_3")):
            header += ' <span class="vec-badge">Vector3 overload</span>'
        header += " " + render_sig(params, ret)
        parts.append(header + "</h3>")
        if desc:
            parts.append('<div class="docblock">%s</div>' % esc(desc))
        else:
            parts.append('<div class="docblock nodoc">No description available.</div>')
        meta_bits = ["<code>%s</code>" % esc(hash_hex)]
        if old:
            meta_bits.append("old name <code>%s</code>" % esc(old))
        meta_bits.append("%d arg%s" % (len(params), "" if len(params) == 1 else "s"))
        meta_bits.append("rust: <code>rrdr_natives::%s::%s</code>" % (module, esc(rust_name)))
        parts.append('<div class="fn-meta">' + " &middot; ".join(meta_bits) + "</div>")
        parts.append("</section>")
    parts.append("</section>")
    parts.append("</main>")
    parts.append(page_foot())
    return "\n".join(parts)


def render_native_page(module, rust_name, decl, prev_fn, next_fn, bucket, bucket_of, module_counts):
    root = "../.."
    cname = decl["name"]
    desc = decl["docs"]["desc"]
    old = decl["docs"]["old"]
    params = decl["params"]
    ret = decl["ret"]

    parts = [page_head("%s::%s - rrdr_natives" % (decl["class"], cname), root)]
    parts.append(topbar_html(root))
    parts.append(sidebar_html(module, bucket_of, module_counts, root=root))
    parts.append('<main class="content">')
    parts.append('<div class="doc-page-head">')
    parts.append('<div class="crumb">'
                 '<a href="%s">rrdr_natives</a> <span class="sep">&#8250;</span> %s '
                 '<span class="sep">&#8250;</span> '
                 '<a href="%s">%s</a> <span class="sep">&#8250;</span> %s</div>' % (
                     esc(root_join(root, "index.html")), esc(bucket),
                     esc(root_join(root, "%s.html" % module)), esc(module),
                     esc(decl["class"]) + "::" + esc(cname)))
    h1 = ('<h1 class="module-title"><span class="ns">%s::</span>%s</h1>'
          % (esc(decl["class"]), esc(cname)))
    if rust_name.endswith(("_VEC3", "_VEC3_2", "_VEC3_3")):
        h1 += ' <span class="vec-badge">Vector3 overload</span>'
    parts.append(h1)
    parts.append("</div>")
    parts.append('<div class="code-header">' + render_sig(params, ret) + "</div>")
    if desc:
        parts.append('<div class="docblock">%s</div>' % esc(desc))
    else:
        parts.append('<div class="docblock nodoc">No description available.</div>')
    meta_bits = ["<code>%s</code>" % esc(decl["hash"])]
    if old:
        meta_bits.append("old name <code>%s</code>" % esc(old))
    meta_bits.append("%d arg%s" % (len(params), "" if len(params) == 1 else "s"))
    meta_bits.append("rust: <code>rrdr_natives::%s::%s</code>" % (module, esc(rust_name)))
    meta_bits.append('module page: <a href="%s">%s.html#%s</a>' % (
        esc(root_join(root, "%s.html#%s" % (module, rust_name))), esc(module), esc(rust_name)))
    parts.append('<div class="fn-meta">' + " &middot; ".join(meta_bits) + "</div>")

    # How to call this native from rrdr-natives
    rust_ret = RETURN_TYPES[ret][0]
    args = ", ".join(["api"] + [sanitize_param(n) for _, n, _ in params])
    call_line = "rrdr_natives::%s::%s(%s);" % (module, rust_name, args)
    if ret != "void":
        call_line += "  // -&gt; %s" % esc(rust_ret)
    usage_lines = [call_line, "", "pub const HASH_%s: u64 = %s;" % (rust_name, esc(decl["hash"]))]
    parts.append('<div class="usage-block">')
    parts.append('<div class="ub-label">From Rust</div>')
    parts.append('<pre class="rust-usage">%s</pre>' % "\n".join(usage_lines))
    parts.append("</div>")

    prev_html = ('<a class="fn-nav-link" href="%s">&#8249; %s</a>' % (
        esc(root_join(root, "n/%s/%s.html" % (module, prev_fn[0]))), esc(prev_fn[0]))
        if prev_fn else "")
    next_html = ('<a class="fn-nav-link" href="%s">%s &#8250;</a>' % (
        esc(root_join(root, "n/%s/%s.html" % (module, next_fn[0]))), esc(next_fn[0]))
        if next_fn else "")
    parts.append('<div class="fn-nav">'
                 '<span class="fn-nav-side">%s</span><span class="fn-nav-side">%s</span></div>'
                 % (prev_html, next_html))
    parts.append("</main>")
    parts.append(page_foot(root=root))
    return "\n".join(parts)


def render_index(module_counts, buckets_present, bucket_of, total, unique, skipped):
    parts = [page_head("rrdr_natives - RRDR natives")]
    parts.append(topbar_html())
    parts.append(sidebar_html("", bucket_of, module_counts))
    parts.append('<main class="content">')
    parts.append('<div class="doc-page-head">')
    parts.append('<h1 class="module-title"><span class="ns">rrdr_</span>natives</h1>')
    parts.append('<div class="self-crumb">Rust bindings for the Red Dead Redemption 2 native API</div>')
    parts.append('<p class="intro">Typed bindings for the RDR2 natives RRDR wraps, generated from '
                 '<code>lib/NativeDecl.cs</code> (ScriptHookRDR2DotNet V2). %d functions across '
                 '%d native namespaces, %d unique hashes. Every call goes through <code>GameApi</code>, '
                 'so it works in game and in the offline simulator.</p>' % (total, len(module_counts), unique))
    parts.append('<p class="usage">Search by <em>name</em> or by <em>description</em> - press <kbd>/</kbd> '
                 'and start typing, e.g. <code>nearest ped</code>, <code>set entity health</code>, '
                 '<code>bleeding</code>. Hashes work too (<code>0x82368787EA73C0F7</code>).</p>')
    if skipped:
        skipped_html = ", ".join(esc(s) for s in skipped)
        parts.append('<p class="usage"><span class="vec-badge">skipped</span> %d variadic natives '
                     'not representable in the bridge ABI: %s.</p>' % (len(skipped), skipped_html))
    parts.append("</div>")
    for bucket, modules in buckets_present.items():
        parts.append('<section class="bucket">')
        parts.append('<h2>%s <span class="bucket-count">%d</span></h2>' % (esc(bucket), len(modules)))
        parts.append('<ul class="bucket-modules">')
        for m in modules:
            parts.append('<li><a class="mod-link" href="%s.html">%s</a>'
                         '<span class="m-count">%d</span></li>' % (m, esc(m), module_counts[m]))
        parts.append("</ul></section>")
    parts.append("</main>")
    parts.append(page_foot())
    return "\n".join(parts)


def render_search_page(module_counts, bucket_of):
    parts = [page_head("Search - rrdr_natives")]
    parts.append(topbar_html())
    parts.append(sidebar_html("", bucket_of, module_counts))
    parts.append('<main class="content">')
    parts.append('<div class="doc-page-head"><h1 class="module-title">Search</h1></div>')
    parts.append('<div id="search-summary" class="mod-stats"></div>')
    parts.append('<div id="search-output" class="results"></div>')
    parts.append("</main>")
    parts.append(page_foot(search_page=True))
    return "\n".join(parts)


# ---------------------------------------------------------------------------
# Search index + client JS
# ---------------------------------------------------------------------------

def build_index(all_fns, bucket_of):
    entries = []
    for rust_name, module, decl in all_fns:
        docs = decl["docs"]
        entries.append({
            "n": decl["class"] + "::" + decl["name"],
            "r": rust_name,
            "m": decl["class"],
            "mo": module,
            "rt": decl["ret"],
            "ps": [{"t": t, "n": n} for t, n, _ in decl["params"]],
            "h": decl["hash"],
            "o": docs["old"] or "",
            "d": docs["desc"],
            "s": docs["short"],
        })
    blob = json.dumps(entries, separators=(",", ":"), ensure_ascii=False)
    return "window.NATIVE_INDEX=" + blob + ";\n"


SEARCH_JS = r"""(function () {
  "use strict";
  var SCRIPT = document.currentScript || (function () {
    var s = document.getElementsByTagName("script");
    return s[s.length - 1];
  })();
  var isSearchPage = SCRIPT && SCRIPT.getAttribute("data-searchpage") === "1";
  var ROOT = (SCRIPT.getAttribute("data-root") || "").replace(/\/+$/, "");
  var rootJoin = function (path) { return ROOT ? ROOT + "/" + path : path; };

  var input = document.getElementById("search-input");
  var box = document.getElementById("live-results");
  var out = isSearchPage ? document.getElementById("search-output") : null;
  var summary = isSearchPage ? document.getElementById("search-summary") : null;

  var INDEX = [];
  var ready = false;
  var failed = false;
  var pending = [];

  var norm = function (s) { return (s || "").toLowerCase(); };

  function useLoadedIndex() {
    var raw = window.NATIVE_INDEX || [];
    if (!raw.length) return false;
    raw.forEach(function (e) {
      e.nl = norm(e.n); e.rl = norm(e.r); e.ml = norm(e.m);
      e.dl = norm(e.d); e.hl = e.h.toLowerCase();
      e.pl = norm((e.ps || []).map(function (p) { return p.n + " " + p.t; }).join(" ") + " " + norm(e.o));
    });
    INDEX = raw;
    ready = true;
    return true;
  }

  var ERROR_HTML = '<div class="no-results big r-error">Search index could not be loaded ' +
    "(search-index.js). Rebuild the site or reload the page.</div>";

  function fail() {
    failed = true;
    if (box) { box.innerHTML = ERROR_HTML; box.hidden = false; }
    if (out) out.innerHTML = ERROR_HTML;
    if (summary) summary.textContent = "";
    while (pending.length) pending.shift()();
  }
  function begin(cb) {
    if (useLoadedIndex()) { cb(); return; }
    if (ready) { cb(); return; }
    pending.push(cb);
    if (pending.length > 1) return;
    var s = document.createElement("script");
    s.src = rootJoin("search-index.js");
    s.onload = function () {
      if (!useLoadedIndex()) { fail(); return; }
      while (pending.length) pending.shift()();
    };
    s.onerror = fail;
    document.head.appendChild(s);
    setTimeout(function () { if (!ready && !failed) fail(); }, 4000);
  }

  function tokenize(q) {
    return norm(q).split(/[\s::]+/).filter(function (t) { return t.length > 0; });
  }

  function scoreEntry(e, ql, tokens) {
    var score = 0;
    var hashLike = /^0?x?[0-9a-f]{8,16}$/i.test(ql);
    if (hashLike) {
      return e.hl.indexOf(ql.replace(/^0x/, "")) >= 0 ? 10000 : 0;
    }
    if (e.nl === ql || e.rl === ql) return 5000;

    var tokenScore = 0;
    var allMatched = tokens.length > 0;
    for (var i = 0; i < tokens.length; i++) {
      var t = tokens[i];
      var s = 0;
      if (e.nl.indexOf(t) >= 0) s += 10;
      else if (e.rl.indexOf(t) >= 0) s += 9;
      if (e.dl.indexOf(t) >= 0) s += 5;
      if (e.ml === t) s += 7; else if (e.ml.indexOf(t) >= 0) s += 3;
      if (e.pl.indexOf(t) >= 0) s += 2;
      if (s === 0) allMatched = false;
      tokenScore += s;
    }
    if (!allMatched) return 0;

    // bonus for contiguous match in the scripting name
    var afterColon = e.nl.slice(e.nl.indexOf("::") + 2);
    if (afterColon.indexOf(ql) === 0) score += 40;
    else if (afterColon.indexOf(ql) > 0) score += 20;
    else if (e.nl.indexOf(ql) >= 0 || e.rl.indexOf(ql) >= 0) score += 15;
    // all tokens found in the name is much better than spread over fields
    var inName = true;
    for (var j = 0; j < tokens.length; j++) {
      if (e.nl.indexOf(tokens[j]) < 0 && e.rl.indexOf(tokens[j]) < 0) inName = false;
    }
    if (inName) score += 30;
    score += tokenScore;
    score -= Math.min(e.n.length, 40) / 10;
    return score;
  }

  function search(q, limit) {
    var ql = norm(q).replace(/^\s+|\s+$/g, "");
    var tokens = tokenize(q);
    if (!ql) return [];
    var results = [];
    for (var i = 0; i < INDEX.length; i++) {
      var s = scoreEntry(INDEX[i], ql, tokens);
      if (s > 0) results.push([s, INDEX[i]]);
    }
    results.sort(function (a, b) { return (b[0] - a[0]) || a[1].n.localeCompare(b[1].n); });
    limit = limit || 150;
    return results.slice(0, limit).map(function (r) { return r[1]; });
  }

  function escapeHtml(s) {
    return String(s).replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
  }
  function mark(escapedText, toks) {
    var outText = escapedText;
    for (var i = 0; i < toks.length; i++) {
      var t = toks[i].replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
      outText = outText.replace(new RegExp("(" + t + ")", "ig"), "<mark>$1</mark>");
    }
    return outText;
  }
  function snippet(desc, toks) {
    var lower = desc.toLowerCase();
    var idx = -1;
    for (var i = 0; i < toks.length; i++) {
      var p = lower.indexOf(toks[i]);
      if (p >= 0 && (idx < 0 || p < idx)) idx = p;
    }
    if (idx < 0) idx = 0;
    var start = Math.max(0, idx - 60);
    var raw = desc.substr(start, 220);
    var pre = start > 0 ? "\u2026" : "";
    var post = start + 220 < desc.length ? "\u2026" : "";
    return pre + mark(escapeHtml(raw), toks) + post;
  }

  function resultRow(e, q) {
    var toks = tokenize(q);
    var params = (e.ps || []).map(function (p) {
      return '<span class="p-name">' + escapeHtml(p.n) + "</span>:" + escapeHtml(p.t);
    }).join(", ");
    var name = e.n;
    var sep = name.indexOf("::");
    var mod = sep >= 0 ? name.slice(0, sep) : e.m;
    var fn = sep >= 0 ? name.slice(sep + 2) : name;
    var href;
    if (e.mo) {
      href = rootJoin("n/" + e.mo + "/" + encodeURIComponent(e.r) + ".html");
    } else {
      // fall back to the old module-page anchor (older cached search index)
      href = rootJoin(mod.toLowerCase() + ".html#" + encodeURIComponent(e.r));
    }
    var htmlOut = '<a class="result" href="' + href + '">' +
      '<div class="r-head"><span class="r-mod">' + mod + '::</span>' +
      '<span class="r-name">' + fn + "</span>" +
      (e.r !== fn ? ' <span class="r-sig">' + escapeHtml(e.r) + "</span>" : "") +
      ' <span class="r-sig">(' + params + " &#8594; " + escapeHtml(e.rt) + ")</span></div>";
    if (e.d) htmlOut += '<div class="r-desc">' + snippet(e.d, toks) + "</div>";
    if (e.o) htmlOut += '<div class="r-meta">old name: ' + escapeHtml(e.o) + "</div>";
    htmlOut += "</a>";
    return htmlOut;
  }

  // live dropdown
  var timer = null;
  input.addEventListener("input", function () {
    clearTimeout(timer);
    timer = setTimeout(function () {
      var q = input.value;
      if (!q.replace(/\s/g, "")) { box.hidden = true; box.innerHTML = ""; return; }
      if (!ready && !failed) {
        box.innerHTML = '<div class="no-results">Loading search index&#8230;</div>';
      }
      begin(function () {
        if (!ready) return;  // the error message is already on screen
        var results = search(q, 12);
        box.innerHTML = results.length
          ? results.map(function (e) { return resultRow(e, q); }).join("")
          : '<div class="no-results">No natives match.</div>';
      });
      box.hidden = false;
    }, 90);
  });
  input.addEventListener("keydown", function (ev) {
    if (ev.key === "Enter") {
      window.location.href = rootJoin("search.html?q=" + encodeURIComponent(input.value));
    }
  });
  document.addEventListener("click", function (ev) {
    if (!box.hidden && !box.contains(ev.target) && ev.target !== input) box.hidden = true;
  });
  input.addEventListener("focus", function () { if (input.value) box.hidden = false; });
  document.addEventListener("keydown", function (ev) {
    if (ev.key === "/" && document.activeElement !== input) {
      ev.preventDefault(); input.focus(); box.hidden = true;
    }
    if (ev.key === "Escape") { box.hidden = true; input.blur(); }
  });

  // dedicated search page
  if (isSearchPage) {
    var params = new URLSearchParams(window.location.search);
    var q = params.get("q") || "";
    if (q) {
      input.value = q;
      summary.textContent = ready ? "" : "Loading search index\u2026";
      begin(function () {
        if (!ready) return;  // the error message is already on screen
        var results = search(q, 100);
        summary.textContent = results.length + ' results for "' + q + '"';
        out.innerHTML = results.length
          ? results.map(function (e) { return resultRow(e, q); }).join("")
          : '<div class="no-results big">Nothing found for "' + escapeHtml(q) + '".</div>';
        document.title = q + " - Search - rrdr_natives";
      });
    }
  }
})();
"""

LOGO_SVG = (
    '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 32 32">\n'
    '  <g fill="#c2341c">\n'
    "    <!-- gear teeth, every 30 degrees -->\n"
    '    <rect x="14.3" y="0.4" width="3.4" height="3.9" rx="0.7"/>\n'
    '    <rect x="14.3" y="0.4" width="3.4" height="3.9" rx="0.7" transform="rotate(30 16 16)"/>\n'
    '    <rect x="14.3" y="0.4" width="3.4" height="3.9" rx="0.7" transform="rotate(60 16 16)"/>\n'
    '    <rect x="14.3" y="0.4" width="3.4" height="3.9" rx="0.7" transform="rotate(90 16 16)"/>\n'
    '    <rect x="14.3" y="0.4" width="3.4" height="3.9" rx="0.7" transform="rotate(120 16 16)"/>\n'
    '    <rect x="14.3" y="0.4" width="3.4" height="3.9" rx="0.7" transform="rotate(150 16 16)"/>\n'
    '    <rect x="14.3" y="0.4" width="3.4" height="3.9" rx="0.7" transform="rotate(180 16 16)"/>\n'
    '    <rect x="14.3" y="0.4" width="3.4" height="3.9" rx="0.7" transform="rotate(210 16 16)"/>\n'
    '    <rect x="14.3" y="0.4" width="3.4" height="3.9" rx="0.7" transform="rotate(240 16 16)"/>\n'
    '    <rect x="14.3" y="0.4" width="3.4" height="3.9" rx="0.7" transform="rotate(270 16 16)"/>\n'
    '    <rect x="14.3" y="0.4" width="3.4" height="3.9" rx="0.7" transform="rotate(300 16 16)"/>\n'
    '    <rect x="14.3" y="0.4" width="3.4" height="3.9" rx="0.7" transform="rotate(330 16 16)"/>\n'
    '    <circle cx="16" cy="16" r="12"/>\n'
    "  </g>\n"
    '  <text x="16" y="17" text-anchor="middle" dominant-baseline="middle"\n'
    '        font-family="Georgia, serif" font-weight="bold" font-size="15"\n'
    '        fill="#ffffff">R</text>\n'
    "</svg>\n"
)

STYLE_CSS = r"""
/* docs.rs-inspired styling for the RRDR natives reference. */
:root {
  --topbar-h: 3.4em;
  --sidebar-w: 300px;
  --accent: #c2341c;
  --link: #0a3069;
  --link-hover: #2f4570;
  --text: #111;
  --muted: #6b6b6b;
  --code-bg: #f0f1f2;
  --side-bg: #fafafb;
  --mark-bg: #ffe9a8;
  --border: #e3e4e6;
}
* { box-sizing: border-box; }
body {
  margin: 0;
  font-family: "Source Serif 4", Georgia, "Times New Roman", serif;
  color: var(--text);
  background: #fff;
  font-size: 16px;
  line-height: 1.6;
}
code, .code-header, input#search-input, .r-name, .r-mod, .module-title, .p-type, .ret-type, .r-sig {
  font-family: "Fira Mono", Consolas, monospace;
}
a { color: var(--link); text-decoration: none; }
a:hover { color: var(--link-hover); }
.sr-only { position: absolute; width: 1px; height: 1px; overflow: hidden; clip: rect(0 0 0 0); }

/* top bar */
.topbar {
  position: fixed;
  top: 0; left: 0; right: 0;
  height: var(--topbar-h);
  background: #101010;
  color: #fff;
  display: flex;
  align-items: center;
  gap: 1.2em;
  padding: 0 1em;
  z-index: 30;
}
.brand { display: flex; align-items: center; gap: .55em; color: #fff; }
img.brand-mark { display: block; width: 1.55em; height: 1.55em; }
.brand-text { font-size: 1.05rem; letter-spacing: .02em; }
.brand-dim { color: #9c9c9c; }
.live-search { position: relative; flex: 1; max-width: 560px; margin-left: auto; }
input#search-input {
  width: 100%;
  background: #1e1e1e;
  border: 1px solid #3d3d3d;
  border-radius: .35em;
  color: #e7e7e7;
  font-size: .9rem;
  padding: .42em 2.4em .42em 2.2em;
  outline: none;
}
input#search-input:focus { border-color: #777; }
.hs-icon {
  position: absolute; left: .6em; top: 50%; transform: translateY(-52%);
  color: #9a9a9a; font-size: 1rem; pointer-events: none;
}
.hs-hint {
  position: absolute; right: .6em; top: 50%; transform: translateY(-52%);
  border: 1px solid #4a4a4a; border-radius: .25em;
  color: #9a9a9a; font-size: .7rem; padding: 0 .4em; pointer-events: none;
}
.live-results {
  position: absolute; top: calc(100% + .35em); left: 0; right: 0;
  background: #fff; color: var(--text);
  border: 1px solid var(--border); border-radius: .4em;
  box-shadow: 0 8px 22px rgba(0,0,0,.14);
  max-height: 65vh; overflow: auto;
}
.result { display: block; padding: .55em .8em; border-bottom: 1px solid var(--border); color: var(--text); }
.result:last-child { border-bottom: 0; }
.result:hover { background: #f6f6f8; }
.r-head { font-size: .92rem; }
.r-mod { color: var(--muted); }
.r-name { color: var(--link); font-weight: 600; }
.result:hover .r-name { text-decoration: underline; }
.r-sig { color: var(--muted); font-size: .8rem; }
.r-desc { font-size: .87rem; color: #444; margin-top: .15em; }
.r-meta { font-size: .8rem; color: var(--muted); }
.no-results { padding: .8em; color: var(--muted); }
.r-error { color: #b3261e; }
.no-results.big { font-size: 1rem; }
mark { background: var(--mark-bg); }

/* layout */
.sidebar {
  position: fixed;
  top: var(--topbar-h);
  bottom: 0; left: 0;
  width: var(--sidebar-w);
  background: var(--side-bg);
  border-right: 1px solid var(--border);
  overflow-y: auto;
  padding: 1.1em .85em 3em 1em;
  z-index: 20;
}
.side-crate { font-size: 1.05rem; margin-bottom: 1em; }
.side-bucket {
  font-family: "Fira Sans", Verdana, sans-serif;
  font-size: .7rem; font-weight: 600; letter-spacing: .08em;
  text-transform: uppercase;
  color: #545454; margin: 1em .25em .35em;
  border-bottom: 1px solid var(--border);
  padding-bottom: .25em;
}
.side-bucket.dim { color: #9c9da1; }
.side-mod {
  display: flex; justify-content: space-between; align-items: baseline;
  font-family: "Fira Sans", Verdana, sans-serif;
  font-size: .84rem;
  padding: .17em .35em;
  border-radius: .25em;
  color: var(--link);
}
.side-mod:hover { background: #eef0f2; color: var(--link-hover); }
.side-mod.active { background: #e2e6ea; font-weight: 600; }
.side-mod .count { font-size: .68rem; color: var(--muted); font-weight: 400; }
.side-fn {
  display: block;
  font-family: "Fira Mono", monospace;
  font-size: .74rem;
  padding: .12em .35em .12em .8em;
  color: var(--link);
  overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
}
.side-fn:hover { background: #eef0f2; }

.content {
  margin-left: var(--sidebar-w);
  margin-top: var(--topbar-h);
  padding: 1.6em 2.6em 5em 2.6em;
  max-width: 1200px;
}

/* page head */
.doc-page-head { margin-bottom: 1.6em; }
.module-title { font-size: 1.7rem; margin: 0 0 .35em; font-weight: 600; }
.module-title .ns { color: var(--muted); font-weight: 400; }
.crumb, .self-crumb {
  font-family: "Fira Sans", Verdana, sans-serif;
  font-size: .85rem; color: var(--muted); margin-bottom: .6em;
}
.crumb .sep { margin: 0 .35em; }
.mod-stats {
  font-family: "Fira Sans", Verdana, sans-serif;
  font-size: .83rem; color: var(--muted); margin-bottom: .6em;
}
.intro { max-width: 48em; margin: .6em 0; }
.intro code, .usage code, .docblock code {
  background: var(--code-bg); border-radius: .25em; padding: .1em .35em; font-size: .9em;
}
.usage { max-width: 52em; }
kbd {
  background: var(--code-bg); border: 1px solid #cfd1d4; border-bottom-width: 2px;
  border-radius: .25em; padding: 0 .4em; font-family: "Fira Mono", monospace; font-size: .8em;
}
.vec-badge {
  background: #dfeefc; color: #1a5f9e;
  font-family: "Fira Sans", sans-serif;
  font-size: .68rem; font-weight: 600;
  border-radius: .8em; padding: .12em .55em; vertical-align: middle;
}

/* index buckets */
.bucket h2 {
  font-family: "Fira Sans", Verdana, sans-serif;
  font-size: 1.02rem; font-weight: 600;
  border-bottom: 1px solid var(--border);
  padding-bottom: .25em; margin: 1.6em 0 .8em;
}
.bucket-count { color: var(--muted); font-weight: 400; font-size: .8rem; margin-left: .4em; }
.bucket-modules {
  list-style: none; margin: 0; padding: 0;
  display: grid; grid-template-columns: repeat(auto-fill, minmax(180px, 1fr));
  gap: .25em 1.4em;
}
.mod-link { font-family: "Fira Sans", Verdana, sans-serif; font-size: .9rem; }
.m-count { font-size: .68rem; color: var(--muted); margin-left: .5em; }

/* fn blocks */
.item-list { margin-top: 1.6em; }
.fn-block {
  margin: 0 0 1.9em;
  padding: .8em 1em;
  border: 1px solid var(--border);
  border-left: 3px solid #d7d9dc;
  border-radius: .4em;
}
.fn-block:target { border-color: var(--accent); border-left-color: var(--accent); background: #fffdf9; }
.code-header {
  font-size: .95rem; font-weight: 500;
  margin: 0 0 .5em;
  overflow-wrap: anywhere;
}
.fn-mod { color: var(--muted); }
.fn-name { color: var(--link); font-weight: 600; }
.fn-name:hover { text-decoration: underline; }
.p-name { color: #5b2d84; }
.p-sep { color: var(--muted); }
.p-type { color: #3d6a00; font-size: .92em; }
.ret-type { color: #3d6a00; font-size: .92em; }
.arrow { color: var(--muted); }
.paren { color: var(--muted); }
.docblock { max-width: 60em; font-size: .95rem; white-space: pre-line; }
.docblock.nodoc { color: var(--muted); font-style: italic; }
.fn-meta {
  margin-top: .55em;
  font-family: "Fira Sans", Verdana, sans-serif;
  font-size: .74rem; color: var(--muted);
}
.fn-meta code { background: var(--code-bg); border-radius: .25em; padding: .06em .3em; font-size: .95em; }

/* search page */
.results { margin-top: 1em; }
.results .result { padding: .8em .2em; border-bottom: 1px solid var(--border); }
.results .result:hover { background: transparent; }
.results .r-head { font-size: 1rem; }

/* native detail page */
.usage-block { margin: 1em 0 1.4em; max-width: 60em; }
.ub-label {
  font-family: "Fira Sans", Verdana, sans-serif;
  font-size: .74rem; font-weight: 600; letter-spacing: .06em;
  text-transform: uppercase; color: var(--muted); margin-bottom: .25em;
}
.rust-usage {
  margin: 0;
  background: var(--code-bg);
  border: 1px solid var(--border);
  border-left: 3px solid #d7d9dc;
  border-radius: .4em;
  padding: .8em 1em;
  font-family: "Fira Mono", Consolas, monospace;
  font-size: .85rem;
  line-height: 1.55;
  overflow-x: auto;
}
.fn-nav {
  display: flex;
  justify-content: space-between;
  gap: 1em;
  margin-top: 2.2em;
  border-top: 1px solid var(--border);
  padding-top: .8em;
  font-family: "Fira Sans", Verdana, sans-serif;
  font-size: .85rem;
}
.fn-nav-side:empty { display: none; }

@media (max-width: 900px) {
  .sidebar { display: none; }
  .content { margin-left: 0; padding: 1.4em 1em 4em; }
}
"""

FONT_FILES = {
    "FiraSans-Regular-0fe48ade.woff2": "FiraSans-Regular.woff2",
    "FiraSans-Medium-e1aa3f0a.woff2": "FiraSans-Medium.woff2",
    "FiraMono-Regular-87c26294.woff2": "FiraMono-Regular.woff2",
    "SourceSerif4-Regular-6b053e98.ttf.woff2": "SourceSerif4-Regular.woff2",
    "SourceSerif4-Bold-6d4fd4c0.ttf.woff2": "SourceSerif4-Bold.woff2",
    "Fira-LICENSE-05ab6dbd.txt": "Fira-LICENSE.txt",
    "SourceSerif4-LICENSE-a2cfd9d5.md": "SourceSerif4-LICENSE.md",
    "normalize-9960930a.css": "normalize.css",
}


def copy_static(static_dir, repo_root):
    src_root = os.path.join(repo_root, "rust", "target", "doc", "static.files")
    os.makedirs(static_dir, exist_ok=True)
    for source_name, dest_name in FONT_FILES.items():
        src = os.path.join(src_root, source_name)
        if os.path.exists(src):
            shutil.copyfile(src, os.path.join(static_dir, dest_name))
    with open(os.path.join(static_dir, "style.css"), "w", encoding="utf-8") as handle:
        handle.write(STYLE_CSS)
    with open(os.path.join(static_dir, "search.js"), "w", encoding="utf-8") as handle:
        handle.write(SEARCH_JS)
    with open(os.path.join(static_dir, "logo.svg"), "w", encoding="utf-8") as handle:
        handle.write(LOGO_SVG)
    with open(os.path.join(static_dir, "favicon.svg"), "w", encoding="utf-8") as handle:
        handle.write(LOGO_SVG)


def collect(source):
    """Parses the declarations and groups them per module."""
    declarations, skipped_variadic = parse_with_docs(source)

    grouped = {}
    for decl in declarations:
        grouped.setdefault((decl["class"], decl["name"]), []).append(decl)

    modules = {}
    for (class_name, _name), group in grouped.items():
        module = class_name.lower()
        for rust_name, decl in choose_names(group):
            modules.setdefault(module, []).append((rust_name, decl))
    for fns in modules.values():
        fns.sort(key=lambda item: item[0])

    return modules, skipped_variadic


def main():
    parser = argparse.ArgumentParser(description="docs.rs-style natives site generator")
    parser.add_argument("--source", default="lib/NativeDecl.cs")
    parser.add_argument("--out", default=SITE_DIR + "/natives")
    args = parser.parse_args()

    modules, skipped_variadic = collect(args.source)

    module_counts = {m: len(fns) for m, fns in modules.items()}
    bucket_of = {}
    for bucket, names in BUCKETS.items():
        for name in names:
            bucket_of[name] = bucket
    for m in module_counts:
        bucket_of.setdefault(m, "System & Utility")
    buckets_present = {
        bucket: [m for m in sorted(names) if m in module_counts]
        for bucket, names in BUCKETS.items()
    }

    all_fns = []
    for module_name in sorted(modules):
        for rust_name, decl in modules[module_name]:
            all_fns.append((rust_name, module_name, decl))

    unique_hashes = len({decl["hash"].lower() for _, _, decl in all_fns})
    total = len(all_fns)

    global INDEX_VERSION
    INDEX_VERSION = str(total)
    out = args.out
    static_dir = os.path.join(out, "static")
    os.makedirs(out, exist_ok=True)
    copy_static(static_dir, os.getcwd())

    index_js = build_index(all_fns, bucket_of)
    with open(os.path.join(out, "search-index.js"), "w", encoding="utf-8") as handle:
        handle.write(index_js)

    with open(os.path.join(out, "index.html"), "w", encoding="utf-8") as handle:
        handle.write(render_index(module_counts, buckets_present, bucket_of,
                                  total, unique_hashes, skipped_variadic))

    for module_name in sorted(modules):
        with open(os.path.join(out, module_name + ".html"), "w", encoding="utf-8") as handle:
            handle.write(render_module_page(module_name, bucket_of.get(module_name, "System & Utility"),
                                            modules[module_name], bucket_of, module_counts))

    native_dir = os.path.join(out, "n")
    for module_name in sorted(modules):
        os.makedirs(os.path.join(native_dir, module_name), exist_ok=True)
        fns = modules[module_name]
        for i, (rust_name, decl) in enumerate(fns):
            prev_fn = fns[i - 1] if i > 0 else None
            next_fn = fns[i + 1] if i + 1 < len(fns) else None
            path = os.path.join(native_dir, module_name, rust_name + ".html")
            with open(path, "w", encoding="utf-8") as handle:
                handle.write(render_native_page(module_name, rust_name, decl, prev_fn, next_fn,
                                                bucket_of.get(module_name, "System & Utility"),
                                                bucket_of, module_counts))

    with open(os.path.join(out, "search.html"), "w", encoding="utf-8") as handle:
        handle.write(render_search_page(module_counts, bucket_of))

    with_docs = sum(1 for _, _, d in all_fns if d["docs"]["desc"])
    print("Wrote %d natives across %d module pages and %d detail pages -> %s/" % (
        total, len(module_counts), total, out))
    print("  unique hashes: %d" % unique_hashes)
    print("  with description: %d (%d%%)" % (with_docs, with_docs * 100 // max(total, 1)))
    print("  skipped variadic: %d" % len(skipped_variadic))


if __name__ == "__main__":
    main()
