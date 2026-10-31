//! Screensaver photos.
//!
//! Camera photos are large (12+ megapixels, several MB). Sending them as-is
//! would choke an old tablet, and letting the browser scale them down gives
//! soft, shimmering results. So the server prepares each photo for the exact
//! screen that asks for it:
//!
//! 1. **Upright:** apply the EXIF orientation (phones often store photos
//!    sideways).
//! 2. **Exact size:** scale to the requesting screen's pixel size —
//!    "cover" (fills the screen; the browser crops the edges) or "contain"
//!    (whole photo visible). Photos are never upscaled.
//! 3. **High quality:** Lanczos3 resampling, a light unsharp mask to restore
//!    crispness lost in downscaling, and JPEG quality 90.
//!
//! Results are cached on disk per (photo, size, fit). Screen sizes that have
//! asked for photos are remembered so the cache can be filled ahead of time.

use image::{DynamicImage, ImageDecoder, ImageReader, imageops, imageops::FilterType};
use std::collections::BTreeSet;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tokio::sync::Notify;

/// JPEG quality for screensaver photos (90 is visually lossless on a tablet).
const QUALITY: u8 = 90;

/// How a photo is fitted to the screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Fit {
    /// Fill the whole screen; edges are cropped by the browser.
    Cover,
    /// Show the whole photo; letterboxed.
    Contain,
}

impl Fit {
    pub fn parse(s: &str) -> Option<Fit> {
        match s {
            "cover" => Some(Fit::Cover),
            "contain" => Some(Fit::Contain),
            _ => None,
        }
    }
}

/// A requested output size. Width/height are rounded up to multiples of 8 and
/// clamped, so odd window sizes don't each create their own cache entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Target {
    pub w: u32,
    pub h: u32,
    pub fit: Fit,
}

impl Target {
    pub fn new(w: u32, h: u32, fit: Fit) -> Target {
        let snap = |v: u32| v.clamp(320, 2560).div_ceil(8) * 8;
        Target {
            w: snap(w),
            h: snap(h),
            fit,
        }
    }

    /// Output dimensions for a source image of `sw`×`sh`.
    ///
    /// A photo whose orientation doesn't match the screen (a portrait photo
    /// on a landscape tablet) is always sized to fit whole — cropping it to
    /// fill would cut away most of it, and the client shows it whole anyway.
    pub fn output_size(&self, sw: u32, sh: u32) -> (u32, u32) {
        let (sw_f, sh_f) = (sw as f64, sh as f64);
        let (sx, sy) = (self.w as f64 / sw_f, self.h as f64 / sh_f);
        let same_orientation = (sw >= sh) == (self.w >= self.h);
        let scale = if self.fit == Fit::Cover && same_orientation {
            sx.max(sy)
        } else {
            sx.min(sy)
        };
        let scale = scale.min(1.0); // never upscale
        (
            ((sw_f * scale).round() as u32).max(1),
            ((sh_f * scale).round() as u32).max(1),
        )
    }
}

pub struct Photos {
    dir: PathBuf,
    cache: PathBuf,
    /// Screen sizes seen recently, used to pre-render the cache.
    targets: Mutex<BTreeSet<Target>>,
    /// Wakes the warmer when a new screen size shows up.
    new_target: Notify,
}

impl Photos {
    pub fn new(dir: &str, data_dir: &Path) -> Self {
        let cache = data_dir.join(".photo-cache");
        let targets = load_targets(&cache.join("targets.txt"));
        Photos {
            dir: PathBuf::from(dir),
            cache,
            targets: Mutex::new(targets),
            new_target: Notify::new(),
        }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Image files in the folder, sorted by name. Clients only ever address
    /// photos by index into this list, so no file path comes from the network.
    pub fn list(&self) -> Vec<PathBuf> {
        let Ok(rd) = std::fs::read_dir(&self.dir) else {
            return Vec::new();
        };
        let mut files: Vec<PathBuf> = rd
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.is_file())
            .filter(|p| {
                let ext = p
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_ascii_lowercase();
                matches!(ext.as_str(), "jpg" | "jpeg" | "png")
            })
            .collect();
        files.sort();
        files
    }

