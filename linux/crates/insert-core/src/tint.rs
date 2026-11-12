use crate::model::Rgb;

pub const NEUTRAL_TINT: Rgb = Rgb {
    r: 0x6B,
    g: 0x6B,
    b: 0x6B,
};

#[derive(Clone, Copy, Default)]
struct Bucket {
    weight: f64,
    red: f64,
    green: f64,
    blue: f64,
}

/// `rgba` holds straight (not premultiplied)
/// RGBA pixels of the icon at about 24 × 24.
pub fn header_tint(rgba: &[u8]) -> Rgb {
    let mut buckets = [Bucket::default(); 12];
    let mut opaque = 0.0;
    let mut colorful = 0.0;
    let mut value_sum = 0.0;

    for pixel in rgba.chunks_exact(4) {
        if f64::from(pixel[3]) / 255.0 <= 0.5 {
            continue;
        }
        let [red, green, blue] = [pixel[0], pixel[1], pixel[2]].map(|c| f64::from(c) / 255.0);
        let value = red.max(green).max(blue);
        let chroma = value - red.min(green).min(blue);
        opaque += 1.0;
        value_sum += value;
        if value <= 0.2 || chroma / value <= 0.3 {
            continue;
        }
        colorful += 1.0;
        let index = ((hue(red, green, blue, value, chroma) * 12.0) as usize).min(11);
        let bucket = &mut buckets[index];
        bucket.weight += chroma;
        bucket.red += red * chroma;
        bucket.green += green * chroma;
        bucket.blue += blue * chroma;
    }

    if opaque == 0.0 {
        return NEUTRAL_TINT;
    }
    let top = buckets
        .iter()
        .copied()
        .fold(Bucket::default(), |best, b| if b.weight > best.weight { b } else { best });
    if colorful / opaque <= 0.08 || top.weight == 0.0 {
        let white = (value_sum / opaque * 0.55).clamp(0.26, 0.46);
        let level = to_u8(white);
        return Rgb {
            r: level,
            g: level,
            b: level,
        };
    }

    let (red, green, blue) = (
        top.red / top.weight,
        top.green / top.weight,
        top.blue / top.weight,
    );
    let value = red.max(green).max(blue);
    let chroma = value - red.min(green).min(blue);
    let h = if chroma == 0.0 {
        0.0
    } else {
        hue(red, green, blue, value, chroma)
    };
    let s = if value == 0.0 { 0.0 } else { chroma / value };
    let bright_hue = (0.11..=0.5).contains(&h);
    let saturation = s.clamp(0.5, 0.86);
    let brightness = value.clamp(0.48, if bright_hue { 0.66 } else { 0.8 });
    hsb_to_rgb(h, saturation, brightness)
}

fn hue(red: f64, green: f64, blue: f64, value: f64, chroma: f64) -> f64 {
    let h = if value == red {
        ((green - blue) / chroma) % 6.0
    } else if value == green {
        (blue - red) / chroma + 2.0
    } else {
        (red - green) / chroma + 4.0
    };
    (if h < 0.0 { h + 6.0 } else { h }) / 6.0
}

fn hsb_to_rgb(h: f64, s: f64, v: f64) -> Rgb {
    let sector = (h * 6.0).rem_euclid(6.0);
    let c = v * s;
    let x = c * (1.0 - ((sector % 2.0) - 1.0).abs());
    let (r, g, b) = match sector as u8 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = v - c;
    Rgb {
        r: to_u8(r + m),
        g: to_u8(g + m),
        b: to_u8(b + m),
    }
}

fn to_u8(component: f64) -> u8 {
    (component.clamp(0.0, 1.0) * 255.0).round() as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    fn icon(pixel: [u8; 4]) -> Vec<u8> {
        pixel.repeat(24 * 24)
    }

    #[test]
    fn transparent_icon_gives_the_neutral_tint() {
        assert_eq!(header_tint(&icon([255, 0, 0, 100])), NEUTRAL_TINT);
    }

    #[test]
    fn gray_icon_gives_a_clamped_gray() {
        assert_eq!(header_tint(&icon([255, 255, 255, 255])), Rgb { r: 117, g: 117, b: 117 });
        assert_eq!(header_tint(&icon([0, 0, 0, 255])), Rgb { r: 66, g: 66, b: 66 });
    }

    #[test]
    fn colorful_icon_keeps_its_hue_with_clamped_brightness() {
        assert_eq!(header_tint(&icon([255, 0, 0, 255])), Rgb { r: 204, g: 29, b: 29 });
        let yellow = header_tint(&icon([255, 230, 0, 255]));
        assert_eq!(yellow.r, 168, "yellow to cyan hues are capped at brightness 0.66");
        assert!(yellow.g > yellow.b);
    }

    #[test]
    fn dominant_hue_bucket_wins() {
        let mut pixels = icon([0, 0, 255, 255]);
        pixels[..4 * 100].copy_from_slice(&[0, 200, 0, 255].repeat(100));
        let tint = header_tint(&pixels);
        assert!(tint.b > tint.g && tint.b > tint.r);
    }
}
