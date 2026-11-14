# Animation and Render Video

PIRA has a basic timeline for frame animation, inherited from PhotoCraft. Full animation tools are
on the roadmap.

## The timeline

- **Window › Timeline** opens the Timeline panel.
- Create a timeline, set its length in frames and its frame rate, and move the playhead to the
  frame you want to show or change.
- Video layers can be imported into a canvas, and **File › Import › Video Frames to Layers…**
  turns a video's frames into layers.

Timeline frames aren't saved in `.pcraft` files yet.

## Render Video

**File › Export › Render Video…** renders every frame of the timeline.

- **On Android** it asks where to save one file and makes an **MP4** with the tablet's video
  encoder, at the timeline's frame rate and up to 1920 pixels on the longer side. Rename the file
  to end in `.gif` to save an **animated GIF** instead.
- **On Windows and Linux** it asks for a folder and writes one numbered PNG per frame
  (`name_0000.png`, `name_0001.png` and so on), ready for a video editor.

A canvas with no timeline renders as a single frame.

## Not to be confused with Timelapse

[Timelapse](Timelapse.md) records you painting. Render Video exports an animation you made on the
timeline.
