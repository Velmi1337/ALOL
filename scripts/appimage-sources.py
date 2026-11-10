#!/usr/bin/env python3
"""Include bundled distro library copyrights and corresponding source archives."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import tarfile
import tempfile
try:
    import tomllib
except ModuleNotFoundError:
    import tomli as tomllib

ROOT = Path(__file__).resolve().parents[1]

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('appdir',type=Path)
    parser.add_argument('output',type=Path)
    args=parser.parse_args()
    version=tomllib.loads((ROOT/'Cargo.toml').read_text())['package']['version']
    packages={'libgtk-3-0','libglib2.0-0','libgdk-pixbuf-2.0-0','libarchive13','libfontconfig1',
              'adwaita-icon-theme','hicolor-icon-theme','gsettings-desktop-schemas'}
    for path in (args.appdir/'usr').rglob('*'):
        if not path.is_file() or path.is_symlink():continue
        with path.open('rb') as stream:elf=stream.read(4)==b'\x7fELF'
        if not elf or path.name=='alol':continue
        result=subprocess.run(['dpkg-query','-S','*/'+path.name],text=True,capture_output=True)
        matches=[line.split(': /',1)[0] for line in result.stdout.splitlines() if ': /' in line]
        if not matches:raise ValueError('No distro source package for '+path.name)
        packages.update(matches)
    sources=set()
    notices=args.appdir/'usr/share/doc/alol-appimage'
    notices.mkdir(parents=True,exist_ok=True)
    records=[]
    for package in sorted(packages):
        source=subprocess.check_output(['dpkg-query','-W','-f=${source:Package}=${source:Version}',package],text=True)
        sources.add(source)
        copyright=Path('/usr/share/doc')/package.split(':')[0]/'copyright'
        if not copyright.is_file():raise ValueError('Missing copyright for '+package)
        (notices/(package.replace(':','_')+'.copyright')).write_bytes(copyright.read_bytes())
        records.append({'package':package,'source':source})
    manifest=json.dumps(records,ensure_ascii=False,indent=2)+'\n'
    (notices/'packages.json').write_text(manifest)
    with tempfile.TemporaryDirectory(prefix='alol-library-sources-') as temporary:
        directory=Path(temporary)
        subprocess.run(['apt-get','source','--download-only',*sorted(sources)],cwd=directory,check=True)
        (directory/'packages.json').write_text(manifest)
        (directory/'README.md').write_text('Corresponding Ubuntu 22.04 sources for libraries bundled in ALOL AppImage.\n'
            'Use dpkg-source -x on each .dsc file, then the Debian packaging build instructions.\n'
            'The AppImage runtime source is in type2-runtime-source.tar.gz; its build instructions\n'
            'and dependency versions are in that upstream repository. ALOL sources ship separately.\n')
        url='https://codeload.github.com/AppImage/type2-runtime/tar.gz/fedfad51acc2860b2916a1eef32ba4a500ff31ef'
        subprocess.run(['curl','-fLsS',url,'-o',str(directory/'type2-runtime-source.tar.gz')],check=True)
        output=args.output/f'alol-{version}-appimage-sources.tar.gz'
        with tarfile.open(output,'w:gz',format=tarfile.PAX_FORMAT) as archive:
            for path in sorted(directory.iterdir()):
                if not path.is_file():continue
                info=archive.gettarinfo(str(path),arcname=path.name)
                info.uid=info.gid=0;info.uname=info.gname='';info.mtime=0
                with path.open('rb') as stream:archive.addfile(info,stream)
        print('Bundled library sources:',output.name,hashlib.sha256(output.read_bytes()).hexdigest())

if __name__=='__main__':main()
