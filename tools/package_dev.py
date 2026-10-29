"""Package only the development deliverables; no game binaries or local research."""
import argparse
import hashlib
from pathlib import Path
import zipfile

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--artifacts", type=Path, default=ROOT / "out")
    parser.add_argument("--windows-lab", type=Path)
    parser.add_argument("--output", type=Path, default=ROOT / "out/PokeReader-Time-dev.zip")
    args = parser.parse_args()
    sources = {
        "luma/plugins/default.3gx": args.artifacts / "default.3gx",
        "3ds/PokeReader-Time/PokeReader-Time.3dsx": args.artifacts / "PokeReader-Time.3dsx",
        "README-zh-CN.md": ROOT / "docs/time-lock-development.zh-CN.md",
        "LICENSE": ROOT / "LICENSE",
        "adapter-status.json": ROOT / "profiles/manifest.json",
        "examples/UltraSun.ini": ROOT / "examples/UltraSun.ini",
        "build-info.txt": args.artifacts / "build-info.txt",
        "baseline-build.txt": args.artifacts / "baseline-build.txt",
    }
    if args.windows_lab:
        sources["windows/PokeReader-Clock-Lab.exe"] = args.windows_lab
    data = {name: path.read_bytes() for name, path in sources.items()}
    if data["luma/plugins/default.3gx"][:4] != b"3GX$":
        parser.error("Invalid 3GX header")
    if data["3ds/PokeReader-Time/PokeReader-Time.3dsx"][:4] != b"3DSX":
        parser.error("Invalid 3DSX header")
    if args.output.resolve() in {path.resolve() for path in sources.values()}:
        parser.error("Output must not overwrite any input")
    sums = "".join(f"{hashlib.sha256(body).hexdigest()}  {name}\n" for name, body in data.items())
    args.output.parent.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(args.output, "w", compression=zipfile.ZIP_DEFLATED) as archive:
        for name, body in data.items():
            archive.writestr(name, body)
        archive.writestr("SHA256SUMS.txt", sums)
    print(f"Created {args.output}: {len(data)} files and SHA256SUMS.txt")


if __name__ == "__main__":
    main()
