#!/usr/bin/env python3
"""Generate typed Rust bindings for RDR2 natives from NativeDecl.cs.

The source file is the generated declaration file shipped with
ScriptHookRDR2DotNet V2 (zlib license, see lib/LICENSE-ScriptHookRDR2DotNet.txt).

Usage:
    python3 tools/generate_natives.py
    python3 tools/generate_natives.py --source lib/NativeDecl.cs \
        --out rust/crates/rrdr-natives/src

The generated code calls every native through `rrdr_bridge::GameApi`, so it works
in game (managed host) and in the offline simulator (mock host).
"""

from __future__ import annotations

import argparse
import os
import re
import sys
from collections import Counter, defaultdict

CLASS_RE = re.compile(r"^\s*public\s+unsafe\s+static\s+class\s+([A-Za-z_][A-Za-z0-9_]*)")
DECL_RE = re.compile(
    r"^\s*public\s+static\s+(?:unsafe\s+)?"
    r"(?P<ret>[A-Za-z_][A-Za-z0-9_\.]*(?:\s*\*)?)\s+"
    r"(?P<name>[A-Za-z_][A-Za-z0-9_]*)\s*"
    r"\((?P<params>.*)\)\s*=>\s*Function\.Call"
    r"(?:<(?P<generic>[^>]+)>)?\s*\(\s*(?P<hash>0x[0-9A-Fa-f]+)"
)
PARAM_RE = re.compile(
    r"^(?P<variadic>params\s+)?"
    r"(?P<type>[A-Za-z_][A-Za-z0-9_\.]*(?:\s*\*)?)\s+"
    r"(?P<name>[A-Za-z_][A-Za-z0-9_]*)$"
)

PARAM_TYPES = {
    "int": "i32",
    "uint": "u32",
    "float": "f32",
    "bool": "bool",
    "ulong": "u64",
    "string": "&str",
    "Vector3": "Vec3",
    "IntPtr": "u64",
    # Pointer out-params are passed through as raw addresses (Arg::Ptr). The
    # size the native actually writes is community-documented for most natives
    # and unvalidated in-game: a 4-byte BOOL through a bool*, or a 24-byte
    # three-slot vector through a Vector3*, overruns a 1- or 12-byte Rust type.
    # Callers in rrdr-game wrap these in padded scratch blocks
    # (rrdr-game/src/scratch.rs) so an over-write corrupts padding, not the
    # stack. Validate each layout in-game before trusting the values.
    "int*": "*mut i32",
    "uint*": "*mut u32",
    "float*": "*mut f32",
    "ulong*": "*mut u64",
    "bool*": "*mut bool",
    "Vector3*": "*mut Vec3",
}

RETURN_TYPES = {
    "void": ("()", "call"),
    "bool": ("bool", "call_bool"),
    "int": ("i32", "call_i32"),
    "uint": ("u32", "call_u32"),
    "float": ("f32", "call_f32"),
    "ulong": ("u64", "call"),
    "IntPtr": ("u64", "call"),
    "Vector3": ("Vec3", "call_vec3"),
    "string": ("String", "call_string"),
}

RUST_KEYWORDS = {
    "as", "break", "const", "continue", "crate", "else", "enum", "extern",
    "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod",
    "move", "mut", "pub", "ref", "return", "self", "Self", "static", "struct",
    "super", "trait", "true", "type", "unsafe", "use", "where", "while",
    "async", "await", "dyn", "abstract", "become", "box", "do", "final",
    "macro", "override", "priv", "typeof", "unsized", "virtual", "yield",
    "try", "union",
}


def sanitize_param(name: str) -> str:
    if name in ("self", "Self", "crate", "super"):
        return name + "_"
    if name in RUST_KEYWORDS:
        return "r#" + name
    if not re.match(r"^[A-Za-z_]", name):
        return "_" + name
    return name


def sanitize_module(name: str) -> str:
    module = name.lower()
    if module in RUST_KEYWORDS:
        module += "_ns"
    return module


