# Line art and colouring

## Flatting with the Paint Bucket

The Paint Bucket and Magic Wand have three options that make colouring line art quick:

- **Sample:** the current layer, all layers, or **only [reference layers](Layers.md)**. Mark your
  inks as a reference layer, then fill on a colour layer underneath: the fill stops at the inks
  even though you're on another layer.
- **Close Gap:** small breaks in the lines still hold the fill in, so a sketchy line doesn't let
  colour leak into the next area. Raise it for gappier lines.
- **Expand:** grows the fill a few pixels so it tucks under the lines, leaving no white halo.

## Lasso Fill

Drag with the Paint Bucket instead of tapping and it becomes **Lasso Fill**. You choose what the
loop fills:

- the shape you drew,
- the closed areas inside it, or
- only the pixels inside it that aren't filled yet, for mopping up the gaps after flatting.

## Recolouring lines

**Line Color** on a layer recolours all its lines at once, so a blue sketch can turn black, or
inks can take a colour that suits the area around them. See [Layers](Layers.md).

## Clean lines

- [Smart Shape](Pens-Pencils-and-Brushes.md#smart-shape) snaps a held stroke to a line, ellipse or
  rectangle.
- [Special rulers](Rulers-and-Guides.md) keep strokes on curves, circles and perspective lines.
- **Draft Layer** keeps a rough sketch in the file but out of exports and fills.

## A typical workflow

1. Sketch on a layer and mark it **Draft Layer**.
2. Ink on a new layer above and mark it **Reference Layer**.
3. Make a colour layer under the inks, pick the Paint Bucket, set Sample to reference layers and
   Close Gap to taste, and tap each area.
4. Use Lasso Fill's "unfilled" mode to catch the leftovers.
5. Shade on clipped layers above the flats.
