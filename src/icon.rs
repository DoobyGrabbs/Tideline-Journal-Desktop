//! Draws the application icon procedurally: the Tideline Journal mark (a ring, a wave across its lower
//! half and a small moon above the wave) in the accent teal on a dark green tile. At 16 pixels
//! only the wave is shown, dark on a teal tile.

/// The icon is designed on a 256x256 grid and scaled to whatever size is asked for.
const DESIGN: f32 = 256.0;

/// Signed distance to a rounded rectangle (negative inside).
fn rrect(px: f32, py: f32, cx: f32, cy: f32, hw: f32, hh: f32, r: f32) -> f32 {
    let qx = (px - cx).abs() - hw + r;
    let qy = (py - cy).abs() - hh + r;
    qx.max(0.0).hypot(qy.max(0.0)) + qx.max(qy).min(0.0) - r
}

fn coverage(d: f32) -> f32 {
    (0.5 - d).clamp(0.0, 1.0)
}

/// Composite `color` with alpha `a` over the accumulator.
fn over(acc: &mut [f32; 4], color: [f32; 3], a: f32) {
    let k = 1.0 - a;
    for i in 0..3 {
        acc[i] = color[i] * a + acc[i] * k;
    }
    acc[3] = a + acc[3] * k;
}

fn dist_segment(p: (f32, f32), a: (f32, f32), b: (f32, f32)) -> f32 {
    let (ab, ap) = ((b.0 - a.0, b.1 - a.1), (p.0 - a.0, p.1 - a.1));
    let len2 = ab.0 * ab.0 + ab.1 * ab.1;
    let t = if len2 == 0.0 { 0.0 } else { ((ap.0 * ab.0 + ap.1 * ab.1) / len2).clamp(0.0, 1.0) };
    (p.0 - (a.0 + ab.0 * t)).hypot(p.1 - (a.1 + ab.1 * t))
}

/// The wave as points on the mark's 24-unit grid.
fn wave() -> Vec<(f32, f32)> {
    (0..=48)
        .map(|i| {
            let x = 5.5 + 13.0 * i as f32 / 48.0;
            (x, 14.5 + 1.4 * ((x - 5.5) / 13.0 * std::f32::consts::TAU * 1.5).sin())
        })
        .collect()
}

/// Straight-alpha RGBA pixels for a square icon of `size` pixels.
pub fn rgba(size: usize) -> Vec<u8> {
    let s = size as f32 / DESIGN;
    let cov = |d: f32| coverage(d * s);
    let rgb = |r: f32, g: f32, b: f32| [r / 255.0, g / 255.0, b / 255.0];
    let (tile, teal, dark) = (rgb(0x15 as f32, 0x30 as f32, 0x2D as f32), rgb(0x4F as f32, 0xB3 as f32, 0xA5 as f32), rgb(0x0B as f32, 0x1F as f32, 0x1C as f32));
    let tiny = size <= 20;
    let wave = wave();

    let mut rgba = Vec::with_capacity(size * size * 4);
    for y in 0..size {
        for x in 0..size {
            let (px, py) = ((x as f32 + 0.5) / s, (y as f32 + 0.5) / s);
            let mut acc = [0.0f32; 4];
            let bg = cov(rrect(px, py, 128.0, 128.0, 126.0, 126.0, 56.0));
            over(&mut acc, if tiny { teal } else { tile }, bg);

            if tiny {
                // Only the wave, large, dark on the teal tile.
                let u = 112.0 / 13.0; // wave width 13 units -> 112 design px
                let to = |(wx, wy): (f32, f32)| (128.0 + (wx - 12.0) * u, 128.0 + (wy - 14.5) * u * 1.4);
                let d = wave.windows(2).map(|w| dist_segment((px, py), to(w[0]), to(w[1]))).fold(f32::MAX, f32::min) - 11.0;
                over(&mut acc, dark, cov(d) * bg);
            } else {
                // The mark fills most of the tile: 24 grid units -> 176 design px.
                let u = 176.0 / 24.0;
                let to = |(gx, gy): (f32, f32)| (40.0 + gx * u, 40.0 + gy * u);
                let stroke = 1.7 * u / 2.0;
                let (cx, cy) = to((12.0, 12.0));
                let ring = ((px - cx).hypot(py - cy) - 9.0 * u).abs() - stroke;
                let wave_d = wave.windows(2).map(|w| dist_segment((px, py), to(w[0]), to(w[1]))).fold(f32::MAX, f32::min) - stroke;
                let (mx, my) = to((12.0, 8.2));
                let moon = (px - mx).hypot(py - my) - 1.6 * u;
                over(&mut acc, teal, cov(ring.min(wave_d).min(moon)) * bg);
            }

            // Back to straight alpha for the icon data.
            let a = acc[3];
            for c in &acc[..3] {
                let v = if a > 0.0 { c / a } else { 0.0 };
                rgba.push((v.clamp(0.0, 1.0) * 255.0).round() as u8);
            }
            rgba.push((a.clamp(0.0, 1.0) * 255.0).round() as u8);
        }
    }
    rgba
}

#[cfg(test)]
mod tests {
    use super::*;

    fn px(rgba: &[u8], size: usize, x: usize, y: usize) -> [u8; 4] {
        let i = (y * size + x) * 4;
        [rgba[i], rgba[i + 1], rgba[i + 2], rgba[i + 3]]
    }

    #[test]
    fn the_icon_is_a_rounded_tile_with_the_mark_on_it() {
        for size in [16, 32, 64, 256] {
            let img = rgba(size);
            assert_eq!(img.len(), size * size * 4);
            assert_eq!(px(&img, size, 0, 0)[3], 0, "{size}px: corners are transparent");
            assert_eq!(px(&img, size, size / 2, size / 20 + 1)[3], 255, "{size}px: the tile is opaque");
        }
        // Big sizes: dark green tile, with teal where the ring is.
        let big = rgba(256);
        assert_eq!(&px(&big, 256, 20, 128)[..3], &[0x15, 0x30, 0x2D]);
        // The ring's left edge (grid x = 3, y = 12) is teal.
        let ring = px(&big, 256, 40 + (3.0 * 176.0 / 24.0) as usize, 128);
        assert!(ring[1] > 150 && ring[0] < 120, "{ring:?}");
        // 16 px: a teal tile with the dark wave.
        let tiny = rgba(16);
        assert_eq!(&px(&tiny, 16, 1, 8)[..3], &[0x4F, 0xB3, 0xA5]);
    }

    /// Set ICON_PNG=path to write the 256 px icon out and look at it.
    #[test]
    fn writes_a_preview_when_asked() {
        if let Ok(path) = std::env::var("ICON_PNG") {
            for size in [256usize, 64, 16] {
                let img = image::RgbaImage::from_raw(size as u32, size as u32, rgba(size)).unwrap();
                img.save(path.replace(".png", &format!("-{size}.png"))).unwrap();
            }
        }
    }
}
