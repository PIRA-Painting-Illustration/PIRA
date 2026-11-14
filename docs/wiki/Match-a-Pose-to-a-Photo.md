# Match a pose to a photo

PIRA can pose a 3D figure like the person, or the hands, in a reference photo.

## How to

1. Put the photo on a layer **under** the 3D layer.
2. Select the figure, open the 3D panel's **Match photo** tab, and tap **Body**, **Both hands**,
   **Left hand** or **Right hand**.
3. The match runs in the background, with progress in the status bar. The first match takes a
   second or two longer while the models load.

The figure turns to face the camera the way the person faced theirs, and keeps its place and
size. Hands take the finger pose and, by default, the wrist angle. The whole match is one undo
step.

## Quick or Accurate

- **Quick** is fast. Try Accurate if a pose comes out wrong.
- **Accurate** uses a larger model that's steadier on hard poses, and is about three times slower.

## Fixing a match

After a body match, **Fix the match** shows the photo's joints as coral dots:

- **Drag a dot** to move that joint, and the figure refits when you let go.
- **Set a limb's depth:** each part of a limb can be set toward or away from the camera, for the
  cases where a flat photo can't tell which way it points.
- **Undo all fixes** goes back to the tracker's own answer.
- **Done** hides the joints.

## What it uses

Tracking uses Google's MediaPipe body and hand models (Apache 2.0), which run on the device. No
photo leaves your tablet or computer. Matching isn't available in the web build.
