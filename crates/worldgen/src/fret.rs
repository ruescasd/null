//! Fretwork: the light screens' pattern (the chasm's rooms, between their
//! pilasters; the void's grooves, between their piers).

use crate::noise::hash01;

/// A light screen's pattern in a `w` by `h` opening, from its lower
/// corner: its bars (x0, y0, x1, y1). A frame; inside it, on the left half
/// (mirrored onto the right), regions split by bars across or up, or framed
/// again within, small squares at the inner frames' corners, some left
/// open; a bar up the middle or not.
pub fn fret(w: f32, h: f32, seed: u32) -> Vec<[f32; 4]> {
    const BAR: f32 = 0.06;
    let mut out = vec![[0.0, 0.0, w, BAR], [0.0, h - BAR, w, h], [0.0, 0.0, BAR, h], [w - BAR, 0.0, w, h]];
    let mut half_bars = Vec::new();
    let mut k = 0u32;
    fn split(r: &mut dyn FnMut() -> f32, (x0, y0, x1, y1): (f32, f32, f32, f32), depth: u32, out: &mut Vec<[f32; 4]>) {
        let (w, h) = (x1 - x0, y1 - y0);
        if depth == 0 || w < 0.3 || h < 0.3 {
            return;
        }
        let f = [1.0 / 3.0, 0.5, 2.0 / 3.0, 0.382, 0.618][(r() * 5.0) as usize % 5];
        match r() {
            c if c < 0.32 && h > 0.6 => {
                let y = y0 + h * f;
                out.push([x0, y - BAR * 0.5, x1, y + BAR * 0.5]);
                split(r, (x0, y0, x1, y), depth - 1, out);
                split(r, (x0, y, x1, y1), depth - 1, out);
            }
            c if c < 0.55 && w > 0.6 => {
                let x = x0 + w * f;
                out.push([x - BAR * 0.5, y0, x + BAR * 0.5, y1]);
                split(r, (x0, y0, x, y1), depth - 1, out);
                split(r, (x, y0, x1, y1), depth - 1, out);
            }
            c if c < 0.85 => {
                let m = (w.min(h) * 0.18).max(0.1);
                let (a0, b0, a1, b1) = (x0 + m, y0 + m, x1 - m, y1 - m);
                out.extend([[a0, b0, a1, b0 + BAR], [a0, b1 - BAR, a1, b1], [a0, b0, a0 + BAR, b1], [a1 - BAR, b0, a1, b1]]);
                // (Small squares at its corners, Wright's.)
                let s = (m * 0.6).min(0.14);
                for (cx, cy) in [(a0, b0), (a1, b0), (a0, b1), (a1, b1)] {
                    out.push([cx - s * 0.5, cy - s * 0.5, cx + s * 0.5, cy + s * 0.5]);
                }
                split(r, (a0 + BAR, b0 + BAR, a1 - BAR, b1 - BAR), depth - 1, out);
            }
            _ => {}
        }
    }
    let mut r = || {
        k += 1;
        hash01(seed as i32, k as i32, 0, 0x5f1)
    };
    // (A tall screen: first a band across at its head.)
    let head = h * 0.78;
    half_bars.push([0.0, head - BAR * 0.5, w * 0.5, head + BAR * 0.5]);
    split(&mut r, (BAR, BAR, w * 0.5, head), 4, &mut half_bars);
    split(&mut r, (BAR, head, w * 0.5, h - BAR), 3, &mut half_bars);
    for b in half_bars {
        out.push(b);
        out.push([w - b[2], b[1], w - b[0], b[3]]);
    }
    if r() < 0.5 {
        out.push([w * 0.5 - BAR * 0.5, 0.0, w * 0.5 + BAR * 0.5, h]);
    }
    out
}
