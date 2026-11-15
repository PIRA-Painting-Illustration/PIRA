# PIRA Pose

PIRA's posable 3D mannequins as an app of their own, for Android tablets, Windows and Linux. It is the
3D layer (the Object tool and the 3D panel) on a canvas of its own: the same wooden mannequins,
pose library, look and lighting, and photo matching.

## One copy of the posing code

PIRA Pose lives in PIRA's workspace and poses with PIRA's own crates, so a change to posing made
for either app reaches both:

| Crate | What it is | Used by |
| --- | --- | --- |
| `crates/scene3d` (`photocraft-scene3d`) | Mannequins, pose maths, pose library, lights, the CPU renderer | PIRA, PIRA Pose |
| `crates/scene3d-egui` (`photocraft-scene3d-egui`) | The Object tool's drags, handles and overlays, pose thumbnails | PIRA's `ui-egui` (`tools/object_tool.rs`), PIRA Pose |
| `crates/posetrack` (`photocraft-posetrack`) | MediaPipe body and hand landmarks for Match photo | PIRA's engine, PIRA Pose |
| `crates/doc` (`photocraft-doc`) | The scene types (`Scene3d`, `Figure3d`, …) | PIRA, PIRA Pose |

What stays in each app is how it is laid out:

- PIRA's options bar and 3D panel are in `crates/ui-egui/src/tools/object_tool.rs`. They run
  edits as engine commands (`scene3d.*`, in `crates/engine/src/cmd/scene3d_cmds.rs`) on a 3D
  layer.
- PIRA Pose's are in `apps/pira-pose/src/app.rs`. It edits its scene directly, with its own undo.

When you change a menu, a panel tab or a command in one app, make the same change in the other.
When you change how posing works (a drag, a handle, a pose, the renderer), change the shared
crates and both apps get it.

The photo-matching glue (tracking a photo, applying the match, refitting it) is still in two
places: `apps/pira-pose/src/photo.rs` and `crates/engine/src/cmd/scene3d_cmds/photo.rs`.

## What it does

- **Pose by dragging.** Drag a body part to swing it about its joint; drag the hips to move the
  figure. **Twist** (or Alt) turns a part about its own bone. **Joints** shows handles to drag a
  hand, foot, elbow, knee or the head to where it should be.
- **Camera.** Drag empty canvas to orbit. Shift-drag pans; Ctrl-drag or the scroll wheel zooms.
  On a tablet, two fingers pan and pinch to zoom. Also **View** angles, parallel projection and
  **Fit**.
- **Figures.** Male and female mannequins, up to 64. **Floor**, **Mirror** and **Figure** work as
  in PIRA.
- **3D panel:** Poses (the library, including the CMU motion-capture poses), Hands, Match photo
  (open a photo; the photo becomes the canvas), Figures.
- **Look and Light** as in PIRA, including the optional second light (a fill or rim light).
- **Background:** the canvas colour (Look menu or Light panel), neutral grey (#808080) by default,
  saved with the scene. Export PNG puts the figures on it; there are also transparent and
  over-the-photo exports.
- **Panels** (Poses, Hands, Match photo, Figures, Light) dock as in PIRA, with craft-ui's docking
  (`src/dock.rs`). Drag a panel by its tab onto another group to tab it, beside or under a group,
  to the left or right edge for a new dock column, or out to float. Window shows and hides panels
  and resets the arrangement. The layout is saved (`%APPDATA%\PIRA Pose\workspace.json` on
  Windows, the app's storage on Android).
- **Files:** `.pirapose` scenes (a bare `Scene3d` JSON from PIRA's `scene3d.get` opens too) and
  PNG export (transparent, or over the photo). On Android the scene is autosaved.

## Building

Windows: `pira-pose.exe`, built in WSL with MinGW; Linux: `pira-pose`, built natively. The poses
and models are built in.

```bash
cargo build --release --target x86_64-pc-windows-gnu -p pira-pose
```

Android: an arm64 APK (`art.pira.pose`, installs beside PIRA). Signing works as in PIRA
(`~/pira-signing/keystore.properties`, else the debug key):

```bash
source ~/android-dev/env.sh
apps/pira-pose/build-android.sh
```

Tests: `cargo test -p pira-pose -p photocraft-scene3d-egui`, and
`cargo test -p photocraft-ui-egui object_tool` for PIRA's side.

PIRA Pose has its own version (`apps/pira-pose/Cargo.toml`), apart from PIRA's.

## Releasing

PIRA Pose is released from PIRA's repo, on the same Releases page, tagged `pose-v<version>`.
It is never marked "Latest" (that stays PIRA's), and PIRA's updaters ignore `pose-v` tags. From
one pushed commit:

```bash
PIRA_RELEASE=1 apps/pira-pose/build-android.sh   # the APK, release-signed
scripts/pira-pose-desktop-build.sh <dist-dir>    # Linux and Windows, in WSL
scripts/pira-pose-release.sh <notes.md> <dist-dir>
```

Copy the APK into the dist folder before any `cargo clean`. Raise the version in `Cargo.toml` for
each new release. Then update the three `pose-v…` download links in `web/landing/index.html` (the
PIRA Pose section); `build_wiki.py` only keeps PIRA's own links current.

## Credits

Poses from [mocap.cs.cmu.edu](http://mocap.cs.cmu.edu) (NSF EIA-0196217). The poses ship inside
the app and are never sold on their own. Photo matching uses Google's MediaPipe models
(Apache 2.0).
