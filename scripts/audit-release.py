#!/usr/bin/env python3
"""Audit exactly the files published in source and pacman release archives."""
import argparse
import hashlib
from pathlib import Path
import re
import subprocess
import tarfile
import tomllib

from importlib.util import module_from_spec, spec_from_file_location

ROOT = Path(__file__).resolve().parents[1]
spec = spec_from_file_location("alol_source", ROOT / "scripts/package-source.py")
source = module_from_spec(spec)
spec.loader.exec_module(source)
EMAIL = re.compile(rb"[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}")


def audit_bytes(name, data):
    if name.endswith(".png"):
        source.audit_png(name, data)
    if source.SECRETS.search(data) or EMAIL.search(data):
        raise ValueError(f"Potential private data in {name} (value hidden)")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path, nargs="?", default=ROOT / "dist")
    args = parser.parse_args()
    metadata = tomllib.loads((ROOT / "Cargo.toml").read_text())["package"]
    version = metadata["version"]
    files = source.source_files()
    if not source.SCREENSHOTS.issubset({path.relative_to(ROOT).as_posix() for path in files}):
        raise ValueError("Capture all three demo screenshots before publishing")
    if (ROOT / ".git").exists():
        tracked = subprocess.check_output(["git", "ls-files", "-z"], cwd=ROOT).decode().split("\0")
        expected_paths = {path.relative_to(ROOT).as_posix() for path in files}
        if set(filter(None, tracked)) != expected_paths:
            raise ValueError("Git contains files outside the audited source manifest")
    for path in files:
        audit_bytes(str(path.relative_to(ROOT)), path.read_bytes())
    archive = args.directory / f"alol-{version}.tar.gz"
    expected = {f"alol-{version}/{path.relative_to(ROOT).as_posix()}" for path in files}
    with tarfile.open(archive) as tar:
        members = tar.getmembers()
        if {member.name for member in members} != expected or len(members) != len(expected):
            raise ValueError("Unexpected or duplicate files in the source archive")
        for member in members:
            if not member.isfile() or member.uid or member.gid or member.uname or member.gname:
                raise ValueError("Unsafe source metadata")
            data = tar.extractfile(member).read()
            audit_bytes(member.name, data)
            local = ROOT / member.name.split("/", 1)[1]
            if data != local.read_bytes():
                raise ValueError(f"Source archive differs from checkout: {member.name}")
    packages = sorted(args.directory.glob(f"alol-{version}-*-x86_64.pkg.tar.zst"))
    if len(packages) != 1:
        raise ValueError("Expected one x86_64 pacman package")
    package = packages[0]
    names = subprocess.check_output(["bsdtar", "-tf", str(package)], text=True).splitlines()
    allowed = {
        ".PKGINFO", ".MTREE", "usr/bin/alol",
        "usr/share/applications/alol.desktop", "usr/share/icons/hicolor/scalable/apps/alol.svg",
        "usr/share/licenses/alol/LICENSE", "usr/share/doc/alol/README.md",
    }
    actual = {name.removeprefix("./") for name in names if not name.endswith("/")}
    if actual != allowed:
        raise ValueError("Unexpected files in pacman package: " + ", ".join(sorted(actual ^ allowed)))
    for name in sorted(actual - {".MTREE"}):
        data = subprocess.check_output(["bsdtar", "-xOf", str(package), name])
        audit_bytes(name, data)
        if name == ".PKGINFO" and b"packager = ALOL contributors\n" not in data:
            raise ValueError("Package author is not anonymous")
    pkgbuild = args.directory / "PKGBUILD"
    data = pkgbuild.read_bytes()
    audit_bytes("PKGBUILD", data)
    source_hash = hashlib.sha256(archive.read_bytes()).hexdigest().encode()
    if source_hash not in data or b"@VERSION@" in data or b"@SHA256@" in data:
        raise ValueError("PKGBUILD source checksum was not generated correctly")
    artifacts = [archive, package, pkgbuild, *[ROOT / name for name in sorted(source.SCREENSHOTS)]]
    (args.directory / "SHA256SUMS").write_text("".join(
        f"{hashlib.sha256(path.read_bytes()).hexdigest()}  {path.name}\n" for path in artifacts
    ))
    print(f"Privacy audit passed: {len(files)} source files and {len(allowed)} package files; SHA256SUMS written.")


if __name__ == "__main__":
    main()
