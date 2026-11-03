#!/usr/bin/env python3
"""Build the opt-in keyboard runtime without replacing any existing runtime.

Only user32.dll is rebuilt, from the exact GE Wine tree and GE patch series.
Complete corresponding patched source and rebuild instructions accompany it.
"""
import argparse
import copy
import fcntl
import gzip
import io
import os
from pathlib import Path
import shutil
import signal
import subprocess
import sys
import tarfile
import tempfile

import build as base

HERE = Path(__file__).resolve().parent
WINE_COMMIT = 'c53678ab22a85f0fad3588dd34f0452a463bd015'
STAGING_COMMIT = '6cc805ea57132eeaf44764e9213823c9b8d0d300'
SOURCES = (
    ('ge-source.tar.gz', 'https://codeload.github.com/GloriousEggroll/proton-ge-custom/tar.gz/refs/tags/GE-Proton11-11',
     123393667, '3131bfac872380ef18f3dc80bfc0d54c7d286e5ffbf46bac5f4bd36bd04524cf', 'proton-ge-custom-GE-Proton11-11'),
    ('wine-source.tar.gz', f'https://codeload.github.com/ValveSoftware/wine/tar.gz/{WINE_COMMIT}',
     56732849, 'a156c7e69af23fc89b6b75b655ab6bc2879ec3caa052de36b0162420d8d25e0c', 'wine-' + WINE_COMMIT),
    ('staging-source.tar.gz', f'https://codeload.github.com/wine-staging/wine-staging/tar.gz/{STAGING_COMMIT}',
     9322882, '46fccc6412d5a0ac5e02047a545bba21c3db45abfa1c161119a1de2f7b8cb360', 'wine-staging-' + STAGING_COMMIT),
    ('restore-configure.patch', 'https://github.com/ValveSoftware/wine/commit/e813ca5771658b00875924ab88d525322e50d39f.patch',
     None, 'dc863a602638557496bf63d56c1d9bbbfbb44936652f4ecc36f6dd19517c649f', None),
)
CONFIGURE = ['--enable-win64', '--enable-archs=x86_64,i386', '--disable-tests', '--without-x',
             '--without-wayland', '--without-alsa', '--without-pulse', '--without-gstreamer',
             '--without-cups', '--without-sane', '--without-opencl', '--without-vulkan']
TARGETS = [f'dlls/user32/{arch}-windows/user32.dll' for arch in ('i386', 'x86_64')]


def source_archive(spec, cache):
    name, url, size, digest, _ = spec
    path = cache / name
    if path.is_symlink():
        raise ValueError('Source cache must not contain symbolic links')
    if not path.is_file():
        partial = path.with_name(name + '.part')
        if partial.is_symlink():
            raise ValueError('Source cache must not contain symbolic links')
        print('Загрузка закреплённых исходников: ' + name, flush=True)
        subprocess.run(['curl', '--fail', '--location', '--silent', '--show-error', '--retry', '3',
                        '--connect-timeout', '15', '--max-time', '1800', '--max-filesize', str(size or 2**20),
                        '--proto', '=https', '--proto-redir', '=https', '--output', str(partial), url], check=True)
        if base.sha256(partial) != digest or (size is not None and partial.stat().st_size != size):
            raise ValueError('Downloaded source failed verification: ' + name)
        partial.replace(path)
    if base.sha256(path) != digest or (size is not None and path.stat().st_size != size):
        raise ValueError('Cached source failed verification: ' + name)
    return path


def command(args, cwd, env, log):
    with log.open('ab') as stream:
        result = subprocess.run(args, cwd=cwd, env=env, stdout=stream, stderr=stream)
    if result.returncode:
        tail = '\n'.join(log.read_text(errors='replace').splitlines()[-15:])
        raise ValueError(f'Build command failed ({args[0]}):\n{tail}\nLog: {log}')


