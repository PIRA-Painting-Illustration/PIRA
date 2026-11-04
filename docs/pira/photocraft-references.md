# PhotoCraft references that users see

PIRA is built on PhotoCraft, and the code underneath keeps that name: crate names
(`photocraft-*`), Rust types (`PhotocraftApp`), the Java package `ai.storyteller.photocraft`,
logs, and the `storytold/photocraft` links in code comments that record where code came from.
None of that changes.

This list covers only what a user sees or what ends up in their files and on their system, so it
can say PIRA, and so the links can point at the PIRA website once there is one. Credit to
PhotoCraft and ArtCraft stays: it moves to a credits page (see the last section).

Found 2026-11-03. Recheck before working through it:
`git grep -n '"[^"]*PhotoCraft' -- ':!*test*'`.

**Done 2026-11-04:** sections 1–4, apart from the items below, now say PIRA, with their
translation keys renamed in all catalogs:
- Help: "PIRA on GitHub" (until the website exists), "Built on PhotoCraft" (upstream credit),
  "ArtCraft Website", "Report an Issue…" (PIRA's issues) and "About PIRA". "Join the ArtCraft
  Discord…" is gone: it isn't PIRA's community.
- Window title, OS app name, Windows .exe details, file-dialog filter, print, rasterize prompt,
  screen picker, the New Document preset and every message in sections 2–3.
- The built-in shortcut set is "PIRA Defaults"; "PhotoCraft Defaults" in saved preferences still
  means it (`shortcut_sets::OLD_DEFAULTS_NAME`). The default action set is "PIRA Actions".
- Android update messages.

**Still open:** section 5 (names written into files, which needs profile aliases), section 6
installers (.wxs, Info.plist, .desktop, Flatpak) and the desktop `APP_ID`, the web build's
loading text, the CLI, the MCP description, the portable-mode folder names, and the user guide
in `docs/user/` (it points at PhotoCraft's issue tracker and Discord).

## 1. Links (switch to the website when it's up)

All Help-menu and start-screen links come from one file,
[`crates/ui-egui/src/links.rs`](../../crates/ui-egui/src/links.rs):

| Constant | Points at now | Used by | Should become |
|---|---|---|---|
| `APP_PAGE` | getartcraft.com/apps/photocraft | Help › "PhotoCraft Website" | PIRA website home |
| `GITHUB` | github.com/storytold/photocraft | Help › "PhotoCraft on GitHub"; start screen "Built on PhotoCraft" | Help: `PIRA_GITHUB`; start screen: credits page |
| `ISSUES` | github.com/storytold/photocraft/issues | Help › "Report an Issue…" | PIRA issues (or the site's support page) |
| `ARTCRAFT_WEBSITE` | getartcraft.com | Help › "ArtCraft Website" | move to the credits page |
| `DISCORD` | discord.gg/artcraft | Help › "Join the ArtCraft Discord…", the Discord button | PIRA community, or remove |
| `PIRA_GITHUB` | github.com/PIRA-Painting-Illustration/PIRA | start screen | keep |

Also:

- **Update check (desktop): fixed 2026-11-03.** It used to be set up as PhotoCraft
  (`craft_update::suite::PHOTOCRAFT`), so it would have offered PhotoCraft's releases and
  installed them over PIRA. It now has PIRA's own entry (`UPDATE_APP` in
  [`crates/ui-egui/src/lib.rs`](../../crates/ui-egui/src/lib.rs): PIRA,
  `PIRA-Painting-Illustration/PIRA`), and `RELEASES` and the user-agent point at PIRA. It stays
  off (`UPDATES_ENABLED = false` in the same file) until PIRA publishes public releases. To turn
  it on, set that to `true`, and set `REPO` in `Updater.java:58` for Android.
- **Update check (Android):** `REPO` in `Updater.java:58` is empty, so it's off. Its messages
  still say PhotoCraft (section 3).
- The test in `links.rs:130-135` lists the old URLs and has to change with them.

## 2. Menus, window titles, dialogs

| Where | Text now |
|---|---|
| [`crates/engine/src/shell_commands.rs:96`](../../crates/engine/src/shell_commands.rs) | Help › "PhotoCraft Website" |
| `shell_commands.rs:98` | Help › "PhotoCraft on GitHub" |
| `shell_commands.rs:101` | Help › "About PhotoCraft" (the dialog itself already says "About PIRA") |
| `shell_commands.rs:95, 97` | "Join the ArtCraft Discord…", "ArtCraft Website" (move to credits) |
| [`apps/photocraft/src/main.rs:152`](../../apps/photocraft/src/main.rs) | desktop window title "PhotoCraft" |
| `apps/photocraft/src/main.rs:493` | app name passed to the OS ("PhotoCraft") |
| [`crates/ui-egui/src/dialogs/screen_picker.rs:265`](../../crates/ui-egui/src/dialogs/screen_picker.rs) | window title "PhotoCraft — Screen Color Picker" |
| [`apps/photocraft/src/startup_report.rs:41, 60`](../../apps/photocraft/src/startup_report.rs) | "PhotoCraft could not start" |
| [`crates/ui-egui/src/dialogs/file_ui/print.rs:505`](../../crates/ui-egui/src/dialogs/file_ui/print.rs) | Print › "PhotoCraft Manages Colors" |
| `print.rs:33` | "PhotoCraft Print Settings" (saved settings label) |
| [`crates/ui-egui/src/dialogs/rasterize_prompt.rs:120, 164`](../../crates/ui-egui/src/dialogs/rasterize_prompt.rs) | prompt title "PhotoCraft" |
| [`crates/ui-egui/src/dialogs/new_doc_ui.rs:20`](../../crates/ui-egui/src/dialogs/new_doc_ui.rs) | New Document preset "Default PhotoCraft Size" |
| [`crates/engine/src/shortcut_sets.rs:21`](../../crates/engine/src/shortcut_sets.rs) | shortcut set "PhotoCraft Defaults" (also `prefs_ui/shortcuts.rs:451, 583`) |
| [`crates/engine/src/cmd/atn_cmds.rs:72`](../../crates/engine/src/cmd/atn_cmds.rs) | default action set "PhotoCraft Actions" |
| [`crates/ui-egui/src/prefs/prefs_ui/fields.rs:261`](../../crates/ui-egui/src/prefs/prefs_ui/fields.rs) | "These settings aren't available in PhotoCraft yet." |
| [`crates/ui-egui/src/prefs/kys_import.rs:89, 219`](../../crates/ui-egui/src/prefs/kys_import.rs) | shortcut import prompts |
| [`crates/ui-egui/src/panels/actions.rs:386-387`](../../crates/ui-egui/src/panels/actions.rs) | "{n} steps have no PhotoCraft equivalent yet" |

Renaming "PhotoCraft Defaults" and "PhotoCraft Actions" changes names that are stored in
preferences and saved action sets. Keep reading the old names so existing setups still load.

## 3. Status, warning and error messages

Users see these in the status bar, in notices or as command errors:

- [`crates/ui-egui/src/notices.rs:85, 87, 100, 107, 110`](../../crates/ui-egui/src/notices.rs): Wayland drag-and-drop and clipboard notices
- [`crates/ui-egui/src/prefs/prefs_ui/save.rs:104`](../../crates/ui-egui/src/prefs/prefs_ui/save.rs), [`prefs_ui/startup.rs:107`](../../crates/ui-egui/src/prefs/prefs_ui/startup.rs): preference save and load warnings
- [`apps/photocraft/src/screen_color.rs:297`](../../apps/photocraft/src/screen_color.rs): macOS screen-recording permission message
- [`crates/engine/src/cmd/atn_cmds.rs:94`](../../crates/engine/src/cmd/atn_cmds.rs), [`automate_cmds.rs:38, 413`](../../crates/engine/src/cmd/automate_cmds.rs), [`face_cmds.rs:17`](../../crates/engine/src/cmd/face_cmds.rs), [`preset_import_cmds.rs:361`](../../crates/engine/src/cmd/preset_import_cmds.rs), [`smart_cmds/filters.rs:165`](../../crates/engine/src/cmd/smart_cmds/filters.rs), [`swatch_cmds.rs:501`](../../crates/engine/src/cmd/swatch_cmds.rs), [`preset_store.rs:293`](../../crates/engine/src/preset_store.rs)
- [`crates/io/src/lib.rs:587`](../../crates/io/src/lib.rs), [`io/src/xcf.rs:844, 852`](../../crates/io/src/xcf.rs), [`io/src/psd_export.rs:936`](../../crates/io/src/psd_export.rs), [`io/src/abr_map.rs:125`](../../crates/io/src/abr_map.rs)
- [`crates/codecs/src/codecs/heif.rs:21`](../../crates/codecs/src/codecs/heif.rs), [`crates/raw/src/jxl.rs:22`](../../crates/raw/src/jxl.rs)
- [`crates/automation/src/workspace.rs:106`](../../crates/automation/src/workspace.rs); [`automation/src/server/conventions.rs:12`](../../crates/automation/src/server/conventions.rs) (the description AI clients see over MCP)
- Android, `apps/photocraft-android/android/app/src/main/java/ai/storyteller/photocraft/Updater.java`, lines 131, 260, 261, 280, 348 and 399: update toasts and dialogs (no effect while `REPO` is empty)
- Web: `apps/photocraft-web/src/web.rs:113`, `apps/photocraft-web-boot/src/main.rs:249, 333, 336`, and the `<title>` and loading text in `apps/photocraft-web/index.html` and `apps/photocraft-web-boot/index.html`

## 4. Translations

The 26 catalogs in `crates/ui-egui/src/i18n/*.tsv` are keyed by the English text. About 28
entries per language contain "PhotoCraft". This matters two ways:

- Each string renamed in sections 2–3 needs the same rename in its catalog key, or that string
  falls back to English.
- **Fixed 2026-11-03:** five strings had been renamed to PIRA in code while the catalogs kept
  the PhotoCraft keys, so they showed in English in every other language: "PIRA Updates",
  "PIRA is up to date.", "PIRA {version} is available.", "PIRA is using the CPU image
  compositor…" and "Save your work and restart PIRA…". Their keys and translations now say PIRA
  in all 26 catalogs (Hindi had none of them).
- Old entries no longer used by the code ("Quit PhotoCraft", "PhotoCraft website",
  "PhotoCraft — an open-source…", the Wayland drag-and-drop sentence) do no harm. The Mac app
  menu translates "Quit {app}" and fills in PIRA.

The translated text also says PhotoCraft, so the translations need updating as well as the keys.

## 5. Names written into users' files

These end up inside files people save and send to others:

| Where | What |
|---|---|
| [`crates/cms/src/builtin.rs:86-94, 149, 308`](../../crates/cms/src/builtin.rs) | built-in colour profile names, e.g. "Linear sRGB (PhotoCraft)", embedded in exported images |
| [`crates/cms/src/synth.rs:32`](../../crates/cms/src/synth.rs) | "PhotoCraft Coated CMYK (…)" profile |
| [`crates/io/src/camera_raw_xmp.rs:278`](../../crates/io/src/camera_raw_xmp.rs) | XMP toolkit `x:xmptk="PhotoCraft"` in sidecars |
| [`crates/engine/src/cmd/file_cmds/export.rs:175`](../../crates/engine/src/cmd/file_cmds/export.rs) | exported .cube LUT: "# Created by PhotoCraft" |
| [`crates/engine/src/cmd/print_cmds.rs:1479`](../../crates/engine/src/cmd/print_cmds.rs) | EPS "%%Creator: PhotoCraft" |
| [`crates/codecs/src/codecs/dpx.rs:671`](../../crates/codecs/src/codecs/dpx.rs) | DPX creator field |

Commands and saved settings can name a built-in profile by its full name
(`Builtin::from_id`, `builtin.rs:117`), so when renaming, add the old names as aliases. Otherwise
saved settings and scripts that use them stop resolving.

## 6. App identity and installers

| Where | What |
|---|---|
| [`crates/ui-egui/src/os_window.rs:6`](../../crates/ui-egui/src/os_window.rs) | `APP_ID = "ai.storyteller.photocraft"`: groups windows on the Linux taskbar and identifies the app to the OS. Android already uses `art.pira`; the desktop should get its own id too, so PIRA and PhotoCraft can be installed side by side. |
| [`apps/photocraft/src/services.rs:43, 91`](../../apps/photocraft/src/services.rs) | file-dialog filter "PhotoCraft (*.pcraft)" |
| [`apps/photocraft/src/app_dirs.rs:21-23`](../../apps/photocraft/src/app_dirs.rs) | portable-mode marker `PhotoCraft.portable` and folder `PhotoCraftData` (keep reading the old names) |
| [`apps/photocraft/build.rs:24-27`](../../apps/photocraft/build.rs), [`apps/photocraft-cli/build.rs:28-31`](../../apps/photocraft-cli/build.rs) | Windows .exe ProductName, FileDescription and copyright (shown in Explorer › Properties and Task Manager) |
| `packaging/windows/photocraft.wxs` | installer: product name, install folder, Start Menu shortcut, "PhotoCraft Document" file type, registry keys |
| `packaging/macos/Info.plist.in` | bundle name, display name, document type "PhotoCraft Document", copyright |
| `packaging/linux/ai.storyteller.photocraft.desktop`, `…metainfo.xml.in`, `flatpak/*.yml` | launcher name, app store description, Flatpak id |
| `apps/photocraft-android/android/app/src/main/res/values/themes.xml:3`, `settings.gradle:15` | Android theme and Gradle project name (not visible; the launcher label is already "PIRA") |

The `.pcraft` file extension can stay. Its display name ("PhotoCraft Document") is what users see.

## 7. Already PIRA (no change)

- About dialog title "About PIRA" and its blurb ([`crates/ui-egui/src/dialogs.rs:830-841, 873`](../../crates/ui-egui/src/dialogs.rs))
- Android launcher label "PIRA" and application id `art.pira`
- Update dialog ("PIRA Updates", "PIRA is up to date."), GPU fallback notices, Mac app menu
  (About, Hide and Quit PIRA)
- `web/landing/index.html`

## 8. Credits page (planned)

When the website is up, one credits page replaces the scattered upstream links:

- What PIRA is built on: PhotoCraft by the ArtCraft contributors (MIT OR Apache-2.0), with links
  to github.com/storytold/photocraft, getartcraft.com and the ArtCraft Discord.
- That PIRA is independent and not made or supported by them. Keep the wording from the About
  dialog, `dialogs.rs:838-839`.
- The other credits already in the app: `ATTRIBUTION.md`, Help › About › Contributors and Models,
  and the CMU motion-capture acknowledgement.

Then in the app:

- Help › "PhotoCraft Website" becomes "PIRA Website".
- Help › "PhotoCraft on GitHub" becomes "PIRA on GitHub".
- "ArtCraft Website" and "Join the ArtCraft Discord…" are replaced by one "Credits" item that
  opens the credits page.
- The start screen's "Built on PhotoCraft" link goes to the credits page.
- The About dialog keeps its one-line upstream credit and gets a link to the credits page.

Suggested shape: add `PIRA_WEBSITE` and `PIRA_CREDITS` constants to `links.rs` next to
`PIRA_GITHUB`, so the URLs change in one place on launch day.
