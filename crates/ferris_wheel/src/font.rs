//! Bitmap fonts for [`crate::Frame::draw_text`].

use std::collections::HashMap;
use std::sync::OnceLock;

use crate::{Error, Rect, Texture, Vec2};

/// The built-in font as readable pixel art; see the file for the format.
const BUILTIN_SOURCE: &str = include_str!("fonts/builtin5x8.txt");
const BUILTIN_GLYPH_W: u32 = 5;
const BUILTIN_GLYPH_H: u32 = 8;
/// The built-in atlas leaves a transparent pixel right of and below each
/// glyph, so scaled or off-grid text never samples a neighbouring glyph.
const BUILTIN_CELL_W: u32 = BUILTIN_GLYPH_W + 1;
const BUILTIN_CELL_H: u32 = BUILTIN_GLYPH_H + 1;
const BUILTIN_COLUMNS: u32 = 16;

/// A monospace bitmap font: every character is drawn from a same-sized cell
/// of one texture.
///
/// Use [`Font::builtin`] for the engine's 5x8 pixel font, or
/// [`Font::monospace`] to load your own from an image.
#[derive(Debug, Clone)]
pub struct Font {
    texture: Texture,
    glyph_size: Vec2,
    /// Gap between characters and between lines, in font pixels.
    spacing: Vec2,
    /// Source rectangle of each character in the texture.
    glyphs: HashMap<char, Rect>,
}

/// Options for [`crate::Frame::draw_text_with`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextParams {
    /// Text color. The font's own colors are multiplied by it, so white
    /// fonts (like the built-in one) take this color exactly.
    pub color: crate::Color,
    /// Size multiplier: 2.0 draws each font pixel as 2x2 world units.
    pub scale: f32,
}

impl Default for TextParams {
    fn default() -> Self {
        Self {
            color: crate::Color::WHITE,
            scale: 1.0,
        }
    }
}

impl Font {
    /// The built-in 5x8 pixel font, covering printable ASCII. Cheap to call:
    /// the font is built once and shared.
    pub fn builtin() -> Font {
        static BUILTIN: OnceLock<Font> = OnceLock::new();
        BUILTIN
            .get_or_init(|| {
                build_builtin(BUILTIN_SOURCE).expect("built-in font data is valid (tested)")
            })
            .clone()
    }

    /// A font from a sprite sheet of equal-sized cells.
    ///
    /// `chars` lists the characters in the sheet in reading order: left to
    /// right, then top to bottom. For example, a sheet with the printable
    /// ASCII characters in order would pass
    /// `" !\"#$%&'()*+,-./0123456789:;<=>?@ABC..."`.
    ///
    /// Characters are placed 1 font pixel apart, lines 2 font pixels apart;
    /// change this with [`Font::with_spacing`].
    pub fn monospace(
        texture: Texture,
        cell_width: u32,
        cell_height: u32,
        chars: &str,
    ) -> Result<Font, Error> {
        if cell_width == 0 || cell_height == 0 {
            return Err(Error::Texture("font cell size must not be zero".into()));
        }
        let columns = texture.width() / cell_width;
        let rows = texture.height() / cell_height;
        let count = chars.chars().count() as u32;
        if columns == 0 || count > columns * rows {
            return Err(Error::Texture(format!(
                "a {}x{} font sheet holds {} cells of {cell_width}x{cell_height}, \
                 but {count} characters were listed",
                texture.width(),
                texture.height(),
                columns * rows
            )));
        }
        let glyphs = chars
            .chars()
            .enumerate()
            .map(|(i, ch)| {
                let (col, row) = (i as u32 % columns, i as u32 / columns);
                let rect = Rect::new(
                    (col * cell_width) as f32,
                    (row * cell_height) as f32,
                    cell_width as f32,
                    cell_height as f32,
                );
                (ch, rect)
            })
            .collect();
        Ok(Font {
            texture,
            glyph_size: Vec2::new(cell_width as f32, cell_height as f32),
            spacing: Vec2::new(1.0, 2.0),
            glyphs,
        })
    }

    /// Sets the gap between characters (`x`) and between lines (`y`), in
    /// font pixels. Use 0 if your sheet's cells already include spacing.
    pub fn with_spacing(mut self, x: f32, y: f32) -> Font {
        self.spacing = Vec2::new(x, y);
        self
    }

    /// Size of one character cell in font pixels.
    pub fn glyph_size(&self) -> Vec2 {
        self.glyph_size
    }

    /// Distance from the top of one line to the top of the next, at scale 1.
    pub fn line_height(&self) -> f32 {
        self.glyph_size.y + self.spacing.y
    }

    /// Size of `text` when drawn at `scale`: the width of its longest line
    /// and the height of all its lines. Useful for centering.
    pub fn measure(&self, text: &str, scale: f32) -> Vec2 {
        let advance = self.glyph_size.x + self.spacing.x;
        let lines: Vec<usize> = text.split('\n').map(line_len).collect();
        let longest = lines.iter().copied().max().unwrap_or(0);
        let width = if longest == 0 {
            0.0
        } else {
            longest as f32 * advance - self.spacing.x
        };
        let height = (lines.len() - 1) as f32 * self.line_height() + self.glyph_size.y;
        Vec2::new(width, height) * scale
    }

