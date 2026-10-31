//! Pre-rendered "glass" backgrounds.
//!
//! Old browsers (Chrome < 76) have no `backdrop-filter`, so the server does
//! the blur once: it serves a sharp wallpaper for the page and a small,
//! heavily blurred copy that tiles show as their own background, offset to
//! line up with the wallpaper behind them. The result looks like frosted
//! glass with zero GPU cost on the tablet.

use image::{DynamicImage, Rgb, RgbImage, imageops};
use std::io::Cursor;
use std::path::Path;
use tracing::{info, warn};

pub const WIDTH: u32 = 1280;
pub const HEIGHT: u32 = 800;

pub struct Backgrounds {
    pub sharp: Vec<u8>,
    pub blurred: Vec<u8>,
}

pub fn build(path: Option<&str>, sigma: f32) -> anyhow::Result<Backgrounds> {
    let base = match path.map(Path::new) {
        Some(p) if p.exists() => {
            info!("using background {}", p.display());
            image::open(p)?
        }
        Some(p) => {
            warn!(
                "background {} not found; using generated gradient",
                p.display()
            );
            gradient()
        }
        None => gradient(),
    };
    let sharp = base.resize_to_fill(WIDTH, HEIGHT, imageops::FilterType::Triangle);

    // Blur a quarter-size copy: 16x fewer pixels, and the browser's upscale
    // adds extra softness for free.
    let small = sharp.resize_exact(WIDTH / 4, HEIGHT / 4, imageops::FilterType::Triangle);
    let mut blurred = imageops::fast_blur(&small.to_rgb8(), (sigma / 4.0).max(1.0));
    lighten(&mut blurred, 0.18); // milky glass tint, keeps text readable

    Ok(Backgrounds {
        sharp: jpeg(&DynamicImage::ImageRgb8(sharp.to_rgb8()), 80)?,
        blurred: jpeg(&DynamicImage::ImageRgb8(blurred), 85)?,
    })
}

/// A dusk-blue diagonal gradient with two soft colour blobs.
fn gradient() -> DynamicImage {
    let mut img = RgbImage::new(WIDTH, HEIGHT);
    let blobs = [
        (0.25f32, 0.30f32, [255.0f32, 120.0, 90.0]),
        (0.80, 0.75, [90.0, 170.0, 255.0]),
    ];
    for (x, y, px) in img.enumerate_pixels_mut() {
        let fx = x as f32 / WIDTH as f32;
        let fy = y as f32 / HEIGHT as f32;
        let t = (fx + fy) / 2.0;
        let mut c = [18.0 + 30.0 * t, 22.0 + 20.0 * t, 48.0 + 40.0 * t];
        for (bx, by, col) in blobs {
            let d2 = (fx - bx).powi(2) * 1.6 + (fy - by).powi(2);
            let w = (-d2 * 9.0).exp() * 0.55;
            for i in 0..3 {
                c[i] = c[i] * (1.0 - w) + col[i] * w;
            }
        }
        *px = Rgb([c[0] as u8, c[1] as u8, c[2] as u8]);
    }
    DynamicImage::ImageRgb8(img)
}

fn lighten(img: &mut RgbImage, amount: f32) {
    for p in img.pixels_mut() {
        for c in p.0.iter_mut() {
            *c = (*c as f32 + (255.0 - *c as f32) * amount) as u8;
        }
    }
}

fn jpeg(img: &DynamicImage, quality: u8) -> anyhow::Result<Vec<u8>> {
    let mut out = Cursor::new(Vec::new());
    let enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, quality);
    img.write_with_encoder(enc)?;
    Ok(out.into_inner())
}

#[cfg(test)]
mod tests {
    #[test]
    fn builds_gradient_backgrounds() {
        let bg = super::build(None, 22.0).unwrap();
        assert!(bg.sharp.starts_with(&[0xFF, 0xD8])); // JPEG magic
        assert!(bg.blurred.len() < bg.sharp.len());
    }
}
