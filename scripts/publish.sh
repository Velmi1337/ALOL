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
alol_title="ALOL $alol_version"
if [[ $alol_version == 1.0.0 ]]; then
    alol_title='ALOL 1.0.0 — первый стабильный релиз'
fi
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
    gh repo create "$alol_repo" --public --description 'ALOL — Arizona Launcher On Linux. Native Rust launcher for Arch Linux, CachyOS and Fedora.'
fi
git -c credential.helper= -c 'credential.helper=!gh auth git-credential' push origin main
git -c credential.helper= -c 'credential.helper=!gh auth git-credential' push origin "v$alol_version"
alol_artifacts=("dist/alol-$alol_version-1-x86_64.pkg.tar.zst" \
    "dist/alol-$alol_version-linux-x86_64" \
    "dist/alol-$alol_version.tar.gz" dist/PKGBUILD dist/alol.spec dist/SHA256SUMS LICENSE NOTICE \
    docs/screenshots/main.png docs/screenshots/settings.png docs/screenshots/mods.png)
alol_runtime_asset=$(python3 -c 'import json; print(json.load(open("runtime/base.json"))["overlay_filename"])')
alol_artifacts+=("dist/$alol_runtime_asset")
if gh release view "v$alol_version" --repo "$alol_repo" >/dev/null 2>&1; then
    if [[ $(gh release view "v$alol_version" --repo "$alol_repo" --json isDraft --jq .isDraft) != true ]]; then
        echo 'This release is already public; refusing to replace its assets.' >&2
        exit 1
    fi
    gh release upload "v$alol_version" --repo "$alol_repo" --clobber "${alol_artifacts[@]}"
else
    gh release create "v$alol_version" --repo "$alol_repo" --verify-tag \
        --draft --title "$alol_title" --notes-file docs/release-notes.md "${alol_artifacts[@]}"
fi
printf '\nBuilding and testing the native Fedora RPM in GitHub Actions...\n'
alol_dispatched_at=$(date -u +%Y-%m-%dT%H:%M:%SZ)
gh workflow run fedora-rpm.yml --repo "$alol_repo" --ref main -f "tag=v$alol_version"
alol_run=''
for alol_attempt in {1..20}; do
    alol_run=$(gh run list --repo "$alol_repo" --workflow fedora-rpm.yml \
        --event workflow_dispatch --commit "$(git rev-parse HEAD)" --created ">=$alol_dispatched_at" --limit 1 \
        --json databaseId --jq '.[0].databaseId // empty')
    if [[ -n $alol_run ]]; then break; fi
    sleep 3
done
if [[ -z $alol_run ]]; then
    echo 'Fedora build was dispatched but its run has not appeared yet. Check the Actions tab.' >&2
    exit 1
fi
gh run watch "$alol_run" --repo "$alol_repo" --exit-status --interval 10
gh release download "v$alol_version" --repo "$alol_repo" --dir dist --clobber \
    --pattern "alol-$alol_version-*.x86_64.rpm" --pattern SHA256SUMS.fedora
python3 scripts/verify-fedora-download.py dist
gh release edit "v$alol_version" --repo "$alol_repo" --draft=false --prerelease=false --latest --title "$alol_title"
printf '\nPublished: https://github.com/%s/releases/tag/v%s\n' "$alol_repo" "$alol_version"
