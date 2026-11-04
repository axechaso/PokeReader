"""Read-only reconnaissance for a user's decompressed ExeFS code.bin.

Outputs hashes and candidate offsets, never game bytes or an installable profile.
An SDK literal or tick SVC alone does not establish a calendar function or ABI.
"""
import argparse
import hashlib
import json
from pathlib import Path
import struct

TITLES = {
    "0004000000055D00": (5, 0x10869C, b"h0VRqB2YEgq39zvO"),
    "0004000000055E00": (5, 0x10869C, b"Slv7vHlUOfqrKMpz"),
    "000400000011C400": (7, 0x1086BC, b"cRFY0WFHNjPh44If"),
    "000400000011C500": (7, 0x1086BC, b"guBwm9TlQvYvncKn"),
    "0004000000164800": (2, 0x3D3A90, b"8QjtffIMWFhiFpTz"),
    "0004000000175E00": (2, 0x3D3A90, b"7mXz0DXR4b4CdD8r"),
    "00040000001B5000": (2, 0x3E5884, b"fnCAH3KrGIl9dgSd"),
    "00040000001B5100": (2, 0x3E5888, b"b3Gq6LF6EqE1bvKy"),
}
LITERALS = (0x1FF81000, 0x1FF81020, 0x1FF81040, 0x1FF81060, 0xEF000028)


def inspect(data: bytes, title: str, base: int = 0x100000) -> dict:
    title = title.upper()
    if title not in TITLES:
        raise ValueError("Expected one of the eight supported PokeReader Title IDs")
    if len(data) < 16 or len(data) > 64 * 1024 * 1024:
        raise ValueError("Expected a decompressed ExeFS code binary (16 bytes to 64 MiB)")
    if data[:4] == b"\x7fELF" or data[0x100:0x104] in (b"NCCH", b"NCSD"):
        raise ValueError("Extract and decompress the ExeFS code binary first")
    if base < 0x100000 or base % 4 or base + len(data) > 0x10000000:
        raise ValueError("Invalid ARM code mapping base or extent")
    version, address, signature = TITLES[title]
    offset = address - base
    references = {}
    for literal in LITERALS:
        needle = struct.pack("<I", literal)
        offsets = [i for i in range(0, len(data) - 3, 4) if data[i:i + 4] == needle]
        references[f"0x{literal:08X}"] = {
            "count": len(offsets), "candidate_addresses": [f"0x{base + i:08X}" for i in offsets[:64]],
            "truncated": len(offsets) > 64,
        }
    return {
        "schema_version": 1, "title_id": title, "candidate_remaster_version": version,
        "size": len(data), "base_address": f"0x{base:08X}",
        "file_sha256": hashlib.sha256(data).hexdigest(),
        "fingerprint_scope": "file bytes [16, EOF); excludes loader-modified entry; NOT a verified memory profile",
        "after_entry_sha256": hashlib.sha256(data[16:]).hexdigest(),
        "upstream_identity_probe_matches": offset >= 0 and data[offset:offset + 16] == signature,
        "references": references, "calendar_abi_verified": False, "first_read_verified": False,
        "installable": False,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("code", type=Path)
    parser.add_argument("--title", required=True)
    parser.add_argument("--base", type=lambda v: int(v, 0), default=0x100000)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    if args.output and args.output.resolve() == args.code.resolve():
        parser.error("Output must not overwrite the input binary")
    try:
        result = inspect(args.code.read_bytes(), args.title, args.base)
    except (ValueError, OSError) as error:
        parser.error(str(error))
    text = json.dumps(result, ensure_ascii=False, indent=2) + "\n"
    if args.output:
        args.output.write_text(text, encoding="utf-8")
    else:
        print(text, end="")


if __name__ == "__main__":
    main()
