# Clip Studio Paint EX: what PIRA lacks, and in what order to build it

Researched 2026-11-06 against Clip Studio Paint 5.0 (released 2026-03-11) and the EX/PRO split
Celsys publishes. Inspiration only: build PIRA's own versions of these workflows, with no
Celsys names, icons, materials or UI look (see master plan §6).

PIRA started as a Photoshop-style editor (PhotoCraft), so it is strong where Photoshop is and
thin where Clip Studio is different: line art, comics, animation and multi-page books.

## Already in PIRA (no work needed, or only polish)

| CSP feature | PIRA |
|---|---|
| Layers, masks, clipping, blend modes, adjustment and fill layers, gradient map | Yes (Photoshop model) |
| Liquify, puppet warp, mesh transform, align and distribute | Yes |
| Perspective ruler with snapping, symmetry (straight and along a path) | Yes |
| 3D figures, pose library, hand poses, pose from a photo (CSP's Pose Scanner) | Yes (`docs/pira/3d-layers.md`) |
| Layer comps (EX-only in CSP) | Yes (`comps_cmds`) |
| Auto actions | Yes (Actions panel, `.atn` import) |
| Text: vertical, on a path, kerning, ligatures | Yes |
| Brushes: dual brush, colour jitter, mixing brushes, `.abr` import, pressure and tilt | Yes |
| Color Match, filters (blur, noise, lens blur and the rest) | Yes |
| Print with bleed and crop marks, PDF export | Yes (single image) |
| Timeline, video layers, animated GIF export | Basic: a playhead over frames; frames aren't saved in `.pcraft` yet |

## What PIRA doesn't have

**Line art and colouring (CSP PRO, used every day)**
- Fill that closes small gaps in line art, and "refer to other layers" (a layer marked as the
  reference for fills and selections).
- Lasso fill ("enclose and fill") and "fill what's left unfilled".
- Draft layers (hidden from export and from fills) and layer colour (show a layer's lines in one
  colour, e.g. blue sketch).
- Border effect: a one-click outline around a layer's pixels (PIRA has Photoshop's Stroke style;
  this is the quick version in Layer Properties).
- More rulers: straight, curve, concentric circle, parallel lines, radial and fisheye
  perspective, with "snap to ruler" per layer.
- Smart Shape (5.0): hold at the end of a stroke and it snaps to a line, ellipse or rectangle.
- Timelapse recording of the drawing session.

**Vector line art (CSP PRO)**
- Vector layers that keep each stroke as a pen path with pressure, so you can re-shape and
  re-thicken lines after drawing.
- Vector eraser that cuts a line back to the nearest intersection.
- Line tools: correct line width, simplify, move control points, connect lines, change the brush
  of an existing line.

**Comic page (CSP PRO)**
- Frame border (panel) layers and a cut tool that splits a page into panels with gutters.
- Speech balloons with tails, linked to their text, plus thought and burst shapes.
- Effect lines: speed lines, focus (saturated) lines, flash/burst.
- Screentone layers (halftone dots, lines or noise at a set line frequency) and "show as tone"
  on any layer.
- Comic paper presets (finish size, bleed, safe area) that set up guides on a new document.

**Animation (PRO up to 24 frames, EX unlimited)**
- Cel-based animation folders: one cel per drawing, held for several frames.
- Onion skin and light table (see the frames before and after while drawing).
- Frames saved in `.pcraft`; a timeline UI built for cels on a tablet.
- Export to MP4/WebM, image sequence and GIF; audio layers and scrubbing.
- Camera moves (pan, zoom, rotate over time), keyframe interpolation, timesheet (EX).

**Multi-page and books (EX only)**
- A project of many pages with a page manager (thumbnails, reorder, spreads, page numbers).
- Story editor: all the dialogue of a book in one text view.
- Export a whole book: PDF, EPUB, image sequence; webtoon export that cuts a tall strip into
  screens.
- Convert to lines and tones (EX): turn a photo or 3D layer into inked lines and tones.

**3D extras (CSP PRO)**
- 3D head model, simple shapes (box, sphere, cylinder), 3D backgrounds and props, OBJ/FBX/VRM
  import, perspective ruler that follows the 3D camera. (Already listed in `3d-layers.md` Next.)