    pub(crate) fn texture(&self) -> &Texture {
        &self.texture
    }

    /// Calls `emit(source, dest)` for each visible character of `text`, with
    /// its top-left corner at `pos`. Characters the font lacks are drawn as
    /// `?` if it has one, and skipped otherwise.
    pub(crate) fn layout(
        &self,
        text: &str,
        pos: Vec2,
        scale: f32,
        mut emit: impl FnMut(Rect, Rect),
    ) {
        // Start on a whole world unit so pixel fonts stay crisp.
        let origin = Vec2::new(pos.x.round(), pos.y.round());
        let advance = (self.glyph_size.x + self.spacing.x) * scale;
        let size = self.glyph_size * scale;
        let fallback = self.glyphs.get(&'?');
        for (row, line) in text.split('\n').enumerate() {
            let y = origin.y + row as f32 * self.line_height() * scale;
            for (col, ch) in line.chars().filter(|&c| c != '\r').enumerate() {
                if ch == ' ' {
                    continue;
                }
                if let Some(&source) = self.glyphs.get(&ch).or(fallback) {
                    let dest = Rect {
                        pos: Vec2::new(origin.x + col as f32 * advance, y),
                        size,
                    };
                    emit(source, dest);
                }
            }
        }
    }
}

/// Characters in a line, ignoring a Windows line-ending `\r`.
fn line_len(line: &str) -> usize {
    line.chars().filter(|&c| c != '\r').count()
}

/// Parses the built-in font text into glyph bitmaps, in file order.
fn parse_builtin(source: &str) -> Result<Vec<(char, Vec<&str>)>, String> {
    let mut glyphs: Vec<(char, Vec<&str>)> = Vec::new();
    for (number, line) in source.lines().enumerate() {
        let number = number + 1;
        // Comments start with "# " (hash, space); pixel rows never contain a
        // space, so a row like "#...#" is not mistaken for one.
        if line.is_empty() || line.starts_with("# ") {
            continue;
        }
        if let Some(header) = line.strip_prefix("-- ") {
            let hex = header.split(' ').next().unwrap_or("");
            let code = u32::from_str_radix(hex, 16)
                .ok()
                .and_then(char::from_u32)
                .ok_or(format!("line {number}: bad character code {hex:?}"))?;
            glyphs.push((code, Vec::new()));
            continue;
        }
        let (_, rows) = glyphs.last_mut().ok_or(format!(
            "line {number}: pixels before the first glyph header"
        ))?;
        if line.len() != BUILTIN_GLYPH_W as usize || line.chars().any(|c| c != '#' && c != '.') {
            return Err(format!(
                "line {number}: expected 5 of `#` or `.`, got {line:?}"
            ));
        }
        rows.push(line);
    }
    for (ch, rows) in &glyphs {
        if rows.len() != BUILTIN_GLYPH_H as usize {
            return Err(format!("glyph {ch:?} has {} rows, expected 8", rows.len()));
        }
    }
    Ok(glyphs)
}

