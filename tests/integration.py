#!/usr/bin/env python3
"""Offline process-level checks. Fake UMU tests orchestration, not game compatibility."""
import json
import os
from pathlib import Path
import signal
import subprocess
import tempfile
import time
import unittest

BINARY = Path(__file__).resolve().parents[1] / "target/release/alol"
WINEBOOT = r"C:\windows\system32\wineboot.exe"
REG = r"C:\windows\system32\reg.exe"
FAKE = r'''
import json, os, pathlib, sys, time
p = pathlib.Path(os.environ['WINEPREFIX'])
args = sys.argv[1:]
exe = args[0]
with open(os.environ['ALOL_TEST_TRACE'], 'a') as f:
    f.write(json.dumps({'args': args, 'prefix': str(p), 'proton': os.environ.get('PROTONPATH'),
        'verb': os.environ.get('PROTON_VERB'), 'wined3d': os.environ.get('PROTON_USE_WINED3D'),
        'winearch': os.environ.get('WINEARCH'), 'prime': os.environ.get('DRI_PRIME'),
        'install_path': os.environ.get('STEAM_COMPAT_INSTALL_PATH'), 'cwd': os.getcwd(),
        'lc_all': os.environ.get('LC_ALL'), 'lang': os.environ.get('LANG'),
        'qt': os.environ.get('QT_QPA_PLATFORM'), 'gdk': os.environ.get('GDK_BACKEND'),
        'wayland': os.environ.get('WAYLAND_DISPLAY'), 'display': os.environ.get('DISPLAY'),
        'session': os.environ.get('XDG_SESSION_TYPE'), 'mangohud': os.environ.get('MANGOHUD'),
        'proton_wayland': os.environ.get('PROTON_ENABLE_WAYLAND')}) + '\n')
if os.environ.get('ALOL_TEST_FAIL') == exe:
    (p / 'drive_c').mkdir(parents=True, exist_ok=True)
    print('injected runner error', flush=True)
    sys.exit(42)
if os.environ.get('ALOL_TEST_SLEEP') == exe:
    pathlib.Path(os.environ['ALOL_TEST_READY']).touch()
    time.sleep(10)
if exe in ('createprefix', r'C:\windows\system32\wineboot.exe'):
    (p / 'drive_c').mkdir(parents=True, exist_ok=True)
    (p / 'system.reg').touch()
    build = pathlib.Path(os.environ['XDG_DATA_HOME']) / 'Steam/compatibilitytools.d/GE-Proton-test'
    (build / 'files/share/fonts').mkdir(parents=True, exist_ok=True)
    (build / 'proton').touch()
    (p / 'config_info').write_text('99\n' + str(build / 'files/share/fonts') + '\n')
    if exe == 'createprefix':
        # Match real UMU: the prefix exists, but its empty EXE fails to launch.
        print('ShellExecuteEx failed: File not found.', flush=True)
        sys.exit(1)
elif pathlib.Path(exe).name.startswith("Arizona's"):
    launcher = p / 'drive_c/Arizona Games Launcher/Arizona Games Launcher.exe'
    launcher.parent.mkdir(parents=True, exist_ok=True)
    launcher.touch()
elif exe == 'winetricks' or pathlib.Path(sys.argv[0]).name == 'winetricks':
    with (p / 'winetricks.log').open('a') as f:
        for verb in args:
            if verb in ('corefonts', 'vcrun2022'):
                f.write(verb + '\n')
elif exe.endswith(('Arizona Games Launcher.exe','gta_sa.exe')) and os.environ.get('ALOL_TEST_DROP_ZONE'):
    time.sleep(0.3)
    sidecar = pathlib.Path(os.environ['ALOL_TEST_DROP_ZONE'])
    sidecar.parent.mkdir(parents=True, exist_ok=True)
    sidecar.write_text('[ZoneTransfer]\nZoneId=3\n')
    time.sleep(2.5)
print('fake runner completed:', exe, flush=True)
'''


