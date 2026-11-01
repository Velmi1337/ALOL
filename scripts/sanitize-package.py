#!/usr/bin/env python3
"""Remove the builder's installed-software inventory from a pacman package."""
import argparse
import gzip
import os
from pathlib import Path
import subprocess
import tempfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("package", type=Path)
    args = parser.parse_args()
    package = args.package.resolve()
    allowed = {
        ".PKGINFO", ".BUILDINFO", ".MTREE", "usr/bin/alol",
        "usr/share/applications/alol.desktop", "usr/share/icons/hicolor/scalable/apps/alol.svg",
        "usr/share/icons/hicolor/256x256/apps/alol.png",
        "usr/share/licenses/alol/LICENSE", "usr/share/doc/alol/README.md",
    }
    names = subprocess.check_output(["bsdtar", "-tf", str(package)], text=True).splitlines()
    files = [name.removeprefix("./") for name in names if not name.endswith("/")]
    if len(files) != len(set(files)) or set(files) != allowed:
        raise ValueError("Refusing to repackage unexpected contents")
    with tempfile.TemporaryDirectory(prefix="alol-public-package-", dir="/tmp") as directory:
        root = Path(directory)
        # Read bytes only: never extract archive links, owners, or absolute paths.
        for name in sorted(allowed - {".BUILDINFO", ".MTREE"}):
            target = root / name
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(subprocess.check_output(["bsdtar", "-xOf", str(package), name]))
            target.chmod(0o755 if name == "usr/bin/alol" else 0o644)
        pkginfo = (root / ".PKGINFO").read_text()
        timestamp = int(next(line.split(" = ", 1)[1] for line in pkginfo.splitlines() if line.startswith("builddate = ")))
        for path in root.rglob("*"):
            if path.is_dir():
                path.chmod(0o755)
            os.utime(path, (timestamp, timestamp))
        entries = sorted(path.relative_to(root).as_posix() for path in root.rglob("*"))
        listing = b"\0".join(name.encode() for name in entries) + b"\0"
        mtree = subprocess.check_output([
            "bsdtar", "-cnf", "-", "--format=mtree", "--uid", "0", "--gid", "0",
            "--options=!all,use-set,type,uid,gid,mode,time,size,sha256,link",
            "--null", "--no-recursion", "--files-from", "-",
        ], input=listing, cwd=root)
        (root / ".MTREE").write_bytes(gzip.compress(mtree, mtime=0))
        (root / ".MTREE").chmod(0o644)
        os.utime(root / ".MTREE", (timestamp, timestamp))
        entries = [".PKGINFO", ".MTREE", *[name for name in entries if name != ".PKGINFO"]]
        listing = b"\0".join(name.encode() for name in entries) + b"\0"
        temporary = package.with_name(package.name + ".public.tmp")
        try:
            subprocess.run([
                "bsdtar", "--zstd", "-cf", str(temporary), "--uid", "0", "--gid", "0",
                "--uname", "root", "--gname", "root", "--no-fflags",
                "--null", "--no-recursion", "--files-from", "-",
            ], input=listing, cwd=root, check=True)
            os.replace(temporary, package)
        finally:
            temporary.unlink(missing_ok=True)
    print("Package metadata normalized; host software inventory removed.")


if __name__ == "__main__":
    main()
