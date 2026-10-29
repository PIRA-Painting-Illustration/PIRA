# Upstream

PIRA started as a copy of the PhotoCraft Android port.

| | |
|---|---|
| Upstream project | [storytold/photocraft](https://github.com/storytold/photocraft), MIT OR Apache-2.0 |
| Port it came from | PhotoCraft for Android, branch `android-port` (local remote `photocraft-port`, push disabled) |
| Fork point | `81c379c49` "README: download 0.16.0-4" (port version 0.16.0-4), 2026-10-29 |
| History | Kept, so every upstream line keeps its author and licence trail |

## Naming policy (for now)

User-facing text says **PIRA**. Machine names are unchanged on purpose: crates and binaries
(`photocraft-*`), Rust types (`PhotocraftApp`), the Java package `ai.storyteller.photocraft` (the
JNI pen hook in `crates/tablet/src/android.rs` is bound to it by name), and the icon file names.
That keeps merges from the port and upstream possible. Renaming them is a later, separate step
(decision D-001/D-002 in [docs/pira/master-plan.md](docs/pira/master-plan.md)).

The Android **application id** is `art.pira`, so PIRA and PhotoCraft install side by side.

## What PIRA changed

- **Themes:** `ThemeKind::Pira` (default) and `PiraLight` (`crates/ui-egui/src/theme.rs`), with
  `pira` / `piraLight` preference values (`crates/engine/src/prefs.rs`).
- **Brushes:** `TiltShading` (`crates/paint/src/brush.rs`, applied in `dynamics.rs`), the PIRA
  Essentials preset group, and `pen_ready` over Dry and Wet Media (`crates/paint/src/presets.rs`);
  the Pen Tilt Shading controls (`crates/ui-egui/src/panels/brush_sections.rs`); the first-launch
  brush (`crates/ui-egui/src/last_brush.rs`).
- **Layout:** Color, Brushes and Layers docked by default (`crates/ui-egui/src/state.rs`).
- **Branding:** window and title bar, start screen, About (with the upstream credit, no ArtCraft
  Discord button), native menu name, update dialog, GPU status, Android label, log tag and
  settings export name.
- **Android:** app id `art.pira`; the updater is off (`Updater.REPO` is empty) until PIRA has its
  own release channel; APK named `pira-<version>-arm64-v8a.apk`.
- **Icon:** placeholder drawn by `tools/pira-icon/make_icon.py`, written over the hicolor PNGs.
  The SVG, `.ico`, `.icns` and 1024 px PNG are still PhotoCraft's kitsune (desktop builds only).

## Not yet rebranded

Many deeper strings (notices, print dialog, preference help, Help menu links to ArtCraft's site and
Discord, translations), the desktop and web apps, and the port's docs under `docs/` and
`apps/photocraft-android/README.md`. Search for `PhotoCraft` in user-facing strings.

## Pulling from the port

```bash
git fetch photocraft-port
git merge photocraft-port/android-port
```

Expect conflicts in the files listed above.
