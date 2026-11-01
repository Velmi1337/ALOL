#!/bin/bash
set -euo pipefail
# Executed only inside the clean Fedora CI container, not on the developer's host.
dnf install --assumeyes rpm-build rust cargo gcc gtk3-devel glib2-devel cairo-devel \
    libarchive-devel python3 desktop-file-utils bsdtar shadow-utils util-linux git redhat-rpm-config
useradd --create-home --home-dir /tmp/alol-builder builder
cp -a /source /tmp/alol-builder/project
chown -R builder:builder /tmp/alol-builder/project
runuser -u builder -- env ALOL_SOURCE_ARCHIVE=/release-source.tar.gz \
    bash /tmp/alol-builder/project/install-fedora.sh --build-only
alol_rpms=(/tmp/alol-builder/project/dist/alol-*.x86_64.rpm)
dnf install --assumeyes "${alol_rpms[@]}"
runuser -u builder -- alol --version
runuser -u builder -- alol doctor > /tmp/alol-doctor.txt
grep -q 'Выбрано: wine /' /tmp/alol-doctor.txt
cp -- "${alol_rpms[@]}" /tmp/alol-builder/project/dist/alol.spec \
    /tmp/alol-builder/project/dist/SHA256SUMS.fedora /out/