def prepare_sources(cache, work, log):
    paths = {}
    for spec in SOURCES:
        archive = source_archive(spec, cache)
        paths[spec[0]] = archive
        if spec[4]:
            base.extract(archive, work, {'upstream_root': spec[4]})
    ge = work / SOURCES[0][4]
    (ge / 'wine').rmdir()
    (work / SOURCES[1][4]).rename(ge / 'wine')
    (ge / 'wine-staging').rmdir()
    (work / SOURCES[2][4]).rename(ge / 'wine-staging')
    original = (ge / 'patches/protonprep-valve-staging.sh').read_text()
    start = original.index('    pushd wine\n')
    body = original[start:original.index('    popd\n', start)]
    body = body.replace('    git reset --hard HEAD\n', '').replace('    git clean -xdf\n', '')
    body = body.replace('    git revert --no-commit e813ca5771658b00875924ab88d525322e50d39f',
                        '    patch --batch -R -Np1 < "$ALOL_RESTORE_CONFIGURE_PATCH"')
    script = work / 'apply-ge.sh'
    script.write_text('''#!/bin/bash
set -euo pipefail
apply_patch() { patch --batch -Np1 < "$1"; }
apply_all_in_dir() { for p in "$1"/*.patch; do apply_patch "$p"; done; }
''' + body)
    env = dict(os.environ, ALOL_RESTORE_CONFIGURE_PATCH=str(paths['restore-configure.patch']),
               XDG_CACHE_HOME=str(work / 'cache'), SOURCE_DATE_EPOCH='1792432585')
    print('Применение точной серии исправлений GE и патча индикатора…', flush=True)
    command(['bash', str(script)], ge, env, log)
    wine = ge / 'wine'
    command(['patch', '--batch', '-Np1', '-i', str(HERE / 'keyboard-user32.patch')], wine, env, log)
    command(['dlls/winevulkan/make_vulkan', '-x', 'vk.xml', '-X', 'video.xml'], wine, env, log)
    command(['tools/make_specfiles'], wine, env, log)
    return wine, env


def compile_dlls(wine, work, env, log):
    build = work / 'wine-build'
    build.mkdir()
    # Strip debug information and remove build-machine paths from __FILE__ too.
    flags = '-O2 -ffile-prefix-map=' + str(work) + '=/usr/src/alol-keyboard'
    env = dict(env, CFLAGS=flags, i386_CFLAGS=flags, x86_64_CFLAGS=flags)
    print('Сборка user32.dll: 32 и 64 бита. Остальные DLL GE не пересобираются…', flush=True)
    command([str(wine / 'configure'), *CONFIGURE], build, env, log)
    command(['make', '-j' + str(min(os.cpu_count() or 2, 8)),
             'i386_CFLAGS=' + flags, 'x86_64_CFLAGS=' + flags, *TARGETS], build, env, log)
    dlls = [build / target for target in TARGETS]
    command(['llvm-strip', '--strip-debug', *(str(path) for path in dlls)], build, env, log)
    for path in dlls:
        if str(work).encode() in path.read_bytes():
            raise ValueError('Build directory leaked into a DLL')
    return dlls


