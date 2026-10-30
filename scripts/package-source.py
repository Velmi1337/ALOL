#!/usr/bin/env python3
"""Create source + PKGBUILD from an explicit file list, never from a user's home."""
import argparse
import gzip
import hashlib
import io
from pathlib import Path
import re
import shutil
import struct
import tarfile
import tomllib
import zlib

ROOT = Path(__file__).resolve().parents[1]
TOP_LEVEL = (
    "Cargo.toml", "Cargo.lock", "LICENSE", "README.md", "CHANGELOG.md",
    ".gitignore", ".gitattributes", "install.sh", "install-arch.sh", "install-fedora.sh",
)
DIRECTORIES = ("src", "assets", "docs", "tests", "packaging", "scripts", ".github")
EXTENSIONS = {".rs", ".py", ".sh", ".md", ".json", ".svg", ".desktop", ".in", ".png", ".yml"}
SCREENSHOTS = {"docs/screenshots/main.png", "docs/screenshots/settings.png", "docs/screenshots/mods.png"}
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
                if path.suffix == ".png" and relative.as_posix() not in SCREENSHOTS:
                    raise ValueError(f"Unexpected screenshot: {relative}")
                paths.append(path)
    for path in paths:
        if path.is_symlink() or not path.is_file():
            raise ValueError(f"Missing or unsafe source: {path.relative_to(ROOT)}")
    return sorted(paths, key=lambda p: p.relative_to(ROOT).as_posix())


def audit_png(name, data):
    """Accept only simple, complete Cairo PNGs without author/EXIF/text chunks."""
    if not data.startswith(b"\x89PNG\r\n\x1a\n") or len(data) > 8 * 1024 * 1024:
        raise ValueError(f"Invalid screenshot: {name}")
    offset, header, pixels, ended = 8, None, bytearray(), False
    allowed = {b"IHDR", b"IDAT", b"IEND", b"bKGD", b"sRGB", b"gAMA", b"cHRM"}
    while offset + 12 <= len(data):
        length = struct.unpack_from(">I", data, offset)[0]
        kind = data[offset + 4:offset + 8]
        content = data[offset + 8:offset + 8 + length]
        end = offset + 12 + length
        if kind not in allowed or end > len(data):
            raise ValueError(f"Unexpected PNG metadata in {name}")
        if zlib.crc32(kind + content) != struct.unpack_from(">I", data, offset + 8 + length)[0]:
            raise ValueError(f"PNG checksum mismatch: {name}")
        if header is None and kind != b"IHDR":
            raise ValueError(f"PNG header missing: {name}")
        if kind == b"IHDR":
            if header is not None or length != 13:
                raise ValueError(f"Duplicate or invalid PNG header: {name}")
            header = struct.unpack(">IIBBBBB", content)
        elif kind == b"IDAT":
            pixels.extend(content)
        elif kind == b"IEND":
            ended = length == 0 and end == len(data)
            break
        offset = end
    if not ended or header is None:
        raise ValueError(f"Incomplete screenshot: {name}")
    width, height, depth, color, compression, filtering, interlace = header
    if not (640 <= width <= 4096 and 480 <= height <= 4096 and depth == 8
            and color in (2, 6) and (compression, filtering, interlace) == (0, 0, 0)):
        raise ValueError(f"Unsupported screenshot dimensions/format: {name}")
    expected = height * (1 + width * (3 if color == 2 else 4))
    decoder = zlib.decompressobj()
    raw = decoder.decompress(pixels, expected + 1)
    if len(raw) != expected or not decoder.eof or decoder.unused_data:
        raise ValueError(f"Invalid screenshot pixel data: {name}")
    return width, height


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path)
    parser.add_argument("--archive", type=Path, help="Use an already-published, byte-verified source archive")
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
        if path.suffix == ".png":
            audit_png(path.relative_to(ROOT).as_posix(), data)
        if SECRETS.search(data):
            raise ValueError(f"Potential private data: {path.relative_to(ROOT)} (value hidden)")
        contents.append((path, data))
    if args.archive:
        expected = {f"alol-{version}/{path.relative_to(ROOT).as_posix()}": data for path, data in contents}
        with tarfile.open(args.archive) as tar:
            members = tar.getmembers()
            if len(members) != len(expected) or {m.name for m in members} != set(expected):
                raise ValueError("Published source archive has unexpected files")
            for member in members:
                if (not member.isfile() or member.uid or member.gid or member.uname or member.gname
                        or tar.extractfile(member).read() != expected[member.name]):
                    raise ValueError("Published source archive differs from checked-out source")
        shutil.copyfile(args.archive, archive)
    else:
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
    template = (ROOT / "packaging/fedora/alol.spec.in").read_text()
    spec = template.replace("@VERSION@", version).replace("@REPOSITORY@", repository).replace("@SHA256@", digest)
    (args.output / "alol.spec").write_text(spec)
    print(f"Source: {archive.name}; {len(files)} files; SHA256 {digest}")


if __name__ == "__main__":
    main()
