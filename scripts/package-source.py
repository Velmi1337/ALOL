#!/usr/bin/env python3
"""Create source + PKGBUILD from an explicit file list, never from a user's home."""
import argparse
import gzip
import hashlib
import io
from pathlib import Path
import re
import tarfile
import tomllib

ROOT = Path(__file__).resolve().parents[1]
TOP_LEVEL = (
    "Cargo.toml", "Cargo.lock", "LICENSE", "README.md", "CHANGELOG.md",
    ".gitignore", ".gitattributes", "install.sh", "install-arch.sh",
)
DIRECTORIES = ("src", "assets", "docs", "tests", "packaging", "scripts")
EXTENSIONS = {".rs", ".py", ".sh", ".md", ".json", ".svg", ".desktop", ".in"}
SECRETS = re.compile(rb"(?:/home/[A-Za-z0-9_.-]+|/Users/[A-Za-z0-9_.-]+|gh[pousr]_[A-Za-z0-9_]{20,}|github_pat_[A-Za-z0-9_]{20,}|AKIA[0-9A-Z]{16}|-----BEGIN (?:RSA |OPENSSH |EC )?PRIVATE KEY-----)")


def source_files():
    paths = [ROOT / name for name in TOP_LEVEL]
    for directory in DIRECTORIES:
        for path in (ROOT / directory).rglob("*"):
            relative = path.relative_to(ROOT)
            if "__pycache__" in relative.parts:
                continue
            if path.is_symlink():
                raise ValueError(f"Source symlinks are not allowed: {relative}")
            if path.is_file():
                if path.suffix not in EXTENSIONS:
                    raise ValueError(f"Unexpected source file: {relative}")
                paths.append(path)
    for path in paths:
        if path.is_symlink() or not path.is_file():
            raise ValueError(f"Missing or unsafe source: {path.relative_to(ROOT)}")
    return sorted(paths, key=lambda p: p.relative_to(ROOT).as_posix())


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    metadata = tomllib.loads((ROOT / "Cargo.toml").read_text())["package"]
    version, repository = metadata["version"], metadata["repository"]
    if not re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+", version):
        raise ValueError("Invalid package version")
    if not re.fullmatch(r"https://github\.com/[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", repository):
        raise ValueError("Invalid repository URL")
    args.output.mkdir(parents=True, exist_ok=True)
    archive = args.output / f"alol-{version}.tar.gz"
    files = source_files()
    contents = []
    for path in files:
        data = path.read_bytes()
        if SECRETS.search(data):
            raise ValueError(f"Potential private data: {path.relative_to(ROOT)} (value hidden)")
        contents.append((path, data))
    with archive.open("wb") as raw, gzip.GzipFile(fileobj=raw, mode="wb", filename="", mtime=0) as zipped:
        with tarfile.open(fileobj=zipped, mode="w", format=tarfile.PAX_FORMAT) as tar:
            for path, data in contents:
                entry = tarfile.TarInfo(f"alol-{version}/{path.relative_to(ROOT).as_posix()}")
                entry.size = len(data)
                entry.mode = 0o755 if path.suffix == ".sh" else 0o644
                entry.uid = entry.gid = 0
                entry.uname = entry.gname = ""
                entry.mtime = 0
                tar.addfile(entry, io.BytesIO(data))
    digest = hashlib.sha256(archive.read_bytes()).hexdigest()
    template = (ROOT / "packaging/arch/PKGBUILD.in").read_text()
    pkgbuild = template.replace("@VERSION@", version).replace("@REPOSITORY@", repository).replace("@SHA256@", digest)
    (args.output / "PKGBUILD").write_text(pkgbuild)
    print(f"Source: {archive.name}; {len(files)} files; SHA256 {digest}")


if __name__ == "__main__":
    main()