def parse(source: str):
    declarations = []
    skipped_variadic = []
    current_class = None
    unparsed = []

    with open(source, encoding="utf-8-sig") as handle:
        lines = handle.read().splitlines()

    for line_number, line in enumerate(lines, 1):
        match = CLASS_RE.match(line)
        if match:
            current_class = match.group(1)
            continue

        if "public static" not in line:
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
                    # params InputArgument[] args: variadic, skipped below.
                    variadic = True
                    continue
                param_match = PARAM_RE.match(raw_param)
                if not param_match:
                    unparsed.append((line_number, "PARAM: " + raw_param))
                    continue
                param_type = param_match.group("type").replace(" ", "")
                param_name = param_match.group("name")
                is_variadic = bool(param_match.group("variadic"))
                if is_variadic:
                    variadic = True
                params.append((param_type, param_name, is_variadic))

        if variadic:
            skipped_variadic.append(f"{current_class}::{match.group('name')}")
            continue

        ret = match.group("ret").replace(" ", "")
        if ret not in RETURN_TYPES:
            unparsed.append((line_number, "RETURN: " + ret))
            continue
        for param_type, _, _ in params:
            if param_type not in PARAM_TYPES:
                unparsed.append((line_number, "PARAM TYPE: " + param_type))
                break
        else:
            declarations.append(
                {
                    "class": current_class,
                    "name": match.group("name"),
                    "ret": ret,
                    "params": params,
                    "hash": match.group("hash"),
                    "line": line_number,
                }
            )

    if unparsed:
        print("Unparsed declarations:", file=sys.stderr)
        for item in unparsed[:20]:
            print("  ", item, file=sys.stderr)
        raise SystemExit(1)

    if any(decl["class"] is None for decl in declarations):
        raise SystemExit("Found a declaration before the first class")

    return declarations, skipped_variadic


def has_vector_value(decl):
    # Only a by-value Vector3 makes a function the convenience overload.
    # A Vector3* is an output pointer and may exist in both variants.
    return any(param_type == "Vector3" for param_type, _, _ in decl["params"])


def choose_names(group):
    """Return [(rust_name, declaration)] for one class+name group.

    All overloads in NativeDecl.cs are scalar + Vector3 convenience variants.
    The scalar version keeps the native name; the Vector3 version gets _VEC3.
    """
    result = []
    if len(group) == 1:
        result.append((group[0]["name"], group[0]))
        return result

    scalar = [decl for decl in group if not has_vector_value(decl)]
    vector = [decl for decl in group if has_vector_value(decl)]

    if len(scalar) == 1 and len(vector) == len(group) - 1:
        result.append((scalar[0]["name"], scalar[0]))
        for index, decl in enumerate(vector, 1):
            suffix = "_VEC3" if index == 1 else f"_VEC3_{index}"
            result.append((decl["name"] + suffix, decl))
        return result

    # Defensive fallback for future API updates.
    for index, decl in enumerate(group):
        suffix = "" if index == 0 else f"_{index + 1}"
        result.append((decl["name"] + suffix, decl))
    return result


def arg_expressions(decl):
    expressions = []
    for param_type, param_name, _ in decl["params"]:
        name = sanitize_param(param_name)
        if param_type == "int":
            expressions.append(f"Arg::I32({name})")
        elif param_type == "uint":
            expressions.append(f"Arg::U32({name})")
        elif param_type == "float":
            expressions.append(f"Arg::F32({name})")
        elif param_type == "bool":
            expressions.append(f"Arg::Bool({name})")
        elif param_type in ("ulong", "IntPtr"):
            expressions.append(f"Arg::U64({name})")
        elif param_type == "string":
            expressions.append(f"Arg::Str({name})")
        elif param_type == "Vector3":
            expressions.append(f"Arg::F32({name}.x)")
            expressions.append(f"Arg::F32({name}.y)")
            expressions.append(f"Arg::F32({name}.z)")
        elif param_type.endswith("*"):
            expressions.append(f"Arg::Ptr({name} as u64)")
        else:
            raise ValueError(f"Unsupported parameter type {param_type}")
    return expressions