def source_bundle(wine, target):
    print('Упаковка полных изменённых исходников Wine и инструкции пересборки…', flush=True)
    instructions = ('These are the complete corresponding Wine sources for the two rebuilt user32 DLLs.\n'
                    'Wine license: wine/COPYING.LIB (LGPL-2.1-or-later). All upstream notices are retained.\n'
                    'Base Wine commit: ' + WINE_COMMIT + '\nGE: GE-Proton11-11, including its patch series.\n'
                    'Our changes: keyboard-user32.patch; already applied to wine/.\n'
                    'Rebuild on x86_64 Linux with gcc, clang, lld, llvm, make, flex, bison, perl and autoconf:\n'
                    'mkdir build\ncd build\n../wine/configure ' + ' '.join(CONFIGURE) + '\n'
                    'make -j8 ' + ' '.join(TARGETS) + '\n'
                    'llvm-strip --strip-debug ' + ' '.join(TARGETS) + '\n'
                    'Replace only these two user32 DLLs in the GE files/lib/wine/<arch>-windows directories.\n'
                    'No Wine Unix component, input translation, DXVK, locale or code page is changed.\n')
    with target.open('wb') as raw, gzip.GzipFile(fileobj=raw, mode='wb', filename='', mtime=0, compresslevel=3) as gz:
        with tarfile.open(fileobj=gz, mode='w|') as archive:
            for path in sorted(wine.rglob('*')):
                if not path.is_file() or path.suffix in ('.orig', '.rej') or '__pycache__' in path.parts:
                    continue
                info = archive.gettarinfo(path, 'wine/' + path.relative_to(wine).as_posix())
                info.uid = info.gid = 0
                info.uname = info.gname = ''
                info.mtime = 1792432585
                with path.open('rb') as stream:
                    archive.addfile(info, stream)
            for name, data in [('REBUILD.txt', instructions.encode()),
                               ('keyboard-user32.patch', (HERE / 'keyboard-user32.patch').read_bytes())]:
                info = tarfile.TarInfo(name)
                info.size, info.mtime = len(data), 1792432585
                archive.addfile(info, io.BytesIO(data))


def assemble(wine, dlls, work, output, cache, archive=None):
    bundle = work / 'ALOL-WINE-SOURCE.tar.gz'
    source_bundle(wine, bundle)
    resources = copy.deepcopy(base.resources_from_repo())
    recipe = resources['recipe']
    for key in ('overlay_url','overlay_filename','overlay_size','overlay_sha256'):
        recipe.pop(key,None)
    recipe.update(runtime_name='ALOL-Proton-0.1.0-kbd1-x86_64', profile_version='0.1.0-kbd1', engine_patched=True,
                  source_bundle_sha256=base.sha256(bundle), wine_commit=WINE_COMMIT,
                  keyboard_patch_sha256=base.sha256(HERE / 'keyboard-user32.patch'))
    resources['engine_overlay'] = {f'files/lib/wine/{arch}-windows/user32.dll': str(path)
                                   for arch, path in zip(('i386', 'x86_64'), dlls)}
    recipe['engine_files'] = {name: base.sha256(Path(path)) for name, path in resources['engine_overlay'].items()}
    resources['source_bundle'] = str(bundle)
    return base.build(resources, output, cache, archive)


def main():
    signal.signal(signal.SIGTERM, lambda *_: sys.exit(143))
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output-root', type=Path, required=True)
    parser.add_argument('--cache', type=Path, required=True)
    parser.add_argument('--archive', type=Path)
    args = parser.parse_args()
    required = ('gcc', 'clang', 'llvm-dlltool', 'lld-link', 'make', 'flex', 'bison', 'autoreconf', 'perl', 'patch', 'llvm-strip', 'curl')
    missing = [tool for tool in required if not shutil.which(tool)]
    if missing:
        parser.error('Missing build tools: ' + ', '.join(missing))
    args.cache = args.cache.resolve()
    args.cache.mkdir(parents=True, exist_ok=True)
    if shutil.disk_usage(args.cache).free < 5 * 1024**3:
        parser.error('At least 5 GB free space is required to compile the keyboard runtime')
    with (args.cache / '.keyboard-build.lock').open('a+b') as lock:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        log = args.cache / 'keyboard-build.log'
        with tempfile.TemporaryDirectory(prefix='keyboard-build-', dir=args.cache) as temp:
            work = Path(temp)
            wine, env = prepare_sources(args.cache, work, log)
            dlls = compile_dlls(wine, work, env, log)
            result = assemble(wine, dlls, work, args.output_root.resolve(), args.cache, args.archive)
            print('Proton с исправлением индикатора: ' + str(result), flush=True)
    return 0


if __name__ == '__main__':
    try:
        sys.exit(main())
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        print('Сборка отменена; существующие версии Proton сохранены: ' + str(error), file=sys.stderr)
        sys.exit(1)
