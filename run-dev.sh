#!/bin/sh
set -eu
alol_project=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
cd "$alol_project"
if command -v cargo >/dev/null 2>&1; then
    cargo build --offline --release
elif [ ! -x target/release/alol ]; then
    echo 'Нужен Rust/Cargo для сборки рабочей копии ALOL.' >&2
    exit 1
else
    echo 'Cargo не найден: запускается последняя собранная рабочая копия.' >&2
fi
export ALOL_DEVELOPMENT=1
export ALOL_DESKTOP_DATA_HOME="${ALOL_DESKTOP_DATA_HOME:-${XDG_DATA_HOME:-$HOME/.local/share}}"
export XDG_CONFIG_HOME="$alol_project/.dev-data/config"
export XDG_DATA_HOME="$alol_project/.dev-data/data"
export XDG_CACHE_HOME="$alol_project/.dev-data/cache"
export XDG_STATE_HOME="$alol_project/.dev-data/state"
mkdir -p "$XDG_CONFIG_HOME/alol" "$XDG_DATA_HOME" "$XDG_CACHE_HOME" "$XDG_STATE_HOME"
if [ ! -e "$XDG_CONFIG_HOME/alol/settings.conf" ]; then
    printf '%s\n' 'update_policy=manual' 'update_notifications=false' > "$XDG_CONFIG_HOME/alol/settings.conf"
fi
exec "$alol_project/target/release/alol" "$@"