    fn path(&self, idx: usize) -> anyhow::Result<PathBuf> {
        self.list()
            .into_iter()
            .nth(idx)
            .ok_or_else(|| anyhow::anyhow!("no photo {idx}"))
    }

    /// Photo `idx` prepared for `target`, from the cache when possible.
    pub fn get(&self, idx: usize, target: Target) -> anyhow::Result<Vec<u8>> {
        self.remember(target);
        let path = self.path(idx)?;
        let fit = if target.fit == Fit::Cover { "c" } else { "f" };
        let name = format!(
            "{:016x}-{}x{}{}.jpg",
            cache_key(&path),
            target.w,
            target.h,
            fit
        );
        self.cached(&name, || {
            let img = open_upright(&path)?;
            let (w, h) = target.output_size(img.width(), img.height());
            let img = if (w, h) == (img.width(), img.height()) {
                img
            } else {
                // Lanczos3 keeps fine detail; the unsharp mask restores the
                // crispness that any downscale softens.
                let resized = img.resize_exact(w, h, FilterType::Lanczos3);
                DynamicImage::ImageRgb8(imageops::unsharpen(&resized.to_rgb8(), 0.6, 2))
            };
            encode_jpeg(&img, QUALITY)
        })
    }

    /// A tiny, heavily blurred copy shown behind photos that don't fill the
    /// screen (e.g. portrait photos). The browser scales it up, which adds
    /// more blur for free — cheaper than a CSS blur filter on an old GPU.
    pub fn get_blurred(&self, idx: usize) -> anyhow::Result<Vec<u8>> {
        let path = self.path(idx)?;
        let name = format!("{:016x}-blur.jpg", cache_key(&path));
        self.cached(&name, || {
            let small = open_upright(&path)?.resize(160, 160, FilterType::Triangle);
            let blurred = imageops::fast_blur(&small.to_rgb8(), 6.0);
            encode_jpeg(&DynamicImage::ImageRgb8(blurred), 70)
        })
    }

    fn cached(
        &self,
        name: &str,
        make: impl FnOnce() -> anyhow::Result<Vec<u8>>,
    ) -> anyhow::Result<Vec<u8>> {
        let file = self.cache.join(name);
        if let Ok(bytes) = std::fs::read(&file) {
            return Ok(bytes);
        }
        let bytes = make()?;
        let _ = std::fs::create_dir_all(&self.cache);
        let _ = std::fs::write(&file, &bytes);
        Ok(bytes)
    }

    /// Record a screen size; new ones wake the warmer and are persisted so
    /// the cache can be filled right after a restart.
    fn remember(&self, target: Target) {
        let mut set = self.targets.lock().unwrap();
        if set.insert(target) {
            // Keep only the most recent few sizes (a tablet, maybe a PC).
            while set.len() > 4 {
                let first = *set.iter().next().unwrap();
                set.remove(&first);
            }
            save_targets(&self.cache.join("targets.txt"), &set);
            self.new_target.notify_one();
        }
    }

    fn targets(&self) -> Vec<Target> {
        self.targets.lock().unwrap().iter().copied().collect()
    }
}

/// Background task: prepare every photo for every known screen size, so the
/// screensaver never waits on a multi-megabyte decode. Runs at start-up, when
/// a new screen size appears, and every 30 minutes (to pick up new photos).
pub async fn warm(photos: Arc<Photos>) {
    loop {
        let p = photos.clone();
        let _ = tokio::task::spawn_blocking(move || {
            let targets = p.targets();
            for i in 0..p.list().len() {
                let result = p
                    .get_blurred(i)
                    .and_then(|_| targets.iter().try_for_each(|t| p.get(i, *t).map(|_| ())));
                if let Err(e) = result {
                    tracing::warn!("photo {i}: {e:#}");
                }
            }
        })
        .await;
        tokio::select! {
            _ = photos.new_target.notified() => {}
            _ = tokio::time::sleep(std::time::Duration::from_secs(30 * 60)) => {}
        }
    }
}

/// Changes whenever the file is renamed, replaced or edited, so stale cache
/// entries are simply never looked up again.
fn cache_key(path: &Path) -> u64 {
    let mut h = DefaultHasher::new();
    path.hash(&mut h);
    if let Ok(m) = std::fs::metadata(path) {
        m.len().hash(&mut h);
        if let Ok(t) = m.modified() {
            t.hash(&mut h);
        }
    }
    h.finish()
}

