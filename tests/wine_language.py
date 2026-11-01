#!/usr/bin/env python3
"""Optional real-Wine locale smoke check, with no game or component downloads."""
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
PROBE = r'''
#include <windows.h>
#include <stdio.h>
#include <string.h>
int main(void) {
    WCHAR name[85]; char system[256], user[256];
    GetSystemDefaultLocaleName(name,85);
    WideCharToMultiByte(CP_UTF8,0,name,-1,system,sizeof(system),NULL,NULL);
    GetUserDefaultLocaleName(name,85);
    WideCharToMultiByte(CP_UTF8,0,name,-1,user,sizeof(user),NULL,NULL);
    printf("ACP=%u OEMCP=%u SYSTEM=%s USER=%s\n",GetACP(),GetOEMCP(),system,user);
    return GetACP()==1251 && GetOEMCP()==866 && !strcmp(system,"ru-RU") && !strcmp(user,"ru-RU") ? 0 : 10;
}
'''


@unittest.skipUnless(all(shutil.which(tool) for tool in ['wine', 'wineboot', 'winegcc', 'wineserver']),
                     'Wine and Wine development tools required')
class RealWineLanguage(unittest.TestCase):
    def test_private_locale_and_wine_fallback_have_russian_codepages(self):
        with tempfile.TemporaryDirectory(prefix='alol-wine-language-') as temporary:
            root = Path(temporary)
            source = root/'probe.c'
            source.write_text(PROBE)
            subprocess.run(['winegcc', '-m64', '-o', str(root/'probe.exe'), str(source)],
                           check=True, capture_output=True, timeout=60)
            for mode in ['automatic', 'no-localedef']:
                with self.subTest(mode=mode):
                    current = root/mode
                    config = current/'config/alol'
                    config.mkdir(parents=True)
                    prefix = current/'prefixes/wine'
                    prefix.parent.mkdir(parents=True)
                    env = dict(os.environ, XDG_CONFIG_HOME=str(current/'config'),
                               XDG_DATA_HOME=str(current/'data'), WINEPREFIX=str(prefix),
                               DISPLAY='', WAYLAND_DISPLAY='', WINEDEBUG='-all',
                               WINEDLLOVERRIDES='winemenubuilder.exe=d', LC_ALL='C.UTF-8')
                    config.joinpath('settings.conf').write_text(
                        f'runner=wine\nwine=wine\nprefix_root={current}/prefixes\n')
                    try:
                        # Initialize under a non-Russian locale first, so this also
                        # checks an existing prefix created before the update.
                        boot = subprocess.run(['wineboot', '-u'], env=env, capture_output=True,
                                              text=True, timeout=90)
                        self.assertEqual(boot.returncode, 0, boot.stdout+boot.stderr)
                        if mode == 'no-localedef':
                            tools = current/'bin'
                            tools.mkdir()
                            for name in ['locale', 'localedef']:
                                tool = tools/name
                                tool.write_text('#!/bin/sh\nexit 1\n')
                                tool.chmod(0o755)
                            env['PATH'] = str(tools)+os.pathsep+env['PATH']
                            # Nonempty LOCPATH bypasses the system locale archive.
                            env['LOCPATH'] = str(current/'missing-locales')
                        result = subprocess.run([str(ROOT/'target/release/alol'), 'repair-language'],
                                                env=env, capture_output=True, text=True, timeout=90)
                        self.assertEqual(result.returncode, 0, result.stdout+result.stderr)
                        if mode == 'no-localedef':
                            self.assertIn('встроенная ru-RU Wine', result.stderr)
                        private = list((current/'data/alol/locales').glob('*/ru_RU.UTF-8/LC_CTYPE'))
                        if private:
                            env['LOCPATH'] = str(private[0].parent.parent)
                        env.update(LC_ALL='ru_RU.UTF-8', LANG='ru_RU.UTF-8')
                        result = subprocess.run(['wine', str(root/'probe.exe.so')], env=env,
                                                capture_output=True, text=True, timeout=60)
                        self.assertEqual(result.returncode, 0, result.stdout+result.stderr)
                        self.assertIn('ACP=1251 OEMCP=866 SYSTEM=ru-RU USER=ru-RU', result.stdout)
                    finally:
                        subprocess.run(['wineserver', '-k'], env=env, capture_output=True, timeout=15)
                        subprocess.run(['wineserver', '-w'], env=env, capture_output=True, timeout=15)


if __name__ == '__main__':
    unittest.main(verbosity=2)
