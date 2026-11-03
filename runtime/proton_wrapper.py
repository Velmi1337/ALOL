#!/usr/bin/env python3
"""ALOL-Proton's Arizona-specific launch layer. Upstream Wine/DXVK are unchanged.

Copyright 2026 ALOL contributors. Licensed under Apache-2.0; see ALOL-LICENSE.
Original Proton and component licenses remain applicable to the upstream files.
"""
import os
from pathlib import Path
import sys


def configure(arguments, environment):
    """Return a private child environment/argv. Unmarked launches pass through."""
    result = dict(environment)
    args = list(arguments)
    kind = result.pop("ALOL_TARGET_KIND", "helper")
    input_compat = result.pop("ALOL_COMPAT_INPUT", "0") == "1"
    launcher_compat = result.pop("ALOL_COMPAT_LAUNCHER", "0") == "1"
    disable_gpu = result.pop("ALOL_LAUNCHER_DISABLE_GPU", "0") == "1"
    if not args or args[0] not in ("run", "waitforexitandrun", "runinprefix"):
        return args, result
    if len(args) < 2 or kind not in ("game", "launcher", "installer"):
        return args, result
    if kind in ("launcher", "installer"):
        if launcher_compat:
            for key in (
                "LD_PRELOAD", "LD_AUDIT", "LD_LIBRARY_PATH", "GTK_THEME", "GTK_MODULES",
                "GTK_PATH", "QT_QPA_PLATFORMTHEME", "QT_STYLE_OVERRIDE", "QT_PLUGIN_PATH",
                "QT_QPA_PLATFORM_PLUGIN_PATH", "QML2_IMPORT_PATH", "ELECTRON_OZONE_PLATFORM_HINT",
                "OZONE_PLATFORM", "MANGOHUD", "MANGOHUD_CONFIG", "MANGOHUD_DLSYM",
                "ENABLE_VKBASALT", "VK_INSTANCE_LAYERS", "VK_LAYER_PATH", "VK_ADD_LAYER_PATH", "DXVK_HUD",
            ):
                result.pop(key, None)
            if result.get("DISPLAY"):
                result.pop("WAYLAND_DISPLAY", None)
                result.update(PROTON_ENABLE_WAYLAND="0", GDK_BACKEND="x11", QT_QPA_PLATFORM="xcb",
                              SDL_VIDEODRIVER="x11", XDG_SESSION_TYPE="x11")
        if kind == "launcher":
            for flag in (["--no-sandbox", "--disable-gpu"] if disable_gpu else ["--no-sandbox"]):
                if flag not in args[2:]:
                    args.append(flag)
    if input_compat:
        result.update(XMODIFIERS="@im=none", GTK_IM_MODULE="xim", QT_IM_MODULE="xim", SDL_IM_MODULE="none")
        if result.get("DISPLAY"):
            result.pop("WAYLAND_DISPLAY", None)
            result.update(PROTON_ENABLE_WAYLAND="0", SDL_VIDEODRIVER="x11")
    # No FPS/VSync, resolution, texture, renderer, sync, GPU or cache overrides.
    # Logging is controlled by ALOL's existing diagnostic option.
    return args, result


def main():
    directory = Path(__file__).resolve().parent
    upstream = directory / "proton.alol-upstream"
    if not upstream.is_file() or upstream.is_symlink():
        raise SystemExit("ALOL-Proton: upstream Proton is missing; prepare the runtime again.")
    args, environment = configure(sys.argv[1:], os.environ)
    os.execve(upstream, [str(upstream), *args], environment)


if __name__ == "__main__":
    main()
