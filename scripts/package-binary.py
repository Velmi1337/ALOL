#!/usr/bin/env python3
"""Prepare the standalone x86_64 executable used for local ALOL updates."""
import argparse
from pathlib import Path
import shutil
import subprocess
import tomllib

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path, nargs="?", default=ROOT / "dist")
    parser.add_argument("--binary", type=Path, default=ROOT / "target/release/alol")
    args = parser.parse_args()
    version = tomllib.loads((ROOT / "Cargo.toml").read_text())["package"]["version"]
    binary = args.binary.resolve()
    header = binary.read_bytes()[:20]
    if header[:6] != b"\x7fELF\x02\x01" or header[18:20] != b"\x3e\x00":
        raise ValueError("Expected an ELF x86_64 executable")
    if subprocess.check_output([str(binary), "--version"], text=True).strip() != f"ALOL {version}":
        raise ValueError("Binary version differs from source")
    args.output.mkdir(parents=True, exist_ok=True)
    output = args.output / f"alol-{version}-linux-x86_64"
    shutil.copyfile(binary, output)
    output.chmod(0o755)
    for name in ("LICENSE", "NOTICE"):
        destination = args.output / name
        if destination.resolve() != (ROOT / name).resolve():
            shutil.copyfile(ROOT / name, destination)
    print(f"Standalone executable: {output.name}; LICENSE and NOTICE included")


if __name__ == "__main__":
    main()
