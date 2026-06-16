//! Rasterised-image export (PNG / JPEG) using resvg.
//!
//! SVG → resvg/usvg/tiny-skia pipeline → in-memory pixmap → PNG or JPEG bytes.
//! The scale factor controls the output resolution relative to the SVG's natural
//! viewBox size (1.0 = 1 device pixel per SVG unit, 2.0 = Retina).

use std::io::Cursor;

use crate::octo_render;

// Re-exported through resvg at 0.47
use resvg::tiny_skia;
use resvg::usvg;

/// Export a DSL diagram to a PNG `Vec<u8>` at the given scale.
///
/// `scale` controls the output density (1.0 = 72-ish dpi, 2.0 = Retina).
/// The viewport is automatically derived from the SVG's viewBox.
pub fn render_dsl_to_png_bytes(dsl: &str, scale: f32) -> Result<Vec<u8>, String> {
    let svg = octo_render(dsl, None)?;
    render_svg_to_png_bytes(&svg, scale)
}

/// Export a DSL diagram to a JPEG `Vec<u8>` at the given scale and quality.
///
/// `quality` is 1–100 (higher = larger file, better quality).
pub fn render_dsl_to_jpeg_bytes(dsl: &str, scale: f32, quality: u8) -> Result<Vec<u8>, String> {
    let svg = octo_render(dsl, None)?;
    render_svg_to_jpeg_bytes(&svg, scale, quality)
}

/// Export a DSL diagram to a PNG file on disk.
pub fn render_dsl_to_png_file(dsl: &str, path: &str, scale: f32) -> Result<(), String> {
    let bytes = render_dsl_to_png_bytes(dsl, scale)?;
    std::fs::write(path, &bytes).map_err(|e| format!("cannot write PNG: {e}"))
}

/// Export a DSL diagram to a JPEG file on disk.
pub fn render_dsl_to_jpeg_file(dsl: &str, path: &str, scale: f32, quality: u8) -> Result<(), String> {
    let bytes = render_dsl_to_jpeg_bytes(dsl, scale, quality)?;
    std::fs::write(path, &bytes).map_err(|e| format!("cannot write JPEG: {e}"))
}

/// Render an SVG string (as produced by `octo_render`) into PNG bytes.
pub fn render_svg_to_png_bytes(svg: &str, scale: f32) -> Result<Vec<u8>, String> {
    let pixmap = render_svg_to_pixmap(svg, scale)?;
    pixmap.encode_png().map_err(|e| format!("PNG encoding failed: {e}"))
}

/// Render an SVG string (as produced by `octo_render`) into JPEG bytes.
///
/// `quality`: 1–100 (higher = larger, better).
pub fn render_svg_to_jpeg_bytes(svg: &str, scale: f32, quality: u8) -> Result<Vec<u8>, String> {
    let pixmap = render_svg_to_pixmap(svg, scale)?;
    let rgba = pixmap.data();

    // tiny-skia stores premultiplied RGBA.  The `image` crate's JPEG encoder
    // wants plain RGB (no alpha), and it must handle the conversion itself.
    // We build an `image::RgbaImage` (not flattening premultiplied — resvg
    // already output non-premultiplied sRGB content) and convert to RGB.
    let w = pixmap.width();
    let h = pixmap.height();

    let img = image::RgbaImage::from_raw(w, h, rgba.to_vec())
        .ok_or_else(|| "failed to create RgbaImage from pixmap data".to_string())?;

    let rgb = image::DynamicImage::ImageRgba8(img).into_rgb8();
    let mut buf = Cursor::new(Vec::new());
    let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, quality);
    encoder
        .encode(
            rgb.as_raw(),
            rgb.width(),
            rgb.height(),
            image::ExtendedColorType::Rgb8,
        )
    .map_err(|e| format!("JPEG encoding failed: {e}"))?;

    Ok(buf.into_inner())
}

// ── internal helpers ──────────────────────────────────────────────────────

