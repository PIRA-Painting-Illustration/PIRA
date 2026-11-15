#!/usr/bin/env bash
# Publish a PIRA release: tag v<version> (the PIRA_VERSION file) at HEAD with the Android APK and
# the Windows and Linux downloads attached. A beta (0.2.0-beta, 0.2.0-beta.2) is marked as a
# pre-release. Installed copies offer it at launch or from Help › Check for Updates…: Android
# (Updater.java) and the desktop (apps/photocraft/src/updates.rs) read the public release list.
#
#   scripts/pira-release.sh <notes.md> [dist-dir]
#
# Build first, from the same commit:
#   PIRA_RELEASE=1 apps/photocraft-android/build.sh   # the APK, signed with the release key
#   scripts/pira-desktop-build.sh [dist-dir]           # Linux and Windows (on Linux or WSL)
# The APK is looked for in $CARGO_TARGET_DIR/android or target/android, the desktop archives in
# dist-dir (default target/pira-dist).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REPO="PIRA-Painting-Illustration/PIRA"
NOTES="${1:?usage: pira-release.sh <notes.md> [dist-dir]}"
DIST="${2:-$ROOT/target/pira-dist}"
V="$(tr -d '[:space:]' < "$ROOT/PIRA_VERSION")"
TAG="v$V"

cd "$ROOT"
if [[ -n "$(git status --porcelain --untracked-files=no)" ]]; then
    echo "error: commit your changes first: the release is tagged at HEAD" >&2
    exit 1
fi
git fetch -q origin main
if ! git merge-base --is-ancestor HEAD origin/main; then
    echo "error: push HEAD first: the release is tagged at a commit GitHub has" >&2
    exit 1
fi
if gh release view "$TAG" -R "$REPO" >/dev/null 2>&1; then
    echo "error: $TAG is already released: raise PIRA_VERSION" >&2
    exit 1
fi

APK=""
for d in "${CARGO_TARGET_DIR:-}/android" "$ROOT/target/android"; do
    [[ -f "$d/pira-$V-arm64-v8a.apk" ]] && APK="$d/pira-$V-arm64-v8a.apk" && break
done
FILES=("$APK" "$DIST/pira-$V-windows-x64.zip" "$DIST/pira-$V-linux-x86_64.tar.gz")
for f in "${FILES[@]}"; do
    if [[ -z "$f" || ! -f "$f" ]]; then
        echo "error: missing ${f:-the APK (pira-$V-arm64-v8a.apk)}: build it first" >&2
        exit 1
    fi
done

PRE=()
[[ "$V" == *-beta* ]] && PRE=(--prerelease)
gh release create "$TAG" -R "$REPO" --target "$(git rev-parse HEAD)" "${PRE[@]}" \
    --title "PIRA $V" --notes-file "$NOTES" "${FILES[@]}"
git fetch -q origin "refs/tags/$TAG:refs/tags/$TAG"
echo "Released $TAG"
