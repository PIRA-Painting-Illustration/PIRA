# 3D layers

Posable reference figures inside the layer stack, after the workflow painting apps already offer
(drag a mannequin onto the canvas, pose it, draw over it). Inspiration only: no other app's
models, poses, names or icons.

## What there is (phase 1)

- **A 3D layer** (`LayerContent::Scene3d`, `crates/doc/src/scene3d.rs`): figures (male or
  female wooden mannequin, joint rotations by name, position, turn, size), an orbit camera whose
  image is the whole canvas, and a look (shaded, toon, flat or silhouette; outline width and
  colour; light direction; floor shadow). It keeps a rendered copy of itself like a type layer, so
  opacity, blend modes, masks, clipping and effects all apply, and other readers see pixels.
  `.pcraft` saves the scene; PSD gets the pixels (with a warning).
- **The renderer** (`crates/scene3d`): a CPU rasterizer, so it runs the same on desktop, Android
  and the web build, and inside engine commands. Draft quality (one sample per pixel) while
  dragging, smooth (four samples) on release; only the figures' bounding box is rendered.
- **Commands** (`crates/engine/src/cmd/scene3d_cmds.rs`): Layer › New › 3D Figure…,
  `scene3d.applyPose / mirrorPose / flipPose / placeOnFloor / figure / camera / look /
  addFigure / removeFigure / set / get / poses`, Layer › Rasterize › 3D Layer.
- **Object tool** (toolbar, under Move): drag a part to swing it in the canvas plane, with Twist
  (or ⌥) to turn it about its bone; drag the hips to slide the figure (Twist: turn it); drag empty
  canvas to orbit the camera, ⇧ to pan, ⌘ to dolly. The options bar has the drag modes, camera
  views, fit, place on floor, mirror, figure, look and light menus, and Rasterize.
- **Light**: the sun mode of the Object tool puts the main light where you drag on a disc around
  the figures (centre: from the camera; rim: side-on; out to the outer ring: from behind, for rim
  light). The Light menu has presets (upper left/right, front, sides, overhead, below, rim), sliders
  for direction, height, strength and ambient, a light colour, the floor shadow, and whether the
  light turns with the camera (the default) or stays fixed in the scene as you orbit
  (`scene3d.look` `lightPreset / lightYaw / lightPitch / lightStrength / lightColor / lightFixed`).
- **3D panel** (the icon under Brushes): new male or female figure, add one to the layer, hand
  poses for either or both hands, and the pose library by category or search: 11 built-in poses
  plus 1,072 motion-capture poses from the CMU Graphics Lab database (see `ATTRIBUTION.md`; not to
  be sold as a separate pack). A tap applies the pose to the selected figure, or makes a new 3D
  layer when the active layer isn't one.
- **Match to photo** (3D panel: Body, Both hands, Left hand, Right hand; `scene3d.matchPhoto`):
  poses the selected figure like the person or hands in a reference photo. The photo is the canvas
  without its 3D layers (or one layer's pixels, `source`), so you put a photo on a layer under the
  figure and tap Body. The figure is turned so it faces the layer's camera as the person faced
  theirs (upright: only the camera's turn counts, not its tilt), and keeps its place and size.
  Hands take the finger pose and, by default, the wrist angle; each hand found goes onto the
  figure's hand of that side, or the clearest one onto the hand you pick. It runs as a background
  job (progress in the status bar; a second or two longer the first time, while the models
  load) and is one undo step.
  - Tracking: `crates/posetrack`, Google's MediaPipe models (person and palm detectors, BlazePose
    body and hand landmarks; Apache 2.0, ONNX from the OpenCV Zoo, `assets/models/mediapipe`, about
    25 MB built into the app) run on the CPU with tract. Not in the browser build.
  - Turning landmarks into joint rotations: `photocraft_scene3d::retarget`, ported from PIRA Pose.

## Next

1. On-canvas object launcher bar and a root gizmo (move, turn, scale handles); rotation rings
   per joint; drag a hand or foot with IK; joint pins.
2. Inner contour lines and tone bands tuned for line art ("convert to lines" without leaving
   the layer); a second light; cast shadows between body parts.
3. A perspective ruler that follows the 3D camera; simple shapes (box, sphere, cylinder) as
   objects.
4. Pose from the live camera (desktop webcam; Android camera through the Java side); draw the
   tracked points over the photo before applying; line the figure up with the person in the
   photo (place and size, not just pose); half-size (fp16) models to trim the app; glTF / VRM
   import.
