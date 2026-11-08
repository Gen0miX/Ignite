//! Capture thumbnails (480 px max, JPEG), made once into `artwork/thumbs/`
//! from the original, which is only read. Steam already ships its own.

use std::path::{Path, PathBuf};

use image::codecs::jpeg::JpegEncoder;

use super::achievement_icons::file_stem;
use super::artwork::cache_root;
use crate::error::{AppError, AppResult};

pub const MAX_SIZE: u32 = 480;

pub fn dir(data_dir: &Path) -> PathBuf {
    cache_root(data_dir).join("thumbs")
}

/// Thumbnail file of a capture: changes when the file changes.
pub fn target(dir: &Path, source: &Path) -> PathBuf {
    let stamp = std::fs::metadata(source)
        .map(|m| {
            let mtime = m
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map_or(0, |d| d.as_secs());
            format!("{mtime}-{}", m.len())
        })
        .unwrap_or_default();
    dir.join(format!(
        "{}.jpg",
        file_stem(&format!("{}|{stamp}", source.display()))
    ))
}

/// Width and height from the file header (no full decode).
pub fn dimensions(path: &Path) -> Option<(u32, u32)> {
    image::image_dimensions(path).ok()
}

/// Writes a thumbnail of `source` at `dest`.
pub fn make(source: &Path, dest: &Path) -> AppResult<()> {
    let img = image::open(source)
        .map_err(|e| AppError::Invalid(format!("cannot read image {}: {e}", source.display())))?;
    let small = img.thumbnail(MAX_SIZE, MAX_SIZE).to_rgb8();
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = dest.with_extension("part");
    {
        let file = std::fs::File::create(&tmp)?;
        let mut out = std::io::BufWriter::new(file);
        small
            .write_with_encoder(JpegEncoder::new_with_quality(&mut out, 80))
            .map_err(|e| AppError::Invalid(format!("cannot write thumbnail: {e}")))?;
    }
    std::fs::rename(&tmp, dest)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn makes_a_small_jpeg_and_leaves_the_original_alone() {
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("shot.png");
        image::RgbImage::from_pixel(1920, 1080, image::Rgb([200, 90, 40]))
            .save(&src)
            .unwrap();
        let before = std::fs::read(&src).unwrap();
        let dest = target(&dir(tmp.path()), &src);
        make(&src, &dest).unwrap();
        assert_eq!(dimensions(&dest), Some((480, 270)));
        assert_eq!(dimensions(&src), Some((1920, 1080)));
        assert_eq!(std::fs::read(&src).unwrap(), before);
        assert_eq!(target(&dir(tmp.path()), &src), dest, "stable name");
    }
}
