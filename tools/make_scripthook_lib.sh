#!/usr/bin/env bash
# Generate a ScriptHookRDR2 import library from ScriptHookRDR2.dll.
#
# The game ships no .lib, so we parse the PE export table and emit a .def,
# then turn it into a COFF import library with llvm-dlltool.
set -euo pipefail

DLL="${1:-}"
OUT="${2:-}"

if [ -z "$DLL" ] || [ -z "$OUT" ]; then
    echo "usage: $0 <ScriptHookRDR2.dll> <output.lib>" >&2
    exit 2
fi

if [ ! -f "$DLL" ]; then
    echo "missing $DLL" >&2
    exit 1
fi

if ! command -v llvm-dlltool >/dev/null 2>&1; then
    echo "llvm-dlltool not found (install llvm)" >&2
    exit 1
fi

mkdir -p "$(dirname "$OUT")"

python3 - "$DLL" > "$OUT.def" <<'PY'
import struct
import sys

dll_path = sys.argv[1]

with open(dll_path, "rb") as fh:
    data = fh.read()

if data[:2] != b"MZ":
    raise SystemExit("not a PE file")

pe_offset = struct.unpack_from("<I", data, 0x3C)[0]
if data[pe_offset:pe_offset + 4] != b"PE\0\0":
    raise SystemExit("missing PE signature")

nsections = struct.unpack_from("<H", data, pe_offset + 6)[0]
opt_size = struct.unpack_from("<H", data, pe_offset + 20)[0]
opt_offset = pe_offset + 24
magic = struct.unpack_from("<H", data, opt_offset)[0]
is_pe32_plus = magic == 0x20B
dd_offset = opt_offset + (112 if is_pe32_plus else 96)
export_rva, _export_size = struct.unpack_from("<II", data, dd_offset)

if export_rva == 0:
    raise SystemExit("no export table")

section_offset = opt_offset + opt_size
sections = []
for index in range(nsections):
    off = section_offset + index * 40
    vsize, vaddr, rawsize, rawptr = struct.unpack_from("<IIII", data, off + 8)
    sections.append((vaddr, max(vsize, rawsize), rawptr))


def rva_to_offset(rva):
    for vaddr, size, rawptr in sections:
        if vaddr <= rva < vaddr + size:
            return rva - vaddr + rawptr
    raise SystemExit("RVA 0x%x outside sections" % rva)


export_off = rva_to_offset(export_rva)
(
    _flags,
    _stamp,
    _major,
    _minor,
    _name_rva,
    _ordinal_base,
    n_functions,
    n_names,
    functions_rva,
    names_rva,
    ordinals_rva,
) = struct.unpack_from("<IIHHIIIIIII", data, export_off)

functions_off = rva_to_offset(functions_rva)
names_off = rva_to_offset(names_rva)
ordinals_off = rva_to_offset(ordinals_rva)

names = []
for index in range(n_names):
    name_rva = struct.unpack_from("<I", data, names_off + index * 4)[0]
    name_off = rva_to_offset(name_rva)
    end = data.index(b"\0", name_off)
    name = data[name_off:end].decode("ascii")
    ordinal = struct.unpack_from("<H", data, ordinals_off + index * 2)[0]
    func_rva = struct.unpack_from("<I", data, functions_off + ordinal * 4)[0]
    if func_rva:
        names.append(name)

if not names:
    raise SystemExit("no named exports")

print("LIBRARY ScriptHookRDR2.dll")
print("EXPORTS")
for name in sorted(names):
    print("    " + name)
PY

llvm-dlltool -m i386:x86-64 -k -d "$OUT.def" -l "$OUT"
rm -f "$OUT.def"

echo "wrote $OUT"
