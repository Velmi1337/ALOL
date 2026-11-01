#!/bin/sh
set -eu
alol_source=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
alol_prefix=${ALOL_INSTALL_PREFIX:-"$HOME/.local"}
case "$alol_prefix" in /*) ;; *) echo 'ALOL_INSTALL_PREFIX must be absolute' >&2; exit 1;; esac
cd "$alol_source"
cargo build --offline --release
install -Dm755 target/release/alol "$alol_prefix/bin/alol"
install -Dm644 assets/alol.svg "$alol_prefix/share/icons/hicolor/scalable/apps/alol.svg"
install -Dm644 assets/alol.png "$alol_prefix/share/icons/hicolor/256x256/apps/alol.png"
python3 - "$alol_prefix" <<'PY'
import pathlib, sys
prefix = pathlib.Path(sys.argv[1])
# Desktop Entry escaping, then Exec argument quoting (including literal % field codes).
executable = str(prefix / 'bin/alol').replace('%', '%%')
for char in ('\\', '"', '`', '$'):
    executable = executable.replace(char, '\\' + char)
executable = executable.replace('\\', '\\\\')
applications = prefix / 'share/applications'
applications.mkdir(parents=True, exist_ok=True)
(applications / 'alol.desktop').write_text(
    '[Desktop Entry]\nType=Application\nName=ALOL\n'
    'GenericName=Arizona Launcher On Linux\n'
    'Comment=Arizona RP через Wine и Proton\n'
    f'Exec="{executable}"\nIcon=alol\nStartupWMClass=alol\nTerminal=false\n'
    'Categories=Game;\nKeywords=Arizona;RP;Wine;Proton;ALOL;\nStartupNotify=false\n',
    encoding='utf-8')
print(f'Установлено: {prefix / "bin/alol"}')
print('ALOL доступен в меню приложений.')
PY
if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "$alol_prefix/share/applications" || true
fi
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache -f -t "$alol_prefix/share/icons/hicolor" >/dev/null 2>&1 || true
fi
