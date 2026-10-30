#!/usr/bin/env python3
"""Process tests for reversible mods and untrusted archives, with official-shaped API fixtures."""
import hashlib
import json
import os
from pathlib import Path
import stat
import unittest
import zipfile
import integration as base

class Mods(unittest.TestCase):
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
        (self.game/'gta_sa.exe').write_bytes(b'original game')
        self.native=self.prefix/'drive_c/users/steamuser/AppData/Roaming/arizona-launcher/settings.json'
        self.native.parent.mkdir(parents=True)
        self.native.write_text(json.dumps({'arizona':{'gamePath':r'C:\Arizona Games Launcher\bin\arizona'},'preserve':{'setting':123}}))

    def states(self):
        return [json.loads(p.read_text()) for p in (self.prefix/'.alol-mods').glob('*/*/state.json')]

    def add(self,path,mode='auto',success=True):
        return self.run_alol('mods-add',success=success,arguments=(path,mode))

    def archive(self,name,items):
        path=self.root/name
        with zipfile.ZipFile(path,'w') as z:
            for k,v in items.items():z.writestr(k,v)
        return path

    def fixture_api(self,catalog,manifest,content=b'mod content',bad_primary=False):
        payload=self.root/'payload.bin';payload.write_bytes(content)
        bad=self.root/'bad.bin';bad.write_bytes(b'x'*len(content))
        catalog_file=self.root/'catalog.json';catalog_file.write_text(json.dumps(catalog))
        manifest_file=self.root/'manifest.json';manifest_file.write_text(json.dumps(manifest))
        mapping={
            'https://api-samp.arizona-five.com/launcher/mods/arizona':str(catalog_file),
            'https://api-samp.arizona-five.com/launcher/mods/arizona/7':str(manifest_file),
            'https://pc.az-ins.com/resource/mods/7/cleo/test.cs':str(bad if bad_primary else payload),
            'https://arz-pc.react-group.tech/resource/mods/7/cleo/test.cs':str(payload),
        }
        fixtures=self.root/'fixtures.json';fixtures.write_text(json.dumps(mapping))
        self.env['ALOL_TEST_HTTP']=str(fixtures)
        curl=self.bin/'curl'
        curl.write_text("""#!/usr/bin/python3
import json,os,pathlib,shutil,sys
args=sys.argv[1:];mapping=json.loads(pathlib.Path(os.environ['ALOL_TEST_HTTP']).read_text())
url=args[-1]
if url not in mapping:sys.exit(22)
shutil.copyfile(mapping[url],args[args.index('--output')+1])
""")
        curl.chmod(0o755)

    def test_custom_mod_disable_enable_and_remove_restore_originals(self):
        (self.game/'test.asi').write_bytes(b'original')
        package=self.archive('custom.zip',{'test.asi':b'mod','scripts/config.ini':b'config'})
        self.add(package,'root')
        record=self.states()[0];self.assertTrue(record['active'])
        self.assertEqual((self.game/'test.asi').read_bytes(),b'mod')
        native=json.loads(self.native.read_text())
        self.assertIn(record['id'],native['installedMods']['arizona'])
        self.assertEqual(native['preserve'],{'setting':123})
        self.run_alol('mods-disable',arguments=(record['id'],))
        self.assertEqual((self.game/'test.asi').read_bytes(),b'original')
        self.assertFalse((self.game/'scripts/config.ini').exists())
        self.run_alol('mods-enable',arguments=(record['id'],))
        self.assertEqual((self.game/'test.asi').read_bytes(),b'mod')
        self.run_alol('mods-remove',arguments=(record['id'],))
        self.assertEqual((self.game/'test.asi').read_bytes(),b'original')
        self.assertEqual(self.states(),[])

    def test_local_script_auto_folder_and_windows_metadata_skip(self):
        package=self.archive('Русский мод.zip',{'wrapper/мой.lua':b'lua','wrapper/мой.lua:Zone.Identifier':b'[ZoneTransfer]\nZoneId=3'})
        self.add(package)
        self.assertEqual((self.game/'moonloader/мой.lua').read_bytes(),b'lua')
        self.assertFalse(list(self.game.rglob('*:Zone.Identifier')))

    def test_archives_cannot_escape_game_or_add_links(self):
        for name,member in [('escape.zip','../outside.asi'),('windows.zip','C:\\escape.asi')]:
            package=self.archive(name,{member:b'bad'})
            self.add(package,success=False)
        path=self.root/'link.zip'
        with zipfile.ZipFile(path,'w') as z:
            info=zipfile.ZipInfo('link.asi');info.create_system=3;info.external_attr=(stat.S_IFLNK|0o777)<<16
            z.writestr(info,'/tmp/elsewhere')
        self.add(path,success=False)
        self.assertFalse((self.root/'outside.asi').exists())
        self.assertEqual((self.game/'gta_sa.exe').read_bytes(),b'original game')

    def test_symlink_destination_is_rejected(self):
        external=self.root/'external';external.mkdir()
        (self.game/'moonloader').symlink_to(external)
        script=self.root/'script.lua';script.write_bytes(b'lua')
        self.add(script,success=False)
        self.assertFalse((external/'script.lua').exists())

    def test_changed_files_are_preserved_when_uninstalling(self):
        script=self.root/'feature.asi';script.write_bytes(b'mod')
        self.add(script)
        record=self.states()[0]
        (self.game/'feature.asi').write_bytes(b'changed later')
        self.run_alol('mods-remove',success=False,arguments=(record['id'],))
        self.assertEqual((self.game/'feature.asi').read_bytes(),b'changed later')
        self.assertTrue(self.states()[0]['active'])

    def test_active_mod_conflict_does_not_replace_files(self):
        a=self.root/'a';b=self.root/'b';a.mkdir();b.mkdir()
        (a/'shared.asi').write_bytes(b'first');(b/'shared.asi').write_bytes(b'second')
        self.add(a/'shared.asi')
        self.add(b/'shared.asi',success=False)
        self.assertEqual((self.game/'shared.asi').read_bytes(),b'first')
        self.assertEqual(sum(v['active'] for v in self.states()),1)

    def test_case_collisions_are_rejected(self):
        package=self.archive('case.zip',{'mod.ini':b'a','MOD.INI':b'b'})
        self.add(package,success=False)
        self.assertFalse((self.game/'mod.ini').exists())

    def test_official_catalog_manifest_hash_fallback_and_install(self):
        content=b'mod content'
        catalog={'topOfMonth':[{'id':7,'name':'Официальный мод','version':2,'description':'Тест'}],
                 'mods':[{'id':7,'name':'Официальный мод'},{'id':8,'visible':False}]}
        manifest={'data':[{'name':'cleo','type':'dir','data':[{'name':'test.cs','type':'file','size':len(content),'hash':hashlib.md5(content).hexdigest()}]}]}
        self.fixture_api(catalog,manifest,content,bad_primary=True)
        self.run_alol('mods-refresh')
        self.run_alol('mods-install',arguments=('7',))
        self.assertEqual((self.game/'cleo/test.cs').read_bytes(),content)
        record=self.states()[0]
        self.assertEqual(record['id'],'official-7')
        self.assertEqual(record['source'],'official')
        native=json.loads(self.native.read_text())
        self.assertEqual(native['installedMods']['arizona']['7']['id'],7)
        self.assertEqual(native['installedMods']['arizona']['7']['version'],2)
        self.assertEqual(native['installedMods']['arizona']['7']['installedFilesRelativePaths'],['cleo/test.cs'])

    def test_bad_official_hash_keeps_game_untouched(self):
        catalog={'mods':[{'id':7,'name':'Bad hash'}]}
        manifest={'data':[{'name':'cleo','type':'dir','data':[{'name':'test.cs','type':'file','size':11,'hash':'0'*32}]}]}
        self.fixture_api(catalog,manifest)
        self.run_alol('mods-refresh')
        self.run_alol('mods-install',success=False,arguments=('7',))
        self.assertFalse((self.game/'cleo/test.cs').exists())
        self.assertEqual((self.game/'gta_sa.exe').read_bytes(),b'original game')

    def test_interrupted_transaction_is_recovered_before_next_change(self):
        original=self.game/'existing.asi';original.write_bytes(b'original')
        path=self.root/'existing.asi';path.write_bytes(b'mod')
        self.add(path)
        state_path=next((self.prefix/'.alol-mods').glob('*/*/state.json'))
        state=json.loads(state_path.read_text());state['status']='installing';state['active']=False
        state_path.write_text(json.dumps(state))
        other=self.root/'other.asi';other.write_bytes(b'other')
        self.add(other)
        self.assertEqual(original.read_bytes(),b'original')
        self.assertEqual((self.game/'other.asi').read_bytes(),b'other')

    def test_case_insensitive_destinations_use_existing_linux_directory(self):
        data=self.game/'data';data.mkdir();(data/'test.cfg').write_bytes(b'original')
        package=self.archive('folder.zip',{'DATA/test.cfg':b'mod'})
        self.add(package,'root')
        self.assertEqual((data/'test.cfg').read_bytes(),b'mod')
        self.assertFalse((self.game/'DATA').exists())
        record=self.states()[0];self.run_alol('mods-disable',arguments=(record['id'],))
        self.assertEqual((data/'test.cfg').read_bytes(),b'original')

    def test_catalog_refresh_failure_preserves_cached_catalog(self):
        catalog={'mods':[{'id':7,'name':'Cached'}]}
        self.fixture_api(catalog,{'data':[]})
        self.run_alol('mods-refresh')
        cache=self.data/'alol/mods/catalog.json';before=cache.read_bytes()
        Path(self.env['ALOL_TEST_HTTP']).write_text('{}')
        self.run_alol('mods-refresh',success=False)
        self.assertEqual(cache.read_bytes(),before)

    @unittest.skipIf(os.geteuid()==0,'requires normal user permissions')
    def test_write_failure_rolls_back_already_replaced_files(self):
        original=self.game/'a.asi';original.write_bytes(b'original')
        directory=self.game/'z';directory.mkdir();directory.chmod(0o555)
        package=self.archive('rollback.zip',{'a.asi':b'mod','z/new.asi':b'new'})
        try:
            self.add(package,'root',success=False)
            self.assertEqual(original.read_bytes(),b'original')
            self.assertFalse((directory/'new.asi').exists())
            self.assertFalse(any(v['active'] for v in self.states()))
        finally:directory.chmod(0o755)

if __name__=='__main__':unittest.main(verbosity=2)
