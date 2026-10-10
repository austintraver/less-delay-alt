# /// script
# requires-python = ">=3.12"
# ///
"""Count where less-delay's signatures match inside a decompressed SSBU `main` NSO.

Run this against every game version the plugin should support before trusting a signature:
each pattern must match exactly once. Pass the uncompressed `.text` segment (or the whole
decompressed `main`); the script parses the patterns straight out of `src/offsets.rs` so the
file checked is the file shipped.

    python tools/scan_needles.py path/to/main.text [--offsets src/offsets.rs]
"""

import argparse
import re
from pathlib import Path

HERE = Path(__file__).resolve().parent


Pattern = list[tuple[int, int]]  # (value, mask) per byte; the game byte must satisfy byte & mask == value

TOKEN = re.compile(r"b\(0x([0-9a-fA-F]{2})\)|m\(0x([0-9a-fA-F]{2}),\s*0x([0-9a-fA-F]{2})\)|\bBL\[\d\]|\bW\b")


def parse_offsets(source: str) -> dict[str, tuple[Pattern, int]]:
    """Return {location_name: (pattern, start_offset)} from offsets.rs."""
    source = re.sub(r"//.*", "", source)
    bl = re.search(r"const BL: \[Byte; 4\] = \[(.*?)\];", source, re.S)
    bl_pattern = parse_tokens(bl.group(1), {}) if bl else []
    patterns: dict[str, tuple[Pattern, int]] = {}
    for block in re.finditer(r"SSBUMemoryLocation \{(.*?)\n\};", source, re.S):
        body = block.group(1)
        signature = re.search(r"signature: &\[(.*?)\],\s*start_offset", body, re.S).group(1)
        pattern = parse_tokens(signature, {"BL": bl_pattern})
        start_offset = int(re.search(r"start_offset: (-?\d+)", body).group(1))
        name = re.search(r'location_name: "([^"]+)"', body).group(1)
        patterns[name] = (pattern, start_offset)
    return patterns


def parse_tokens(text: str, arrays: dict[str, Pattern]) -> Pattern:
    pattern: Pattern = []
    for match in TOKEN.finditer(text):
        exact, value, mask = match.group(1), match.group(2), match.group(3)
        token = match.group(0)
        if exact:
            pattern.append((int(exact, 16), 0xFF))
        elif value:
            mask_value = int(mask, 16)
            pattern.append((int(value, 16) & mask_value, mask_value))
        elif token.startswith("BL["):
            pattern.append(arrays["BL"][int(token[3])])
        else:
            pattern.append((0, 0))
    return pattern


def find_all(haystack: bytes, pattern: Pattern) -> list[int]:
    anchor_index, (anchor_value, _) = next((i, vm) for i, vm in enumerate(pattern) if vm[1] == 0xFF)
    hits = []
    position = haystack.find(bytes([anchor_value]), anchor_index)
    while position != -1:
        start = position - anchor_index
        if start + len(pattern) <= len(haystack) and all(
            haystack[start + i] & mask == value for i, (value, mask) in enumerate(pattern) if mask
        ):
            hits.append(start)
        position = haystack.find(bytes([anchor_value]), position + 1)
    return hits


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("binary", type=Path, help="decompressed main NSO or its .text segment")
    parser.add_argument("--offsets", type=Path, default=HERE.parent / "src" / "offsets.rs")
    args = parser.parse_args()

    haystack = args.binary.read_bytes()
    exit_code = 0
    for name, (pattern, start_offset) in parse_offsets(args.offsets.read_text()).items():
        hits = find_all(haystack, pattern)
        verdict = "OK" if len(hits) == 1 else "FAIL"
        if verdict == "FAIL":
            exit_code = 1
        targets = ", ".join(f"{hit + start_offset:#x}" for hit in hits[:5])
        print(f"{verdict:4s} {name}: {len(hits)} match(es){' -> hook at ' + targets if hits else ''}")
    return exit_code


if __name__ == "__main__":
    raise SystemExit(main())
