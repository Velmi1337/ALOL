#!/usr/bin/env python3
"""Publish the current XKB language for one Wine session; never set a keymap.

stdin is a lifetime pipe owned by ALOL. EOF or SIGTERM clears the mapping.
Only layout/map events are observed; key presses and typed text are not read.
"""
import ctypes as C
import mmap
import os
from pathlib import Path
import selectors
import signal
import struct
import sys


class Names(C.Structure):
    _fields_ = [(name, C.c_ulong) for name in ('keycodes', 'geometry', 'symbols', 'types', 'compat')]
    _fields_ += [('vmods', C.c_ulong * 16), ('indicators', C.c_ulong * 32), ('groups', C.c_ulong * 4)]


class Keyboard(C.Structure):
    _fields_ = [('display', C.c_void_p), ('flags', C.c_ushort), ('device', C.c_ushort),
                ('min_key', C.c_ubyte), ('max_key', C.c_ubyte)]
    _fields_ += [(name, C.c_void_p) for name in ('controls', 'server', 'map', 'indicators')]
    _fields_ += [('names', C.POINTER(Names)), ('compat', C.c_void_p), ('geometry', C.c_void_p)]


def language(name):
    # This bridge reports a language for the game's indicator, not a replacement
    # keyboard map. Unrecognised names clear the override instead of guessing RU.
    names = {'English (US)': 0x0409, 'English (UK)': 0x0809, 'Russian': 0x0419,
             'Ukrainian': 0x0422, 'Belarusian': 0x0423, 'German': 0x0407,
             'French': 0x040c, 'Polish': 0x0415, 'Spanish': 0x040a,
             'Italian': 0x0410, 'Portuguese': 0x0816, 'Turkish': 0x041f,
             'Czech': 0x0405, 'Finnish': 0x040b, 'Swedish': 0x041d}
    if name in names:
        return names[name]
    # XKB includes variants in brackets, such as Russian (phonetic). Keep the
    # language without attempting to remap keys or invent a Windows variant ID.
    for prefix, value in names.items():
        if name.startswith(prefix.removesuffix(')') + ',') or name.startswith(prefix + ' ('):
            return value
    return 0


class Xkb:
    def __init__(self):
        self.x = C.CDLL('libX11.so.6')
        signatures = {
            'XOpenDisplay': (C.c_void_p, [C.c_char_p]),
            'XCloseDisplay': (C.c_int, [C.c_void_p]),
            'XConnectionNumber': (C.c_int, [C.c_void_p]),
            'XkbQueryExtension': (C.c_int, [C.c_void_p] + [C.POINTER(C.c_int)] * 5),
            'XkbSelectEvents': (C.c_int, [C.c_void_p, C.c_uint, C.c_uint, C.c_uint]),
            'XkbGetMap': (C.POINTER(Keyboard), [C.c_void_p, C.c_uint, C.c_uint]),
            'XkbGetNames': (C.c_int, [C.c_void_p, C.c_uint, C.POINTER(Keyboard)]),
            'XkbFreeKeyboard': (None, [C.POINTER(Keyboard), C.c_uint, C.c_int]),
            'XkbGetState': (C.c_int, [C.c_void_p, C.c_uint, C.c_void_p]),
            'XGetAtomName': (C.c_void_p, [C.c_void_p, C.c_ulong]),
            'XFree': (C.c_int, [C.c_void_p]),
            'XPending': (C.c_int, [C.c_void_p]),
            'XNextEvent': (C.c_int, [C.c_void_p, C.c_void_p]),
            'XFlush': (C.c_int, [C.c_void_p]),
        }
        for name, (result, arguments) in signatures.items():
            function = getattr(self.x, name)
            function.restype, function.argtypes = result, arguments
        self.display = self.x.XOpenDisplay(None)
        if not self.display:
            raise RuntimeError('X11 display is unavailable')
        values = [C.c_int(value) for value in (0, 0, 0, 1, 0)]
        if not self.x.XkbQueryExtension(self.display, *(C.byref(value) for value in values)):
            self.close()
            raise RuntimeError('XKB extension is unavailable')
        self.event_base = values[1].value
        # NewKeyboard, Map, State and Names; no KeyPress/KeyRelease subscription.
        mask = (1 << 0) | (1 << 1) | (1 << 2) | (1 << 6)
        if not self.x.XkbSelectEvents(self.display, 0x100, mask, mask):
            self.close()
            raise RuntimeError('Cannot observe XKB layout changes')
        self.x.XFlush(self.display)

    def close(self):
        if self.display:
            self.x.XCloseDisplay(self.display)
            self.display = None

    def current(self):
        state = C.create_string_buffer(64)
        if self.x.XkbGetState(self.display, 0x100, state):
            return 0
        group = state.raw[0]
        if group >= 4:
            return 0
        keyboard = self.x.XkbGetMap(self.display, 7, 0x100)  # XkbAllClientInfoMask
        if not keyboard:
            return 0
        try:
            if self.x.XkbGetNames(self.display, 1 << 12, keyboard):  # XkbGroupNamesMask
                return 0
            if not keyboard.contents.names:
                return 0
            atom = keyboard.contents.names.contents.groups[group]
            pointer = self.x.XGetAtomName(self.display, atom) if atom else None
            if not pointer:
                return 0
            try:
                return language(C.string_at(pointer).decode('utf-8', errors='replace'))
            finally:
                self.x.XFree(pointer)
        finally:
            self.x.XkbFreeKeyboard(keyboard, 0, 1)

    def changed(self):
        changed = False
        event = (C.c_long * 24)()  # XEvent is 24 native longs.
        while self.x.XPending(self.display):
            self.x.XNextEvent(self.display, event)
            changed |= C.cast(event, C.POINTER(C.c_int))[0] == self.event_base
        return changed


def monitor(path):
    with Path(path).open('r+b') as stream:
        if os.fstat(stream.fileno()).st_size != 4:
            raise RuntimeError('Invalid keyboard state size')
        with mmap.mmap(stream.fileno(), 4) as state:
            keyboard = None
            try:
                keyboard = Xkb()
                with selectors.DefaultSelector() as selector:
                    selector.register(sys.stdin.buffer, selectors.EVENT_READ, 'lifetime')
                    selector.register(keyboard.x.XConnectionNumber(keyboard.display),
                                      selectors.EVENT_READ, 'xkb')
                    while True:
                        value = keyboard.current()
                        struct.pack_into('<I', state, 0, value | (value << 16))
                        # Xlib may have buffered an event while answering a query.
                        if keyboard.changed():
                            continue
                        for key, _ in selector.select():
                            if key.data == 'lifetime' and not os.read(sys.stdin.fileno(), 1):
                                return
                            if key.data == 'xkb':
                                keyboard.changed()
            finally:
                struct.pack_into('<I', state, 0, 0)
                if keyboard:
                    keyboard.close()


def main():
    signal.signal(signal.SIGTERM, lambda *_: sys.exit(0))
    try:
        monitor(sys.argv[1])
    except (OSError, RuntimeError, ValueError, IndexError) as error:
        print(f'ALOL: синхронизация индикатора раскладки недоступна: {error}', file=sys.stderr)
        return 1
    return 0


if __name__ == '__main__':
    sys.exit(main())