**Leave out**
- AI colorize and other AI generation (out of scope in the master plan).
- Teamwork (shared projects through Celsys's cloud), the Assets store (PIRA's own community
  library is planned separately), Companion mode (PIRA has its own Shortcut Pad).

## Dev plan, in order

Order: what a tablet illustrator hits first, then what a single comic page needs, then the
bigger systems (vectors, animation, books). Each stage ships on its own as dev releases.
Sizes are rough for one developer with Claude: S = days, M = 1–2 weeks, L = a month or more.

### Stage 1: line art and colouring basics (S–M each)
1. **Fill gap closing and reference layers**: Close Gap amount on the bucket, a "Reference
   layer" flag, and bucket/magic wand sampling "reference layers". The single biggest day-to-day
   gap for colouring line art.
2. **Lasso fill and "fill unfilled areas"**.
3. **Draft layers and layer colour**: two layer flags; draft is skipped by export, print and
   reference sampling.
4. **Border effect** in Layer Properties (reuses the Stroke style's renderer).
5. **Smart Shape**: hold the pen still at the end of a stroke to snap it to a line, ellipse or
   rectangle; drag before lifting to adjust.

### Stage 2: rulers (M)
6. Ruler layer model (rulers belong to a layer, show on that layer or all), then straight, curve,
   concentric, parallel and radial rulers with snap; fisheye perspective. Builds on
   `stroke_constraint::perspective`.

### Stage 3: comic page (M–L)
7. **Comic paper presets** and guides (finish size, bleed, safe area) in New Document.
8. **Frame border layers and the panel cut tool** (a frame is a vector outline that masks its
   contents; cutting splits it with a gutter).
9. **Balloons** (shape, tail, link to text, auto-fit) on top of PIRA's text layers.
10. **Effect lines** (speed, focus, flash) as editable fill-like layers.
11. **Screentone layers** and "show as tone" (halftone rendering at print line frequency).

### Stage 4: vector layers (L)
12. A stroke layer kind that keeps pen paths with per-point pressure and the brush used, drawn
    by the same brush engine; vector eraser to intersections; line width correction, simplify
    and control point editing. Needs care with `.pcraft` and PSD (rasterize on PSD export).

### Stage 5: animation (L)
13. Save video-layer frames in `.pcraft` (the noted follow-up), then cel animation folders.
14. Onion skin and light table.
15. A tablet timeline for cels, playback, exposure (hold) per cel.
16. Export: MP4/WebM, PNG sequence, GIF (exists), with audio layers after.
17. Camera moves and keyframe interpolation; timesheet last.

### Stage 6: books and EX-only tools (L)
18. Multi-page projects and the page manager (each page a `.pcraft`, a project file listing
    them, spreads, page numbers).
19. Book export: PDF and image sequence first, EPUB and webtoon strip export after.
20. Story editor.
21. Convert to lines and tones for photo and 3D layers (pairs with 3D Next item 2).

### Stage 7: 3D extras (M–L, can run alongside stages 3–6)
22. The items in `3d-layers.md` Next: simple shapes, head model, OBJ/glTF/VRM import, camera
    linked perspective ruler.

Alongside any stage: **timelapse recording** (S–M), since it's popular for sharing work.

## Note on scope

The master plan lists "animation timeline" and "full Clip Studio Paint parity" as out of scope
for the MVP. Stages 1–3 fit the MVP's illustrator and comic-artist personas; stages 4–6 are
beyond it and should wait until the MVP release gate unless the plan changes.

## Sources

- [PRO vs EX: what is the difference? (Clip Studio support)](https://support.clip-studio.com/en-us/faq/articles/20200186)
- [Feature list (clipstudio.net)](https://www.clipstudio.net/en/functional_list/)
- [New features up to Ver. 5.0 (clipstudio.net)](https://www.clipstudio.net/en/functional_list_ver/)
- [Clip Studio Paint 5.0 release (Celsys)](https://www.celsys.com/en/topic/20260311)
- [Clip Studio Paint 4 (CG Channel)](https://www.cgchannel.com/2025/03/celsys-unveils-clip-studio-paint-4/)
