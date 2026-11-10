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
alol_commit=$(git rev-parse HEAD)
if [[ $(git branch --show-current) != main ]] || \
    { git rev-parse "v$alol_version^{commit}" >/dev/null 2>&1 && \
      [[ $(git rev-parse "v$alol_version^{commit}") != "$alol_commit" ]]; }; then
    echo 'Publish clean main; any existing version tag must point to HEAD.' >&2
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
    gh release create "v$alol_version" --repo "$alol_repo" --target "$alol_commit" \
        --draft --title "$alol_title" --notes-file docs/release-notes.md "${alol_artifacts[@]}"
fi
printf '\nBuilding and testing Fedora and AppImage in GitHub Actions...\n'
alol_runs=()
for alol_workflow in fedora-rpm.yml appimage.yml; do
    alol_dispatched_at=$(date -u +%Y-%m-%dT%H:%M:%SZ)
    gh workflow run "$alol_workflow" --repo "$alol_repo" --ref main \
        -f "tag=v$alol_version" -f "source_ref=$alol_commit"
    alol_run=''
    for alol_attempt in {1..20}; do
        alol_run=$(gh run list --repo "$alol_repo" --workflow "$alol_workflow" \
            --event workflow_dispatch --commit "$alol_commit" --created ">=$alol_dispatched_at" --limit 1 \
            --json databaseId --jq '.[0].databaseId // empty')
        if [[ -n $alol_run ]]; then break; fi
        sleep 3
    done
    if [[ -z $alol_run ]]; then
        echo 'CI build was dispatched but has not appeared yet. Check the Actions tab.' >&2
        exit 1
    fi
    alol_runs+=("$alol_run")
done
for alol_run in "${alol_runs[@]}"; do
    gh run watch "$alol_run" --repo "$alol_repo" --exit-status --interval 10
done
gh release download "v$alol_version" --repo "$alol_repo" --dir dist --clobber \
    --pattern "alol-$alol_version-*.x86_64.rpm" --pattern SHA256SUMS.fedora
python3 scripts/verify-fedora-download.py dist
gh release download "v$alol_version" --repo "$alol_repo" --dir dist --clobber \
    --pattern "alol-$alol_version-x86_64.AppImage" \
    --pattern "alol-$alol_version-appimage-sources.tar.gz" --pattern SHA256SUMS.appimage
python3 scripts/verify-appimage.py dist
if [[ $(git rev-parse HEAD) != "$alol_commit" ]] || [[ -n $(git status --porcelain) ]]; then
    echo 'Checkout changed during CI; refusing to tag or publish.' >&2
    exit 1
fi
if ! git rev-parse "v$alol_version" >/dev/null 2>&1; then
    git tag -a "v$alol_version" -m "ALOL $alol_version"
fi
git -c credential.helper= -c 'credential.helper=!gh auth git-credential' push origin "v$alol_version"
gh release edit "v$alol_version" --repo "$alol_repo" --draft=false --prerelease=false --latest \
    --target "$alol_commit" --title "$alol_title" --notes-file docs/release-notes.md
printf '\nPublished: https://github.com/%s/releases/tag/v%s\n' "$alol_repo" "$alol_version"
