# Layers

PIRA's layers work like Photoshop's: pixel layers, groups, masks, clipping, blend modes,
opacity and fill, adjustment and fill layers, layer styles, text and shape layers, and
[3D layers](3D-Figures.md). The Layers panel is in the right column.

## The Background layer

A new canvas has a **Background** layer filled with the paper colour, and an empty **Layer 1**
above it to draw on (see [Starting a canvas](Starting-a-Canvas.md)). The Background is locked in
place at the bottom of the stack, like Photoshop's.

## Line art layer settings

PIRA adds four settings for line art and colouring. Each is in **Layer** and in the **Line Art**
section of the Properties panel, is saved in the `.pcraft` file, and shows a marker on the
layer's row.

| Setting | What it does |
|---|---|
| **Reference Layer** | Marks the line art that fills and selections read from. The Paint Bucket and Magic Wand can sample only reference layers while you fill on another layer. |
| **Draft Layer** | For rough sketches: left out of exports, printing and fills, so you can keep the sketch in the file without it showing up in the result. |
| **Line Color** | Recolours the whole layer's lines (blue sketch lines to black, say) without repainting them. You can change or remove it later. |
| **Border Effect** | Draws an outline around everything on the layer, like a sticker edge. |

See [Line art and colouring](Line-Art-and-Colouring.md) for how they work with the fill tools.

## Useful habits

- Ink on one layer and colour on another layer underneath, with the ink marked as a reference
  layer.
- Use clipping (Alt-click between two layers, or **Layer › Create Clipping Mask**) to shade inside
  flat colours without going over the edges.
- Lock transparent pixels to repaint lines without spilling outside them.

## Saving layers

`.pcraft` keeps everything PIRA has, including 3D layers, rulers, timelapse and the line-art
settings. PSD keeps the layers Photoshop understands; PIRA-only layers are saved as pixels there.
See [Files, saving and exporting](Files-Saving-and-Exporting.md).