class Workflow(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix="alol-integration-")
        self.root = Path(self.tmp.name)
        self.data = self.root / "data"
        self.trace = self.root / "trace.jsonl"
        self.env = dict(os.environ, XDG_CONFIG_HOME=str(self.root / "config"),
                        XDG_DATA_HOME=str(self.data), ALOL_TEST_TRACE=str(self.trace),
                        ALOL_TEST_READY=str(self.root / "ready"),
                        PROTON_VERB="destroyprefix", WINEARCH="win32")
        self.bin = self.root / "bin"
        self.bin.mkdir()
        for name in ("umu-run", "wine", "winetricks"):
            runner = self.bin / name
            runner.write_text("#!/usr/bin/python3\n" + FAKE)
            runner.chmod(0o755)
        locale = self.bin / 'locale'
        locale.write_text("#!/usr/bin/python3\nimport os\nprint('C' if os.environ.get('ALOL_TEST_NO_RU_LOCALE') else 'C\\nru_RU.utf8')\n")
        locale.chmod(0o755)
        self.env["PATH"] = str(self.bin) + os.pathsep + os.environ["PATH"]
        self.installer = self.root / "Arizona's Game = $(touch PWNED).exe"
        self.installer.touch()
        self.config = self.root / "config/alol/settings.conf"
        self.config.parent.mkdir(parents=True)
        self.write_config()

    def tearDown(self):
        self.tmp.cleanup()

    def write_config(self, **extra):
        self.settings = dict(runner="proton", proton="GE-Proton", wine="wine",
                             installer=str(self.installer), executable="",
                             prefix_root=str(self.data / "prefixes"), wined3d="false",
                             disable_gpu="true", discrete_gpu="true", debug="false", no_sync="false")
        self.settings.update(extra)
        self.config.write_text("".join(f"{k}={v}\n" for k, v in self.settings.items()))

    def run_alol(self, action, success=True, arguments=(), **env):
        p = subprocess.run([str(BINARY), action, *map(str,arguments)], env=dict(self.env, **env),
                           text=True, capture_output=True, timeout=8)
        self.assertEqual(p.returncode == 0, success, p.stdout + p.stderr)
        return p

    def calls(self):
        return [json.loads(line) for line in self.trace.read_text().splitlines()]

    @property
    def prefix(self):
        return self.data / "prefixes" / self.settings["runner"]

    def test_new_profile_uses_available_wine_but_explicit_proton_never_switches_prefix(self):
        self.config.unlink()
        (self.bin/'umu-run').unlink()
        self.env['PATH']=str(self.bin)
        self.run_alol('prepare')
        self.assertTrue(self.calls()[0]['prefix'].endswith('/prefixes/wine'))
        self.assertEqual(Path(self.calls()[0]['prefix']).name,'wine')
        count=len(self.calls())
        self.write_config(runner='proton')
        result=self.run_alol('prepare',success=False)
        self.assertIn('umu-run',result.stderr)
        self.assertEqual(len(self.calls()),count)

    def test_prepare_install_launch_and_pin_actual_build(self):
        self.run_alol("install")
        calls = self.calls()
        self.assertEqual(calls[0]["args"], [WINEBOOT, "-u"])
        self.assertEqual(calls[0]["install_path"], str(self.prefix))
        self.assertEqual(calls[0]["cwd"], str(self.prefix))
        self.assertEqual(calls[1]["args"], [REG, "add", "HKCU\\Software\\Wine", "/v", "Version", "/t", "REG_SZ", "/d", "win10", "/f"])
        self.assertEqual(calls[2]["args"], ["winetricks", "-q", "corefonts", "vcrun2022"])
        self.assertEqual(calls[3]["args"], [REG, 'import', r'C:\alol\locale-ru.reg'])
        self.assertEqual(calls[4]["args"], [str(self.installer), "/S", "/D=C:\\Arizona Games Launcher"])
        pinned = str(self.data / "Steam/compatibilitytools.d/GE-Proton-test")
        self.assertIn("proton=" + pinned, self.config.read_text())
        self.assertEqual(calls[4]["proton"], pinned)
        self.run_alol("launch")
        last = self.calls()[-1]
        self.assertEqual(last["args"][1:], ["--no-sandbox", "--disable-gpu"])
        self.assertEqual(last["prefix"], str(self.prefix))
        self.assertEqual(last["verb"], "waitforexitandrun")
        self.assertIsNone(last["winearch"])
        self.assertEqual(last["prime"], "1")
        self.assertEqual(last['lc_all'],'ru_RU.UTF-8')
        self.assertFalse((self.root / "PWNED").exists())

    def test_runner_failure_is_reported_without_false_install(self):
        p = self.run_alol("prepare", success=False, ALOL_TEST_FAIL=WINEBOOT)
        self.assertIn("42", p.stderr)
        self.assertEqual(len(self.calls()), 1)
        self.assertIn("proton=GE-Proton\n", self.config.read_text())
        self.assertIn("✗ Окружение", self.run_alol("doctor").stdout)

    def test_missing_executable_and_invalid_command_do_not_start_runner(self):
        self.run_alol("launch", success=False)
        self.run_alol("not-a-command", success=False)
        self.assertFalse(self.trace.exists())

    def test_existing_prefix_does_not_hide_runner_failure(self):
        self.run_alol("prepare")
        self.assertTrue((self.prefix / "system.reg").is_file())
        self.run_alol("prepare", success=False, ALOL_TEST_FAIL=WINEBOOT)
        self.assertTrue((self.prefix / "system.reg").is_file())
        self.assertNotIn("createprefix", [call["args"][0] for call in self.calls()])

    def test_repair_preserves_account_settings_and_backups(self):
        self.run_alol("prepare")
        profile = self.prefix / "drive_c/users/steamuser/AppData/Roaming/arizona-launcher"
        (profile / "GPUCache").mkdir(parents=True)
        (profile / "GPUCache/data").write_bytes(b"cache")
        (profile / "settings.json").write_bytes(b"account settings")
        self.run_alol("repair")
        self.assertEqual((profile / "settings.json").read_bytes(), b"account settings")
        copies = list(profile.glob("GPUCache.alol-backup-*"))
        self.assertEqual(len(copies), 1)
        self.assertEqual((copies[0] / "data").read_bytes(), b"cache")
        game = self.prefix / "drive_c/users/steamuser/Documents/GTA San Andreas User Files"
        game.mkdir(parents=True)
        (game / "gta_sa.set").write_bytes(b"game settings")
        self.run_alol("reset-game")
        self.assertFalse((game / "gta_sa.set").exists())
        self.assertEqual(next(game.glob("gta_sa.set.alol-backup-*")).read_bytes(), b"game settings")
        self.run_alol("libraries")
        component_calls = [c for c in self.calls() if c['args'][0] == 'winetricks']
        self.assertEqual([c['args'] for c in component_calls], [["winetricks", "-q", "corefonts", "vcrun2022"]])

    def test_components_failure_stops_installation_and_can_be_retried(self):
        self.run_alol("install", success=False, ALOL_TEST_FAIL="winetricks")
        self.assertFalse((self.prefix / 'drive_c/Arizona Games Launcher/Arizona Games Launcher.exe').exists())
        self.run_alol("install")
        self.assertTrue((self.prefix / 'winetricks.log').exists())

    def test_only_missing_components_are_installed(self):
        self.prefix.mkdir(parents=True)
        (self.prefix / 'winetricks.log').write_text('corefonts\n')
        self.run_alol('prepare')
        self.assertEqual([c['args'] for c in self.calls() if c['args'][0]=='winetricks'], [['winetricks', '-q', 'vcrun2022']])
        count = len(self.calls())
        self.run_alol('libraries')
        self.assertEqual(len(self.calls()), count)

    def test_font_and_dll_zone_metadata_is_backed_up_without_touching_game_files(self):
        self.run_alol('install')
        game = self.prefix / 'drive_c/Arizona Games Launcher/bin/arizona'
        fonts = game / 'fontcustom'; fonts.mkdir(parents=True)
        font = fonts / 'GOST 2.ttf'; font.write_bytes(b'real font')
        sidecar = fonts / 'GOST 2.ttf:Zone.Identifier'; sidecar.write_text('[ZoneTransfer]\nZoneId=3\n')
        dll = game / 'lib.dll'; dll.write_bytes(b'real library')
        (game / 'lib.dll:Zone.Identifier').write_text('[ZoneTransfer]\nZoneId=3\n')
        unknown = game / 'custom.dll:Zone.Identifier'; unknown.write_text('user data, not Windows metadata')
        outside = self.root / 'external'; outside.mkdir()
        outside_file = outside / 'outside.ttf:Zone.Identifier'; outside_file.write_text('[ZoneTransfer]\nZoneId=3\n')
        (game / 'link').symlink_to(outside)
        self.run_alol('repair-game')
        self.assertFalse(sidecar.exists())
        self.assertFalse((game / 'lib.dll:Zone.Identifier').exists())
        self.assertEqual(font.read_bytes(), b'real font')
        self.assertEqual(dll.read_bytes(), b'real library')
        self.assertTrue(unknown.exists()); self.assertTrue(outside_file.exists())
        backups = list((self.data / 'alol/backups/zone-identifiers').rglob('*:Zone.Identifier'))
        self.assertEqual(len(backups), 2)
        self.run_alol('repair-game')
        self.assertEqual(len(list((self.data / 'alol/backups/zone-identifiers').rglob('*:Zone.Identifier'))), 2)

    def test_new_font_metadata_is_cleaned_while_launcher_is_running(self):
        self.run_alol('install')
        sidecar = self.prefix / 'drive_c/Arizona Games Launcher/bin/arizona/fontcustom/new.ttf:Zone.Identifier'
        self.run_alol('launch', ALOL_TEST_DROP_ZONE=str(sidecar))
        self.assertFalse(sidecar.exists())
        self.assertEqual(len(list((self.data / 'alol/backups/zone-identifiers').rglob('new.ttf:Zone.Identifier'))), 1)

    def test_two_workers_cannot_modify_same_prefix_and_stop_releases_lock(self):
        p = subprocess.Popen([str(BINARY), "prepare"], env=dict(self.env, ALOL_TEST_SLEEP=WINEBOOT),
                             stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, start_new_session=True)
        try:
            deadline = time.monotonic() + 4
            while not (self.root / "ready").exists() and time.monotonic() < deadline:
                time.sleep(0.02)
            self.assertTrue((self.root / "ready").exists())
            other = self.run_alol("prepare", success=False)
            self.assertIn("уже используется", other.stderr)
            os.killpg(p.pid, signal.SIGTERM)
            p.communicate(timeout=3)
            self.assertNotEqual(p.returncode, 0)
            self.run_alol("prepare")
        finally:
            if p.poll() is None:
                os.killpg(p.pid, signal.SIGKILL)
                p.communicate()

    def test_wine_has_separate_prefix_and_opengl_override(self):
        self.write_config(runner="wine", wined3d="true")
        self.run_alol("prepare")
        self.assertTrue((self.prefix / "drive_c").is_dir())
        self.assertFalse((self.data / "prefixes/proton").exists())
        self.assertEqual(self.calls()[0]["args"], [WINEBOOT, "-u"])
        self.assertEqual(self.calls()[0]["wined3d"], "1")

    def test_russian_locale_and_codepage_are_applied_once_and_can_be_repaired(self):
        self.run_alol('prepare')
        reg=(self.prefix/'drive_c/alol/locale-ru.reg').read_text(encoding='utf-16')
        self.assertIn('"ACP"="1251"',reg)
        self.assertIn('"OEMCP"="866"',reg)
        self.assertIn('"LocaleName"="ru-RU"',reg)
        self.assertTrue((self.prefix/'.alol-language-ru-v1').is_file())
        self.run_alol('prepare')
        imports=[c for c in self.calls() if c['args'][0]==REG and c['args'][1]=='import']
        self.assertEqual(len(imports),1)
        self.run_alol('repair-language')
        self.assertEqual(len([c for c in self.calls() if c['args'][0]==REG and c['args'][1]=='import']),2)

    def test_missing_linux_locale_stops_setup_with_actionable_error(self):
        result=self.run_alol('install',success=False,ALOL_TEST_NO_RU_LOCALE='1')
        self.assertIn('locale-gen',result.stderr)
        self.assertFalse((self.prefix/'.alol-language-ru-v1').exists())

    def test_launcher_compatibility_uses_xwayland_without_dotfile_overrides(self):
        overrides = dict(DISPLAY=":123", WAYLAND_DISPLAY="wayland-test", XDG_SESSION_TYPE="wayland",
                         QT_QPA_PLATFORM="wayland", GDK_BACKEND="wayland", MANGOHUD="1", PROTON_ENABLE_WAYLAND="1")
        self.run_alol('install', **overrides)
        self.run_alol('launch', **overrides)
        call = self.calls()[-1]
        self.assertEqual(call['display'], ':123')
        self.assertIsNone(call['wayland'])
        self.assertIsNone(call['mangohud'])
        self.assertEqual(call['proton_wayland'], '0')
        self.assertEqual(call['qt'], 'xcb')
        self.assertEqual(call['gdk'], 'x11')
        self.assertEqual(call['session'], 'x11')

    def test_launcher_native_mode_preserves_custom_environment(self):
        self.write_config(launcher_compatibility='false')
        self.run_alol('install', DISPLAY='', WAYLAND_DISPLAY='wayland-test', XDG_SESSION_TYPE='wayland',
                      QT_QPA_PLATFORM='wayland', GDK_BACKEND='wayland', MANGOHUD='1')
        call = self.calls()[-1]
        self.assertEqual(call['wayland'], 'wayland-test')
        self.assertEqual(call['qt'], 'wayland')
        self.assertEqual(call['mangohud'], '1')

    def test_missing_xwayland_has_actionable_niri_error(self):
        result = self.run_alol('install', success=False, DISPLAY='', WAYLAND_DISPLAY='wayland-test', XDG_SESSION_TYPE='wayland')
        self.assertIn('xwayland-satellite', result.stderr)
        self.assertFalse((self.prefix/'drive_c/Arizona Games Launcher/Arizona Games Launcher.exe').exists())

    def test_failed_launcher_retries_without_gpu_once(self):
        self.write_config(disable_gpu='false')
        self.run_alol('install')
        exe = self.prefix/'drive_c/Arizona Games Launcher/Arizona Games Launcher.exe'
        before = len(self.calls())
        result = self.run_alol('launch', success=False, ALOL_TEST_FAIL=str(exe))
        calls = self.calls()[before:]
        self.assertEqual(len(calls), 2)
        self.assertEqual(calls[0]['args'][1:], ['--no-sandbox'])
        self.assertEqual(calls[1]['args'][1:], ['--no-sandbox', '--disable-gpu'])
        self.assertIn('Повторный запуск', result.stdout)
        self.assertIn('disable_gpu=false', self.config.read_text())


if __name__ == "__main__":
    if not BINARY.is_file():
        raise SystemExit("Build first: cargo build --offline --release")
    unittest.main(verbosity=2)
