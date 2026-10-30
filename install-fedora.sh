#!/bin/bash
set -euo pipefail
alol_root=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
alol_mode=${1:-install}
case "$alol_mode" in
    install|--build-only) ;;
    -h|--help)
        echo 'Usage: ./install-fedora.sh [--build-only]'
        echo 'Build a native Fedora RPM, test it and install with dnf.'
        exit 0 ;;
    *) echo 'Unknown option. Use --help.' >&2; exit 2 ;;
esac
if (( EUID == 0 )); then
    echo 'Run as a regular user; sudo is used only for dnf.' >&2
    exit 1
fi
alol_os_id=$(
    . /etc/os-release
    printf '%s' "${ID:-}"
)
if [[ $alol_os_id != fedora || $(uname -m) != x86_64 ]] || ! command -v dnf >/dev/null; then
    echo 'This installer requires Fedora x86_64 with dnf.' >&2
    exit 1
fi
if [[ $alol_mode == install ]]; then
    sudo dnf install --assumeyes rpm-build rust cargo gcc gtk3-devel glib2-devel \
        cairo-devel libarchive-devel python3 desktop-file-utils bsdtar redhat-rpm-config
fi
for alol_tool in python3 rpmbuild rpm bsdtar cargo; do
    if ! command -v "$alol_tool" >/dev/null; then
        echo 'Missing build tool. Install the Fedora build dependencies listed in README.md.' >&2
        exit 1
    fi
done
alol_build=$(mktemp -d /tmp/alol-rpm.XXXXXXXX)
trap 'rm -rf -- "$alol_build"' EXIT
alol_source_args=()
if [[ -n ${ALOL_SOURCE_ARCHIVE:-} ]]; then
    alol_source_args=(--archive "$ALOL_SOURCE_ARCHIVE")
fi
python3 "$alol_root/scripts/package-source.py" "$alol_build/source" "${alol_source_args[@]}"
mkdir -p "$alol_build"/{BUILD,BUILDROOT,RPMS,SOURCES,SPECS,SRPMS,tmp} "$alol_root/dist"
cp -- "$alol_build"/source/alol-*.tar.gz "$alol_build/SOURCES/"
cp -- "$alol_build/source/alol.spec" "$alol_build/SPECS/"
rpmbuild -bb --define "_topdir $alol_build" --define "_tmppath $alol_build/tmp" \
    --define '_buildhost alol-builder' "$alol_build/SPECS/alol.spec"
alol_packages=("$alol_build"/RPMS/x86_64/alol-*.rpm)
if (( ${#alol_packages[@]} != 1 )) || [[ ! -f ${alol_packages[0]} ]]; then
    echo 'Expected one completed alol RPM.' >&2
    exit 1
fi
cp -- "${alol_packages[@]}" "$alol_build/source/alol.spec" "$alol_build"/source/alol-*.tar.gz "$alol_root/dist/"
PYTHONDONTWRITEBYTECODE=1 python3 "$alol_root/scripts/audit-release.py" --platform fedora
if [[ $alol_mode == install ]]; then
    sudo dnf install --assumeyes "$alol_root/dist/$(basename -- "${alol_packages[0]}")"
fi
printf '\nFedora package and SHA256SUMS.fedora: %s/dist\n' "$alol_root"
