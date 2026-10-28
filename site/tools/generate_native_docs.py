#!/usr/bin/env python3
"""Build the native documentation the generators use, and resolve parameter names.

Two inputs:

- `tools/rdr3_natives.json`, a vendored copy of the RDR3 native database
  (alloc8or/rdr3-nativedb-data, zlib license), refreshed by
  `tools/fetch_native_docs.py`.
- `lib/NativeDecl.cs`, the ScriptHookRDR2DotNet V2 declaration file. Its
  `<summary>` blocks are the only descriptions that ship in the repository.

For each native the merged docblock is:

1. the comment from `lib/NativeDecl.cs` when it carries real text;
2. otherwise the comment from the RDR3 database;
3. otherwise a sentence built from the native name, its return type and its
   parameters, so every native ends up with a description.

"Carries real text" means: not empty, not only `Old name: ...` lines and not
only a bare URL. Those fall back to the database, which usually has the same
native documented with a sentence. A database sentence also goes in front of a
declaration comment that only holds a placeholder ("_CLEAR*", "Params: p1 = 23").

Parameter names are resolved in this order:

1. the name in `lib/NativeDecl.cs`;
2. when that name is a bare placeholder (`p0`, `p1`, ...), the name in the RDR3
   database if it says something (`ped`, `durationMs`, ...);
3. otherwise a name derived from the declared type (`float` -> `value`,
   `ulong` -> `hash`, `bool` -> `toggle`, `Vector3*` -> `outVec`, ...), made
   unique in the signature.

The result is written to `tools/native_docs.json` (`desc`, `old`, `source`,
`extra`, `lib_params`, `params`), so `generate_natives.py` and
`generate_natives_site.py` agree without duplicating the merge logic.

Usage:
    python3 tools/generate_native_docs.py            # rewrite tools/native_docs.json
    python3 tools/generate_native_docs.py --check    # exit 1 when out of date
"""

from __future__ import annotations

import argparse
import json
import os
import re
import sys
from collections import defaultdict

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from generate_natives import parse  # noqa: E402

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
SOURCE_CS = os.path.join(ROOT, "lib", "NativeDecl.cs")
DOCS_JSON = os.path.join(HERE, "native_docs.json")
VENDOR_JSON = os.path.join(HERE, "rdr3_natives.json")

GENERIC_PARAM_RE = re.compile(r"^_?p\d+$")
PLACEHOLDER_RE = re.compile(
    r"^(unknown|unk|unused|none|nothing|dummy|value|todo|_+|\.\.\.)$",
    re.IGNORECASE,
)
# "p1 = ...", "x: see ...", "ped (out) - the ped" style lines of a comment.
PARAM_LINE_RE = re.compile(
    r"^\s*`?(?P<name>[A-Za-z_][A-Za-z0-9_]*)`?\s*(?:\([^)]*\))?\s*(?:[:=]|->)\s*(?P<text>.+)$"
)

# Name derived from the declared type when the database has nothing better.
TYPE_FALLBACK = {
    "int": "handle",
    "uint": "hash",
    "float": "value",
    "bool": "toggle",
    "ulong": "hash",
    "string": "name",
    "Vector3": "vec",
    "IntPtr": "address",
    "int*": "outInt",
    "uint*": "outHash",
    "float*": "outFloat",
    "ulong*": "outData",
    "bool*": "outBool",
    "Vector3*": "outVec",
}

# What to say about a parameter when the database has no line for it.
TYPE_NOTE = {
    "int": "",  # the name already says what it is (ped, vehicle, handle...)
    "uint": "Hash.",
    "float": "",
    "bool": "",
    "ulong": "Hash or opaque 64-bit value.",
    "string": "String; the game hashes it in most natives.",
    "Vector3": "World position or direction (three floats).",
    "IntPtr": "Raw memory address.",
    "int*": "Output: ints written by the native.",
    "uint*": "Output: u32 values written by the native.",
    "float*": "Output: floats written by the native.",
    "ulong*": "Output: script struct or array written by the native; size undocumented.",
    "bool*": "Output: a BOOL written by the native (4 bytes).",
    "Vector3*": "Output: a Vector3, or a matrix, written by the native.",
}


