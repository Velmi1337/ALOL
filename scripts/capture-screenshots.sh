#!/bin/sh
set -eu
alol_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$alol_root"
cargo build --offline --release
GDK_SCALE=1 GDK_DPI_SCALE=1 GSETTINGS_BACKEND=memory \
    ./target/release/alol --screenshots docs/screenshots
echo 'Три PNG сохранены в docs/screenshots. Использован временный демонстрационный профиль.'
