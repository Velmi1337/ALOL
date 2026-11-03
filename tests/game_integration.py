#!/usr/bin/env python3
"""Direct game launch tests: no Electron process, persisted profile and native flags."""
import json
import hashlib
from pathlib import Path
import unittest
import integration as base

class DirectGame(unittest.TestCase):
    tearDown=base.Workflow.tearDown
    write_config=base.Workflow.write_config
    run_alol=base.Workflow.run_alol
    calls=base.Workflow.calls
    prefix=base.Workflow.prefix

    def setUp(self):
        base.Workflow.setUp(self)
        self.run_alol('install')
        self.game=self.prefix/'drive_c/Arizona Games Launcher/bin/arizona'
        self.game.mkdir(parents=True)
        self.exe=self.game/'gta_sa.exe';self.exe.write_bytes(b'patched Arizona game')
        self.native=self.prefix/'drive_c/users/steamuser/AppData/Roaming/arizona-launcher/settings.json'
        self.native.parent.mkdir(parents=True)
        self.native.write_text(json.dumps({'arizona':{'userName':'Native_Name','lastSelectedServerId':30,
            'password':{'value':'keep-existing-password'},'memory':{'value':3072},
            'options':[{'id':'wideScreen','value':True},{'id':'windowed','value':False},{'id':'autoLogin','value':True}]},
            'fprint':'private-existing-identity','installedMods':{'arizona':{'7':{'flag':'-custom_fx'}}},'keep':{'other':123}}))
        self.player=self.root/'config/alol/player.json'

    def game_calls(self):
        return [c for c in self.calls() if c['args'][0]==str(self.exe)]

    def keyboard_runtime(self):
        runtime = self.root/'keyboard-runtime'
        runtime.mkdir()
        (runtime/'proton').touch()
        files = {}
        for arch in ('i386','x86_64'):
            relative = f'files/lib/wine/{arch}-windows/user32.dll'
            path = runtime/relative
            path.parent.mkdir(parents=True)
            path.write_bytes(b'compiled keyboard DLL fixture')
            files[relative] = hashlib.sha256(path.read_bytes()).hexdigest()
        (runtime/'alol-runtime.json').write_text(json.dumps(
            {'keyboard_patch':{'protocol':'xkb-state-v1'},'recipe':{'engine_files':files}}))
        self.write_config(proton=str(runtime))
        return runtime

    def test_keyboard_bridge_only_reaches_game_and_cleans_its_state(self):
        self.keyboard_runtime()
        self.run_alol('play',DISPLAY=':invalid-alol-test',ALOL_KEYBOARD_STATE='inherited-stale-state')
        game = self.game_calls()[-1]
        self.assertTrue(game['keyboard_state'].startswith('C:\\alol\\keyboard-state-'))
        self.assertTrue(game['keyboard_state_exists'])
        self.assertEqual(list((self.prefix/'drive_c/alol').glob('keyboard-state-*.bin')),[])
        self.run_alol('launch',DISPLAY=':invalid-alol-test',ALOL_KEYBOARD_STATE='inherited-stale-state')
        self.assertIsNone(self.calls()[-1]['keyboard_state'])

    def test_damaged_keyboard_dll_stops_before_game_without_touching_it(self):
        runtime = self.keyboard_runtime()
        (runtime/'files/lib/wine/i386-windows/user32.dll').write_bytes(b'damaged DLL')
        before = self.exe.read_bytes()
        result = self.run_alol('play',success=False,DISPLAY=':invalid-alol-test')
        self.assertIn('повреждён',result.stderr)
        self.assertEqual(self.game_calls(),[])
        self.assertEqual(self.exe.read_bytes(),before)

    def test_keyboard_patch_without_x11_preserves_native_wayland_fallback(self):
        self.keyboard_runtime()
        self.run_alol('play',DISPLAY='',WAYLAND_DISPLAY='wayland-test')
        self.assertIsNone(self.game_calls()[-1]['keyboard_state'])
        self.assertEqual(self.game_calls()[-1]['wayland'],'wayland-test')

    def test_keyboard_alias_uses_only_runtime_in_selected_profile(self):
        runtime = self.keyboard_runtime()
        selected = self.data/'alol/runtimes/ALOL-Proton-0.1.0-kbd1-x86_64'
        selected.parent.mkdir(parents=True)
        runtime.rename(selected)
        self.write_config(proton='ALOL-Proton-Keyboard')
        self.run_alol('play',DISPLAY=':invalid-alol-test')
        self.assertEqual(self.game_calls()[-1]['proton'],str(selected))
        self.assertTrue(self.game_calls()[-1]['keyboard_state_exists'])

    def test_keyboard_patch_with_native_input_does_not_report_xwayland_state(self):
        self.keyboard_runtime()
        self.write_config(proton=self.settings['proton'],input_compatibility='false')
        self.run_alol('play',DISPLAY=':invalid-alol-test',WAYLAND_DISPLAY='wayland-test',PROTON_ENABLE_WAYLAND='1')
        self.assertIsNone(self.game_calls()[-1]['keyboard_state'])
        self.assertEqual(self.game_calls()[-1]['wayland'],'wayland-test')

    def test_imports_nickname_and_server_and_launches_game_without_electron(self):
        before=len(self.calls())
        result=self.run_alol('play')
        calls=self.calls()[before:]
        self.assertEqual(len(calls),1)
        args=calls[0]['args']
        self.assertEqual(args[0],str(self.exe))
        self.assertEqual(args[args.index('-n')+1],'Native_Name')
        self.assertEqual(args[args.index('-h')+1],'love.arizona-rp.com')
        self.assertEqual(args[args.index('-p')+1],'7777')
        self.assertEqual(args[args.index('-mem')+1],'3072')
        self.assertEqual(args[args.index('-userId')+1],'private-existing-identity')
        self.assertEqual(args[args.index('-cdn')+1],'0,0,0')
        for flag in ['-arizona','-window','-modern_scale','-widescreen','-custom_fx']:self.assertIn(flag,args)
        self.assertEqual(calls[0]['cwd'],str(self.game))
        self.assertEqual(calls[0]['lc_all'],'ru_RU.UTF-8')
        self.assertNotIn('private-existing-identity',result.stdout+result.stderr)
        self.assertNotIn('keep-existing-password',result.stdout+result.stderr)
        profile=json.loads(self.player.read_text())
        self.assertEqual(profile['server_id'],'30');self.assertEqual(profile['nickname'],'Native_Name')
        self.assertNotIn('logins',profile)
        self.assertNotIn('-x',args)

    def test_remembers_nickname_for_each_server_across_processes(self):
        self.run_alol('play',arguments=('1','First_Name'))
        self.run_alol('play',arguments=('2','Second_Name'))
        self.run_alol('play',arguments=('1',))
        self.assertEqual(self.game_calls()[-1]['args'][self.game_calls()[-1]['args'].index('-n')+1],'First_Name')
        self.run_alol('play')
        profile=json.loads(self.player.read_text())
        self.assertEqual(profile['server_id'],'1')
        self.assertEqual(profile['nicknames']['2'],'Second_Name')
        native=json.loads(self.native.read_text())
        self.assertEqual(native['arizona']['userName'],'First_Name')
        self.assertEqual(native['arizona']['lastSelectedServerId'],1)
        self.assertEqual(native['arizona']['password']['value'],'keep-existing-password')
        self.assertEqual(native['keep'],{'other':123})

    def test_invalid_nickname_or_server_never_launches_game(self):
        count=len(self.calls());native=self.native.read_bytes()
        for nick in ['AB','Русский_Ник','Nick Name','Nick;touch','A'*21]:
            self.run_alol('play',success=False,arguments=('1',nick))
        self.run_alol('play',success=False,arguments=('99999','Nick_Name'))
        self.assertEqual(len(self.calls()),count)
        self.assertEqual(self.native.read_bytes(),native)

    def test_missing_game_does_not_launch_official_launcher_implicitly(self):
        self.exe.unlink();before=len(self.calls())
        self.run_alol('play',success=False)
        self.assertEqual(len(self.calls()),before)

    def test_game_watch_cleans_new_metadata_in_actual_game_directory(self):
        sidecar=self.game/'fontcustom/game.ttf:Zone.Identifier'
        self.run_alol('play',ALOL_TEST_DROP_ZONE=str(sidecar),ALOL_TEST_ASSERT_ZONE_CLEANED='1')
        self.assertFalse(sidecar.exists())
        self.assertEqual(len(list((self.data/'alol/backups/zone-identifiers').rglob('game.ttf:Zone.Identifier'))),1)

    def test_game_watch_falls_back_to_polling_if_inotify_is_unavailable(self):
        import shutil,subprocess
        compiler=shutil.which('cc')
        if not compiler:self.skipTest('C compiler required for inotify failure injection')
        library=self.root/'no-inotify.so'
        code='#include <errno.h>\nint inotify_init1(int flags) { errno=38; return -1; }\n'
        subprocess.run([compiler,'-shared','-fPIC','-x','c','-','-o',str(library)],input=code,text=True,check=True,capture_output=True)
        sidecar=self.game/'fontcustom/fallback.ttf:Zone.Identifier'
        self.run_alol('play',LD_PRELOAD=str(library),ALOL_TEST_DROP_ZONE=str(sidecar),ALOL_TEST_ASSERT_ZONE_CLEANED='1')
        self.assertFalse(sidecar.exists())

    def test_alternative_native_game_executable_priority(self):
        alternative=self.game/'enb.exe';alternative.write_bytes(b'provided game wrapper')
        self.run_alol('play')
        self.assertEqual(self.calls()[-1]['args'][0],str(alternative))

    def test_server_catalog_refresh_and_cached_population(self):
        fixture=self.root/'servers.json'
        fixture.write_text(json.dumps({'arizona':[{'number':30,'name':'Love','ip':'192.0.2.30','port':7788,'online':12,'maxplayers':1000}]}))
        curl=self.bin/'curl'
        curl.write_text("#!/usr/bin/python3\nimport sys,shutil,os\na=sys.argv[1:]\nshutil.copyfile(os.environ['ALOL_TEST_SERVERS'],a[a.index('--output')+1])\n")
        curl.chmod(0o755);self.env['ALOL_TEST_SERVERS']=str(fixture)
        self.run_alol('servers-refresh')
        self.run_alol('play')
        args=self.game_calls()[-1]['args']
        self.assertEqual(args[args.index('-h')+1],'192.0.2.30')
        self.assertEqual(args[args.index('-p')+1],'7788')
        cache=self.data/'alol/servers/catalog.json';before=cache.read_bytes()
        fixture.write_text('{invalid')
        self.run_alol('servers-refresh',success=False)
        self.assertEqual(cache.read_bytes(),before)

    def test_offline_server_is_not_launched(self):
        cache=self.data/'alol/servers/catalog.json';cache.parent.mkdir(parents=True)
        cache.write_text(json.dumps({'arizona':[{'number':30,'name':'Love','ip':'192.0.2.30','port':7777,'online':0,'maxplayers':0}]}))
        count=len(self.calls());self.run_alol('play',success=False)
        self.assertEqual(len(self.calls()),count)

    def test_game_options_override_native_flags_and_preserve_unknown_settings(self):
        options=self.root/'config/alol/game-options.json'
        settings={'windowed':True,'preload':True,'shitPc':True,'newGraphics':True,
                  'grass':True,'oldResolution':True,'hdrResolution':True,'authCef':True,'cefDirtyRects':True}
        options.write_text(json.dumps({'memory':4096,'options':settings}))
        native=json.loads(self.native.read_text())
        native['arizona']['options'].append({'id':'autoClean','value':False,'extra':'preserved'})
        native['arizona']['memory']['min']=1024
        self.native.write_text(json.dumps(native))
        self.run_alol('play')
        args=self.game_calls()[-1]['args']
        self.assertEqual(args[args.index('-mem')+1],'4096')
        for flag in ['-ldo','-t','-trees_new','-enable_grass','-16bpp','-allow_hdr','-auth_cef_enable','-cef_dirty_rects']:
            self.assertIn(flag,args)
        self.assertNotIn('-modern_scale',args);self.assertNotIn('-widescreen',args)
        new=json.loads(self.native.read_text())
        self.assertEqual(new['arizona']['memory']['min'],1024)
        self.assertIn({'id':'autoClean','value':False,'extra':'preserved'},new['arizona']['options'])
        self.assertEqual(new['keep'],{'other':123})
        self.run_alol('launch')
        self.assertEqual(json.loads(self.native.read_text())['arizona']['memory']['value'],4096)

    def test_invalid_game_memory_does_not_launch_or_change_native_settings(self):
        options=self.root/'config/alol/game-options.json'
        options.write_text(json.dumps({'memory':999999,'options':{}}))
        count=len(self.calls());before=self.native.read_bytes()
        self.run_alol('play',success=False)
        self.assertEqual(len(self.calls()),count);self.assertEqual(self.native.read_bytes(),before)

    def test_fps_display_preserves_graphics_sync_and_legacy_settings_are_ignored(self):
        self.write_config(fps_hud='true',alt_tab='true',fps_limit='60',game_vsync='off')
        self.run_alol('play',DXVK_CONFIG='user configuration',DXVK_FRAME_RATE='123',DXVK_HUD='devinfo')
        call=self.game_calls()[-1]
        self.assertEqual(call['dxvk_config'],'user configuration')
        self.assertEqual(call['dxvk_frame_rate'],'123')
        self.assertEqual(call['dxvk_hud'],'devinfo,fps,frametimes')
        self.assertIn('-modern_scale',call['args'])
        self.assertFalse(next(o for o in json.loads(self.native.read_text())['arizona']['options'] if o['id']=='windowed')['value'])
        self.write_config(wined3d='true',fps_hud='true')
        self.run_alol('play',DXVK_HUD='devinfo')
        self.assertEqual(self.game_calls()[-1]['dxvk_hud'],'devinfo')

    def test_game_mode_wraps_only_game_without_modifying_graphics(self):
        trace=self.root/'gamemode-calls'
        wrapper=self.bin/'gamemoderun'
        wrapper.write_text('#!/usr/bin/python3\nimport os,sys,pathlib\np=pathlib.Path(os.environ["ALOL_GAME_MODE_TRACE"]);p.write_text("started")\nos.execvp(sys.argv[1],sys.argv[1:])\n')
        wrapper.chmod(0o755)
        self.write_config(game_mode='true')
        options=self.root/'config/alol/game-options.json'
        config={'memory':3072,'options':{'windowed':False,'wideScreen':True,'newGraphics':True,'grass':True}}
        options.write_text(json.dumps(config))
        self.run_alol('play',ALOL_GAME_MODE_TRACE=str(trace),DXVK_CONFIG='custom',DXVK_FRAME_RATE='144')
        self.assertTrue(trace.exists())
        saved=json.loads(options.read_text())
        self.assertEqual(saved['memory'],config['memory'])
        for key,value in config['options'].items(): self.assertEqual(saved['options'][key],value)
        call=self.game_calls()[-1]
        self.assertIn('-trees_new',call['args']);self.assertIn('-enable_grass',call['args'])
        self.assertNotIn('-t',call['args'])
        self.assertEqual(call['dxvk_config'],'custom');self.assertEqual(call['dxvk_frame_rate'],'144')
        trace.unlink()
        self.run_alol('launch',ALOL_GAME_MODE_TRACE=str(trace))
        self.assertFalse(trace.exists())
        self.write_config(game_mode='false')
        self.run_alol('play',ALOL_GAME_MODE_TRACE=str(trace))
        self.assertFalse(trace.exists())

    def test_missing_game_mode_is_nonfatal(self):
        self.write_config(game_mode='true')
        # The fixture PATH deliberately has no GameMode. ALOL must still launch.
        self.run_alol('play')
        self.assertEqual(len(self.game_calls()),1)

    def test_input_compatibility_bypasses_ime_and_uses_real_xwayland_socket(self):
        original = self.native.read_bytes()
        self.run_alol('play', DISPLAY=':123', WAYLAND_DISPLAY='wayland-test',
                      PROTON_ENABLE_WAYLAND='1', XMODIFIERS='@im=ibus', GTK_IM_MODULE='ibus',
                      QT_IM_MODULE='fcitx', MANGOHUD='1')
        call = self.game_calls()[-1]
        self.assertEqual(call['gameid'], 'umu-alolarizonadev')
        metadata = self.data/'applications/steam_app_alolarizonadev.desktop'
        self.assertIn('NoDisplay=true', metadata.read_text())
        self.assertFalse((self.data/'applications/steam_app_default.desktop').exists())
        self.assertEqual(call['display'], ':123')
        self.assertIsNone(call['wayland'])
        self.assertEqual(call['proton_wayland'], '0')
        self.assertEqual(call['xim'], '@im=none')
        self.assertEqual(call['mangohud'], '1')
        reg = (self.prefix/'drive_c/alol/input.reg').read_text(encoding='utf-16')
        self.assertIn('AppDefaults\\gta_sa.exe\\X11 Driver', reg)
        self.assertIn('"UseXIM"="N"', reg)
        self.assertEqual(json.loads(self.native.read_bytes())['arizona']['password'], json.loads(original)['arizona']['password'])

    def test_input_compatibility_can_be_disabled_and_reenabled(self):
        self.write_config(input_compatibility='false')
        self.run_alol('play', DISPLAY=':123', WAYLAND_DISPLAY='wayland-test',
                      PROTON_ENABLE_WAYLAND='1', XMODIFIERS='@im=ibus')
        call = self.game_calls()[-1]
        self.assertEqual(call['wayland'], 'wayland-test')
        self.assertEqual(call['proton_wayland'], '1')
        self.assertEqual(call['xim'], '@im=ibus')
        self.assertIn('"UseXIM"=-', (self.prefix/'drive_c/alol/input.reg').read_text(encoding='utf-16'))
        count = len(self.calls())
        self.run_alol('play')
        self.assertEqual(len(self.calls()), count+1)
        self.write_config(input_compatibility='true')
        self.run_alol('play')
        self.assertIn('"UseXIM"="N"', (self.prefix/'drive_c/alol/input.reg').read_text(encoding='utf-16'))

    def test_missing_display_keeps_native_wayland_input_available(self):
        self.run_alol('play', DISPLAY='', WAYLAND_DISPLAY='wayland-test', PROTON_ENABLE_WAYLAND='1')
        call = self.game_calls()[-1]
        self.assertEqual(call['wayland'], 'wayland-test')
        self.assertEqual(call['proton_wayland'], '1')

    def test_game_preserves_user_graphics_environment(self):
        self.write_config(input_compatibility='false')
        self.run_alol('play', DISPLAY=':123', WAYLAND_DISPLAY='wayland-test', GDK_BACKEND='wayland',
                      QT_QPA_PLATFORM='wayland', MANGOHUD='1', PROTON_ENABLE_WAYLAND='1')
        call = self.game_calls()[-1]
        self.assertEqual(call['wayland'], 'wayland-test')
        self.assertEqual(call['mangohud'], '1')
        self.assertEqual(call['proton_wayland'], '1')

    def test_upgrade_removes_passwords_and_only_owned_helper_files(self):
        old={'server_id':'30','nickname':'Native_Name','nicknames':{'30':'Native_Name','1':'Other_Name'},
             'logins':{'30':{'nickname':'Native_Name','enabled':True,'password':'retired-test-secret'}}}
        self.player.write_text(json.dumps(old))
        helper=self.game/'moonloader/alol_autopassword.lua';helper.parent.mkdir(parents=True)
        helper.write_text('-- ALOL owned autopassword helper v1\nold helper')
        other=helper.with_name('my_mod.lua');other.write_text('-- my mod')
        loader=self.game/'MoonLoader.asi';loader.write_bytes(b'keep loader')
        lease=self.prefix/'drive_c/alol'/('login-'+'b'*48+'.bin');lease.write_bytes(b'retired-test-secret')
        unrelated=lease.with_name('login-my-file.bin');unrelated.write_bytes(b'keep file')
        self.run_alol('repair-game')
        profile=json.loads(self.player.read_text())
        self.assertEqual(profile,{k:v for k,v in old.items() if k!='logins'})
        self.assertEqual(self.player.stat().st_mode & 0o777,0o600)
        self.assertFalse(helper.exists());self.assertFalse(lease.exists())
        self.assertEqual(other.read_text(),'-- my mod')
        self.assertEqual(loader.read_bytes(),b'keep loader')
        self.assertEqual(unrelated.read_bytes(),b'keep file')
        # Unknown files with the old helper's name belong to the user.
        helper.write_text('-- user-owned script')
        result=self.run_alol('play')
        self.assertEqual(helper.read_text(),'-- user-owned script')
        self.assertNotIn('-x',self.game_calls()[-1]['args'])
        self.assertNotIn('retired-test-secret',result.stdout+result.stderr)

if __name__=='__main__':unittest.main(verbosity=2)
