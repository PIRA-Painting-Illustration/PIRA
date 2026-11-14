# 3D figures

PIRA has posable wooden mannequins you can draw over, inside the layer stack. A 3D layer behaves
like any other layer: opacity, blend modes, masks and clipping all apply, so you can fade the
figure and sketch on a layer above it.

## Adding a figure

- **Layer › New › 3D Figure…**, or
- open the **3D** panel and tap a pose: it goes onto the selected figure, or makes a new 3D layer
  when the current layer isn't one.

The 3D panel can add a male or female figure, put more than one figure on a layer, and remove
them.

## Posing with the Object tool

Pick the **Object tool** (with the Move tool in the toolbar), then:

- **Drag a body part** to swing it. Turn on **Twist** (or hold Alt) to turn it about its own bone.
- **Drag the hips** to slide the whole figure; with Twist, to turn it.
- **Joints** mode: drag the head, elbows, wrists, knees and ankles to where they should be, and the
  limbs follow.
- **Drag empty canvas** to orbit the camera; Shift pans and Ctrl moves closer or further.

The options bar has the drag modes, camera views, fit, place on floor, mirror and flip, and menus
for the figure, its look and the light. **Layer › Rasterize › 3D Layer** turns it into plain pixels.

## The pose library

The 3D panel lists poses by category, with a search:

- 11 built-in poses, and
- 1,072 motion-capture poses from the CMU Graphics Lab Motion Capture Database.
- **Hand poses** for the left, right or both hands.

Tap a pose to apply it, then fine-tune any joint with the Object tool.

## Look

The look menu sets shaded, toon, flat or silhouette rendering, an outline's width and colour, and
the floor shadow.

## Lighting

Lighting is in the **3D Light** panel:

- **Drag the sun:** in Light mode, drag on the disc around the figure. The centre lights from the
  camera, the rim from the side, and out to the outer ring from behind, for rim light.
- **Presets:** upper left or right, front, sides, overhead, below, rim.
- **Sliders** for direction, height, strength and ambient light, and a light colour.
- **Turns with the camera** (the default) or stays fixed in the scene as you orbit.
- **Second light:** an optional fill or rim light with its own direction, strength and colour. With
  the floor shadow on, it casts its own lighter shadow. Its smaller sun, marked 2, can be dragged
  too.

## Gravity

The 3D panel's **Gravity** tab drops the figure so it falls and hangs with real weight and joint
limits:

1. Pick the Object tool's **Pin** mode and tap the parts to hold still: pin a hand to hang the
   figure from it, or the feet to keep them planted. **Remove All Pins** clears them.
2. Set a **Push** (which way and how hard) and **Muscle tone** (how limp or stiff).
3. Tap **Drop**. Play or scrub through the fall over the canvas.
4. **Keep** the moment you want (one undo step), or **Cancel**. **Drop again** tries another fall.

## Match a pose to a photo

Put a photo under the 3D layer and PIRA poses the figure like the person in it. See
[Match a pose to a photo](Match-a-Pose-to-a-Photo.md).

## Saving

`.pcraft` saves the whole scene, so you can come back and re-pose it. PSD saves the figure as
pixels.

## Credits

Motion-capture poses come from the CMU Graphics Lab Motion Capture Database,
[mocap.cs.cmu.edu](http://mocap.cs.cmu.edu), created with funding from NSF EIA-0196217. They are
not to be sold as a separate pack.
