//! Line-art fill extras for the Paint Bucket and Magic Wand: Close Gap (a fill doesn't leak
//! through small breaks in the lines) and Expand (the fill reaches a few pixels under the lines,
//! so no pale fringe is left between colour and ink).
//!
//! Close Gap thickens the walls (pixels unlike the seed) by half the gap, floods from the seed
//! inside what is left, then grows the result back out by the same amount through pixels like the
//! seed, so the fill still reaches the real lines. A seed inside the thickened walls (a spot
//! narrower than the gap) fills as if there were no gap closing.

/// Close Gap and Expand, in pixels. Zero is off.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GapOptions {
    /// Breaks in the lines up to this wide don't let the fill through (contiguous fills only).
    pub close_gap: u32,
    /// Grow the filled area by this many pixels in every direction.
    pub expand: u32,
}

/// Widest gap closed and furthest expansion: kept small so the work stays local and the chamfer
/// distances fit in a byte.
pub const MAX_GAP: u32 = 64;
pub const MAX_EXPAND: u32 = 32;

/// Bounding box of the marked pixels: `(x0, y0, x1, y1)` inclusive, `x0 == usize::MAX` if none.
pub(super) type Bbox = (usize, usize, usize, usize);

/// Apply `opts` to a flood's `marks` (`w × h`, 255 = filled). `like_seed(i)` says whether pixel
/// `i` matches the seed; `seed` is the seed's index. Returns the new bounding box.
/// A flood's marks: `w × h` bytes (255 = filled) and their bounding box.
pub(super) struct Marks<'a> {
    pub marks: &'a mut [u8],
    pub w: usize,
    pub h: usize,
    pub bbox: Bbox,
}

/// Apply `opts` to a flood's marks. `like_seed(i)` says whether pixel `i` matches the seed;
/// `seed` is the seed's index. Returns the new bounding box.
pub(super) fn apply(m: Marks, seed: usize, contiguous: bool, opts: GapOptions, like_seed: &(dyn Fn(usize) -> bool + Sync)) -> Bbox {
    let Marks { marks, w, h, mut bbox } = m;
    let gap = opts.close_gap.min(MAX_GAP);
    if contiguous
        && gap > 0
        && let Some(closed) = close_gap(w, h, seed, gap, like_seed)
    {
        marks.copy_from_slice(&closed.0);
        bbox = closed.1;
    }
    let expand = opts.expand.min(MAX_EXPAND);
    if expand > 0 && bbox.0 != usize::MAX {
        bbox = grow(marks, w, h, expand, &|_| true, bbox);
    }
    bbox
}

/// Grow a `w × h` mask's marked pixels (non-zero) by `by` pixels in every direction (8-connected
/// steps), marking the new pixels 255. Lasso Fill's Expand.
pub fn expand_marks(marks: &mut [u8], w: usize, h: usize, by: u32) {
    let mut bbox: Bbox = (usize::MAX, usize::MAX, 0, 0);
    for (i, &m) in marks.iter().enumerate() {
        if m != 0 {
            let (x, y) = (i % w, i / w);
            bbox = (bbox.0.min(x), bbox.1.min(y), bbox.2.max(x), bbox.3.max(y));
        }
    }
    if bbox.0 != usize::MAX && h > 0 {
        grow(marks, w, h, by.min(MAX_EXPAND), &|_| true, bbox);
    }
}

