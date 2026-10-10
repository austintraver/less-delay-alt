# /// script
# requires-python = ">=3.12"
# dependencies = ["lz4>=4.3"]
# ///
"""Write the decompressed .text segment of an NSO0 (`main` from a game's exefs) to a file.

The NSO header stores each segment's file offset, memory offset and size, a per-segment
LZ4 flag, and the build ID at 0x40. Printing the build ID lets you tell dumps apart.

    uv run tools/nso_text.py path/to/main path/to/main.text
"""

import argparse
import struct
from pathlib import Path

import lz4.block

TEXT, RODATA, DATA = range(3)


def read_segment(blob: bytes, index: int) -> bytes:
    flags = struct.unpack_from("<I", blob, 0x0C)[0]
    file_offset, memory_offset, size = struct.unpack_from("<III", blob, 0x10 + 0x10 * index)
    compressed_size = struct.unpack_from("<I", blob, 0x60 + 4 * index)[0]
    raw = blob[file_offset:file_offset + compressed_size]
    if flags & (1 << index):
        raw = lz4.block.decompress(raw, uncompressed_size=size)
    assert len(raw) == size, f"segment {index}: got {len(raw)} bytes, header says {size}"
    return raw


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("nso", type=Path)
    parser.add_argument("out", type=Path, help="where to write the decompressed .text bytes")
    args = parser.parse_args()

    blob = args.nso.read_bytes()
    if blob[:4] != b"NSO0":
        raise SystemExit(f"{args.nso} is not an NSO0 file")
    build_id = blob[0x40:0x50].hex().upper()
    text = read_segment(blob, TEXT)
    args.out.write_bytes(text)
    print(f"build id {build_id}  .text {len(text):#x} bytes -> {args.out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
