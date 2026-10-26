(function () {
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