def load_vendor(path=VENDOR_JSON):
    """The vendored RDR3 database: {hash: {ns, comment, params, return_type}}."""
    with open(path, encoding="utf-8") as handle:
        return json.load(handle)


# ---------------------------------------------------------------------------
# Descriptions
# ---------------------------------------------------------------------------

def split_comment(text):
    """Comment -> (description lines, optional old name)."""
    old = None
    lines = []
    for raw in text.splitlines():
        line = raw.strip()
        match = re.match(r"(?i)^old names?\s*:\s*(\S.*)$", line)
        if match:
            old = old or match.group(1).strip()
            continue
        lines.append(line)
    while lines and not lines[0]:
        lines.pop(0)
    while lines and not lines[-1]:
        lines.pop()
    return lines, old


def is_real_comment(lines):
    """True when the lines say more than `Old name: ...` or a lone URL."""
    for line in lines:
        text = line.strip()
        if not text:
            continue
        if re.match(r"(?i)^old names?\s*:", text):
            continue
        if re.match(r"^(https?://|www\.)", text):
            continue
        return True
    return False


NAME_GLOB_RE = re.compile(r"^_?[A-Z_*]+\*?\s*-\s*_?[A-Z_*]+\*?$")


def describes_native(lines):
    """True when the docblock reads like a sentence, not like a name guess."""
    for line in lines:
        text = " ".join(line.split())
        if not text or re.match(r"^(https?://|www\.)", text):
            continue
        if re.match(r"(?i)^(params?|args?|note|notes)\s*:", text):
            continue
        if NAME_GLOB_RE.match(text):
            continue
        if re.match(r"^`?[A-Za-z_][A-Za-z0-9_]*`?\s*(?:\([^)]*\))?\s*[:=]", text):
            # "ped: see ..." alone documents a parameter, not the native.
            continue
        if re.search(r"[a-z]{3,}\s+[a-z]{3,}", text) and text.endswith((".", "!", "?", ")")):
            return True
    return False


def first_sentence(lines):
    """First sentence of a docblock, for natives with no comment of their own."""
    for line in lines:
        text = " ".join(line.split())
        if not text or re.match(r"^(https?://|www\.|_?[A-Z_]+\(.*\)\b)", text):
            continue
        if text.endswith((".", "!", "?")):
            return text
        if text.endswith("."):
            return text
        return text + "." if text else ""
    return ""


RETURN_NAMES = {
    "void": "",
    "bool": "a BOOL",
    "int": "an int",
    "uint": "a u32",
    "float": "a float",
    "ulong": "a u64",
    "string": "a string",
    "Vector3": "a Vector3",
    "IntPtr": "a u64 handle",
}


def generic_description(decl, db_lines):
    """Last resort: the database comment in one line, else the native name."""
    summary = first_sentence(db_lines)
    if summary:
        return summary

    name = decl["name"].strip("_")
    if re.fullmatch(r"0x[0-9a-fA-F]{16}", name):
        opening = ("Undocumented %s native, hash-named in both the declaration file "
                   "and the RDR3 database." % decl["class"])
        if decl["params"]:
            count = len(decl["params"])
            opening += " Takes %d argument%s." % (count, "" if count == 1 else "s")
        ret = RETURN_NAMES.get(decl["ret"])
        if ret:
            opening += " Returns %s." % ret
        return opening

    plain = name.lower().replace("_", " ")
    return " ".join((name_summary(name, plain), argument_summary(decl),
                     "The RDR3 native database has no comment for it."))