/// The flood from `seed` with gaps up to `gap` pixels closed, or `None` when the seed sits inside
/// the thickened walls.
fn close_gap(w: usize, h: usize, seed: usize, gap: u32, like_seed: &(dyn Fn(usize) -> bool + Sync)) -> Option<(Vec<u8>, Bbox)> {
    let r = gap.div_ceil(2);
    let dist = wall_distance(w, h, like_seed);
    // Chamfer units: 2 per straight step. Open = further than `r` pixels from any wall.
    let open = |i: usize| u32::from(dist[i]) > 2 * r;
    if !open(seed) {
        return None;
    }
    let mut marks = vec![0u8; w * h];
    let mut bbox: Bbox = (usize::MAX, usize::MAX, 0, 0);
    let mut stack = vec![(seed % w, seed / w)];
    while let Some((x, y)) = stack.pop() {
        let row = y * w;
        if marks[row + x] != 0 {
            continue;
        }
        let (mut l, mut rr) = (x, x);
        while l > 0 && marks[row + l - 1] == 0 && open(row + l - 1) {
            l -= 1;
        }
        while rr + 1 < w && marks[row + rr + 1] == 0 && open(row + rr + 1) {
            rr += 1;
        }
        marks[row + l..=row + rr].fill(255);
        bbox = (bbox.0.min(l), bbox.1.min(y), bbox.2.max(rr), bbox.3.max(y));
        for ny in [y.wrapping_sub(1), y + 1] {
            if ny >= h {
                continue;
            }
            let nrow = ny * w;
            let mut in_run = false;
            for nx in l..=rr {
                let ok = marks[nrow + nx] == 0 && open(nrow + nx);
                if ok && !in_run {
                    stack.push((nx, ny));
                }
                in_run = ok;
            }
        }
    }
    // Back out to the lines: `r` steps plus one for the pixels the walls' own edge covered.
    let bbox = grow(&mut marks, w, h, r + 1, like_seed, bbox);
    Some((marks, bbox))
}

/// Chamfer distance (2 per straight step, 3 per diagonal, capped at 255) from each pixel to the
/// nearest pixel unlike the seed.
fn wall_distance(w: usize, h: usize, like_seed: &(dyn Fn(usize) -> bool + Sync)) -> Vec<u8> {
    let mut d = vec![255u8; w * h];
    #[cfg(not(target_arch = "wasm32"))]
    {
        use rayon::prelude::*;
        d.par_chunks_mut(w).enumerate().for_each(|(y, row)| {
            for (x, v) in row.iter_mut().enumerate() {
                if !like_seed(y * w + x) {
                    *v = 0;
                }
            }
        });
    }
    #[cfg(target_arch = "wasm32")]
    for (i, v) in d.iter_mut().enumerate() {
        if !like_seed(i) {
            *v = 0;
        }
    }
    let step = |a: u8, add: u8| a.saturating_add(add);
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            let mut v = d[i];
            if v == 0 {
                continue;
            }
            if x > 0 {
                v = v.min(step(d[i - 1], 2));
            }
            if y > 0 {
                v = v.min(step(d[i - w], 2));
                if x > 0 {
                    v = v.min(step(d[i - w - 1], 3));
                }
                if x + 1 < w {
                    v = v.min(step(d[i - w + 1], 3));
                }
            }
            d[i] = v;
        }
    }
    for y in (0..h).rev() {
        for x in (0..w).rev() {
            let i = y * w + x;
            let mut v = d[i];
            if v == 0 {
                continue;
            }
            if x + 1 < w {
                v = v.min(step(d[i + 1], 2));
            }
            if y + 1 < h {
                v = v.min(step(d[i + w], 2));
                if x + 1 < w {
                    v = v.min(step(d[i + w + 1], 3));
                }
                if x > 0 {
                    v = v.min(step(d[i + w - 1], 3));
                }
            }
            d[i] = v;
        }
    }
    d
}

/// Grow the marked area by `steps` pixels (8-connected) into pixels `allowed` lets it, keeping
/// the bounding box up to date. Only the area's edge is visited.
fn grow(marks: &mut [u8], w: usize, h: usize, steps: u32, allowed: &(dyn Fn(usize) -> bool + Sync), bbox: Bbox) -> Bbox {
    let mut bbox = bbox;
    // The first frontier: marked pixels with an unmarked neighbour, inside the box.
    let mut frontier = Vec::new();
    for y in bbox.1..=bbox.3 {
        for x in bbox.0..=bbox.2 {
            let i = y * w + x;
            if marks[i] != 0 && neighbours(x, y, w, h).any(|j| marks[j] == 0) {
                frontier.push(i);
            }
        }
    }
    for _ in 0..steps {
        let mut next = Vec::new();
        for &i in &frontier {
            let (x, y) = (i % w, i / w);
            for j in neighbours(x, y, w, h) {
                if marks[j] == 0 && allowed(j) {
                    marks[j] = 255;
                    let (jx, jy) = (j % w, j / w);
                    bbox = (bbox.0.min(jx), bbox.1.min(jy), bbox.2.max(jx), bbox.3.max(jy));
                    next.push(j);
                }
            }
        }
        if next.is_empty() {
            break;
        }
        frontier = next;
    }
    bbox
}

