#!/usr/bin/env python3
"""Real 32/64-bit Wine indicator regression on a private Xvfb and prefix.

--baseline verifies reproduction of the old constant-RU result, not a fix.
Neither the user's display/keymap, installed Wine prefix nor game is touched.
"""
import argparse
import ctypes as C
import importlib.util
import json
import os
from pathlib import Path
import shutil
import struct
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parents[1]
PROBE = r'''
#include <windows.h>
static void report(const char *kind, UINT value) {
 char line[256], name[9], wide_name[9]; WCHAR wide[9]; DWORD written;
 GetKeyboardLayoutNameA(name); GetKeyboardLayoutNameW(wide);
 WideCharToMultiByte(CP_UTF8,0,wide,-1,wide_name,sizeof(wide_name),0,0);
 int len=wsprintfA(line,"%s=%08X HKL=%08X NAME=%s WNAME=%s ACP=%u OEM=%u\n",kind,value,
                  (UINT)(UINT_PTR)GetKeyboardLayout(0),name,wide_name,GetACP(),GetOEMCP());
 WriteFile(GetStdHandle(STD_OUTPUT_HANDLE),line,len,&written,0);
}
static LRESULT CALLBACK proc(HWND w,UINT m,WPARAM a,LPARAM b) {
 if(m==WM_CHAR) report("CHAR",(UINT)a);
 if(m==WM_KEYDOWN) {report("KEY",(UINT)a);if(a==VK_ESCAPE)PostQuitMessage(0);}
 return DefWindowProcW(w,m,a,b);
}
void mainCRTStartup(void) {
 WCHAR russian[]={0x041f,0x0440,0x0438,0x0432,0x0435,0x0442,0};
 const BYTE expected[]={0xcf,0xf0,0xe8,0xe2,0xe5,0xf2}; char encoded[8]; BOOL used=FALSE;
 int count=WideCharToMultiByte(CP_ACP,0,russian,6,encoded,sizeof(encoded),0,&used);
 if(count!=6||used||GetACP()!=1251||GetOEMCP()!=866){report("FAIL",11);ExitProcess(11);}
 for(int i=0;i<6;i++)if((BYTE)encoded[i]!=expected[i]){report("FAIL",12);ExitProcess(12);}
 if(GetUserDefaultLCID()!=0x419||GetSystemDefaultLCID()!=0x419){report("FAIL",13);ExitProcess(13);}
 WNDCLASSW c={0};c.lpfnWndProc=proc;c.lpszClassName=L"ALOLLayoutProbe";RegisterClassW(&c);
 HWND w=CreateWindowW(c.lpszClassName,L"ALOL layout probe",WS_OVERLAPPEDWINDOW|WS_VISIBLE,0,0,400,300,0,0,0,0);
 if(!w)ExitProcess(14);
 SetForegroundWindow(w);SetFocus(w);report("READY",0);
 WCHAR state[2];
 if(!GetEnvironmentVariableW(L"ALOL_KEYBOARD_STATE",state,2))
  ExitProcess((UINT)(UINT_PTR)GetKeyboardLayout(0)==0x04190419 ? 0 : 16);
 for(unsigned i=0;i<900;i++) {
  MSG m;while(PeekMessageW(&m,0,0,0,PM_REMOVE)){
   if(m.message==WM_QUIT)ExitProcess(0);TranslateMessage(&m);DispatchMessageW(&m);
  }
  report("SAMPLE",i);Sleep(50);
 }
 ExitProcess(15);
}
'''


def wait_for(condition, timeout=15):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        result = condition()
        if result:
            return result
        time.sleep(.03)
    raise AssertionError('Timed out waiting for keyboard test condition')


def compile_probe(root, arch):
    source = root / 'probe.c'
    source.write_text(PROBE)
    obj, exe = root / f'{arch}.obj', root / f'{arch}.exe'
    subprocess.run(['clang', '--target=' + ('i686' if arch == 'i386' else arch) + '-windows-gnu',
                    '-ffreestanding', '-fno-stack-protector', '-Os', '-I/usr/include/wine/windows',
                    '-I/usr/include/wine/msvcrt', '-c', str(source), '-o', str(obj)], check=True, capture_output=True)
    libraries = Path('/usr/lib/wine') / (arch + '-windows')
    subprocess.run(['lld-link', '/entry:mainCRTStartup', '/subsystem:console', '/nodefaultlib',
                    '/safeseh:no', '/out:' + str(exe), str(obj), str(libraries / 'libuser32.a'),
                    str(libraries / 'libkernel32.a')], check=True, capture_output=True)
    return exe


