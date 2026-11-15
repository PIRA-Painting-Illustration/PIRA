#!/bin/bash
# Builds PIRA's portable desktop downloads: Linux x86_64 natively and Windows x64 with the MinGW
# cross toolchain (x86_64-pc-windows-gnu), each packed with the licences, NOTICE, ATTRIBUTION.md
# and a README. Run it on Linux or in WSL:
#
#   scripts/pira-desktop-build.sh [out-dir]
#
# The version is the PIRA_VERSION file (APP_VERSION overrides it); the app reports it in About and
# `--version`, and the desktop updater compares it with the releases. Archives land in out-dir
# (default target/pira-dist): pira-<version>-linux-x86_64.tar.gz and pira-<version>-windows-x64.zip.
#
# BUILD_COPY=<dir> copies the source there first (rsync, skipping targets and .git), which is much
# faster than building on a Windows drive from WSL.
set -euo pipefail
source "$HOME/.cargo/env" 2>/dev/null || true
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
V="$(tr -d '[:space:]' <<<"${APP_VERSION:-$(cat "$ROOT/PIRA_VERSION")}")"
OUT="$(mkdir -p "${1:-$ROOT/target/pira-dist}" && cd "${1:-$ROOT/target/pira-dist}" && pwd)"
SHA="$(git -C "$ROOT" rev-parse HEAD 2>/dev/null || echo unknown)"
SRC="$ROOT"
if [[ -n "${BUILD_COPY:-}" ]]; then
    mkdir -p "$BUILD_COPY"
    rsync -a --checksum --delete --exclude target --exclude .git --exclude corpus --exclude 'apps/*/android/app/build' "$ROOT/" "$BUILD_COPY/"
    SRC="$BUILD_COPY"
fi
cd "$SRC"
export PHOTOCRAFT_OVERRIDE_VERSION="$V" PHOTOCRAFT_BUILD_SHA="$SHA" PHOTOCRAFT_BUILD_DATE="$(date -u +%F)"
TARGET="${CARGO_TARGET_DIR:-$SRC/target}"
extras() { cp "$ROOT"/LICENSE-APACHE "$ROOT"/LICENSE-MIT "$ROOT"/NOTICE "$ROOT"/ATTRIBUTION.md "$1/"; }

echo "== PIRA $V for Linux"
cargo build --release -p photocraft --bins
D="$(mktemp -d)/pira-$V-linux-x86_64"
mkdir -p "$D"
cp "$TARGET/release/photocraft" "$D/pira"
extras "$D"
printf 'PIRA %s for Linux (x86_64)\n\nRun ./pira to start. Settings are kept in ~/.config/pira.\nCreate an empty file named portable.txt next to pira to keep them in a PIRAData folder here instead.\n\nGuide: https://pira-art.com/wiki/\n' "$V" > "$D/README.txt"
tar -C "$(dirname "$D")" -czf "$OUT/pira-$V-linux-x86_64.tar.gz" "$(basename "$D")"

echo "== PIRA $V for Windows"
cargo build --release -p photocraft --bins --target x86_64-pc-windows-gnu
W="$(mktemp -d)/pira-$V-windows-x64"
mkdir -p "$W"
cp "$TARGET/x86_64-pc-windows-gnu/release/photocraft.exe" "$W/PIRA.exe"
extras "$W"
printf 'PIRA %s for Windows (64-bit)\r\n\r\nRun PIRA.exe to start. Settings are kept in %%APPDATA%%\\PIRA.\r\nCreate an empty file named portable.txt next to PIRA.exe to keep them in a PIRAData folder here instead.\r\nWindows may warn that the app is from an unknown publisher: it is not code-signed yet.\r\n\r\nGuide: https://pira-art.com/wiki/\r\n' "$V" > "$W/README.txt"
rm -f "$OUT/pira-$V-windows-x64.zip"
(cd "$(dirname "$W")" && python3 -m zipfile -c "$OUT/pira-$V-windows-x64.zip" "$(basename "$W")")

ls -la "$OUT"/pira-"$V"-*
