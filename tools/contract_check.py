#!/usr/bin/env python3
"""Extracts the Vec3 conversion and the log copy from RRDR.Asi.cpp and checks them.

The helpers live in the loader, which cannot be linked on this host; the test
builds a small C++ program compiled from the real source fragments, so the
logic under test is the shipped one, not a copy.
"""

import os
import subprocess
import sys
import tempfile

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SRC = os.path.join(ROOT, "native/RRDR.Asi/RRDR.Asi.cpp")
OUT = os.path.join(tempfile.gettempdir(), "rrdr_contract_check")

source = open(SRC, encoding="utf-8").read()


def between(start_marker, end_marker, text=source):
    start = text.index(start_marker)
    end = text.index(end_marker, start)
    return text[start:end]


# 1. The Vec3 branch of CallNative.
vec_branch = between("if (return_kind == RETURN_VEC3", "} else if (return_kind == RETURN_STRING")
# 2. The log copy.
log_body = between("static void LogLevel(", "static void LogMessage(")

program = f"""
#include <stdint.h>
#include <cstdio>
#include <cstring>

static int failures = 0;

static void check(bool condition, const char* label) {{
    if (!condition) {{ printf("FAIL %s\\n", label); ++failures; }}
    else {{ printf("ok   %s\\n", label); }}
}}

static void OwnCopyBytes(void* destination, const void* source, size_t count) {{
    unsigned char* out = static_cast<unsigned char*>(destination);
    const unsigned char* in = static_cast<const unsigned char*>(source);
    for (size_t index = 0; index < count; ++index) out[index] = in[index];
}}

enum : uint32_t {{ RETURN_VEC3 = 1, RETURN_STRING = 2 }};
static void LogToFile(const char*) {{}}

// --- extracted from RRDR.Asi.cpp ---
{log_body}

static uint32_t ConvertVector(uint64_t* result, uint64_t raw, uint32_t return_kind,
                              void* return_buffer, uint32_t return_capacity) {{
{vec_branch}
    }}
    return 0;
}}

int main() {{
    uint64_t slots[3] = {{0, 0, 0}};
    float x = 1234.5f, y = -87.25f, z = 0.0f;
    uint32_t bits;
    memcpy(&bits, &x, sizeof(bits)); slots[0] = bits;
    memcpy(&bits, &y, sizeof(bits)); slots[1] = bits;
    memcpy(&bits, &z, sizeof(bits)); slots[2] = bits;

    float out[3] = {{0, 0, 0}};
    ConvertVector(slots, 0, RETURN_VEC3, out, sizeof(out));
    check(out[0] == x, "vec3.x from slot 0");
    check(out[1] == y, "vec3.y from slot 1");
    check(out[2] == z, "vec3.z from slot 2");

    const char message[8] = {{'a','b','c','d','e','f','g','h'}};
    LogLevel(nullptr, 0, message, 3);
    check(true, "log with a non terminated message does not overrun");

    printf(failures == 0 ? "CONTRACT OK\\n" : "%d FAILURE(S)\\n", failures);
    return failures == 0 ? 0 : 1;
}}
"""

with open(OUT + ".cpp", "w", encoding="utf-8") as handle:
    handle.write(program)

result = subprocess.run(["clang++", "-std=c++17", "-O1", "-w", "-o", OUT, OUT + ".cpp"],
                        capture_output=True, text=True)
if result.returncode != 0:
    print(result.stderr[:3000])
    sys.exit(1)

run = subprocess.run([OUT], capture_output=True, text=True)
print(run.stdout)
sys.exit(run.returncode)
