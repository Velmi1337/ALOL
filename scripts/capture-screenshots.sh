#!/bin/sh
set -eu
alol_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$alol_root"
cargo build --offline --release
alol_capture_dir=$(mktemp -d /tmp/alol-screenshots.XXXXXXXX)
trap 'rm -rf -- "$alol_capture_dir"' EXIT
cp target/release/alol "$alol_capture_dir/alol"
ALOL_DEVELOPMENT=0 GDK_SCALE=1 GDK_DPI_SCALE=1 GSETTINGS_BACKEND=memory \
    "$alol_capture_dir/alol" --screenshots docs/screenshots
echo 'Три PNG сохранены в docs/screenshots. Использован временный демонстрационный профиль.'