def run(runtime, xvfb, baseline, umu=False):
    spec = importlib.util.spec_from_file_location('xkb_layout', ROOT / 'runtime/xkb_layout.py')
    bridge = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(bridge)
    parent = ROOT / '.dev-data/checks/keyboard' if umu else None
    if parent:
        parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='alol-keyboard-check-', dir=parent) as temp:
        root = Path(temp)
        prefix = root / 'prefixes/proton'
        prefix.parent.mkdir()
        env = dict(os.environ, WAYLAND_DISPLAY='', WINEPREFIX=str(prefix), WINEDEBUG='-all',
                   WINEDLLOVERRIDES='winemenubuilder.exe,mscoree,mshtml=d', LC_ALL='C.UTF-8',
                   XDG_CONFIG_HOME=str(root / 'config'), XDG_DATA_HOME=str(root / 'data'),
                   XDG_CACHE_HOME=str(root / 'cache'), XMODIFIERS='@im=none',
                   GTK_IM_MODULE='xim', QT_IM_MODULE='xim', SDL_IM_MODULE='none')
        env.pop('ALOL_KEYBOARD_STATE', None)
        if umu:
            # Reuse only the development copy's cached Steam runtime. Settings,
            # prefix and the Windows probe remain in this temporary directory.
            (root / 'data').mkdir()
            (root / 'data/umu').symlink_to(ROOT / '.dev-data/data/umu', target_is_directory=True)
            (root / 'cache').mkdir()
            (root / 'cache/umu').symlink_to(ROOT / '.dev-data/cache/umu', target_is_directory=True)
            env.update(PROTONPATH=str(runtime),
                       GAMEID='umu-alol-keyboard-test', STORE='none', PROTON_VERB='waitforexitandrun',
                       ALOL_TARGET_KIND='game', ALOL_COMPAT_INPUT='1')
        server = monitor = probe = keyboard = None
        wine, wineserver = str(runtime / 'files/bin/wine'), str(runtime / 'files/bin/wineserver')
        launcher = ['umu-run'] if umu else [wine]
        serverlog = (root / 'xvfb.log').open('w')
        bootlog = (root / 'wine.log').open('w')
        result = []
        try:
            displayfile = (root / 'display').open('w')
            server = subprocess.Popen([xvfb, '-displayfd', str(displayfile.fileno()), '-screen', '0',
                                       '800x600x24', '-nolisten', 'tcp', '-noreset'],
                                      pass_fds=(displayfile.fileno(),), stdout=serverlog, stderr=serverlog)
            wait_for(lambda: (root / 'display').read_text().strip(), 10)
            displayfile.close()
            env['DISPLAY'] = ':' + (root / 'display').read_text().strip()
            # This environment change is local to the diagnostic process; Xkb
            # opens this private server, never the user's compositor display.
            previous_display = os.environ.get('DISPLAY')
            os.environ['DISPLAY'] = env['DISPLAY']
            keyboard = bridge.Xkb()
            x = keyboard.x
            for name, args, restype in [
                ('XkbLockGroup', [C.c_void_p, C.c_uint, C.c_uint], C.c_int),
                ('XDefaultRootWindow', [C.c_void_p], C.c_ulong),
                ('XSetInputFocus', [C.c_void_p, C.c_ulong, C.c_int, C.c_ulong], C.c_int),
                ('XQueryTree', [C.c_void_p, C.c_ulong, C.POINTER(C.c_ulong), C.POINTER(C.c_ulong),
                               C.POINTER(C.POINTER(C.c_ulong)), C.POINTER(C.c_uint)], C.c_int),
                ('XFetchName', [C.c_void_p, C.c_ulong, C.POINTER(C.c_char_p)], C.c_int),
            ]:
                getattr(x, name).argtypes, getattr(x, name).restype = args, restype
            xtest = C.CDLL('libXtst.so.6')
            xtest.XTestFakeKeyEvent.argtypes = [C.c_void_p, C.c_uint, C.c_int, C.c_ulong]
            def key(code):
                xtest.XTestFakeKeyEvent(keyboard.display, code, 1, 0)
                xtest.XTestFakeKeyEvent(keyboard.display, code, 0, 0)
                x.XFlush(keyboard.display)
            def keymap(layout):
                subprocess.run(['setxkbmap', '-display', env['DISPLAY'], '-layout', layout, '-option', ''],
                               env=env, check=True, capture_output=True)
            keymap('us,ru')
            locale = root / 'locales/ru_RU.UTF-8'
            locale.parent.mkdir()
            subprocess.run(['localedef', '--no-archive', '-i', 'ru_RU', '-f', 'UTF-8', str(locale)],
                           env=dict(env, LC_ALL='C'), check=True, capture_output=True, timeout=60)
            env.update(LOCPATH=str(locale.parent), LC_ALL='ru_RU.UTF-8', LANG='ru_RU.UTF-8', HOST_LC_ALL='ru_RU.UTF-8')
            subprocess.run([wine, 'wineboot', '-u'], env=env, stdout=bootlog, stderr=bootlog, check=True, timeout=90)
            if umu:
                # wineboot may return before wineserver has flushed system.reg.
                # End only this temporary prefix's bootstrap server before ALOL
                # validates it and UMU takes over the runtime environment.
                subprocess.run([wineserver, '-k'], env=env, stdout=bootlog, stderr=bootlog, check=True, timeout=15)
                subprocess.run([wineserver, '-w'], env=env, stdout=bootlog, stderr=bootlog, check=True, timeout=15)
                # ALOL imports its locale registry after UMU's first migration,
                # not just before it. Exercise that same production path.
                config = root / 'config/alol'
                config.mkdir(parents=True)
                (config / 'settings.conf').write_text(
                    f'runner=proton\nproton={runtime}\nprefix_root={root}/prefixes\n')
                subprocess.run([str(ROOT / 'target/release/alol'), 'repair-language'], env=env,
                               stdout=bootlog, stderr=bootlog, check=True, timeout=120)
            registry = ['umu-run', r'C:\windows\system32\reg.exe'] if umu else [wine, 'reg']
            subprocess.run([*registry, 'add', r'HKCU\Software\Wine\X11 Driver', '/v', 'UseXIM', '/d', 'N', '/f'],
                           env=env, stdout=bootlog, stderr=bootlog, check=True, timeout=60)
            state = prefix / 'drive_c/alol/keyboard-test.bin'
            state.parent.mkdir(exist_ok=True)
            state.write_bytes(b'\0' * 4)
            monitor = subprocess.Popen(['python3', str(ROOT / 'runtime/xkb_layout.py'), str(state)], env=env,
                                       stdin=subprocess.PIPE, stdout=subprocess.DEVNULL, stderr=bootlog)
            def state_is(value):
                return state.read_bytes() == struct.pack('<I', value | (value << 16))
            wait_for(lambda: state_is(0x409))
            for arch in ('i386', 'x86_64'):
                logfile = root / (arch + '.log')
                executable = compile_probe(root, arch)
                if umu:
                    shutil.copyfile(executable, state.parent / executable.name)
                    executable = 'C:\\alol\\' + executable.name
                with logfile.open('w') as log:
                    probe = subprocess.Popen([*launcher, str(executable)],
                                             env=dict(env, ALOL_KEYBOARD_STATE=r'C:\alol\keyboard-test.bin'),
                                             stdout=log, stderr=log)
                    def ready():
                        data = logfile.read_text(errors='replace')
                        if 'FAIL=' in data or probe.poll() is not None:
                            raise AssertionError('Windows probe failed before readiness:\n' + data[-4000:])
                        return 'READY=' in data
                    wait_for(ready, 60)
                    a, b, children, count = C.c_ulong(), C.c_ulong(), C.POINTER(C.c_ulong)(), C.c_uint()
                    x.XQueryTree(keyboard.display, x.XDefaultRootWindow(keyboard.display),
                                 C.byref(a), C.byref(b), C.byref(children), C.byref(count))
                    focused = False
                    for i in range(count.value):
                        name = C.c_char_p()
                        x.XFetchName(keyboard.display, children[i], C.byref(name))
                        if name.value == b'ALOL layout probe':
                            x.XSetInputFocus(keyboard.display, children[i], 1, 0)
                            focused = True
                        if name:
                            x.XFree(C.cast(name, C.c_void_p))
                    x.XFree(children)
                    assert focused, 'Probe window was not found'
                    x.XFlush(keyboard.display)
                    stages = []
                    for group, lang, character in [(0, 0x409, 0x77), (1, 0x419, 0x446), (0, 0x409, 0x77)]:
                        assert x.XkbLockGroup(keyboard.display, 0x100, group)
                        x.XFlush(keyboard.display)
                        wait_for(lambda: state_is(lang))
                        offset = len(logfile.read_text())
                        expected = 0x419 if baseline else lang
                        wanted = f'HKL={expected:04X}{expected:04X} NAME={expected:08X} WNAME={expected:08X} ACP=1251 OEM=866'
                        wait_for(lambda: wanted in logfile.read_text()[offset:])
                        # All four movement keys retain their physical VKs;
                        # text follows the selected layout independently.
                        for code, vk, latin, cyrillic in [(25, 0x57, 0x77, 0x446), (38, 0x41, 0x61, 0x444),
                                                         (39, 0x53, 0x73, 0x44b), (40, 0x44, 0x64, 0x432)]:
                            key_offset = len(logfile.read_text())
                            key(code)
                            text = latin if group == 0 else cyrillic
                            wait_for(lambda: f'CHAR={text:08X}' in logfile.read_text()[key_offset:])
                            assert f'KEY={vk:08X}' in logfile.read_text()[key_offset:], 'Physical WASD VK changed'
                        stages.append({'xkb_lang': f'{lang:04x}', 'reported_lang': f'{expected:04x}', 'character': f'{character:04x}'})
                    # Wayland compositors may replace the entire map instead of
                    # changing groups. Exercise that path as well.
                    for layout, lang in [('ru', 0x419), ('us', 0x409)]:
                        keymap(layout)
                        wait_for(lambda: state_is(lang))
                        expected = 0x419 if baseline else lang
                        offset = len(logfile.read_text())
                        wait_for(lambda: f'HKL={expected:04X}{expected:04X}' in logfile.read_text()[offset:])
                    # A stopped helper invalidates the mapped state, and the
                    # same running process immediately falls back to Wine.
                    monitor.stdin.close()
                    assert monitor.wait(timeout=5) == 0
                    monitor = None
                    assert state.read_bytes() == b'\0' * 4
                    offset = len(logfile.read_text())
                    wait_for(lambda: 'HKL=04190419 NAME=00000419 WNAME=00000419' in logfile.read_text()[offset:])
                    key(9)  # Escape exits only our probe.
                    assert probe.wait(timeout=10) == 0, logfile.read_text()
                    probe = None
                    # No bridge variable: the rebuilt DLL preserves ordinary
                    # GE behaviour and does not require a monitor at all.
                    fallback = subprocess.run([*launcher, str(executable)], env=env,
                                              capture_output=True, timeout=45)
                    assert fallback.returncode == 0, fallback.stdout + fallback.stderr
                    assert b'HKL=04190419 NAME=00000419 WNAME=00000419' in fallback.stdout
                    monitor = subprocess.Popen(['python3', str(ROOT / 'runtime/xkb_layout.py'), str(state)], env=env,
                                               stdin=subprocess.PIPE, stdout=subprocess.DEVNULL, stderr=bootlog)
                    keymap('us,ru')
                    wait_for(lambda: state_is(0x409))
                    result.append({'architecture': arch, 'stages': stages, 'keymap_replacement': 'passed',
                                   'cyrillic_cp1251': 'passed', 'system_user_locale': 'ru-RU',
                                   'physical_wasd': 'passed', 'fallback_with_without_bridge': 'passed'})
            monitor.stdin.close()
            assert monitor.wait(timeout=5) == 0
            monitor = None
            assert state.read_bytes() == b'\0' * 4, 'The helper left stale layout state'
        except Exception:
            print('Wine/helper diagnostic log:\n' + (root / 'wine.log').read_text(errors='replace')[-6000:])
            for path in root.glob('*.log'):
                if path.name not in ('wine.log', 'xvfb.log'):
                    print(path.name + ':\n' + path.read_text(errors='replace')[-2500:])
            raise
        finally:
            if probe and probe.poll() is None:
                probe.terminate()
                probe.wait(timeout=10)
            if monitor and monitor.poll() is None:
                monitor.stdin.close()
                monitor.wait(timeout=5)
            subprocess.run([wineserver, '-k'], env=env, stdout=bootlog, stderr=bootlog, timeout=15)
            subprocess.run([wineserver, '-w'], env=env, stdout=bootlog, stderr=bootlog, timeout=15)
            if keyboard:
                keyboard.close()
            if server:
                server.terminate()
                server.wait(timeout=10)
            if 'previous_display' in locals():
                if previous_display is None:
                    os.environ.pop('DISPLAY', None)
                else:
                    os.environ['DISPLAY'] = previous_display
            bootlog.close()
            serverlog.close()
        return {'mode': 'baseline_bug_reproduced' if baseline else 'fixed', 'launcher': 'umu' if umu else 'wine', 'checks': result,
                'helper_shutdown_clears_state': True}


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--runtime', required=True, type=Path)
    parser.add_argument('--xvfb', default=shutil.which('Xvfb'))
    parser.add_argument('--baseline', action='store_true')
    parser.add_argument('--umu', action='store_true', help='Also exercise the actual UMU/Proton launch chain using development caches')
    parser.add_argument('--output', type=Path)
    args = parser.parse_args()
    if not args.xvfb:
        parser.error('Xvfb is required (the user display is deliberately never used)')
    report = run(args.runtime.resolve(), args.xvfb, args.baseline, args.umu)
    content = json.dumps(report, indent=2, ensure_ascii=False) + '\n'
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(content)
    print(content)
