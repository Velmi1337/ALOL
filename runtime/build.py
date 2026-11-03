#!/usr/bin/env python3
"""Assemble an experimental Proton distribution from an exact verified GE release.

This assembler accepts a verified user32 overlay produced by build_keyboard.py;
the default recipe keeps the upstream engine unchanged.
No prefix, game, system installation or existing Proton tree is modified.
"""
import argparse
import fcntl
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import platform
import posixpath
import re
import shutil
import signal
import subprocess
import sys
import tarfile
import tempfile


def sha256(path):
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def validate(recipe):
    for key in ("runtime_name", "profile_version", "upstream_root", "base_tag", "base_filename"):
        if not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9_.-]{0,120}", recipe[key]):
            raise ValueError("Invalid runtime recipe name")
    if not re.fullmatch(r"[0-9a-f]{64}", recipe["base_sha256"]):
        raise ValueError("Invalid base SHA256")
    if not isinstance(recipe["base_size"], int) or not 1 <= recipe["base_size"] <= 2 * 1024**3:
        raise ValueError("Invalid base size")
    if not recipe["base_url"].startswith("https://github.com/GloriousEggroll/proton-ge-custom/releases/download/"):
        raise ValueError("Unexpected upstream download location")
    if recipe["architecture"] != "x86_64" or platform.machine() not in ("x86_64", "amd64"):
        raise ValueError("This experimental recipe currently supports x86_64 only")
    if recipe["engine_patched"] is True:
        files = recipe.get("engine_files", {})
        expected = {f"files/lib/wine/{arch}-windows/user32.dll" for arch in ("i386", "x86_64")}
        if set(files) != expected or any(not re.fullmatch(r"[0-9a-f]{64}", value) for value in files.values()):
            raise ValueError("Invalid keyboard engine overlay")
        if not re.fullmatch(r"[0-9a-f]{64}", recipe.get("source_bundle_sha256", "")):
            raise ValueError("The keyboard engine requires its complete source bundle")
        if "overlay_url" in recipe:
            if (recipe["overlay_url"] != "https://github.com/Velmi1337/ALOL/releases/download/v0.5.8/" + recipe.get("overlay_filename", "")
                    or not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9_.-]{0,120}\.tar\.gz", recipe.get("overlay_filename", ""))
                    or not re.fullmatch(r"[0-9a-f]{64}", recipe.get("overlay_sha256", ""))
                    or not isinstance(recipe.get("overlay_size"), int) or not 1 <= recipe["overlay_size"] <= 256 * 1024**2):
                raise ValueError("Invalid pinned keyboard overlay download")
    elif recipe["engine_patched"] is not False:
        raise ValueError("Invalid engine_patched value")


def ready(directory, resources):
    try:
        marker = json.loads((directory / "alol-runtime.json").read_text())
        if resources["recipe"]["engine_patched"]:
            if (marker.get("keyboard_patch", {}).get("protocol") != "xkb-state-v1"
                    or sha256(directory / "ALOL-WINE-SOURCE.tar.gz") != resources["recipe"]["source_bundle_sha256"]
                    or any(sha256(directory / name) != value for name, value
                           in resources["recipe"]["engine_files"].items())):
                return False
        return (not directory.is_symlink() and marker["recipe"] == resources["recipe"]
                and (directory / "toolmanifest.vdf").is_file()
                and (directory / "files/bin/wine").is_file()
                and sha256(directory / "compatibilitytool.vdf") == marker["tool_vdf_sha256"]
                and sha256(directory / "proton") == hashlib.sha256(resources["wrapper"].encode()).hexdigest()
                and sha256(directory / "proton.alol-upstream") == marker["upstream_proton_sha256"])
    except (OSError, ValueError, KeyError):
        return False


def download(recipe, cache):
    cache.mkdir(parents=True, exist_ok=True)
    archive = cache / recipe["base_filename"]
    partial = archive.with_name(archive.name + ".part")
    if archive.is_symlink() or partial.is_symlink():
        raise ValueError("The runtime cache must not contain symbolic links")
    if archive.is_file():
        if archive.stat().st_size == recipe["base_size"] and sha256(archive) == recipe["base_sha256"]:
            return archive
        raise ValueError("Cached upstream archive failed verification; it has been preserved")
    print(f"Загрузка {recipe['base_tag']}: {recipe['base_size'] // (1024**2)} МБ. Проверка SHA256 обязательна.", flush=True)
    subprocess.run([
        "curl", "--fail", "--location", "--silent", "--show-error", "--connect-timeout", "10",
        "--max-time", "1800", "--retry", "2", "--proto", "=https", "--proto-redir", "=https",
        "--max-filesize", str(recipe["base_size"]), "--output", str(partial), recipe["base_url"],
    ], check=True)
    if partial.stat().st_size != recipe["base_size"] or sha256(partial) != recipe["base_sha256"]:
        raise ValueError("Downloaded base failed size/SHA256 verification; no runtime was activated")
    partial.replace(archive)
    return archive