/// Parse SVG, render to a tiny-skia Pixmap at the given scale factor.
fn render_svg_to_pixmap(svg: &str, scale: f32) -> Result<tiny_skia::Pixmap, String> {
    let scale = scale.max(0.1).min(10.0); // 0.1x – 10x

    let opt = usvg::Options::default();
    let tree = usvg::Tree::from_str(svg, &opt)
        .map_err(|e| format!("SVG parse failed: {e}"))?;

    let svg_size = tree.size();
    let w = (svg_size.width().ceil() as u32 * scale as u32).max(1);
    let h = (svg_size.height().ceil() as u32 * scale as u32).max(1);

    let mut pixmap = tiny_skia::Pixmap::new(w, h)
        .ok_or_else(|| "failed to allocate pixmap".to_string())?;

    let transform = tiny_skia::Transform::from_scale(scale, scale);
    resvg::render(&tree, transform, &mut pixmap.as_mut());
    Ok(pixmap)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal DSL that produces a valid 2-node diagram
    fn sample_dsl() -> &'static str {
        "Idle -> Active : check\nActive -> Done : finish"
    }

    #[test]
    fn test_png_bytes_nonempty() {
        let bytes = render_dsl_to_png_bytes(sample_dsl(), 1.0).unwrap();
        assert!(!bytes.is_empty(), "PNG bytes should not be empty");
        // PNG header magic
        assert_eq!(&bytes[..8], &[137, 80, 78, 71, 13, 10, 26, 10]);
    }

    #[test]
    fn test_png_bytes_retina() {
        let normal = render_dsl_to_png_bytes(sample_dsl(), 1.0).unwrap();
        let retina = render_dsl_to_png_bytes(sample_dsl(), 2.0).unwrap();
        // Retina should produce more bytes (larger image)
        assert!(retina.len() > normal.len(), "2x scale should yield more bytes");
    }

    #[test]
    fn test_jpeg_bytes_nonempty() {
        let bytes = render_dsl_to_jpeg_bytes(sample_dsl(), 1.0, 85).unwrap();
        assert!(!bytes.is_empty(), "JPEG bytes should not be empty");
        // JPEG SOI marker
        assert_eq!(bytes[0], 0xFF);
        assert_eq!(bytes[1], 0xD8);
    }

    #[test]
    fn test_jpeg_varying_quality() {
        let high = render_dsl_to_jpeg_bytes(sample_dsl(), 1.0, 95).unwrap();
        let low = render_dsl_to_jpeg_bytes(sample_dsl(), 1.0, 10).unwrap();
        // Higher quality should produce more bytes (less compression)
        assert!(high.len() > low.len(), "higher quality JPEG should be larger");
    }

    #[test]
    fn test_png_file_roundtrip() {
        let path = std::env::temp_dir().join("octovia_test.png");
        let p = path.to_string_lossy().to_string();
        render_dsl_to_png_file(sample_dsl(), &p, 1.0).unwrap();
        assert!(path.exists(), "PNG file should exist");
        let meta = std::fs::metadata(&path).unwrap();
        assert!(meta.len() > 0, "PNG file should be non-empty");
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_jpeg_file_roundtrip() {
        let path = std::env::temp_dir().join("octovia_test.jpg");
        let p = path.to_string_lossy().to_string();
        render_dsl_to_jpeg_file(sample_dsl(), &p, 1.0, 80).unwrap();
        assert!(path.exists(), "JPEG file should exist");
        let meta = std::fs::metadata(&path).unwrap();
        assert!(meta.len() > 0, "JPEG file should be non-empty");
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_empty_dsl_png() {
        // Empty DSL still produces a valid SVG, so PNG should work
        let bytes = render_dsl_to_png_bytes("", 1.0).unwrap();
        assert!(!bytes.is_empty());
        assert_eq!(&bytes[..8], &[137, 80, 78, 71, 13, 10, 26, 10]);
    }

    #[test]
    fn test_scales_are_clamped() {
        // Very large scale should be clamped to 10
        let bytes_big = render_dsl_to_png_bytes(sample_dsl(), 100.0).unwrap();
        assert!(!bytes_big.is_empty());

        // Tiny scale should be clamped to 0.1
        let bytes_tiny = render_dsl_to_png_bytes(sample_dsl(), 0.01).unwrap();
        assert!(!bytes_tiny.is_empty());
    }

    #[test]
    fn test_invalid_dsl_fails_gracefully() {
        let result = render_dsl_to_png_bytes("garbage input /// not valid", 1.0);
        assert!(result.is_err(), "invalid DSL should produce an error");
    }

    #[test]
    fn test_render_svg_to_png_explicit() {
        let svg = octo_render(sample_dsl(), None).unwrap();
        let bytes = render_svg_to_png_bytes(&svg, 1.0).unwrap();
        assert_eq!(&bytes[..8], &[137, 80, 78, 71, 13, 10, 26, 10]);
    }

    #[test]
    fn test_render_svg_to_jpeg_explicit() {
        let svg = octo_render(sample_dsl(), None).unwrap();
        let bytes = render_svg_to_jpeg_bytes(&svg, 1.0, 75).unwrap();
        assert_eq!(bytes[0], 0xFF);
        assert_eq!(bytes[1], 0xD8);
    }
}
