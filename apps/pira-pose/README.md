# PIRA Pose

PIRA's posable 3D mannequins as an app of their own, for Android tablets and Windows. It is the
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
- **Look and Light** as in PIRA.
- **Files:** `.pirapose` scenes (a bare `Scene3d` JSON from PIRA's `scene3d.get` opens too) and
  PNG export (transparent, or over the photo). On Android the scene is autosaved.

## Building

Windows: `pira-pose.exe`, built in WSL with MinGW. The poses and models are built in.

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

## Credits

Poses from [mocap.cs.cmu.edu](http://mocap.cs.cmu.edu) (NSF EIA-0196217). The poses ship inside
the app and are never sold on their own. Photo matching uses Google's MediaPipe models
(Apache 2.0).