/// Decode an image and rotate it according to its EXIF orientation tag.
fn open_upright(path: &Path) -> anyhow::Result<DynamicImage> {
    let mut decoder = ImageReader::open(path)?
        .with_guessed_format()?
        .into_decoder()?;
    let orientation = decoder.orientation()?;
    let mut img = DynamicImage::from_decoder(decoder)?;
    img.apply_orientation(orientation);
    Ok(img)
}

fn encode_jpeg(img: &DynamicImage, quality: u8) -> anyhow::Result<Vec<u8>> {
    let mut out = Cursor::new(Vec::new());
    let enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, quality);
    DynamicImage::ImageRgb8(img.to_rgb8()).write_with_encoder(enc)?;
    Ok(out.into_inner())
}

/// targets.txt holds one "WIDTHxHEIGHT fit" line per remembered screen.
fn load_targets(file: &Path) -> BTreeSet<Target> {
    let text = std::fs::read_to_string(file).unwrap_or_default();
    text.lines()
        .filter_map(|l| {
            let (size, fit) = l.split_once(' ')?;
            let (w, h) = size.split_once('x')?;
            Some(Target::new(
                w.parse().ok()?,
                h.parse().ok()?,
                Fit::parse(fit)?,
            ))
        })
        .collect()
}

fn save_targets(file: &Path, set: &BTreeSet<Target>) {
    let text: String = set
        .iter()
        .map(|t| {
            format!(
                "{}x{} {}\n",
                t.w,
                t.h,
                if t.fit == Fit::Cover {
                    "cover"
                } else {
                    "contain"
                }
            )
        })
        .collect();
    let _ = std::fs::create_dir_all(file.parent().unwrap_or(Path::new(".")));
    let _ = std::fs::write(file, text);
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgb, RgbImage};

    #[test]
    fn sizes_for_cover_contain_and_orientation() {
        let tablet = Target::new(1024, 600, Fit::Cover);
        // Landscape 3:2 photo covering a 1024x600 screen: width-bound.
        assert_eq!(tablet.output_size(3000, 2000), (1024, 683));
        // Same photo shown whole: height-bound.
        assert_eq!(
            Target::new(1024, 600, Fit::Contain).output_size(3000, 2000),
            (900, 600)
        );
        // Portrait photo on a landscape screen is always fitted whole.
        assert_eq!(tablet.output_size(2000, 3000), (400, 600));
        // Never upscale small images.
        assert_eq!(tablet.output_size(800, 400), (800, 400));
        // Odd sizes round up to multiples of 8; common screen sizes stay exact.
        assert_eq!(
            Target::new(1023, 601, Fit::Cover),
            Target {
                w: 1024,
                h: 608,
                fit: Fit::Cover
            }
        );
    }

    #[test]
    fn lists_images_and_caches_per_size() {
        let root = std::env::temp_dir().join(format!("gp-photos-{}", std::process::id()));
        let dir = root.join("pics");
        std::fs::create_dir_all(&dir).unwrap();
        RgbImage::from_pixel(3000, 2000, Rgb([200, 100, 50]))
            .save(dir.join("b.jpg"))
            .unwrap();
        RgbImage::from_pixel(10, 10, Rgb([0, 0, 0]))
            .save(dir.join("a.PNG"))
            .unwrap();
        std::fs::write(dir.join("notes.txt"), "x").unwrap();

        let p = Photos::new(dir.to_str().unwrap(), &root);
        let names: Vec<_> = p
            .list()
            .iter()
            .map(|f| f.file_name().unwrap().to_owned())
            .collect();
        assert_eq!(names, ["a.PNG", "b.jpg"]);

        let t = Target::new(1024, 600, Fit::Cover);
        let img = image::load_from_memory(&p.get(1, t).unwrap()).unwrap();
        assert_eq!((img.width(), img.height()), (1024, 683));
        assert!(p.get_blurred(1).is_ok());
        assert!(p.get(5, t).is_err());

        // The screen size is remembered across restarts.
        let reopened = Photos::new(dir.to_str().unwrap(), &root);
        assert_eq!(reopened.targets(), vec![t]);
        let _ = std::fs::remove_dir_all(&root);
    }
}