def generate_function(decl, rust_name):
    signature_params = ["api: &mut dyn GameApi"]
    for param_type, param_name, _ in decl["params"]:
        signature_params.append(f"{sanitize_param(param_name)}: {PARAM_TYPES[param_type]}")

    return_type, call_method = RETURN_TYPES[decl["ret"]]
    args = arg_expressions(decl)
    args_array = "&[" + ", ".join(args) + "]" if args else "&[]"
    const_name = "HASH_" + rust_name
    call = f"api.{call_method}({const_name}, {args_array})"

    if decl["ret"] == "void":
        body = f"    {call};"
        return_clause = ""
    else:
        body = f"    {call}"
        return_clause = f" -> {return_type}"

    variant = " (Vector3 overload)" if rust_name.endswith(("_VEC3", "_VEC3_2", "_VEC3_3")) else ""
    doc = f"/// `{decl['class']}::{decl['name']}`{variant} - `{decl['hash']}`."

    return (
        f"{doc}\n"
        f"pub const {const_name}: u64 = {decl['hash']};\n"
        f"pub fn {rust_name}({', '.join(signature_params)}){return_clause} {{\n"
        f"{body}\n"
        f"}}\n"
    )


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--source", default="lib/NativeDecl.cs")
    parser.add_argument("--out", default="rust/crates/rrdr-natives/src")
    args = parser.parse_args()

    declarations, skipped_variadic = parse(args.source)

    grouped = defaultdict(list)
    for decl in declarations:
        grouped[(decl["class"], decl["name"])].append(decl)

    modules = defaultdict(list)
    for (class_name, _), group in grouped.items():
        for rust_name, decl in choose_names(group):
            modules[sanitize_module(class_name)].append((rust_name, decl))

    os.makedirs(args.out, exist_ok=True)
    # Only delete files this generator produced (they carry the @generated
    # header). Hand-written modules placed next to the generated ones survive
    # a regeneration.
    for filename in os.listdir(args.out):
        if not filename.endswith(".rs"):
            continue
        path = os.path.join(args.out, filename)
        with open(path, encoding="utf-8", errors="replace") as handle:
            head = handle.read(200)
        if "// @generated" in head:
            os.remove(path)

    function_count = 0
    vec3_count = 0

    for module_name in sorted(modules):
        filename = os.path.join(args.out, module_name + ".rs")
        functions = sorted(modules[module_name], key=lambda item: item[0])

        lines = [
            "// @generated by tools/generate_natives.py from lib/NativeDecl.cs. Do not edit.",
            "#![allow(non_snake_case)]",
            "#![allow(non_upper_case_globals)]",
            "#![allow(clippy::too_many_arguments)]",
            "#[allow(unused_imports)]",
            "use rrdr_bridge::{Arg, GameApi, Vec3};",
            "",
        ]

        for rust_name, decl in functions:
            lines.append(generate_function(decl, rust_name))
            function_count += 1
            if rust_name.endswith(("_VEC3", "_VEC3_2", "_VEC3_3")):
                vec3_count += 1

        with open(filename, "w", encoding="utf-8", newline="\n") as handle:
            handle.write("\n".join(lines))

    lib_lines = [
        "// @generated by tools/generate_natives.py from lib/NativeDecl.cs. Do not edit.",
        "//! Typed Rust bindings for the Red Dead Redemption 2 native API.",
        "//!",
        "//! Every function takes `&mut dyn GameApi` as its first argument. The",
        "//! bindings are generated from ScriptHookRDR2DotNet V2's `NativeDecl.cs`;",
        "//! hashes and parameter order match that file exactly.",
        "//!",
        "//! `_VEC3` variants are convenience overloads that take `Vec3` instead of",
        "//! three floats. They call the same native hash.",
        "#![allow(non_snake_case)]",
        "#![allow(non_upper_case_globals)]",
        "",
        "pub use rrdr_bridge::Vec3;",
        "",
    ]
    for module_name in sorted(modules):
        lib_lines.append(f"pub mod {module_name};")

    lib_lines.append("")
    lib_lines.append(f"/// Number of generated native functions: {function_count}.")
    lib_lines.append(f"pub const NATIVE_FUNCTION_COUNT: usize = {function_count};")
    lib_lines.append("")
    lib_lines.append("/// Natives that use `params InputArgument[]`; not representable in the")
    lib_lines.append("/// current bridge ABI and therefore skipped.")
    lib_lines.append("pub const SKIPPED_VARIADIC: &[&str] = &[")
    for name in sorted(skipped_variadic):
        lib_lines.append(f'    "{name}",')
    lib_lines.append("];")
    lib_lines.append("")

    with open(os.path.join(args.out, "lib.rs"), "w", encoding="utf-8", newline="\n") as handle:
        handle.write("\n".join(lib_lines))

    print(f"Generated {function_count} functions in {len(modules)} modules")
    print(f"  Vector3 overloads: {vec3_count}")
    print(f"  Skipped variadic: {len(skipped_variadic)}")
    for name in skipped_variadic:
        print(f"    - {name}")

    hashes = Counter(decl["hash"].lower() for decl in declarations)
    print(f"  Unique hashes: {len(hashes)}")


if __name__ == "__main__":
    main()