/// Builds the built-in font's texture atlas from its text source.
fn build_builtin(source: &str) -> Result<Font, String> {
    let glyphs = parse_builtin(source)?;
    let rows = (glyphs.len() as u32).div_ceil(BUILTIN_COLUMNS);
    let (width, height) = (BUILTIN_COLUMNS * BUILTIN_CELL_W, rows * BUILTIN_CELL_H);
    let mut pixels = vec![0u8; (width * height * 4) as usize];
    let mut map = HashMap::new();
    for (i, (ch, bitmap)) in glyphs.iter().enumerate() {
        let x0 = (i as u32 % BUILTIN_COLUMNS) * BUILTIN_CELL_W;
        let y0 = (i as u32 / BUILTIN_COLUMNS) * BUILTIN_CELL_H;
        for (y, row) in bitmap.iter().enumerate() {
            for (x, pixel) in row.chars().enumerate() {
                if pixel == '#' {
                    let index = (((y0 + y as u32) * width + x0 + x as u32) * 4) as usize;
                    pixels[index..index + 4].copy_from_slice(&[255; 4]);
                }
            }
        }
        let rect = Rect::new(
            x0 as f32,
            y0 as f32,
            BUILTIN_GLYPH_W as f32,
            BUILTIN_GLYPH_H as f32,
        );
        map.insert(*ch, rect);
    }
    let texture = Texture::from_rgba8(width, height, pixels).map_err(|e| e.to_string())?;
    Ok(Font {
        texture,
        glyph_size: Vec2::new(BUILTIN_GLYPH_W as f32, BUILTIN_GLYPH_H as f32),
        spacing: Vec2::new(1.0, 2.0),
        glyphs: map,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_covers_printable_ascii_in_order() {
        let glyphs = parse_builtin(BUILTIN_SOURCE).unwrap();
        let chars: Vec<char> = glyphs.iter().map(|(c, _)| *c).collect();
        let expected: Vec<char> = (0x20u8..=0x7e).map(char::from).collect();
        assert_eq!(chars, expected);
    }

    #[test]
    fn builtin_glyph_pixels_match_source() {
        let glyphs = parse_builtin(BUILTIN_SOURCE).unwrap();
        let (_, h) = glyphs.iter().find(|(c, _)| *c == 'H').unwrap();
        assert_eq!(h[0], "#...#");
        assert_eq!(h[3], "#####");
        assert_eq!(h[7], ".....");
    }

    #[test]
    fn parser_reports_bad_input() {
        assert!(parse_builtin("-- 41 A\n#####\n").is_err()); // too few rows
        assert!(parse_builtin("# a comment\n").is_ok()); // comment only, no glyphs
        assert!(parse_builtin("#####\n").is_err()); // a pixel row, not a comment
        assert!(parse_builtin("..#..\n").is_err()); // pixels before a header
        assert!(parse_builtin("-- zz x\n").is_err()); // bad code
        let mut wide = String::from("-- 41 A\n");
        wide.push_str(&"######\n".repeat(8));
        assert!(parse_builtin(&wide).is_err());
    }

    #[test]
    fn builtin_is_shared() {
        assert_eq!(
            Font::builtin().texture().id(),
            Font::builtin().texture().id()
        );
    }

    fn layout(font: &Font, text: &str, pos: Vec2, scale: f32) -> Vec<(Rect, Rect)> {
        let mut out = Vec::new();
        font.layout(text, pos, scale, |s, d| out.push((s, d)));
        out
    }

    #[test]
    fn layout_advances_and_wraps_lines() {
        let font = Font::builtin(); // 5x8, spacing 1 x 2
        let quads = layout(&font, "AB\nC", Vec2::new(10.0, 20.0), 1.0);
        let positions: Vec<Vec2> = quads.iter().map(|(_, d)| d.pos).collect();
        assert_eq!(
            positions,
            [
                Vec2::new(10.0, 20.0),
                Vec2::new(16.0, 20.0),
                Vec2::new(10.0, 30.0)
            ]
        );
        assert!(quads.iter().all(|(_, d)| d.size == Vec2::new(5.0, 8.0)));
    }

    #[test]
    fn layout_scales_skips_spaces_and_snaps_origin() {
        let font = Font::builtin();
        let quads = layout(&font, "A B", Vec2::new(0.4, 0.6), 2.0);
        assert_eq!(quads.len(), 2, "space draws nothing");
        assert_eq!(quads[0].1, Rect::new(0.0, 1.0, 10.0, 16.0));
        assert_eq!(quads[1].1.pos, Vec2::new(24.0, 1.0), "space still advances");
    }

    #[test]
    fn unknown_characters_fall_back_to_question_mark() {
        let font = Font::builtin();
        let unknown = layout(&font, "é", Vec2::ZERO, 1.0);
        let question = layout(&font, "?", Vec2::ZERO, 1.0);
        assert_eq!(unknown, question);
    }

    #[test]
    fn measure_matches_layout() {
        let font = Font::builtin();
        assert_eq!(font.measure("", 1.0), Vec2::new(0.0, 8.0));
        assert_eq!(font.measure("A", 1.0), Vec2::new(5.0, 8.0));
        assert_eq!(font.measure("AB", 1.0), Vec2::new(11.0, 8.0));
        assert_eq!(font.measure("ABC\nD", 2.0), Vec2::new(34.0, 36.0));
        assert_eq!(font.measure("AB\r\n", 1.0), Vec2::new(11.0, 18.0));
    }

    #[test]
    fn monospace_maps_cells_in_reading_order() {
        let texture = Texture::from_rgba8(16, 16, vec![0; 16 * 16 * 4]).unwrap();
        let font = Font::monospace(texture, 8, 8, "abc").unwrap();
        assert_eq!(font.glyphs[&'a'], Rect::new(0.0, 0.0, 8.0, 8.0));
        assert_eq!(font.glyphs[&'b'], Rect::new(8.0, 0.0, 8.0, 8.0));
        assert_eq!(font.glyphs[&'c'], Rect::new(0.0, 8.0, 8.0, 8.0));
        assert_eq!(font.with_spacing(0.0, 0.0).line_height(), 8.0);
    }

    #[test]
    fn monospace_rejects_too_many_chars() {
        let texture = Texture::from_rgba8(16, 8, vec![0; 16 * 8 * 4]).unwrap();
        assert!(Font::monospace(texture.clone(), 8, 8, "ab").is_ok());
        assert!(Font::monospace(texture.clone(), 8, 8, "abc").is_err());
        assert!(Font::monospace(texture, 0, 8, "a").is_err());
    }
}