def extract(archive, destination, recipe):
    if not hasattr(tarfile, "data_filter"):
        raise ValueError("Python's safe tar extraction filter is required; update Python")
    with tarfile.open(archive, "r:gz") as stream:
        members = stream.getmembers()
        if len(members) > 100000 or sum(max(0, m.size) for m in members) > 8 * 1024**3:
            raise ValueError("Upstream archive exceeds extraction limits")
        seen = set()
        for member in members:
            path = PurePosixPath(member.name)
            if (path.is_absolute() or ".." in path.parts or not path.parts
                    or path.parts[0] != recipe["upstream_root"] or member.name in seen):
                raise ValueError("Unexpected or duplicate path in base archive")
            if not (member.isfile() or member.isdir() or member.issym() or member.islnk()):
                raise ValueError("Special files in base archive are not allowed")
            if member.issym() or member.islnk():
                link = PurePosixPath(member.linkname)
                resolved = PurePosixPath(posixpath.normpath(str(link if member.islnk() else path.parent / link)))
                if link.is_absolute() or not resolved.parts or resolved.parts[0] != recipe["upstream_root"] or ".." in resolved.parts:
                    raise ValueError("Base archive link leaves the Proton tree")
            seen.add(member.name)
        # Unlike mod archives, upstream Proton needs internal relative symlinks.
        # Python's data filter refuses links escaping the temporary extraction tree.
        stream.extractall(destination, members=members, filter="data")


def verify_overlay(archive, recipe):
    """Inspect exactly two DLLs and their complete corresponding source archive."""
    expected = dict(recipe["engine_files"], **{"ALOL-WINE-SOURCE.tar.gz": recipe["source_bundle_sha256"]})
    with tarfile.open(archive, "r:gz") as stream:
        members = stream.getmembers()
        if len(members) != len(expected) or {member.name for member in members} != set(expected):
            raise ValueError("Unexpected files in keyboard overlay")
        for member in members:
            limit = 128 * 1024**2 if member.name.endswith(".tar.gz") else 16 * 1024**2
            if not member.isfile() or not 1 <= member.size <= limit:
                raise ValueError("Invalid keyboard overlay member")
            digest = hashlib.sha256()
            with stream.extractfile(member) as source:
                for chunk in iter(lambda: source.read(1024 * 1024), b""):
                    digest.update(chunk)
            if digest.hexdigest() != expected[member.name]:
                raise ValueError("Keyboard overlay member failed SHA256 verification")


def downloaded_overlay(recipe, cache, destination):
    if "overlay_url" not in recipe:
        raise ValueError("Missing compiled keyboard DLLs or pinned download recipe")
    archive = download({"base_tag": "исправления ALOL-Proton", "base_filename": recipe["overlay_filename"],
                        "base_size": recipe["overlay_size"], "base_sha256": recipe["overlay_sha256"],
                        "base_url": recipe["overlay_url"]}, cache)
    verify_overlay(archive, recipe)
    with tarfile.open(archive, "r:gz") as stream:
        for member in stream.getmembers():
            target = destination / member.name
            target.parent.mkdir(parents=True, exist_ok=True)
            with stream.extractfile(member) as source, target.open("xb") as output:
                shutil.copyfileobj(source, output)
    return {"engine_overlay": {name: str(destination / name) for name in recipe["engine_files"]},
            "source_bundle": str(destination / "ALOL-WINE-SOURCE.tar.gz")}


