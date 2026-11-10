#!/bin/bash
set -euo pipefail
# Only for the clean Ubuntu 22.04 CI container, never the developer's system.
export DEBIAN_FRONTEND=noninteractive
export CARGO_HOME=/opt/alol-cargo RUSTUP_HOME=/opt/alol-rustup
export PATH="$CARGO_HOME/bin:$PATH"
if [[ $(id -u) == 0 ]]; then
sed -i 's|http://archive.ubuntu.com/ubuntu|http://azure.archive.ubuntu.com/ubuntu|g' /etc/apt/sources.list
sed -n 's/^deb /deb-src /p' /etc/apt/sources.list > /etc/apt/sources.list.d/alol-sources.list
apt-get update -qq
apt-get install -y --no-install-recommends build-essential curl ca-certificates \
    libgtk-3-dev libfontconfig1-dev libarchive-dev pkg-config patchelf \
    desktop-file-utils file xvfb xauth python3 python3-tomli squashfs-tools dpkg-dev \
    adwaita-icon-theme hicolor-icon-theme gsettings-desktop-schemas util-linux passwd
curl --proto '=https' --tlsv1.2 -fsS https://sh.rustup.rs -o /tmp/rustup-init.sh
sh /tmp/rustup-init.sh -y --no-modify-path --profile minimal --default-toolchain 1.99.1
mkdir -p /usr/src/alol
cp -a /source/. /usr/src/alol/
useradd --create-home --home-dir /tmp/alol-builder builder
chown -R builder:builder /usr/src/alol "$CARGO_HOME" "$RUSTUP_HOME" /out
exec runuser -u builder -- env CARGO_HOME="$CARGO_HOME" RUSTUP_HOME="$RUSTUP_HOME" \
    PATH="$PATH" bash /usr/src/alol/packaging/appimage/build-container.sh
fi
cd /usr/src/alol
export CARGO_ENCODED_RUSTFLAGS='--remap-path-prefix=/usr/src/alol=/usr/src/alol'
export CARGO_INCREMENTAL=0
python3 scripts/package-source.py /tmp/alol-source-check --archive /release-source.tar.gz
cargo build --frozen --offline --release
cargo test --frozen --offline
python3 tests/integration.py
python3 -m unittest discover -s tests -p '*_integration.py'
version=$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml)
mkdir -p /tmp/alol-appimage-tools
python3 - <<'PY'
import hashlib,json,pathlib,subprocess
for name, spec in json.loads(pathlib.Path('packaging/appimage/tools.json').read_text()).items():
    path=pathlib.Path('/tmp/alol-appimage-tools')/name
    subprocess.run(['curl','--fail','--location','--silent','--show-error','--retry','3',spec['url'],'--output',str(path)],check=True)
    if hashlib.sha256(path.read_bytes()).hexdigest()!=spec['sha256']:
        raise ValueError('Build tool checksum mismatch: '+name)
    path.chmod(0o755)
PY
export PATH="/tmp/alol-appimage-tools:$PATH"
export APPIMAGE_EXTRACT_AND_RUN=1 DEPLOY_GTK_VERSION=3 NO_STRIP=1
linuxdeploy-x86_64.AppImage --appdir AppDir --executable target/release/alol \
    --desktop-file assets/alol.desktop --icon-file assets/alol.png --plugin gtk \
    --custom-apprun packaging/appimage/AppRun.sh
# ALOL supports native GTK Wayland; avoid the plugin's blanket X11 override.
sed -i '/^export GDK_BACKEND=x11 /d' AppDir/apprun-hooks/linuxdeploy-plugin-gtk.sh
install -Dm644 LICENSE AppDir/usr/share/licenses/alol/LICENSE
install -Dm644 NOTICE AppDir/usr/share/licenses/alol/NOTICE
install -Dm644 /tmp/alol-appimage-tools/gtk-plugin.LICENSE AppDir/usr/share/licenses/alol-appimage/gtk-plugin.LICENSE
install -Dm644 /tmp/alol-appimage-tools/appimage-runtime.LICENSE AppDir/usr/share/licenses/alol-appimage/appimage-runtime.LICENSE
python3 scripts/appimage-sources.py AppDir /out
export LDAI_RUNTIME_FILE=/tmp/alol-appimage-tools/runtime-x86_64 LDAI_NO_APPSTREAM=1
export LDAI_OUTPUT="/out/alol-$version-x86_64.AppImage" LDAI_VERSION="$version"
linuxdeploy-x86_64.AppImage --appdir AppDir --custom-apprun packaging/appimage/AppRun.sh --output appimage
chmod 755 "$LDAI_OUTPUT"
alol_smoke=$(mktemp -d)
export HOME="$alol_smoke/home" XDG_CONFIG_HOME="$alol_smoke/config" \
    XDG_DATA_HOME="$alol_smoke/data" XDG_CACHE_HOME="$alol_smoke/cache" XDG_STATE_HOME="$alol_smoke/state"
mkdir -p "$HOME"
[[ $("$LDAI_OUTPUT" --appimage-extract-and-run --version) == "ALOL $version" ]]
"$LDAI_OUTPUT" --appimage-extract-and-run --license > /tmp/alol-appimage-license.txt
grep -q 'Apache License' /tmp/alol-appimage-license.txt
grep -q 'SIL OPEN FONT LICENSE' /tmp/alol-appimage-license.txt
xvfb-run -a "$LDAI_OUTPUT" --appimage-extract-and-run --smoke-test
cp /release-source.tar.gz "/out/alol-$version.tar.gz"
python3 scripts/verify-appimage.py /out --write
