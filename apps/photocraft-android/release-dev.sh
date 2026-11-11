#!/usr/bin/env bash
# Publish a dev build to PIRA's update channel: build the APK with the next build number, then
# make a GitHub pre-release tagged android-v<version>-<build> with the APK attached. Installed
# copies offer it at launch or from Help › Check for Updates… (Updater.java).
#
#   apps/photocraft-android/release-dev.sh      # needs gh, the Android toolchain (build.sh)
#                                               # and the update token (see app/build.gradle)
#
# The repository is private: the release is only visible to the repository's members and to
# builds that carry the update token. Dev releases carry that token inside the APK, so before
# going live delete them all and revoke the token.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
REPO="PIRA-Painting-Illustration/PIRA"

TOKEN_FILE="$HOME/pira-signing/update-token/token.txt"
if [[ -z "${PIRA_UPDATE_TOKEN:-}" && ! -s "$TOKEN_FILE" ]]; then
    echo "error: no update token ($TOKEN_FILE): builds without it can't see the releases" >&2
    exit 1
fi

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

VERSION="${APP_VERSION:-$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)}"
export APP_VERSION="$VERSION"

# The next build number for this version (android-v0.1.0-3 → 4). The version code allows 99.
LAST="$(gh release list -R "$REPO" --limit 100 --json tagName --jq '.[].tagName' \
    | sed -n "s/^android-v${VERSION//./\\.}-\([0-9]\+\)$/\1/p" | sort -n | tail -1)"
BUILD=$(( ${LAST:-0} + 1 ))
if (( BUILD > 99 )); then
    echo "error: $VERSION already has 99 builds: raise the version" >&2
    exit 1
fi
TAG="android-v$VERSION-$BUILD"
PREV="$(git tag --list "android-v$VERSION-*" --sort=-v:refname | head -1)"

echo "Building $TAG"
RELEASE_BUILD="$BUILD" "$HERE/build.sh"
APK="${CARGO_TARGET_DIR:-$ROOT/target}/android/pira-$VERSION-$BUILD-arm64-v8a.apk"

NOTES="$(mktemp)"
trap 'rm -f "$NOTES"' EXIT
{
    echo "Dev build $VERSION-$BUILD."
    echo
    echo "## New in this build"
    if [[ -n "$PREV" ]]; then
        git log --no-merges -30 --format='- %s' "$PREV..HEAD"
    else
        git log --no-merges --format='- %s' -15
    fi
} > "$NOTES"

gh release create "$TAG" -R "$REPO" --target "$(git rev-parse HEAD)" --prerelease \
    --title "PIRA $VERSION-$BUILD (dev)" --notes-file "$NOTES" "$APK"
git fetch -q origin "refs/tags/$TAG:refs/tags/$TAG"
echo "Released $TAG"
