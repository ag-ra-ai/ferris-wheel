//! Images that can be drawn as sprites.

use std::fmt;
use std::io::Cursor;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::{Error, Vec2};

/// An RGBA image that can be drawn with [`crate::Frame::draw_sprite`].
///
/// Create textures anywhere, including before [`crate::run`] starts. The
/// pixels are uploaded to the GPU the first time the texture is drawn.
///
/// Cloning is cheap: clones share the same pixels and GPU texture. The GPU
/// copy is freed once every clone has been dropped.
#[derive(Clone)]
pub struct Texture(Arc<TextureData>);

pub(crate) struct TextureData {
    pub(crate) id: u64,
    pub(crate) width: u32,
    pub(crate) height: u32,
    /// Straight (non-premultiplied) RGBA8 in sRGB, row by row from the top.
    pub(crate) pixels: Vec<u8>,
}

impl Texture {
    /// A texture from raw RGBA8 pixels (sRGB, straight alpha), row by row
    /// from the top-left. `pixels` must hold `width * height * 4` bytes.
    pub fn from_rgba8(width: u32, height: u32, pixels: Vec<u8>) -> Result<Self, Error> {
        if width == 0 || height == 0 {
            return Err(Error::Texture(format!(
                "texture size must not be zero (got {width}x{height})"
            )));
        }
        let expected = width as usize * height as usize * 4;
        if pixels.len() != expected {
            return Err(Error::Texture(format!(
                "{width}x{height} RGBA texture needs {expected} bytes, got {}",
                pixels.len()
            )));
        }
        static NEXT_ID: AtomicU64 = AtomicU64::new(0);
        Ok(Self(Arc::new(TextureData {
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            width,
            height,
            pixels,
        })))
    }

    /// A texture from the bytes of a PNG file, e.g. from `include_bytes!`.
    pub fn from_png_bytes(bytes: &[u8]) -> Result<Self, Error> {
        let (width, height, pixels) = decode_png(bytes).map_err(Error::Texture)?;
        Self::from_rgba8(width, height, pixels)
    }

    /// A texture from a PNG file on disk.
    ///
    /// Browsers have no file system, so this always fails there; embed images
    /// with `include_bytes!` and use [`Texture::from_png_bytes`] instead.
    pub fn from_png_file(path: impl AsRef<Path>) -> Result<Self, Error> {
        let path = path.as_ref();
        let bytes = std::fs::read(path)
            .map_err(|e| Error::Texture(format!("could not read {}: {e}", path.display())))?;
        Self::from_png_bytes(&bytes).map_err(|e| Error::Texture(format!("{}: {e}", path.display())))
    }

    /// Width in pixels.
    pub fn width(&self) -> u32 {
        self.0.width
    }

    /// Height in pixels.
    pub fn height(&self) -> u32 {
        self.0.height
    }

    /// Size in pixels.
    pub fn size(&self) -> Vec2 {
        Vec2::new(self.0.width as f32, self.0.height as f32)
    }

    pub(crate) fn data(&self) -> &Arc<TextureData> {
        &self.0
    }

    pub(crate) fn id(&self) -> u64 {
        self.0.id
    }
}

impl fmt::Debug for Texture {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Texture")
            .field("id", &self.0.id)
            .field("width", &self.0.width)
            .field("height", &self.0.height)
            .finish_non_exhaustive()
    }
}

/// Decodes any PNG (grayscale, RGB, palette, any bit depth) to RGBA8.
fn decode_png(bytes: &[u8]) -> Result<(u32, u32, Vec<u8>), String> {
    let mut decoder = png::Decoder::new(Cursor::new(bytes));
    // Palette -> RGB(A), sub-8-bit -> 8-bit, 16-bit -> 8-bit.
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().map_err(|e| e.to_string())?;
    let size = reader
        .output_buffer_size()
        .ok_or("image is too large to decode")?;
    let mut buf = vec![0; size];
    let info = reader.next_frame(&mut buf).map_err(|e| e.to_string())?;
    buf.truncate(info.buffer_size());

    let rgba = match info.color_type {
        png::ColorType::Rgba => buf,
        png::ColorType::Rgb => {
            let (pixels, _) = buf.as_chunks::<3>();
            pixels
                .iter()
                .flat_map(|&[r, g, b]| [r, g, b, 255])
                .collect()
        }
        png::ColorType::GrayscaleAlpha => {
            let (pixels, _) = buf.as_chunks::<2>();
            pixels.iter().flat_map(|&[g, a]| [g, g, g, a]).collect()
        }
        png::ColorType::Grayscale => buf.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        png::ColorType::Indexed => return Err("palette was not expanded".into()),
    };
    Ok((info.width, info.height, rgba))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encode_png(width: u32, height: u32, color: png::ColorType, data: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut encoder = png::Encoder::new(&mut out, width, height);
        encoder.set_color(color);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(data).unwrap();
        writer.finish().unwrap();
        out
    }

    #[test]
    fn rgba_length_is_checked() {
        assert!(Texture::from_rgba8(2, 2, vec![0; 16]).is_ok());
        assert!(Texture::from_rgba8(2, 2, vec![0; 15]).is_err());
        assert!(Texture::from_rgba8(0, 2, vec![]).is_err());
    }

    #[test]
    fn clones_share_an_id_new_textures_do_not() {
        let a = Texture::from_rgba8(1, 1, vec![0; 4]).unwrap();
        let b = Texture::from_rgba8(1, 1, vec![0; 4]).unwrap();
        assert_eq!(a.id(), a.clone().id());
        assert_ne!(a.id(), b.id());
    }

    #[test]
    fn decodes_rgba_png() {
        let pixels = [255, 0, 0, 255, 0, 255, 0, 128];
        let tex =
            Texture::from_png_bytes(&encode_png(2, 1, png::ColorType::Rgba, &pixels)).unwrap();
        assert_eq!((tex.width(), tex.height()), (2, 1));
        assert_eq!(tex.0.pixels, pixels);
    }

    #[test]
    fn rgb_and_gray_pngs_become_opaque_rgba() {
        let rgb = encode_png(1, 1, png::ColorType::Rgb, &[10, 20, 30]);
        assert_eq!(
            Texture::from_png_bytes(&rgb).unwrap().0.pixels,
            [10, 20, 30, 255]
        );

        let gray = encode_png(2, 1, png::ColorType::Grayscale, &[0, 200]);
        assert_eq!(
            Texture::from_png_bytes(&gray).unwrap().0.pixels,
            [0, 0, 0, 255, 200, 200, 200, 255]
        );

        let gray_alpha = encode_png(1, 1, png::ColorType::GrayscaleAlpha, &[50, 60]);
        assert_eq!(
            Texture::from_png_bytes(&gray_alpha).unwrap().0.pixels,
            [50, 50, 50, 60]
        );
    }

    #[test]
    fn bad_png_is_an_error() {
        assert!(Texture::from_png_bytes(b"not a png").is_err());
        assert!(Texture::from_png_file("does/not/exist.png").is_err());
    }
}
