#!/usr/bin/env bash
# Publish a PIRA Pose release: tag pose-v<version> (apps/pira-pose/Cargo.toml's) at HEAD with the
# Android APK and the Windows and Linux downloads attached. It sits in PIRA's release list but is
# never marked "Latest" (that stays PIRA's), and PIRA's updaters ignore pose-v tags.
#
#   scripts/pira-pose-release.sh <notes.md> [dist-dir]
#
# Build first, from the same commit:
#   PIRA_RELEASE=1 apps/pira-pose/build-android.sh     # the APK, signed with the release key
#   scripts/pira-pose-desktop-build.sh [dist-dir]      # Linux and Windows (on Linux or WSL)
# The APK is looked for in dist-dir, $CARGO_TARGET_DIR/android or target/android, the desktop
# archives in dist-dir (default target/pira-dist).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REPO="PIRA-Painting-Illustration/PIRA"
NOTES="${1:?usage: pira-pose-release.sh <notes.md> [dist-dir]}"
DIST="${2:-$ROOT/target/pira-dist}"
V="$(sed -n 's/^version = "\(.*\)"/\1/p' "$ROOT/apps/pira-pose/Cargo.toml" | tr -d '\r' | head -1)"
TAG="pose-v$V"

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
    echo "error: $TAG is already released: raise the version in apps/pira-pose/Cargo.toml" >&2
    exit 1
fi

APK=""
for d in "$DIST" "${CARGO_TARGET_DIR:-}/android" "$ROOT/target/android"; do
    [[ -f "$d/pira-pose-$V-arm64-v8a.apk" ]] && APK="$d/pira-pose-$V-arm64-v8a.apk" && break
done
FILES=("$APK" "$DIST/pira-pose-$V-windows-x64.zip" "$DIST/pira-pose-$V-linux-x86_64.tar.gz")
for f in "${FILES[@]}"; do
    if [[ -z "$f" || ! -f "$f" ]]; then
        echo "error: missing ${f:-the APK (pira-pose-$V-arm64-v8a.apk)}: build it first" >&2
        exit 1
    fi
done

PRE=()
[[ "$V" == *-* ]] && PRE=(--prerelease)
gh release create "$TAG" -R "$REPO" --target "$(git rev-parse HEAD)" "${PRE[@]}" --latest=false \
    --title "PIRA Pose $V" --notes-file "$NOTES" "${FILES[@]}"
git fetch -q origin "refs/tags/$TAG:refs/tags/$TAG"
echo "Released $TAG"