fn neighbours(x: usize, y: usize, w: usize, h: usize) -> impl Iterator<Item = usize> {
    let (x, y) = (x as isize, y as isize);
    (-1isize..=1).flat_map(move |dy| (-1isize..=1).map(move |dx| (dx, dy))).filter_map(move |(dx, dy)| {
        let (nx, ny) = (x + dx, y + dy);
        ((dx, dy) != (0, 0) && nx >= 0 && ny >= 0 && (nx as usize) < w && (ny as usize) < h).then(|| ny as usize * w + nx as usize)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 40×20 picture: a box outline from (5,5) to (34,14) with a `gap`-pixel break in its top.
    fn boxed(gap: usize) -> (Vec<bool>, usize, usize) {
        let (w, h) = (40, 20);
        let mut wall = vec![false; w * h];
        for x in 5..=34 {
            wall[5 * w + x] = !(18..18 + gap).contains(&x);
            wall[14 * w + x] = true;
        }
        for y in 5..=14 {
            wall[y * w + 5] = true;
            wall[y * w + 34] = true;
        }
        (wall, w, h)
    }

    fn fill(wall: &[bool], w: usize, h: usize, seed: (usize, usize), opts: GapOptions) -> Vec<u8> {
        let mut marks = vec![0u8; w * h];
        // A plain flood first, as `wand_region` does.
        let mut stack = vec![seed];
        while let Some((x, y)) = stack.pop() {
            let i = y * w + x;
            if marks[i] != 0 || wall[i] {
                continue;
            }
            marks[i] = 255;
            for j in neighbours(x, y, w, h).filter(|j| (j % w == x) || (j / w == y)) {
                stack.push((j % w, j / w));
            }
        }
        let like = |i: usize| !wall[i];
        apply(Marks { marks: &mut marks, w, h, bbox: (0, 0, w - 1, h - 1) }, seed.1 * w + seed.0, true, opts, &like);
        marks
    }

    #[test]
    fn a_small_gap_holds_the_fill_in() {
        let (wall, w, h) = boxed(3);
        let leaky = fill(&wall, w, h, (20, 10), GapOptions::default());
        assert_eq!(leaky[w + 1], 255, "without Close Gap the fill leaks out");
        let held = fill(&wall, w, h, (20, 10), GapOptions { close_gap: 4, expand: 0 });
        assert_eq!(held[w + 1], 0, "the outside stays empty");
        assert_eq!(held[10 * w + 6], 255, "the fill still reaches the left line");
        assert_eq!(held[13 * w + 33], 255, "and the far corner");
        assert!((0..w * h).all(|i| held[i] == 0 || !wall[i]), "lines are never filled");
    }

    #[test]
    fn a_gap_wider_than_the_setting_still_leaks() {
        let (wall, w, h) = boxed(8);
        let m = fill(&wall, w, h, (20, 10), GapOptions { close_gap: 4, expand: 0 });
        assert_eq!(m[w + 1], 255);
    }

    #[test]
    fn a_seed_in_a_narrow_spot_fills_as_usual() {
        let (wall, w, h) = boxed(3);
        // Right next to the left line: inside the thickened wall.
        let m = fill(&wall, w, h, (6, 10), GapOptions { close_gap: 6, expand: 0 });
        assert_eq!(m[10 * w + 20], 255, "fell back to the plain fill");
    }

    #[test]
    fn expand_reaches_under_the_lines() {
        let (wall, w, h) = boxed(0);
        let m = fill(&wall, w, h, (20, 10), GapOptions { close_gap: 0, expand: 1 });
        assert_eq!(m[10 * w + 5], 255, "the left line is covered");
        assert_eq!(m[10 * w + 4], 0, "but not beyond it");
    }
}
