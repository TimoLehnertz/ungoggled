use anyhow::{Context, Result, bail};
use image::{GenericImageView, ImageReader, Limits};
use serde_json::{Value, json};
use std::{
    fs,
    io::Cursor,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

pub const MAX_UPLOAD: usize = 12 * 1024 * 1024;
pub fn normalize(bytes: &[u8]) -> Result<Vec<u8>> {
    if bytes.len() > MAX_UPLOAD {
        bail!("Image must be smaller than 12 MiB");
    }
    let mut reader = ImageReader::new(Cursor::new(bytes)).with_guessed_format()?;
    if !matches!(
        reader.format(),
        Some(image::ImageFormat::Png | image::ImageFormat::Jpeg | image::ImageFormat::WebP)
    ) {
        bail!("Use PNG, JPEG or WebP");
    }
    let mut limits = Limits::default();
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    limits.max_alloc = Some(128 * 1024 * 1024);
    reader.limits(limits);
    let img = reader.decode().context("Cannot decode image")?;
    let img = img.resize(1920, 1080, image::imageops::FilterType::Triangle);
    let mut out = Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png)?;
    Ok(out.into_inner())
}
pub fn upload(dir: &Path, bytes: &[u8]) -> Result<Value> {
    if fs::read_dir(dir)?
        .filter_map(Result::ok)
        .filter(|f| f.path().extension().is_some_and(|e| e == "png"))
        .count()
        >= 16
    {
        bail!("Image library is full (16 images). Delete an unused image first.");
    }
    let png = normalize(bytes)?;
    let id = format!(
        "image-{}.png",
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
    );
    crate::settings::atomic_write(&dir.join(&id), &png)?;
    Ok(json!({"id":id,"url":format!("/api/images/{id}")}))
}
pub fn list(dir: &Path) -> Vec<Value> {
    let mut entries: Vec<_> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter_map(|e| {
            let id = e.file_name().to_string_lossy().into_owned();
            if !crate::settings::valid_id(&id) {
                return None;
            }
            Some(json!({"id":id,"url":format!("/api/images/{id}")}))
        })
        .collect();
    entries.sort_by_key(|v| v["id"].as_str().unwrap().to_owned());
    entries
}
pub fn pixels(path: Option<&Path>, width: u32, height: u32) -> Vec<u8> {
    let mut canvas = image::RgbaImage::from_pixel(width, height, image::Rgba([16, 18, 22, 255]));
    if let Some(img) = path.and_then(|p| image::open(p).ok()) {
        let scaled = img.resize(width, height, image::imageops::FilterType::Triangle);
        let (w, h) = scaled.dimensions();
        image::imageops::overlay(
            &mut canvas,
            &scaled,
            ((width - w) / 2).into(),
            ((height - h) / 2).into(),
        );
    }
    // DRM XRGB8888 is B,G,R,X in little endian memory.
    let mut data = canvas.into_raw();
    for p in data.as_chunks_mut::<4>().0 {
        p.swap(0, 2);
        p[3] = 255;
    }
    data
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_invalid_upload_and_letterboxes() {
        assert!(normalize(b"not an image").is_err());
        assert_eq!(pixels(None, 2, 1), [22, 18, 16, 255, 22, 18, 16, 255]);
        let source = image::DynamicImage::new_rgb8(3840, 2160);
        let mut b = Cursor::new(Vec::new());
        source.write_to(&mut b, image::ImageFormat::Png).unwrap();
        let small = image::load_from_memory(&normalize(b.get_ref()).unwrap()).unwrap();
        assert_eq!(small.dimensions(), (1920, 1080));
    }
}
