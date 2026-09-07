use bytes::Bytes;
use image::codecs::jpeg::JpegEncoder;
use image::{imageops, DynamicImage, RgbaImage};

pub fn composite_hybrid_tile(sat_bytes: &[u8], overlay_bytes: &[u8]) -> Result<Bytes, String> {
    let sat_img = image::load_from_memory(sat_bytes)
        .map_err(|e| format!("Failed to load satellite image: {e}"))?;
    let mut sat_rgba = sat_img.to_rgba8();

    let overlay_img = image::load_from_memory(overlay_bytes)
        .map_err(|e| format!("Failed to load overlay image: {e}"))?;

    let (sat_w, sat_h) = (sat_rgba.width(), sat_rgba.height());
    let (ov_w, ov_h) = (overlay_img.width(), overlay_img.height());

    let overlay_rgba: RgbaImage = if sat_w != ov_w || sat_h != ov_h {
        let resized = overlay_img.resize_exact(sat_w, sat_h, imageops::FilterType::Triangle);
        resized.to_rgba8()
    } else {
        overlay_img.to_rgba8()
    };

    imageops::overlay(&mut sat_rgba, &overlay_rgba, 0, 0);

    let rgb_img = DynamicImage::ImageRgba8(sat_rgba).to_rgb8();

    let mut buf = Vec::new();
    let mut encoder = JpegEncoder::new_with_quality(&mut buf, 85);
    encoder
        .encode_image(&rgb_img)
        .map_err(|e| format!("Failed to encode JPEG: {e}"))?;

    Ok(Bytes::from(buf))
}
