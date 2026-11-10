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
static int check_game_fonts(void) {
    const WCHAR *names[] = {L"segoeui.ttf", L"seguisb.ttf"};
    for (int i=0; i<2; ++i) {
        WCHAR path[MAX_PATH], face[128];
        if (!GetWindowsDirectoryW(path, MAX_PATH)) return 20;
        lstrcatW(path, L"\\Fonts\\"); lstrcatW(path, names[i]);
        HANDLE file=CreateFileW(path,GENERIC_READ,FILE_SHARE_READ,NULL,OPEN_EXISTING,0,NULL);
        if (file==INVALID_HANDLE_VALUE) return 21;
        CloseHandle(file);
        if (!AddFontResourceExW(path,FR_PRIVATE,NULL)) return 22;
        HDC dc=CreateCompatibleDC(NULL);
        HFONT font=CreateFontW(-16,0,0,0,FW_NORMAL,0,0,0,DEFAULT_CHARSET,0,0,0,0,L"Google Sans");
        if (!dc || !font) return 23;
        HGDIOBJ previous=SelectObject(dc,font);
        WORD glyphs[16];
        const WCHAR text[]=L"\x041d\x0430\x0441\x0442\x0440\x043e\x0439\x043a\x0438\x0401\x0451";
        int count=lstrlenW(text);
        if (GetGlyphIndicesW(dc,text,count,glyphs,GGI_MARK_NONEXISTING_GLYPHS)==GDI_ERROR) return 24;
        for (int j=0; j<count; ++j) if (glyphs[j]==0xffff) return 25;
        if (!GetTextFaceW(dc,128,face) || lstrcmpW(face,L"Google Sans")) return 26;
        SelectObject(dc,previous); DeleteObject(font); DeleteDC(dc);
        RemoveFontResourceExW(path,FR_PRIVATE,NULL);
    }
    puts("GAME_FONTS=readable CYRILLIC=passed");
    return 0;
}
int main(void) {
    WCHAR name[85]; char system[256], user[256];
    GetSystemDefaultLocaleName(name,85);
    WideCharToMultiByte(CP_UTF8,0,name,-1,system,sizeof(system),NULL,NULL);
    GetUserDefaultLocaleName(name,85);
    WideCharToMultiByte(CP_UTF8,0,name,-1,user,sizeof(user),NULL,NULL);
    printf("ACP=%u OEMCP=%u SYSTEM=%s USER=%s\n",GetACP(),GetOEMCP(),system,user);
    if (GetACP()!=1251 || GetOEMCP()!=866 || strcmp(system,"ru-RU") || strcmp(user,"ru-RU")) return 10;
    return check_game_fonts();
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
            subprocess.run(['winegcc', '-m64', '-o', str(root/'probe.exe'), str(source), '-lgdi32'],
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
                        self.assertIn('GAME_FONTS=readable CYRILLIC=passed', result.stdout)
                    finally:
                        subprocess.run(['wineserver', '-k'], env=env, capture_output=True, timeout=15)
                        subprocess.run(['wineserver', '-w'], env=env, capture_output=True, timeout=15)


if __name__ == '__main__':
    unittest.main(verbosity=2)