def build(resources, output_root, cache, archive=None):
    recipe = resources["recipe"]
    validate(recipe)
    output_root.mkdir(parents=True, exist_ok=True)
    target = output_root / recipe["runtime_name"]
    with (output_root / ".build.lock").open("a+b") as lock:
        try:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            raise ValueError("Another ALOL-Proton build is running") from None
        if ready(target, resources):
            print(f"ALOL-Proton {recipe['profile_version']} уже подготовлен.", flush=True)
            return target
        if target.exists() or target.is_symlink():
            raise ValueError("The versioned runtime directory already exists but failed verification; it was not replaced")
        if archive is None:
            archive = download(recipe, cache)
        if not archive.is_file() or archive.is_symlink():
            raise ValueError("Choose a regular upstream archive")
        print("Проверка исходного архива…", flush=True)
        if archive.stat().st_size != recipe["base_size"] or sha256(archive) != recipe["base_sha256"]:
            raise ValueError("Upstream archive size/SHA256 mismatch; existing runtimes are unchanged")
        if shutil.disk_usage(output_root).free < 4 * 1024**3:
            raise ValueError("At least 4 GB free space is required to prepare Proton")
        print("Создание отдельного ALOL-Proton. Оригинальный GE-Proton не изменяется…", flush=True)
        with tempfile.TemporaryDirectory(prefix=".build-", dir=output_root) as temp:
            stage = Path(temp)
            if recipe["engine_patched"] and not resources.get("engine_overlay"):
                resources = dict(resources, **downloaded_overlay(recipe, cache, stage / "overlay"))
            extract(archive, stage, recipe)
            root = stage / recipe["upstream_root"]
            for name in ("proton", "version", "toolmanifest.vdf", "compatibilitytool.vdf", "files/bin/wine"):
                if not (root / name).is_file():
                    raise ValueError(f"Upstream is missing required file: {name}")
            if (root / "proton").is_symlink() or (root / "proton.alol-upstream").exists():
                raise ValueError("Unexpected upstream entry point")
            upstream_hash = sha256(root / "proton")
            (root / "proton").replace(root / "proton.alol-upstream")
            (root / "proton").write_text(resources["wrapper"], encoding="utf-8")
            (root / "proton").chmod(0o755)
            if recipe["engine_patched"]:
                overlay = resources.get("engine_overlay", {})
                if set(overlay) != set(recipe["engine_files"]):
                    raise ValueError("Missing compiled keyboard DLLs")
                for name, expected in recipe["engine_files"].items():
                    source = Path(overlay[name])
                    if not source.is_file() or source.is_symlink() or sha256(source) != expected:
                        raise ValueError("Compiled keyboard DLL failed verification")
                    destination = root / name
                    if destination.is_symlink() or not destination.is_file():
                        raise ValueError("Unexpected upstream keyboard DLL")
                    destination.replace(destination.with_name("user32.alol-upstream.dll"))
                    shutil.copyfile(source, destination)
                    destination.chmod(0o644)
                sources = Path(resources["source_bundle"])
                if sources.is_symlink() or sha256(sources) != recipe["source_bundle_sha256"]:
                    raise ValueError("Keyboard engine source bundle failed verification")
                shutil.copyfile(sources, root / "ALOL-WINE-SOURCE.tar.gz")
            # Preserve the upstream version file: it controls prefix migrations.
            (root / "compatibilitytool.vdf").write_text(
                '"compatibilitytools"\n{\n  "compat_tools"\n  {\n    "' + recipe["runtime_name"] + '"\n    {\n'
                '      "install_path" "."\n      "display_name" "ALOL-Proton ' + recipe["profile_version"]
                + ' experimental (' + recipe["base_tag"] + ')"\n      "from_oslist" "windows"\n'
                '      "to_oslist" "linux"\n    }\n  }\n}\n')
            (root / "ALOL-LICENSE").write_text(resources["license"], encoding="utf-8")
            (root / "ALOL-NOTICE").write_text(resources["notice"] + "\n\n"
                "This experimental distribution is based on "
                + recipe["base_tag"] + ". Their licenses and notices are retained.\n"
                "Source: " + recipe["source_url"] + "\n"
                "Modified files: proton (launch layer), compatibilitytool.vdf (tool registration).\n"
                "Original entry point: proton.alol-upstream.\n" + (
                    "Wine user32.dll is rebuilt for the XKB indicator bridge (LGPL-2.1-or-later).\n"
                    "Modified Wine files: dlls/user32/input.c, dlls/user32/user32.spec.\n"
                    "Complete corresponding patched Wine source, LGPL license, patch and rebuild instructions: ALOL-WINE-SOURCE.tar.gz.\n"
                    "DXVK and all other GE engine files are unchanged. Original user32 DLLs: user32.alol-upstream.dll.\n"
                    if recipe["engine_patched"] else "No Wine/DXVK source patch is included.\n"), encoding="utf-8")
            marker = {"recipe": recipe, "wrapper_sha256": sha256(root / "proton"),
                      "upstream_proton_sha256": upstream_hash,
                      "tool_vdf_sha256": sha256(root / "compatibilitytool.vdf")}
            if recipe["engine_patched"]:
                marker["keyboard_patch"] = {"protocol": "xkb-state-v1", "license": "LGPL-2.1-or-later"}
            (root / "alol-runtime.json").write_text(json.dumps(marker, ensure_ascii=False, sort_keys=True, indent=2) + "\n")
            if not ready(root, resources):
                raise ValueError("Assembled runtime failed verification")
            os.rename(root, target)
        print(f"Готов ALOL-Proton {recipe['profile_version']} на базе {recipe['base_tag']}. Прирост FPS пока не измерен.", flush=True)
        return target


def resources_from_repo():
    here = Path(__file__).resolve().parent
    return {"recipe": json.loads((here / "base.json").read_text()),
            "wrapper": (here / "proton_wrapper.py").read_text(),
            "license": (here.parent / "LICENSE").read_text(), "notice": (here.parent / "NOTICE").read_text()}


def main():
    # GTK's Stop signals the whole worker group. Raise through TemporaryDirectory
    # so a normal cancellation cleans the unfinished extraction before exiting.
    signal.signal(signal.SIGTERM, lambda *_: sys.exit(143))
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output-root", type=Path, required=True)
    parser.add_argument("--cache", type=Path, required=True)
    parser.add_argument("--archive", type=Path)
    parser.add_argument("--embedded", action="store_true", help=argparse.SUPPRESS)
    args = parser.parse_args()
    resources = json.load(sys.stdin) if args.embedded else resources_from_repo()
    try:
        build(resources, args.output_root, args.cache, args.archive)
    except (OSError, ValueError, tarfile.TarError, subprocess.CalledProcessError) as error:
        raise SystemExit(f"ALOL-Proton: {error}") from error


if __name__ == "__main__":
    main()
