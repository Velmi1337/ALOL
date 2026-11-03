#!/usr/bin/env python3
"""Audit the exact public keyboard payload and its corresponding Wine source."""
from importlib.util import module_from_spec, spec_from_file_location
import json
import posixpath
from pathlib import Path, PurePosixPath
import tarfile

ROOT = Path(__file__).resolve().parents[1]


def audit(directory):
    spec = spec_from_file_location('alol_runtime_builder', ROOT / 'runtime/build.py')
    builder = module_from_spec(spec)
    spec.loader.exec_module(builder)
    recipe = json.loads((ROOT / 'runtime/base.json').read_text())
    builder.validate(recipe)
    if not recipe['engine_patched']:
        return None
    artifact = directory / recipe['overlay_filename']
    if (artifact.is_symlink() or artifact.stat().st_size != recipe['overlay_size']
            or builder.sha256(artifact) != recipe['overlay_sha256']):
        raise ValueError('Public runtime payload differs from its pinned recipe')
    builder.verify_overlay(artifact, recipe)
    private_home = str(Path.home()).encode()
    private_wide = str(Path.home()).encode('utf-16le')
    found = set()
    required = {'wine/COPYING.LIB', 'wine/dlls/user32/input.c', 'wine/dlls/user32/user32.spec',
                'keyboard-user32.patch', 'REBUILD.txt'}
    with tarfile.open(artifact, 'r:gz') as outer:
        for member in outer:
            if member.uid or member.gid or member.uname or member.gname:
                raise ValueError('Private runtime owner metadata')
            if member.name.endswith('.dll'):
                data = outer.extractfile(member).read()
                if data[:2] != b'MZ' or private_home in data or private_wide in data:
                    raise ValueError('Invalid DLL or private build directory in runtime')
                continue
            with outer.extractfile(member) as source, tarfile.open(fileobj=source, mode='r|gz') as wine:
                count, total = 0, 0
                for item in wine:
                    count += 1
                    total += item.size
                    path = PurePosixPath(item.name)
                    if (count > 100000 or total > 2 * 1024**3 or path.is_absolute() or '..' in path.parts
                            or any(part in ('.git', '.dev-data', '__pycache__', 'drive_c', 'dosdevices') for part in path.parts)
                            or item.uid or item.gid or item.uname or item.gname):
                        raise ValueError('Unexpected or private Wine source metadata')
                    if not item.isfile():
                        link = PurePosixPath(item.linkname)
                        resolved = PurePosixPath(posixpath.normpath(str(link if item.islnk() else path.parent / link)))
                        if (not (item.issym() or item.islnk()) or link.is_absolute()
                                or not resolved.parts or resolved.parts[0] != 'wine' or '..' in resolved.parts):
                            raise ValueError('Unexpected special file or external Wine source link')
                        continue
                    if not 0 <= item.size <= 64 * 1024**2:
                        raise ValueError('Unexpected Wine source file size')
                    data = wine.extractfile(item).read()
                    # Third-party author emails and upstream test keys belong to
                    # Wine, so inspect home paths rather than classifying them
                    # as the developer's credentials or stripping valid source.
                    if private_home in data or private_wide in data:
                        raise ValueError('Private home directory in corresponding Wine source')
                    if item.name in required:
                        found.add(item.name)
                        if item.name == 'wine/COPYING.LIB' and data != (ROOT / 'runtime/WINE-LICENSE.md').read_bytes():
                            raise ValueError('Wine license differs from the corresponding source')
                        if item.name == 'keyboard-user32.patch' and data != (ROOT / 'runtime/keyboard-user32.patch').read_bytes():
                            raise ValueError('Wine patch differs from the corresponding source')
                        if item.name == 'wine/dlls/user32/input.c' and b'ALOLGetKeyboardLayout' not in data:
                            raise ValueError('The source bundle lacks the actual keyboard fix')
    if found != required:
        raise ValueError('Missing corresponding source, license, patch or rebuild instructions')
    return artifact


if __name__ == '__main__':
    import argparse
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('directory', type=Path, nargs='?', default=ROOT / 'dist')
    args = parser.parse_args()
    artifact = audit(args.directory)
    print('Runtime privacy/source audit passed: ' + (artifact.name if artifact else 'no engine overlay'))
