#!/bin/bash
set -euo pipefail
alol_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$alol_root"
export PYTHONDONTWRITEBYTECODE=1
python3 scripts/audit-release.py
if [[ -n $(git status --porcelain) ]]; then
    echo 'Commit or remove local changes before publishing.' >&2
    exit 1
fi
readarray -t alol_metadata < <(python3 - <<'PY'
import tomllib
from pathlib import Path
p=tomllib.loads(Path('Cargo.toml').read_text())['package']
print(p['repository'].removeprefix('https://github.com/'))
print(p['version'])
PY
)
alol_repo=${alol_metadata[0]}
alol_version=${alol_metadata[1]}
alol_remote="https://github.com/$alol_repo.git"
if [[ $(git branch --show-current) != main ]] || \
    [[ $(git rev-parse "v$alol_version^{commit}") != "$(git rev-parse HEAD)" ]]; then
    echo 'Publish main with the version tag pointing to the audited commit.' >&2
    exit 1
fi
if git remote get-url origin >/dev/null 2>&1; then
    if [[ $(git remote get-url origin) != "$alol_remote" ]]; then
        echo 'origin points to another repository; refusing to publish.' >&2
        exit 1
    fi
else
    git remote add origin "$alol_remote"
fi
if ! gh repo view "$alol_repo" --json name >/dev/null 2>&1; then
    gh repo create "$alol_repo" --public --description 'ALOL — Arizona Launcher On Linux. Native Rust launcher for Arch Linux and CachyOS.'
fi
git -c credential.helper= -c 'credential.helper=!gh auth git-credential' push origin main
git -c credential.helper= -c 'credential.helper=!gh auth git-credential' push origin "v$alol_version"
alol_artifacts=("dist/alol-$alol_version-1-x86_64.pkg.tar.zst" \
    "dist/alol-$alol_version.tar.gz" dist/PKGBUILD dist/SHA256SUMS \
    docs/screenshots/main.png docs/screenshots/settings.png docs/screenshots/mods.png)
if gh release view "v$alol_version" --repo "$alol_repo" >/dev/null 2>&1; then
    gh release upload "v$alol_version" --repo "$alol_repo" --clobber "${alol_artifacts[@]}"
else
    gh release create "v$alol_version" --repo "$alol_repo" --verify-tag \
        --title "ALOL $alol_version" --notes-file docs/release-notes.md "${alol_artifacts[@]}"
fi
printf '\nPublished: https://github.com/%s/releases/tag/v%s\n' "$alol_repo" "$alol_version"
