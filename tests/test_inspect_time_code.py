import hashlib
import importlib.util
import json
from pathlib import Path
import struct
import unittest

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("inspect_time_code", ROOT / "tools/inspect_time_code.py")
tool = importlib.util.module_from_spec(spec)
spec.loader.exec_module(tool)


class InspectTests(unittest.TestCase):
    def test_synthetic_code_reports_identity_and_candidates_without_game_bytes(self):
        title = "0004000000055D00"
        _, address, signature = tool.TITLES[title]
        data = bytearray(address - 0x100000 + 16)
        data[-16:] = signature
        struct.pack_into("<I", data, 0x20, 0x1FF81020)
        struct.pack_into("<I", data, 0x24, 0xEF000028)
        original = bytes(data)
        result = tool.inspect(original, title)
        self.assertTrue(result["upstream_identity_probe_matches"])
        self.assertEqual(result["file_sha256"], hashlib.sha256(original).hexdigest())
        self.assertEqual(result["references"]["0x1FF81020"]["candidate_addresses"], ["0x00100020"])
        self.assertNotIn(signature.decode(), json.dumps(result))
        self.assertFalse(result["installable"])
        self.assertFalse(result["first_read_verified"])
        self.assertEqual(original, bytes(data))
        self.assertFalse(tool.inspect(original, "0004000000055E00")["upstream_identity_probe_matches"])

    def test_containers_and_bad_mapping_are_rejected(self):
        for data in [b"short", b"\x7fELF" + bytes(512), bytes(256) + b"NCCH" + bytes(256)]:
            with self.assertRaises(ValueError):
                tool.inspect(data, "0004000000055D00")
        with self.assertRaises(ValueError):
            tool.inspect(bytes(512), "BAD")
        with self.assertRaises(ValueError):
            tool.inspect(bytes(512), "0004000000055D00", 0x100001)

    def test_no_unverified_profile_is_shipped(self):
        manifest = json.loads((ROOT / "profiles/manifest.json").read_text(encoding="utf-8"))
        self.assertEqual(manifest["game_clock_adapters"], [])
        self.assertEqual(manifest["release_channel"], "development")


if __name__ == "__main__":
    unittest.main()
