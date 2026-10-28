#!/usr/bin/env python3
"""Vendor the RDR3 native documentation used to fill the empty docblocks.

Source: https://github.com/alloc8or/rdr3-nativedb-data (natives.json), the
community-maintained RDR3 database of native comments, parameter names and
return descriptions. It is licensed under the zlib license (see
lib/LICENSE-rdr3-nativedb-data.txt).

The generators in this folder read the vendored copy and never hit the
network, so a build is reproducible. Re-run this script to refresh it:

    python3 tools/fetch_native_docs.py
    python3 tools/generate_native_docs.py
    python3 tools/generate_natives.py
    python3 tools/generate_natives_site.py

Output: tools/rdr3_natives.json, keyed by lower-case native hash, holding the
namespace, the comment, the parameter names and the return type. The merged
documentation the generators read is built from it by
`tools/generate_native_docs.py` (tools/native_docs.json).
"""

from __future__ import annotations

import json
import os
import sys
import urllib.request

SOURCE_URL = "https://raw.githubusercontent.com/alloc8or/rdr3-nativedb-data/master/natives.json"
HERE = os.path.dirname(os.path.abspath(__file__))
OUTPUT = os.path.join(HERE, "rdr3_natives.json")


def fetch(url=SOURCE_URL):
    request = urllib.request.Request(url, headers={"User-Agent": "RRDR natives docs fetch"})
    with urllib.request.urlopen(request, timeout=60) as response:
        return json.load(response)


def flatten(database):
    """{namespace: {hash: entry}} -> {hash: {comment, params, return_type, ...}}."""
    flat = {}
    for namespace, entries in database.items():
        for hash_hex, entry in entries.items():
            flat[hash_hex.lower()] = {
                "ns": namespace,
                "comment": entry.get("comment", ""),
                "params": [p.get("name", "") for p in entry.get("params", [])],
                "return_type": entry.get("return_type", ""),
            }
    return flat


def fetch_alloc8or():
    """The site serves the same export; useful when GitHub is not reachable."""
    return fetch("https://alloc8or.re/rdr3/nativedb/static/natives.json")


def main():
    if len(sys.argv) > 1:
        with open(sys.argv[1], encoding="utf-8") as handle:
            database = json.load(handle)
    else:
        try:
            database = fetch()
        except OSError:
            database = fetch_alloc8or()

    flat = flatten(database)
    documented = sum(1 for entry in flat.values() if entry["comment"].strip())
    with open(OUTPUT, "w", encoding="utf-8", newline="\n") as handle:
        json.dump(flat, handle, ensure_ascii=False, sort_keys=True, separators=(",", ":"))
        handle.write("\n")

    print("Wrote %d natives to %s" % (len(flat), OUTPUT))
    print("  with a comment: %d" % documented)
    print("  on disk: %.1f KiB" % (os.path.getsize(OUTPUT) / 1024))


if __name__ == "__main__":
    main()
