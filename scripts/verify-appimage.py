#!/usr/bin/env python3
"""Verify portable release identity and its independent checksum manifest."""
import argparse
import hashlib
from pathlib import Path
import re
import subprocess
import tarfile
import tempfile
try:
    import tomllib
except ModuleNotFoundError:
    import tomli as tomllib

ROOT=Path(__file__).resolve().parents[1]

def digest(path):
    h=hashlib.sha256()
    with path.open('rb') as stream:
        for block in iter(lambda:stream.read(1024*1024),b''):h.update(block)
    return h.hexdigest()

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('directory',type=Path)
    parser.add_argument('--write',action='store_true')
    args=parser.parse_args()
    version=tomllib.loads((ROOT/'Cargo.toml').read_text())['package']['version']
    image=args.directory/f'alol-{version}-x86_64.AppImage'
    sources=args.directory/f'alol-{version}-appimage-sources.tar.gz'
    archive=args.directory/f'alol-{version}.tar.gz'
    with image.open('rb') as stream:header=stream.read(20)
    if header[:6]!=b'\x7fELF\x02\x01' or header[8:11]!=b'AI\x02' or header[18:20]!=b'\x3e\x00':
        raise ValueError('Not an x86_64 type 2 AppImage')
    paths={p.name:p for p in [image,sources,archive,ROOT/'LICENSE',ROOT/'NOTICE']}
    checksums={name:digest(path) for name,path in paths.items()}
    manifest=args.directory/'SHA256SUMS.appimage'
    if args.write:
        with tempfile.TemporaryDirectory(prefix='alol-portable-audit-') as temporary:
            subprocess.run([str(image.resolve()),'--appimage-extract'],cwd=temporary,check=True,stdout=subprocess.DEVNULL)
            root=Path(temporary)/'squashfs-root'
            for name in ['AppRun','alol.desktop','alol.png','usr/bin/alol','usr/share/licenses/alol/LICENSE','usr/share/licenses/alol/NOTICE','usr/share/licenses/alol-appimage/gtk-plugin.LICENSE','usr/share/licenses/alol-appimage/appimage-runtime.LICENSE','usr/share/doc/alol-appimage/packages.json']:
                if not (root/name).is_file():raise ValueError('Incomplete AppImage: '+name)
            for name in ('LICENSE','NOTICE'):
                if (root/'usr/share/licenses/alol'/name).read_bytes()!=(ROOT/name).read_bytes():raise ValueError('Portable license differs')
            import importlib.util
            spec=importlib.util.spec_from_file_location('source',ROOT/'scripts/package-source.py')
            module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
            data=(root/'usr/bin/alol').read_bytes()
            if module.SECRETS.search(data):raise ValueError('Private path or token in portable binary')
            for path in root.rglob('*'):
                if path.is_symlink() and not path.resolve().is_relative_to(root.resolve()):raise ValueError('External link in portable bundle')
        with tarfile.open(sources) as tar:
            if not {'packages.json','README.md','type2-runtime-source.tar.gz'}.issubset(tar.getnames()):raise ValueError('Missing library sources')
        manifest.write_text(''.join(f'{value}  {name}\n' for name,value in checksums.items()))
    else:
        recorded={}
        for line in manifest.read_text().splitlines():
            value,name=line.split('  ',1)
            if name in recorded or not re.fullmatch('[0-9a-f]{64}',value):raise ValueError('Invalid portable checksum entry')
            recorded[name]=value
        if recorded!=checksums:raise ValueError('Portable release checksum mismatch')
    print('AppImage, library sources and release checksums verified.')

if __name__=='__main__':main()
