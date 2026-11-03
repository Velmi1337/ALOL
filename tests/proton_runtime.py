#!/usr/bin/env python3
"""Offline checks for the experimental distribution and its actual launch layer."""
import fcntl
import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import subprocess
import tarfile
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]


def load(name, source):
    spec = importlib.util.spec_from_file_location(name, source)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


builder = load("alol_builder", ROOT / "runtime/build.py")
wrapper = load("alol_wrapper", ROOT / "runtime/proton_wrapper.py")
ENGINE = b'''#!/usr/bin/python3
import json, os, sys
keys = ('DISPLAY','WAYLAND_DISPLAY','XMODIFIERS','DXVK_CONFIG','DXVK_FRAME_RATE','MANGOHUD','ALOL_TARGET_KIND')
print(json.dumps({'args':sys.argv[1:], 'environment':{k:os.environ.get(k) for k in keys}}))
'''


class Distribution(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="alol-runtime-")
        self.root = Path(self.temp.name)
        self.resources = builder.resources_from_repo()
        self.recipe = self.resources["recipe"]
        self.recipe.update(runtime_name="ALOL-Proton-fixture", upstream_root="GE-Proton-fixture")
        self.recipe["engine_patched"] = False
        for key in ('engine_files','source_bundle_sha256','overlay_url','overlay_filename','overlay_size','overlay_sha256'):
            self.recipe.pop(key,None)
        self.output = self.root / "runtimes"
        self.cache = self.root / "cache"

    def tearDown(self):
        self.temp.cleanup()

    def archive(self, extra=(), missing=()):
        archive = self.root / "base.tar.gz"
        payload = {"proton":ENGINE, "version":b"123 GE-Proton-fixture\n", "toolmanifest.vdf":b'"manifest" {}',
                   "compatibilitytool.vdf":b'"compatibilitytools" {}', "files/bin/wine64":b'fixture binary',
                   "LICENSE.upstream":b'upstream license retained'}
        for arch in ('i386','x86_64'):
            payload[f'files/lib/wine/{arch}-windows/user32.dll'] = b'original upstream DLL'
        with tarfile.open(archive, "w:gz") as stream:
            for name, value in payload.items():
                if name in missing: continue
                member = tarfile.TarInfo("GE-Proton-fixture/" + name)
                member.size = len(value)
                member.mode = 0o755 if name == "proton" else 0o644
                stream.addfile(member, io.BytesIO(value))
            if "files/bin/wine" not in missing:
                member = tarfile.TarInfo("GE-Proton-fixture/files/bin/wine")
                member.type = tarfile.SYMTYPE
                member.linkname = "wine64"
                stream.addfile(member)
            for member in extra: stream.addfile(member)
        self.recipe.update(base_size=archive.stat().st_size, base_sha256=builder.sha256(archive))
        return archive

    def build(self, archive):
        return builder.build(self.resources, self.output, self.cache, archive)

    def test_assembly_preserves_original_binary_licenses_and_prefix_version(self):
        output = self.build(self.archive())
        self.assertEqual((output / "proton.alol-upstream").read_bytes(), ENGINE)
        self.assertEqual((output / "version").read_bytes(), b"123 GE-Proton-fixture\n")
        self.assertEqual((output / "LICENSE.upstream").read_bytes(), b'upstream license retained')
        self.assertTrue((output / "files/bin/wine").is_file())
        self.assertTrue((output / "ALOL-LICENSE").is_file())
        self.assertIn("No Wine/DXVK source patch", (output / "ALOL-NOTICE").read_text())
        marker = json.loads((output / "alol-runtime.json").read_text())
        self.assertFalse(marker['recipe']['engine_patched'])
        self.assertTrue(builder.ready(output, self.resources))
        try:
            import vdf
        except ImportError:
            pass
        else:
            registration=vdf.loads((output/'compatibilitytool.vdf').read_text())
            tool=registration['compatibilitytools']['compat_tools'][self.recipe['runtime_name']]
            self.assertEqual(tool['install_path'],'.')
            self.assertIn(self.recipe['profile_version'],tool['display_name'])
        self.assertFalse(list(self.output.glob('.build-*')))

    def test_real_wrapper_delegates_and_keeps_graphics_and_frame_limits(self):
        output = self.build(self.archive())
        env = dict(os.environ, ALOL_TARGET_KIND='game', ALOL_COMPAT_INPUT='1', DISPLAY=':123',
                   WAYLAND_DISPLAY='wayland-test', DXVK_CONFIG='custom rendering', DXVK_FRAME_RATE='144', MANGOHUD='1')
        result = subprocess.run([str(output / 'proton'), 'waitforexitandrun', r'C:\Arizona\gta_sa.exe', '-arizona'],
                                env=env, capture_output=True, text=True, check=True)
        data = json.loads(result.stdout)
        self.assertEqual(data['args'], ['waitforexitandrun', r'C:\Arizona\gta_sa.exe', '-arizona'])
        self.assertEqual(data['environment']['DXVK_CONFIG'], 'custom rendering')
        self.assertEqual(data['environment']['DXVK_FRAME_RATE'], '144')
        self.assertEqual(data['environment']['MANGOHUD'], '1')
        self.assertEqual(data['environment']['XMODIFIERS'], '@im=none')
        self.assertIsNone(data['environment']['WAYLAND_DISPLAY'])
        self.assertIsNone(data['environment']['ALOL_TARGET_KIND'])
        self.assertEqual(env['WAYLAND_DISPLAY'], 'wayland-test')

    def test_keyboard_overlay_keeps_originals_sources_and_verifies_each_dll(self):
        archive = self.archive()
        dll = self.root/'patched.dll'
        dll.write_bytes(b'patched user32 fixture')
        sources = self.root/'sources.tar.gz'
        sources.write_bytes(b'corresponding complete source fixture')
        files = {f'files/lib/wine/{arch}-windows/user32.dll':builder.sha256(dll) for arch in ('i386','x86_64')}
        self.recipe.update(engine_patched=True,engine_files=files,source_bundle_sha256=builder.sha256(sources))
        self.resources.update(engine_overlay={name:str(dll) for name in files},source_bundle=str(sources))
        output = self.build(archive)
        self.assertTrue(builder.ready(output,self.resources))
        for name in files:
            self.assertEqual((output/name).read_bytes(),dll.read_bytes())
            self.assertEqual((output/name).with_name('user32.alol-upstream.dll').read_bytes(),b'original upstream DLL')
        self.assertEqual((output/'ALOL-WINE-SOURCE.tar.gz').read_bytes(),sources.read_bytes())
        self.assertIn('LGPL-2.1-or-later',(output/'ALOL-NOTICE').read_text())
        (output/next(iter(files))).write_bytes(b'tampered DLL')
        self.assertFalse(builder.ready(output,self.resources))

    def test_missing_keyboard_sources_prevents_publishing_runtime(self):
        archive = self.archive()
        self.recipe.update(engine_patched=True,engine_files={
            f'files/lib/wine/{arch}-windows/user32.dll':'a'*64 for arch in ('i386','x86_64')})
        with self.assertRaisesRegex(ValueError,'source bundle'):
            self.build(archive)
        self.assertFalse((self.output/self.recipe['runtime_name']).exists())

    def keyboard_download(self, extra=None):
        files = {f'files/lib/wine/{arch}-windows/user32.dll': b'new keyboard DLL' for arch in ('i386','x86_64')}
        files['ALOL-WINE-SOURCE.tar.gz'] = b'complete source fixture'
        overlay = self.root/'keyboard-overlay.tar.gz'
        with tarfile.open(overlay,'w:gz') as stream:
            for name,data in files.items():
                member=tarfile.TarInfo(name);member.size=len(data)
                stream.addfile(member,io.BytesIO(data))
            if extra:stream.addfile(extra)
        self.recipe.update(engine_patched=True,engine_files={name:hashlib.sha256(data).hexdigest() for name,data in files.items() if name.endswith('.dll')},
            source_bundle_sha256=hashlib.sha256(files['ALOL-WINE-SOURCE.tar.gz']).hexdigest(),
            overlay_url='https://github.com/Velmi1337/ALOL/releases/download/v0.5.8/keyboard-overlay.tar.gz',
            overlay_filename=overlay.name,overlay_sha256=builder.sha256(overlay),overlay_size=overlay.stat().st_size)
        return overlay

    def test_default_prepare_downloads_verified_overlay_without_compiler(self):
        archive=self.archive();overlay=self.keyboard_download()
        with patch.object(builder,'download',return_value=overlay) as download:
            output=self.build(archive)
        self.assertEqual(download.call_count,1)
        self.assertTrue(builder.ready(output,self.resources))
        self.assertEqual((output/'files/lib/wine/i386-windows/user32.dll').read_bytes(),b'new keyboard DLL')

    def test_extra_overlay_file_is_rejected_without_half_runtime(self):
        archive=self.archive();member=tarfile.TarInfo('../../escaped')
        overlay=self.keyboard_download(extra=member)
        with patch.object(builder,'download',return_value=overlay):
            with self.assertRaisesRegex(ValueError,'Unexpected files'):
                self.build(archive)
        self.assertFalse((self.output/self.recipe['runtime_name']).exists())
        self.assertFalse(list(self.output.glob('.build-*')))

    def test_hash_mismatch_does_not_create_runtime_or_modify_archive(self):
        archive = self.archive()
        data = archive.read_bytes()
        self.recipe['base_sha256'] = '0' * 64
        with self.assertRaisesRegex(ValueError, 'SHA256 mismatch'): self.build(archive)
        self.assertEqual(archive.read_bytes(), data)
        self.assertFalse((self.output / self.recipe['runtime_name']).exists())

    def test_escape_paths_and_external_links_are_rejected(self):
        for name, link in [('GE-Proton-fixture/../../escaped', None), ('GE-Proton-fixture/link', '../../escaped')]:
            member = tarfile.TarInfo(name)
            if link:
                member.type = tarfile.SYMTYPE
                member.linkname = link
            with self.assertRaises((ValueError, tarfile.TarError)): self.build(self.archive([member]))
            self.assertFalse((self.root / 'escaped').exists())
            self.assertFalse((self.output / self.recipe['runtime_name']).exists())

    def test_missing_components_and_interruption_leave_no_half_runtime(self):
        with self.assertRaises(ValueError): self.build(self.archive(missing=['files/bin/wine']))
        with patch.object(builder, 'extract', side_effect=KeyboardInterrupt):
            with self.assertRaises(KeyboardInterrupt): self.build(self.archive())
        self.assertFalse(list(self.output.glob('.build-*')))
        self.assertFalse((self.output / self.recipe['runtime_name']).exists())

    def test_ready_build_is_reused_but_damaged_version_is_never_overwritten(self):
        archive = self.archive()
        output = self.build(archive)
        before = (output / 'proton').stat().st_mtime_ns
        self.assertEqual(output, self.build(archive))
        self.assertEqual((output / 'proton').stat().st_mtime_ns, before)
        (output / 'proton').write_text('modified later')
        with self.assertRaisesRegex(ValueError, 'was not replaced'): self.build(archive)
        self.assertEqual((output / 'proton').read_text(), 'modified later')

    def test_build_lock_blocks_parallel_prepare(self):
        archive = self.archive()
        self.output.mkdir()
        with (self.output / '.build.lock').open('a+b') as lock:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
            with self.assertRaisesRegex(ValueError, 'Another'): self.build(archive)


