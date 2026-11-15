#!/bin/bash
# Builds PIRA Pose's portable desktop downloads: Linux x86_64 natively and Windows x64 with the
# MinGW cross toolchain (x86_64-pc-windows-gnu), each packed with the licences, NOTICE,
# ATTRIBUTION.md and a README. Run it on Linux or in WSL:
#
#   scripts/pira-pose-desktop-build.sh [out-dir]
#
# The version is apps/pira-pose/Cargo.toml's (PIRA Pose ships apart from PIRA). Archives land in
# out-dir (default target/pira-dist): pira-pose-<version>-linux-x86_64.tar.gz and
# pira-pose-<version>-windows-x64.zip.
#
# BUILD_COPY=<dir> copies the source there first (rsync, skipping targets and .git), which is much
# faster than building on a Windows drive from WSL. Use the same BUILD_COPY and CARGO_TARGET_DIR as
# scripts/pira-desktop-build.sh: the two apps share most of their crates.
set -euo pipefail
source "$HOME/.cargo/env" 2>/dev/null || true
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
V="$(sed -n 's/^version = "\(.*\)"/\1/p' "$ROOT/apps/pira-pose/Cargo.toml" | tr -d '\r' | head -1)"
[[ -n "$V" ]] || { echo "error: no version in apps/pira-pose/Cargo.toml" >&2; exit 1; }
OUT="$(mkdir -p "${1:-$ROOT/target/pira-dist}" && cd "${1:-$ROOT/target/pira-dist}" && pwd)"
SRC="$ROOT"
if [[ -n "${BUILD_COPY:-}" ]]; then
    mkdir -p "$BUILD_COPY"
    rsync -a --checksum --delete --exclude target --exclude .git --exclude corpus --exclude 'apps/*/android/app/build' "$ROOT/" "$BUILD_COPY/"
    SRC="$BUILD_COPY"
fi
cd "$SRC"
TARGET="${CARGO_TARGET_DIR:-$SRC/target}"
extras() { cp "$ROOT"/LICENSE-APACHE "$ROOT"/LICENSE-MIT "$ROOT"/NOTICE "$ROOT"/ATTRIBUTION.md "$1/"; }
CREDITS='Poses from mocap.cs.cmu.edu (NSF EIA-0196217). Photo matching uses Google MediaPipe models (Apache 2.0).'

echo "== PIRA Pose $V for Linux"
cargo build --release -p pira-pose --bin pira-pose
D="$(mktemp -d)/pira-pose-$V-linux-x86_64"
mkdir -p "$D"
cp "$TARGET/release/pira-pose" "$D/pira-pose"
extras "$D"
printf 'PIRA Pose %s for Linux (x86_64)\n\nRun ./pira-pose to start. The panel layout is kept in ~/.config/PIRA Pose.\n\n%s\n' "$V" "$CREDITS" > "$D/README.txt"
tar -C "$(dirname "$D")" -czf "$OUT/pira-pose-$V-linux-x86_64.tar.gz" "$(basename "$D")"

echo "== PIRA Pose $V for Windows"
cargo build --release -p pira-pose --bin pira-pose --target x86_64-pc-windows-gnu
W="$(mktemp -d)/pira-pose-$V-windows-x64"
mkdir -p "$W"
cp "$TARGET/x86_64-pc-windows-gnu/release/pira-pose.exe" "$W/PIRA Pose.exe"
extras "$W"
printf 'PIRA Pose %s for Windows (64-bit)\r\n\r\nRun "PIRA Pose.exe" to start. The panel layout is kept in %%APPDATA%%\\PIRA Pose.\r\nWindows may warn that the app is from an unknown publisher: it is not code-signed yet.\r\n\r\n%s\r\n' "$V" "$CREDITS" > "$W/README.txt"
rm -f "$OUT/pira-pose-$V-windows-x64.zip"
(cd "$(dirname "$W")" && python3 -m zipfile -c "$OUT/pira-pose-$V-windows-x64.zip" "$(basename "$W")")

ls -la "$OUT"/pira-pose-"$V"-*
