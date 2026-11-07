#!/usr/bin/env python3
"""Two-project workflows using isolated prefixes, fake Wine and official-shaped APIs."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import time
import unittest
import zipfile
import integration as base


class Projects(unittest.TestCase):
    tearDown = base.Workflow.tearDown
    write_config = base.Workflow.write_config
    run_alol = base.Workflow.run_alol
    calls = base.Workflow.calls
    prefix = base.Workflow.prefix

    def setUp(self):
        base.Workflow.setUp(self)
        self.env['ALOL_DESKTOP_DATA_HOME'] = str(self.data)
        self.run_alol('install')
        self.games = {}
        for project in ('arizona', 'rodina'):
            directory = self.prefix / 'drive_c/Arizona Games Launcher/bin' / project
            directory.mkdir(parents=True)
            (directory / 'gta_sa.exe').write_bytes(project.encode())
            self.games[project] = directory
        self.native = self.prefix / 'drive_c/users/steamuser/AppData/Roaming/arizona-launcher/settings.json'
        self.native.parent.mkdir(parents=True)
        self.initial = {
            'arizona': {'userName': 'Arizona_Player', 'lastSelectedServerId': 1, 'password': {'value': 'private-arizona'},
                        'memory': {'value': 3072}, 'options': [{'id': 'authCef', 'value': True}, {'id': 'newGraphics', 'value': True}]},
            'rodina': {'userName': 'Rodina_Player', 'lastSelectedServerId': 3, 'password': {'value': 'private-rodina'},
                       'memory': {'value': 2048}, 'options': [{'id': 'graphics', 'value': True}, {'id': 'wideScreen', 'value': True}, {'id': 'autoLogin', 'value': True}]},
            'fprint': 'private-existing-identity', 'keep': {'setting': 123},
        }
        self.native.write_text(json.dumps(self.initial))

    def run_for(self, project, action, arguments=(), success=True, **env):
        result = subprocess.run([str(base.BINARY), '--project', project, action, *map(str, arguments)],
                                env=dict(self.env, **env), capture_output=True, text=True, timeout=12)
        self.assertEqual(result.returncode == 0, success, result.stdout + result.stderr)
        return result

    def game_calls(self, project):
        return [call for call in self.calls() if call['args'][0] == str(self.games[project] / 'gta_sa.exe')]

    def http(self, mapping):
        manifest = self.root / 'http.json'
        manifest.write_text(json.dumps({url: str(path) for url, path in mapping.items()}))
        self.env['ALOL_TEST_HTTP'] = str(manifest)
        self.env['ALOL_PROJECT_HTTP_TRACE'] = str(self.root / 'http.trace')
        curl = self.bin / 'curl'
        curl.write_text('''#!/usr/bin/python3
import json,os,pathlib,shutil,sys
args=sys.argv[1:];url=args[-1]
with open(os.environ['ALOL_PROJECT_HTTP_TRACE'],'a') as f:f.write(url+'\\n')
mapping=json.loads(pathlib.Path(os.environ['ALOL_TEST_HTTP']).read_text())
if url not in mapping:sys.exit(22)
shutil.copyfile(mapping[url],args[args.index('--output')+1])
''')
        curl.chmod(0o755)

    def test_rodina_launch_uses_its_server_flags_directory_and_compatible_runtime(self):
        result = self.run_for('rodina', 'play', DISPLAY=':project-test', WAYLAND_DISPLAY='wayland-test')
        call = self.game_calls('rodina')[-1]
        args = call['args']
        self.assertEqual(args[args.index('-h') + 1], 'northern.rodina-rp.com')
        self.assertEqual(args[args.index('-p') + 1], '8904')
        self.assertRegex(args[args.index('-n') + 1], r'^Player_[0-9a-f]{12}$')
        self.assertIn('-rodina', args)
        self.assertIn('-trees_new', args)
        for flag in ('-arizona', '-auth_cef_enable', '-enable_grass'):
            self.assertNotIn(flag, args)
        self.assertEqual(call['cwd'], str(self.games['rodina']))
        self.assertEqual(call['gameid'], 'umu-alolrodinadev')
        self.assertEqual(call['xim'], '@im=none')
        self.assertIsNone(call['wayland'])
        self.assertEqual(call['lc_all'], 'ru_RU.UTF-8')
        self.assertNotIn('private-existing-identity', result.stdout + result.stderr)
        native = json.loads(self.native.read_text())
        self.assertEqual(native['arizona'], self.initial['arizona'])
        self.assertEqual(native['rodina']['password'], self.initial['rodina']['password'])
        self.assertEqual(native['lastSelectedGameId'], 'rodina')
        metadata = self.data / 'applications/steam_app_alolrodinadev.desktop'
        self.assertIn('--project rodina launch', metadata.read_text())

    def test_rodina_needs_no_character_name_and_preserves_arizona_profile(self):
        self.run_for('arizona', 'play', ('1', 'Arizona_One'))
        arizona = self.root / 'config/alol/player.json'
        before = arizona.read_bytes()
        self.run_for('rodina', 'play', ('1', 'Rodina_One'))
        self.run_for('rodina', 'play', ('2', 'Rodina_Two'))
        self.run_for('rodina', 'play', ('1',))
        self.assertEqual(arizona.read_bytes(), before)
        profile = json.loads((self.root / 'config/alol/projects/rodina/player.json').read_text())
        self.assertEqual(profile, {'server_id': '1'})
        self.assertEqual(json.loads(self.native.read_text())['rodina']['userName'], 'Rodina_Player')
        names = [call['args'][call['args'].index('-n') + 1] for call in self.game_calls('rodina')]
        self.assertEqual(len(set(names)), 3)
        for name in names:
            self.assertRegex(name, r'^Player_[0-9a-f]{12}$')
        self.run_for('arizona', 'play')
        args = self.game_calls('arizona')[-1]['args']
        self.assertEqual(args[args.index('-n') + 1], 'Arizona_One')

    def test_missing_or_old_invalid_rodina_nickname_does_not_block_launch(self):
        native = json.loads(self.native.read_text())
        native['rodina'].pop('userName')
        self.native.write_text(json.dumps(native))
        self.run_for('rodina', 'play')
        profile = self.root / 'config/alol/projects/rodina/player.json'
        profile.write_text(json.dumps({'server_id': '2', 'nickname': 'old invalid name', 'nicknames': {'2': 'old invalid name'}}))
        self.run_for('rodina', 'play')
        self.assertEqual(json.loads(profile.read_text()), {'server_id': '2'})
        self.assertNotIn('userName', json.loads(self.native.read_text())['rodina'])

    def test_graphics_memory_and_native_settings_are_separate(self):
        arizona = self.root / 'config/alol/game-options.json'
        arizona.write_text(json.dumps({'memory': 4096, 'options': {'authCef': True, 'newGraphics': True}}))
        rodina = self.root / 'config/alol/projects/rodina/game-options.json'
        rodina.parent.mkdir(parents=True)
        rodina.write_text(json.dumps({'memory': 1024, 'options': {'graphics': True, 'preload': True}}))
        before = arizona.read_bytes()
        self.run_for('rodina', 'play')
        args = self.game_calls('rodina')[-1]['args']
        self.assertEqual(args[args.index('-mem') + 1], '1024')
        self.assertIn('-ldo', args)
        self.assertIn('-trees_new', args)
        self.assertNotIn('-auth_cef_enable', args)
        self.assertEqual(arizona.read_bytes(), before)
        native = json.loads(self.native.read_text())
        self.assertEqual(native['arizona'], self.initial['arizona'])
        self.assertTrue(next(o['value'] for o in native['rodina']['options'] if o['id'] == 'autoLogin'))
        self.assertFalse(any(o['id'] == 'authCef' for o in native['rodina']['options']))

    def test_official_launcher_opens_selected_project_and_preserves_other_game(self):
        self.run_for('rodina', 'launch')
        self.assertTrue(self.calls()[-1]['args'][0].endswith('Arizona Games Launcher.exe'))
        native = json.loads(self.native.read_text())
        self.assertEqual(native['lastSelectedGameId'], 'rodina')
        self.assertEqual(native['arizona'], self.initial['arizona'])
        self.assertEqual(native['keep'], self.initial['keep'])

    def test_custom_rodina_game_path_and_fps_hud_use_selected_game(self):
        destination = self.prefix / 'drive_c/Other Games/Rodina'
        destination.mkdir(parents=True)
        (destination / 'gta_sa.exe').write_bytes(b'Rodina custom install')
        native = json.loads(self.native.read_text())
        native['rodina']['gamePath'] = r'C:\Other Games\Rodina'
        self.native.write_text(json.dumps(native))
        self.write_config(fps_hud='true')
        self.run_for('rodina', 'play')
        call = self.calls()[-1]
        self.assertEqual(call['args'][0], str(destination / 'gta_sa.exe'))
        self.assertEqual(call['cwd'], str(destination))
        self.assertIn('fps', call['dxvk_hud'])
        self.assertIn('frametimes', call['dxvk_hud'])

    def test_launcher_watches_custom_rodina_download_folder_only(self):
        destination = self.prefix / 'drive_c/Other Games/Rodina'
        destination.mkdir(parents=True)
        native = json.loads(self.native.read_text())
        native['rodina']['gamePath'] = r'C:\Other Games\Rodina'
        self.native.write_text(json.dumps(native))
        untouched = self.games['arizona'] / 'old.ttf:Zone.Identifier'
        untouched.write_text('[ZoneTransfer]\nZoneId=3\n')
        dropped = destination / 'fonts/new.ttf:Zone.Identifier'
        self.run_for('rodina', 'launch', ALOL_TEST_DROP_ZONE=str(dropped), ALOL_TEST_ASSERT_ZONE_CLEANED='1')
        self.assertFalse(dropped.exists())
        self.assertTrue(untouched.is_file())

    def test_local_archive_install_and_rollback_affect_only_rodina(self):
        for project in self.games:
            (self.games[project] / 'shared.asi').write_bytes(project.encode())
        archive = self.root / 'mod.zip'
        with zipfile.ZipFile(archive, 'w') as stream:
            stream.writestr('shared.asi', b'custom mod')
        self.run_for('rodina', 'mods-add', (archive, 'root'))
        self.assertEqual((self.games['arizona'] / 'shared.asi').read_bytes(), b'arizona')
        self.assertEqual((self.games['rodina'] / 'shared.asi').read_bytes(), b'custom mod')
        native = json.loads(self.native.read_text())
        mod_id = next(iter(native['installedMods']['rodina']))
        self.assertNotIn('arizona', native['installedMods'])
        self.run_for('arizona', 'mods-remove', (mod_id,), success=False)
        self.assertEqual((self.games['rodina'] / 'shared.asi').read_bytes(), b'custom mod')
        self.run_for('rodina', 'mods-remove', (mod_id,))
        self.assertEqual((self.games['rodina'] / 'shared.asi').read_bytes(), b'rodina')

    def test_official_mod_uses_rodina_catalog_manifest_and_cdn(self):
        content = b'official Rodina payload'
        catalog = self.root / 'catalog.json'
        catalog.write_text(json.dumps({'mods': [{'id': 7, 'name': 'Rodina test', 'version': 1}]}))
        manifest = self.root / 'manifest.json'
        manifest.write_text(json.dumps({'data': [{'name': 'shared.asi', 'type': 'file', 'size': len(content), 'hash': hashlib.sha256(content).hexdigest()}]}))
        payload = self.root / 'payload.bin'
        payload.write_bytes(content)
        self.http({'https://api-samp.arizona-five.com/launcher/mods/rodina': catalog,
                   'https://api-samp.arizona-five.com/launcher/mods/rodina/7': manifest,
                   'https://pc.rod-ins.com/resource/mods/7/shared.asi': payload})
        self.run_for('rodina', 'mods-refresh')
        self.run_for('rodina', 'mods-install', ('7',))
        self.assertEqual((self.games['rodina'] / 'shared.asi').read_bytes(), content)
        self.assertFalse((self.games['arizona'] / 'shared.asi').exists())
        self.assertFalse((self.data / 'alol/mods/catalog.json').exists())
        self.assertTrue((self.data / 'alol/mods/rodina/catalog.json').is_file())
        self.assertNotIn('/mods/arizona', (self.root / 'http.trace').read_text())

    def test_server_refresh_caches_only_the_selected_project(self):
        source = self.root / 'servers.json'
        source.write_text(json.dumps({'rodina': [{'number': 3, 'name': 'Rodina fixture', 'ip': 'rodina.example', 'port': 8904}],
                                      'arizona': [{'number': 1, 'name': 'Arizona fixture', 'ip': 'arizona.example', 'port': 7777}]}))
        self.http({'https://api.arizona-five.com/launcher/servers': source})
        self.run_for('rodina', 'servers-refresh')
        self.run_for('rodina', 'play')
        args = self.game_calls('rodina')[-1]['args']
        self.assertEqual(args[args.index('-h') + 1], 'rodina.example')
        self.assertTrue((self.data / 'alol/servers/rodina/catalog.json').is_file())
        self.assertFalse((self.data / 'alol/servers/catalog.json').exists())

    def test_reset_rodina_graphics_preserves_arizona_and_legacy_settings(self):
        directory = self.prefix / 'drive_c/users/steamuser/Documents/GTA San Andreas User Files'
        directory.mkdir(parents=True)
        for name in ('rodina.set', 'arizona.set', 'gta_sa.set'):
            (directory / name).write_bytes(name.encode())
        self.run_for('rodina', 'reset-game')
        self.assertFalse((directory / 'rodina.set').exists())
        for name in ('arizona.set', 'gta_sa.set'):
            self.assertEqual((directory / name).read_bytes(), name.encode())
        self.assertEqual(len(list(directory.glob('rodina.set.alol-backup-*'))), 1)

    def test_project_cli_selection_is_explicit_and_invalid_ids_do_not_change_config(self):
        self.run_alol('project', arguments=('rodina',))
        self.assertIn('project=rodina\n', self.config.read_text())
        before = self.config.read_bytes()
        self.run_alol('project', arguments=('../../arizona',), success=False)
        self.assertEqual(self.config.read_bytes(), before)
        self.run_for('arizona', 'play')
        self.assertEqual(self.config.read_bytes(), before)

    def test_running_command_stays_on_rodina_when_saved_selection_changes(self):
        env = dict(self.env, ALOL_TEST_SLEEP=str(self.games['rodina'] / 'gta_sa.exe'),
                   ALOL_TEST_SLEEP_SECONDS='1')
        child = subprocess.Popen([str(base.BINARY), '--project', 'rodina', 'play'], env=env,
                                 stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        try:
            deadline = time.monotonic() + 5
            while not (self.root / 'ready').exists() and time.monotonic() < deadline:
                time.sleep(.02)
            self.assertTrue((self.root / 'ready').exists())
            self.run_alol('project', arguments=('arizona',))
            out, error = child.communicate(timeout=5)
            self.assertEqual(child.returncode, 0, out + error)
            self.assertIn('project=arizona\n', self.config.read_text())
            self.assertEqual(len(self.game_calls('rodina')), 1)
            self.assertFalse(self.game_calls('arizona'))
            self.assertFalse((self.root / 'config/alol/player.json').exists())
        finally:
            if child.poll() is None:
                child.terminate()
                child.wait(timeout=5)


if __name__ == '__main__':
    unittest.main(verbosity=2)