class Profile(unittest.TestCase):
    def test_installer_keeps_its_own_silent_install_arguments(self):
        args=['waitforexitandrun', r'C:\ArizonaInstaller.exe', '/S', '/D=C:\\Arizona Games Launcher']
        env=dict(ALOL_TARGET_KIND='installer', ALOL_COMPAT_INPUT='1', ALOL_COMPAT_LAUNCHER='1',
                 ALOL_LAUNCHER_DISABLE_GPU='1', DISPLAY=':123', WAYLAND_DISPLAY='wayland-test', MANGOHUD='1')
        changed_args, changed_env=wrapper.configure(args,env)
        self.assertEqual(changed_args,args)
        self.assertNotIn('MANGOHUD',changed_env)
        self.assertNotIn('WAYLAND_DISPLAY',changed_env)

    def test_launcher_cleanup_preserves_sockets_and_deduplicates_flags(self):
        env = dict(ALOL_TARGET_KIND='launcher', ALOL_COMPAT_INPUT='1', ALOL_COMPAT_LAUNCHER='1',
                   ALOL_LAUNCHER_DISABLE_GPU='1', DISPLAY=':123', WAYLAND_DISPLAY='wayland-test',
                   MANGOHUD='1', LD_PRELOAD='injected', DXVK_CONFIG='keep', DXVK_FRAME_RATE='90')
        args, changed = wrapper.configure(['waitforexitandrun', r'C:\Arizona Games Launcher.exe', '--no-sandbox'], env)
        self.assertEqual(args.count('--no-sandbox'), 1)
        self.assertEqual(args.count('--disable-gpu'), 1)
        self.assertNotIn('LD_PRELOAD', changed)
        self.assertNotIn('MANGOHUD', changed)
        self.assertEqual(changed['DISPLAY'], ':123')
        self.assertEqual(changed['DXVK_CONFIG'], 'keep')
        self.assertEqual(changed['DXVK_FRAME_RATE'], '90')
        self.assertEqual(env['LD_PRELOAD'], 'injected')

    def test_helpers_unmarked_games_and_disabled_compatibility_pass_through(self):
        env = dict(WAYLAND_DISPLAY='wayland-test', XMODIFIERS='@im=ibus', DXVK_CONFIG='keep')
        args = ['waitforexitandrun', r'C:\Other\game.exe']
        self.assertEqual(wrapper.configure(args, env), (args, env))
        for kind in ('helper', 'game', 'launcher'):
            _, changed = wrapper.configure(args, dict(env, ALOL_TARGET_KIND=kind))
            self.assertEqual(changed, env)
        _, native = wrapper.configure(args, dict(env, ALOL_TARGET_KIND='game', ALOL_COMPAT_INPUT='1'))
        self.assertEqual(native['WAYLAND_DISPLAY'], 'wayland-test')


if __name__ == '__main__':
    unittest.main(verbosity=2)
