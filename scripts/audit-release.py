#!/usr/bin/env python3
"""Audit exactly the files published in source and pacman release archives."""
import argparse
import hashlib
from pathlib import Path
import re
import stat
import subprocess
import tarfile
try:
    import tomllib
except ModuleNotFoundError:
    import tomli as tomllib

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


def audit_fedora(directory, version):
    packages = sorted(directory.glob(f"alol-{version}-*.x86_64.rpm"))
    if len(packages) != 1:
        raise ValueError("Expected one native Fedora x86_64 RPM")
    package = packages[0]
    query = "%{NAME}\n%{VERSION}\n%{ARCH}\n%{PACKAGER}\n%{BUILDHOST}\n%{RELEASE}\n%{LICENSE}\n"
    fields = subprocess.check_output(["rpm", "-qp", "--qf", query, str(package)], text=True).splitlines()
    if (fields[:5] != ["alol", version, "x86_64", "ALOL contributors", "alol-builder"]
            or not re.fullmatch(r"1\.fc[0-9]+", fields[5]) or fields[6] != "Apache-2.0 AND OFL-1.1"):
        raise ValueError("Unexpected RPM identity or private build metadata")
    info = subprocess.check_output([
        "rpm", "-qp", "--qf", "[%{FILENAMES}\t%{FILEMODES:octal}\t%{FILEUSERNAME}\t%{FILEGROUPNAME}\n]", str(package)
    ], text=True)
    regular = set()
    for line in info.splitlines():
        name, mode, user, group = line.split("\t")
        if (user, group) != ("root", "root"):
            raise ValueError("Private RPM file owner metadata")
        mode = int(mode, 8)
        if stat.S_ISREG(mode):
            regular.add(name.removeprefix("/"))
        elif not stat.S_ISDIR(mode):
            raise ValueError("Unexpected RPM link or special file")
    expected = {
        "usr/bin/alol", "usr/share/applications/alol.desktop",
        "usr/share/icons/hicolor/scalable/apps/alol.svg",
        "usr/share/icons/hicolor/256x256/apps/alol.png",
        "usr/share/licenses/alol/LICENSE", "usr/share/licenses/alol/NOTICE",
        "usr/share/doc/alol/README.md",
    }
    if regular != expected:
        raise ValueError("Unexpected payload in RPM")
    names = subprocess.check_output(["bsdtar", "-tf", str(package)], text=True).splitlines()
    paths = {name.removeprefix("./"): name for name in names}
    for name in expected:
        data = subprocess.check_output(["bsdtar", "-xOf", str(package), paths[name]])
        audit_bytes(name, data)
        if name.startswith("usr/share/licenses/alol/") and data != (ROOT / Path(name).name).read_bytes():
            raise ValueError("RPM license/notice differs from source")
    return package


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path, nargs="?", default=ROOT / "dist")
    parser.add_argument("--platform", choices=("arch", "fedora"), default="arch")
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
    source_hash = hashlib.sha256(archive.read_bytes()).hexdigest().encode()
    spec = args.directory / "alol.spec"
    data = spec.read_bytes()
    audit_bytes("alol.spec", data)
    if source_hash not in data or b"@VERSION@" in data or b"@SHA256@" in data:
        raise ValueError("RPM spec source checksum was not generated correctly")
    artifacts = [archive, spec]
    if args.platform == "fedora":
        artifacts.append(audit_fedora(args.directory, version))
    else:
        binary = args.directory / f"alol-{version}-linux-x86_64"
        data = binary.read_bytes()
        if data[:6] != b"\x7fELF\x02\x01" or data[18:20] != b"\x3e\x00":
            raise ValueError("Standalone executable is not ELF x86_64")
        audit_bytes(binary.name, data)
        if subprocess.check_output([str(binary.resolve()), "--version"], text=True).strip() != f"ALOL {version}":
            raise ValueError("Standalone executable has the wrong version")
        artifacts.append(binary)
        packages = sorted(args.directory.glob(f"alol-{version}-*-x86_64.pkg.tar.zst"))
        if len(packages) != 1:
            raise ValueError("Expected one x86_64 pacman package")
        package = packages[0]
        names = subprocess.check_output(["bsdtar", "-tf", str(package)], text=True).splitlines()
        allowed = {
            ".PKGINFO", ".MTREE", "usr/bin/alol",
            "usr/share/applications/alol.desktop", "usr/share/icons/hicolor/scalable/apps/alol.svg",
            "usr/share/icons/hicolor/256x256/apps/alol.png",
            "usr/share/licenses/alol/LICENSE", "usr/share/licenses/alol/NOTICE",
            "usr/share/doc/alol/README.md",
        }
        actual = {name.removeprefix("./") for name in names if not name.endswith("/")}
        if actual != allowed:
            raise ValueError("Unexpected files in pacman package: " + ", ".join(sorted(actual ^ allowed)))
        for name in sorted(actual - {".MTREE"}):
            data = subprocess.check_output(["bsdtar", "-xOf", str(package), name])
            audit_bytes(name, data)
            if name == ".PKGINFO" and b"packager = ALOL contributors\n" not in data:
                raise ValueError("Package author is not anonymous")
            if name == ".PKGINFO" and not all(value in data for value in (b"license = Apache-2.0\n", b"license = OFL-1.1\n")):
                raise ValueError("Package must list Apache-2.0 and OFL-1.1")
            if name.startswith("usr/share/licenses/alol/") and data != (ROOT / Path(name).name).read_bytes():
                raise ValueError("Package license/notice differs from source")
        pkgbuild = args.directory / "PKGBUILD"
        data = pkgbuild.read_bytes()
        audit_bytes("PKGBUILD", data)
        if source_hash not in data or b"@VERSION@" in data or b"@SHA256@" in data:
            raise ValueError("PKGBUILD source checksum was not generated correctly")
        artifacts.extend([package, pkgbuild])
    artifacts.extend(ROOT / name for name in ("LICENSE", "NOTICE"))
    if args.platform != "fedora":
        runtime_spec = spec_from_file_location("alol_runtime_audit", ROOT / "scripts/audit-runtime.py")
        runtime_audit = module_from_spec(runtime_spec)
        runtime_spec.loader.exec_module(runtime_audit)
        runtime_artifact = runtime_audit.audit(args.directory)
        if runtime_artifact:
            artifacts.append(runtime_artifact)
    artifacts.extend(ROOT / name for name in sorted(source.SCREENSHOTS))
    manifest = "SHA256SUMS.fedora" if args.platform == "fedora" else "SHA256SUMS"
    (args.directory / manifest).write_text("".join(
        f"{hashlib.sha256(path.read_bytes()).hexdigest()}  {path.name}\n" for path in artifacts
    ))
    print(f"Privacy audit passed: {len(files)} source files and {args.platform} package; {manifest} written.")


if __name__ == "__main__":
    main()
