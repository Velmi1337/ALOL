#!/bin/bash
set -euo pipefail

alol_root=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
alol_mode=${1:-install}
case "$alol_mode" in
    install|--build-only) ;;
    -h|--help)
        echo 'Usage: ./install-arch.sh [--build-only]'
        echo 'Build an Arch/CachyOS package; install it with pacman unless --build-only is given.'
        exit 0 ;;
    *) echo 'Unknown option. Use --help.' >&2; exit 2 ;;
esac
if (( EUID == 0 )); then
    echo 'Run this script as a regular user. makepkg will ask for sudo when needed.' >&2
    exit 1
fi
if [[ $(uname -m) != x86_64 ]]; then
    echo 'Arizona and this package require x86_64 Linux.' >&2
    exit 1
fi
for alol_tool in makepkg python3; do
    if ! command -v "$alol_tool" >/dev/null; then
        echo 'Install build tools: sudo pacman -S --needed base-devel rust python git' >&2
        exit 1
    fi
done
alol_build=$(mktemp -d /tmp/alol-package.XXXXXXXX)
trap 'rm -rf -- "$alol_build"' EXIT
python3 "$alol_root/scripts/package-source.py" "$alol_build"
mkdir -p "$alol_root/dist"
export PKGDEST="$alol_root/dist" SRCDEST="$alol_build" BUILDDIR="$alol_build"
export PACKAGER='ALOL contributors'
cd "$alol_build"
if [[ $alol_mode == --build-only ]]; then
    makepkg --cleanbuild --clean --nosign --force
else
    makepkg --syncdeps --cleanbuild --clean --nosign --needed --force
fi
cp -- PKGBUILD alol.spec alol-*.tar.gz "$alol_root/dist/"
mapfile -t alol_packages < <(makepkg --packagelist)
if (( ${#alol_packages[@]} != 1 )) || [[ ! -f ${alol_packages[0]} ]]; then
    echo 'Expected exactly one completed alol package.' >&2
    exit 1
fi
for alol_package in "${alol_packages[@]}"; do
    python3 "$alol_root/scripts/sanitize-package.py" "$alol_package"
done
PYTHONDONTWRITEBYTECODE=1 python3 "$alol_root/scripts/audit-release.py"
if [[ $alol_mode == install ]]; then
    sudo pacman -U --needed "${alol_packages[@]}"
fi
printf '\nPackages and source: %s/dist\n' "$alol_root"