def name_summary(name, plain):
    """A one-line sentence about the native, from its name alone."""
    lower = name.strip("_").lower()

    if lower in {"sin", "cos", "tan", "asin", "acos", "atan", "sqrt", "pow", "exp", "log10"}:
        return "Computes %s of the given value." % lower
    if lower in {"floor", "ceil", "round"}:
        verb = {"floor": "down", "ceil": "up", "round": "to the nearest integer"}[lower]
        return "Rounds the value %s." % verb
    if lower in {"timera", "timerb"}:
        return "Returns the %s script timer, in milliseconds." % lower
    if lower in {"settimera", "settimerb"}:
        return "Sets the %s script timer value, in milliseconds." % lower
    if lower in {"to_float", "to_int"}:
        return "Converts the value to a %s." % ("float" if lower == "to_float" else "int")
    if lower == "wait":
        return "Pauses the calling script for `ms` milliseconds."
    if lower in {"vmag", "vmag2"}:
        return "Returns the magnitude of a vector%s." % (
            "" if lower == "vmag" else ", without the square root")
    if lower in {"vdist", "vdist2"}:
        return "Returns the distance between two vectors%s." % (
            ", in meters" if lower == "vdist" else ", without the square root")

    # "TASK_ENTER_COVER" -> "Starts the task enter cover";
    # "IS_ENTITY_A_VEHICLE" -> "Checks whether the entity is a vehicle".
    words = plain.split()
    auxiliaries = {"is", "has", "can", "does", "did", "are", "should", "will", "was", "were"}
    checks = len(words) > 2 and sum(1 for w in words if w in auxiliaries) >= 2

    if words[0] in {"get", "return"} and len(words) > 1 and "number" in words[1:]:
        opening = "Returns " + plain[len(words[0]) + 1:]
    elif checks:
        opening = "Checks the condition " + plain
    elif len(words) == 1:
        opening = "Calls the %s native." % words[0]
    elif words[0] == "task":
        opening = "Starts the task " + plain[5:]
    elif words[0] in {"add", "create"}:
        opening = "Adds " + plain[len(words[0]) + 1:]
    elif words[0] == "set":
        opening = "Sets " + plain[4:]
    elif words[0] in {"release", "disable", "enable", "remove", "delete", "clear", "reset",
                      "stop", "start", "load", "request", "play", "force", "apply", "give",
                      "trigger", "fire", "update", "register", "destroy", "attach", "detach",
                      "allow", "block", "open", "close", "lock", "unlock", "abort", "cancel"}:
        opening = plain
    else:
        opening = "Handles " + plain
    return opening[0].upper() + opening[1:] + "."


def argument_summary(decl):
    """`Takes 2 arguments; returns a BOOL.` from the declared signature."""
    count = len(decl["params"])
    parts = []
    if count:
        parts.append("Takes %d argument%s" % (count, "" if count == 1 else "s"))
    ret = RETURN_NAMES.get(decl["ret"])
    if ret:
        parts.append("returns %s" % ret)
    if not parts:
        return "The native takes no argument and returns nothing."
    if len(parts) == 2 and parts[1].startswith("returns "):
        # "Takes 2 arguments; returns a BOOL."
        return parts[0] + "; " + parts[1] + "."
    sentence = ", ".join(parts)
    return sentence[0].upper() + sentence[1:] + "."


def docblock(decl, entry):
    """(description lines, old name, source key, extra lines) for one declaration.

    Order: the declaration file's text, then the database's, then a sentence
    built from the name. `extra` holds the notes of the source that did not
    win: a database sentence under a placeholder comment ("_CLEAR*") replaces
    it, while a database comment that only repeats the declaration file's
    notes is dropped.
    """
    cs_lines, cs_old = split_comment("\n".join(decl.get("docs_lines") or []))
    db_lines, db_old = split_comment((entry or {}).get("comment", ""))
    old = cs_old or db_old

    if is_real_comment(cs_lines):
        extra = []
        if is_real_comment(db_lines) and " ".join(db_lines).lower() != " ".join(cs_lines).lower():
            kept = [line for line in db_lines
                    if line.strip() and not re.match(r"(?i)^old names?\s*:", line)
                    and line.strip() not in {line.strip() for line in cs_lines}]
            if kept and not describes_native(cs_lines):
                # The declaration file only has a placeholder ("_CLEAR*",
                # "Params: p1 = 23"); the database sentence goes first.
                return db_lines, old, "cs+db", cs_lines
            extra = kept
        return cs_lines, old, "cs", extra
    if is_real_comment(db_lines):
        return db_lines, old, "db", []
    return [generic_description(decl, db_lines)], old, "generic", []


def usable_db_name(name):
    return bool(name) and not GENERIC_PARAM_RE.match(name) and not PLACEHOLDER_RE.match(name)


def unique_name(base, used, kept):
    """`base`, then `base2`, `base3`... skipping names the signature keeps as-is."""
    candidate = base
    suffix = 2
    while candidate in used and candidate not in kept:
        candidate = "%s%d" % (base, suffix)
        suffix += 1
    return candidate


