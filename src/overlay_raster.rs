//! Software rendering of the panel source marker (a coloured ring) as a premultiplied
//! BGRA bitmap, suitable for `UpdateLayeredWindow` with per-pixel alpha.

/// Outer size of the overlay window in pixels (circle plus margin).
pub const SIZE: usize = 40;
/// Offset from the window's top-left corner to the circle centre.
pub const CENTER_OFFSET: i32 = (SIZE / 2) as i32;

const RING_IN: f32 = 9.0;
const RING_OUT: f32 = 15.0;

fn band(r: f32, r_in: f32, r_out: f32) -> f32 {
    ((r - r_in + 0.5).clamp(0.0, 1.0)) * ((r_out - r + 0.5).clamp(0.0, 1.0))
}

/// One pixel as premultiplied `0xAARRGGBB` (the in-memory BGRA layout on little endian).
pub fn pixel(x: usize, y: usize, rgb: u32, editable: bool) -> u32 {
    let c = SIZE as f32 / 2.0;
    let dx = x as f32 + 0.5 - c;
    let dy = y as f32 + 0.5 - c;
    let r = (dx * dx + dy * dy).sqrt();

    let outline = band(r, RING_IN - 1.0, RING_OUT + 1.0);
    let ring = band(r, RING_IN, RING_OUT);
    let alpha_f = ring + outline * (1.0 - ring);

    // Inside the ring the window is almost transparent but still hit-testable when editable.
    let alpha = if alpha_f <= 0.0 && editable && r < RING_IN - 1.0 {
        1.0 / 255.0
    } else {
        alpha_f
    };

    let a = (alpha * 255.0).round() as u32;
    let rr = ((rgb >> 16) & 0xFF) as f32;
    let gg = ((rgb >> 8) & 0xFF) as f32;
    let bb = (rgb & 0xFF) as f32;
    let pm = |v: f32| (v * ring).round() as u32; // premultiplied by the ring coverage only
    (a << 24) | (pm(rr) << 16) | (pm(gg) << 8) | pm(bb)
}

pub fn render(rgb: u32, editable: bool) -> Vec<u32> {
    let mut v = Vec::with_capacity(SIZE * SIZE);
    for y in 0..SIZE {
        for x in 0..SIZE {
            v.push(pixel(x, y, rgb, editable));
        }
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    fn alpha(p: u32) -> u32 {
        p >> 24
    }

    #[test]
    fn ring_is_opaque_and_coloured() {
        // a pixel on the ring: 12px right of centre
        let p = pixel(SIZE / 2 + 12, SIZE / 2, 0xFF0000, true);
        assert_eq!(alpha(p), 255);
        assert_eq!((p >> 16) & 0xFF, 0xFF);
        assert_eq!(p & 0xFF, 0);
    }

    #[test]
    fn outside_is_fully_transparent() {
        assert_eq!(pixel(0, 0, 0xFF0000, true), 0);
        assert_eq!(pixel(SIZE - 1, SIZE - 1, 0xFF0000, false), 0);
    }

    #[test]
    fn interior_is_hit_testable_only_when_editable() {
        let c = SIZE / 2;
        assert_eq!(alpha(pixel(c, c, 0x00FF00, true)), 1);
        assert_eq!(alpha(pixel(c, c, 0x00FF00, false)), 0);
    }

    #[test]
    fn premultiplied_channels_never_exceed_alpha() {
        for p in render(0xFFFFFF, true) {
            let a = p >> 24;
            assert!(((p >> 16) & 0xFF) <= a.max(1));
            assert!(((p >> 8) & 0xFF) <= a.max(1));
            assert!((p & 0xFF) <= a.max(1));
        }
    }
}
