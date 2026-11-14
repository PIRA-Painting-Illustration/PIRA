# Timelapse

PIRA can record your painting as you go and turn it into a short video, like Clip Studio Paint's
timelapse.

## Recording

Turn recording on in either place:

- **File › New…** › **Record timelapse**, for a new canvas, or
- **File › Timelapse › Record Timelapse** for the open canvas. A tick shows it's on.

Then paint as usual. After each change PIRA keeps a small snapshot of the canvas, up to 1280
pixels on its longer side. Pauses and menu use don't show; undoing a stroke is recorded too.
Snapshots are taken in the background, at most every 0.4 seconds, so painting doesn't slow down.

- The recording is saved **inside the `.pcraft` file** and carries on when you reopen it.
- A long painting keeps at most 2,000 snapshots. After that PIRA thins them out evenly, so the
  start and the end are both kept.
- **Turning recording off deletes the recording** for good.

## Exporting

**File › Timelapse › Export Timelapse…** opens the export window:

| Option | Choices |
|---|---|
| **Length** | 15 s, 30 s, 60 s, or **All** (every snapshot once) |
| **Size** | 720, 1080 or 1280 pixels on the longer side |
| **Shape** | Original (the canvas's shape), 16:9, 4:3, 1:1, 4:5 or 9:16 |

The window shows the video's size and length before you export. Shapes other than Original fill
the space around the canvas with black.

Tap **Export…**, choose where to save, and a progress bar runs while the MP4 is made. The video
plays at 30 frames a second and ends on the finished picture, held for 2 seconds.

## Notes

- Export uses the tablet's own video encoder, so it's available on **Android**. On Windows and
  Linux, recording works but export isn't available yet.
- A long recording adds to the `.pcraft` file's size, as it does in Clip Studio Paint.