def aligned_db_names(decl, entry):
    names = (entry or {}).get("params") or []
    if len(names) != len(decl["params"]):
        return [""] * len(decl["params"])
    return names


def type_fallback_names(decl, db_names):
    """Names for the placeholders with no documented name, one per parameter.

    A bare type name is not enough: two ints would both become `handle`. The
    name stays readable and unique: `handle`, `handle2`, ... except when the
    meanings differ, then the closest other word of the native name is used
    first (`ped`, then `targetPed` for `SET_PLAYER_FORCED_AIM`).
    """
    words = [w for w in re.split(r"_+", decl["name"].strip("_").lower()) if w]
    stop = {"get", "set", "is", "has", "the", "of", "a", "an", "to", "in", "on", "and", "or"}
    hints = [w for w in words if w not in stop and not re.fullmatch(r"\d+", w)]

    by_type = defaultdict(list)
    for index, (param_type, lib_name, _) in enumerate(decl["params"]):
        if GENERIC_PARAM_RE.match(lib_name) and not usable_db_name(db_names[index]):
            by_type[param_type].append(index)

    names = {}
    for param_type, indexes in by_type.items():
        if len(indexes) == 1:
            names[indexes[0]] = TYPE_FALLBACK.get(param_type, "arg")
            continue
        # Split the hint words across the same-typed placeholders in order,
        # then fall back to the numbered type name.
        candidates = [hint for hint in hints if hint not in {TYPE_FALLBACK.get(t) for t in by_type}]
        for slot, index in enumerate(indexes):
            if slot < len(candidates):
                names[index] = candidates[slot]
            else:
                base = TYPE_FALLBACK.get(param_type, "arg")
                names[index] = base if slot == 0 else "%s%d" % (base, slot + 1)

    # Drop the hints already taken by another parameter of the same type.
    taken = set(names.values())
    for index, (param_type, lib_name, _) in enumerate(decl["params"]):
        if index in names:
            continue
        if GENERIC_PARAM_RE.match(lib_name) and usable_db_name(db_names[index]):
            taken.add(db_names[index])

    for index, (param_type, lib_name, _) in enumerate(decl["params"]):
        if index in names:
            base = names[index]
            candidate, suffix = base, 2
            while candidate in taken and candidate != base:
                candidate = "%s%d" % (base, suffix)
                suffix += 1
            if candidate in taken:
                base = TYPE_FALLBACK.get(param_type, "arg")
                candidate, suffix = base, 2
                while candidate in taken:
                    candidate = "%s%d" % (base, suffix)
                    suffix += 1
            names[index] = candidate
            taken.add(candidate)
    return names


def resolve_param_names(decl, entry):
    """[(name, db name, source)] for the generated Rust signature."""
    db_names = aligned_db_names(decl, entry)
    kept = {name for _, name, _ in decl["params"]}
    fallbacks = type_fallback_names(decl, db_names)
    used = set()
    resolved = []
    for index, ((param_type, lib_name, _), db_name) in enumerate(zip(decl["params"], db_names)):
        if not GENERIC_PARAM_RE.match(lib_name):
            name, source = lib_name, "api"
        elif usable_db_name(db_name):
            name, source = db_name, "db"
        else:
            name, source = fallbacks[index], "type"
        used.add(name)
        resolved.append((name, db_name, source))
    return resolved


def param_lines(entry, resolved):
    """Per-database-name comment taken from the comment body, or ""."""
    documented = {}
    for line in (entry or {}).get("comment", "").splitlines():
        match = PARAM_LINE_RE.match(line)
        if match:
            documented.setdefault(match.group("name"), " ".join(match.group("text").split()))

    table = {}
    for _name, db_name, _source in resolved:
        table[db_name] = documented.get(db_name, "")
    return table


# ---------------------------------------------------------------------------
# Build
# ---------------------------------------------------------------------------

def pending_summary(lines, index):
    """The `///` block directly above line `index` (1-based)."""
    collected = []
    i = index - 2
    while i >= 0 and lines[i].strip().startswith("///"):
        collected.insert(0, lines[i].strip()[3:].lstrip())
        i -= 1
    return [line for line in collected if line.strip() not in ("<summary>", "</summary>")]


