#!/usr/bin/env python3
"""Verify the native CI RPM and its checksum manifest without RPM tools on Arch."""
import argparse
import hashlib
from pathlib import Path
import re
try:
    import tomllib
except ModuleNotFoundError:
    import tomli as tomllib

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path)
    parser.add_argument("--archive", type=Path)
    args = parser.parse_args()
    version = tomllib.loads((ROOT / "Cargo.toml").read_text())["package"]["version"]
    packages = sorted(args.directory.glob(f"alol-{version}-1.fc*.x86_64.rpm"))
    if len(packages) != 1 or not re.fullmatch(rf"alol-{re.escape(version)}-1\.fc[0-9]+\.x86_64\.rpm", packages[0].name):
        raise ValueError("Expected one native Fedora release RPM")
    package = packages[0]
    if package.read_bytes()[:4] != b"\xed\xab\xee\xdb":
        raise ValueError("Invalid RPM signature")
    pngs = {name: ROOT / "docs/screenshots" / name for name in ("main.png", "settings.png", "mods.png")}
    expected = {package.name, "alol.spec", f"alol-{version}.tar.gz", "LICENSE", "NOTICE", *pngs}
    checksums = {}
    for line in (args.directory / "SHA256SUMS.fedora").read_text().splitlines():
        digest, name = line.split("  ", 1)
        if name not in expected or name in checksums or not re.fullmatch(r"[a-f0-9]{64}", digest):
            raise ValueError("Unexpected Fedora checksum entry")
        checksums[name] = digest
    if set(checksums) != expected:
        raise ValueError("Incomplete Fedora release checksums")
    paths = {package.name: package, "alol.spec": args.directory / "alol.spec",
             "LICENSE": ROOT / "LICENSE", "NOTICE": ROOT / "NOTICE", **pngs}
    archive = args.archive or args.directory / f"alol-{version}.tar.gz"
    if not archive.is_file():
        raise ValueError("Released source archive is required for verification")
    paths[f"alol-{version}.tar.gz"] = archive
    for name, path in paths.items():
        if hashlib.sha256(path.read_bytes()).hexdigest() != checksums[name]:
            raise ValueError(f"Fedora release checksum mismatch: {name}")
    source_hash = checksums[f"alol-{version}.tar.gz"]
    spec = (args.directory / "alol.spec").read_text()
    if source_hash not in spec or f"Version:        {version}\n" not in spec:
        raise ValueError("Fedora spec does not match released source")
    print("Native Fedora release assets and source checksum verified.")


if __name__ == "__main__":
    main()