def build(source=SOURCE_CS, vendor=None):
    # Raw comments for the merge: the vendored database text and the
    # declaration file's <summary> blocks. Only those two are read, so a run
    # never feeds its own output back in and stays idempotent.
    raw = vendor if vendor is not None else load_vendor()

    # The declarations come from the file; the merged documents only matter to
    # the generators, so an empty map keeps the per-declaration docblock cheap.
    declarations, _skipped = parse(source, {})

    with open(source, encoding="utf-8-sig") as handle:
        lines = handle.read().splitlines()

    produced = {}
    stats = {"cs": 0, "cs+db": 0, "db": 0, "generic": 0,
             "params_db": 0, "params_filled": 0, "params_api": 0}
    for decl in declarations:
        entry = raw.get(decl["hash"].lower())
        decl["docs_lines"] = pending_summary(lines, decl["line"])
        description, old, provenance, extra = docblock(decl, entry)
        stats[provenance] += 1

        resolved = resolve_param_names(decl, entry)
        documented = param_lines(entry, resolved)
        params = []
        for (param_type, _lib_name, _), (name, db_name, source_key) in zip(decl["params"], resolved):
            stats["params_" + ("db" if source_key == "db" else
                               "filled" if source_key == "type" else "api")] += 1
            doc = documented.get(db_name, "") if db_name else ""
            if not doc and source_key == "type":
                doc = TYPE_NOTE.get(param_type, "")
            params.append({"name": name, "type": param_type, "doc": doc,
                           "out": param_type.endswith("*")})

        produced[decl["hash"].lower()] = {
            "ns": decl["class"],
            "desc": "\n".join(description),
            "old": old or "",
            "source": provenance,
            "extra": extra,
            "lib_params": [item[0] for item in resolved],
            "params": params,
            "return_type": (entry or {}).get("return_type", ""),
        }

    # Database entries the declaration file does not expose keep a description
    # so a lookup by hash still finds something.
    for hash_hex, entry in raw.items():
        if hash_hex in produced or not entry.get("comment", "").strip():
            continue
        description, old = split_comment(entry["comment"])
        produced[hash_hex] = {
            "ns": entry.get("ns", ""),
            "desc": "\n".join(description),
            "old": old or "",
            "source": "db",
            "extra": [],
            "lib_params": entry.get("params") or [],
            "params": [{"name": name, "type": "", "doc": ""} for name in entry.get("params") or []],
            "return_type": entry.get("return_type", ""),
        }

    return produced, stats


def serialise(data):
    return json.dumps(data, ensure_ascii=False, sort_keys=True, indent=1) + "\n"


def main():
    parser = argparse.ArgumentParser(description="Merge the native descriptions.")
    parser.add_argument("--source", default=SOURCE_CS)
    parser.add_argument("--docs", default=DOCS_JSON, help="merged documentation JSON, rewritten in place")
    parser.add_argument("--vendor", default=VENDOR_JSON,
                        help="vendored RDR3 database (tools/rdr3_natives.json)")
    parser.add_argument("--check", action="store_true", help="verify the JSON is up to date")
    args = parser.parse_args()

    vendor = load_vendor(args.vendor)
    data, stats = build(args.source, vendor=vendor)
    text = serialise(data)

    if args.check:
        current = ""
        if os.path.exists(args.docs):
            with open(args.docs, encoding="utf-8") as handle:
                current = handle.read()
        if current != text:
            print("tools/native_docs.json is out of date; run tools/generate_native_docs.py",
                  file=sys.stderr)
            return 1
        print("tools/native_docs.json is up to date")
        return 0

    with open(args.docs, "w", encoding="utf-8", newline="\n") as handle:
        handle.write(text)

    print("Documented %d natives" % len(data))
    for key, label in (("cs", "description from lib/NativeDecl.cs"),
                       ("cs+db", "database text added to a placeholder comment"),
                       ("db", "description from the RDR3 database"),
                       ("generic", "description built from the name"),
                       ("params_api", "parameter names from lib/NativeDecl.cs"),
                       ("params_db", "parameter names taken from the RDR3 database"),
                       ("params_filled", "parameter names derived from the type")):
        print("  %-45s %5d" % (label, stats[key]))
    return 0


if __name__ == "__main__":
    sys.exit(main())
