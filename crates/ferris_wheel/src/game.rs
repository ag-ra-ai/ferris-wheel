//! The trait a game implements, and what the engine passes to it.

use std::time::Duration;

use crate::audio::Audio;
use crate::viewport::{Scaling, Viewport};
use crate::{Color, Font, Input, Rect, Sound, SoundParams, TextParams, Texture, Vec2};

/// A game the engine can run. See [`crate::run`].
pub trait Game {
    /// Advances the game by one fixed step of [`Context::dt`] seconds.
    ///
    /// Called at a steady rate ([`crate::Config::updates_per_second`]),
    /// independent of how fast frames are drawn. Put movement, physics and
    /// game rules here.
    fn update(&mut self, ctx: &mut Context);

    /// Draws the current state. Called once per displayed frame.
    fn draw(&mut self, frame: &mut Frame);
}

/// Engine state available during [`Game::update`].
#[derive(Debug)]
pub struct Context {
    pub(crate) input: Input,
    pub(crate) step: Duration,
    pub(crate) view_size: Vec2,
    pub(crate) scaling: Scaling,
    pub(crate) quit_requested: bool,
    pub(crate) audio: Audio,
}

impl Context {
    pub(crate) fn new(step: Duration, viewport: Viewport) -> Self {
        Self {
            input: Input::new(viewport),
            step,
            view_size: viewport.view_size,
            scaling: viewport.scaling,
            quit_requested: false,
            audio: Audio::new(),
        }
    }

    /// Size of the game view in world units ([`crate::Config::resolution`]).
    pub fn view_size(&self) -> Vec2 {
        self.view_size
    }

    /// Current keyboard and mouse state.
    pub fn input(&self) -> &Input {
        &self.input
    }

    /// Length of one update in seconds. Always the same value; multiply
    /// speeds by it to get distance per update.
    pub fn dt(&self) -> f32 {
        self.step.as_secs_f32()
    }

    /// How the view is currently scaled to the window.
    pub fn scaling(&self) -> Scaling {
        self.scaling
    }

    /// Changes how the view is scaled to the window, from the next frame on.
    /// Mouse positions follow the new scaling too.
    pub fn set_scaling(&mut self, scaling: Scaling) {
        self.scaling = scaling;
    }

    /// Closes the window and returns from [`crate::run`] after this update.
    pub fn quit(&mut self) {
        self.quit_requested = true;
    }

    /// Plays a sound once. Any number of sounds can play at the same time.
    ///
    /// Browsers allow sound only after the player clicks or presses a key
    /// on the page; sounds played before that are skipped.
    pub fn play_sound(&mut self, sound: &Sound) {
        self.audio.play(sound, SoundParams::default());
    }

    /// Plays a sound once with a different volume, speed or stereo position.
    pub fn play_sound_with(&mut self, sound: &Sound, params: SoundParams) {
        self.audio.play(sound, params);
    }

    /// Plays a sound on repeat as background music, replacing any music
    /// already playing. In browsers it starts once the player first clicks
    /// or presses a key.
    pub fn play_music(&mut self, sound: &Sound) {
        self.audio.play_music(sound, SoundParams::default());
    }

    /// Like [`Context::play_music`], with a different volume, speed or
    /// stereo position.
    pub fn play_music_with(&mut self, sound: &Sound, params: SoundParams) {
        self.audio.play_music(sound, params);
    }

    /// Fades out the music, if any is playing.
    pub fn stop_music(&mut self) {
        self.audio.stop_music();
    }

    /// The music volume, which scales music but not sound effects. Starts
    /// at 1.0.
    pub fn music_volume(&self) -> f32 {
        self.audio.music_volume()
    }

    /// Sets the music volume, for the current music and any played later.
    /// Playing music fades to the new volume over a tenth of a second, so
    /// this also suits quieter music while the game is paused.
    pub fn set_music_volume(&mut self, volume: f32) {
        self.audio.set_music_volume(volume);
    }

    /// The master volume, which scales all sounds and music. Starts at 1.0.
    pub fn volume(&self) -> f32 {
        self.audio.volume()
    }

    /// Sets the master volume: 1.0 leaves sounds unchanged, 0.0 mutes
    /// everything.
    pub fn set_volume(&mut self, volume: f32) {
        self.audio.set_volume(volume);
    }
}

/// One frame to draw into, passed to [`Game::draw`].
///
/// Coordinates are world units: (0, 0) is the top-left of the view and
/// [`Frame::view_size`] is the bottom-right. Shapes are drawn in call order,
/// so later shapes cover earlier ones.
#[derive(Debug)]
pub struct Frame {
    pub(crate) clear_color: Color,
    pub(crate) quads: Vec<Quad>,
    pub(crate) interpolation: f32,
    view_size: Vec2,
}

/// A rectangle waiting to be drawn: a sprite, or a plain color when
/// `texture` is `None`.
#[derive(Debug, Clone)]
pub(crate) struct Quad {
    pub(crate) rect: Rect,
    /// Texture coordinates (0..1); a negative size flips the image.
    pub(crate) uv: Rect,
    pub(crate) color: Color,
    pub(crate) texture: Option<Texture>,
}

/// Options for [`Frame::draw_sprite_with`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpriteParams {
    /// Part of the texture to draw, in pixels. `None` draws the whole
    /// texture. Use this to pick one frame from a sprite sheet.
    pub source: Option<Rect>,
    /// Multiplied with every pixel. White leaves the image unchanged; lower
    /// alpha makes it see-through.
    pub tint: Color,
    /// Mirror the image left-to-right.
    pub flip_x: bool,
    /// Mirror the image top-to-bottom.
    pub flip_y: bool,
}

impl Default for SpriteParams {
    fn default() -> Self {
        Self {
            source: None,
            tint: Color::WHITE,
            flip_x: false,
            flip_y: false,
        }
    }
}

impl Frame {
    pub(crate) fn new(view_size: Vec2) -> Self {
        Self {
            clear_color: Color::BLACK,
            quads: Vec::new(),
            interpolation: 0.0,
            view_size,
        }
    }

    /// Empties the frame for reuse, keeping its allocation.
    pub(crate) fn reset(&mut self) {
        self.clear_color = Color::BLACK;
        self.quads.clear();
    }

    /// Size of the game view in world units.
    pub fn view_size(&self) -> Vec2 {
        self.view_size
    }

    /// How far this frame is between the last update and the next one, from
    /// 0 up to (but not including) 1.
    ///
    /// Updates run at a fixed rate, but frames are drawn at the screen's
    /// refresh rate. Drawing moving things at the position from the last
    /// update makes them stutter on screens faster than the update rate.
    /// Instead, keep the position from before the last update too, and draw
    /// at `previous.lerp(current, frame.interpolation())`.
    pub fn interpolation(&self) -> f32 {
        self.interpolation
    }

    /// Fills the whole view with `color`. Black if not called.
    pub fn clear(&mut self, color: Color) {
        self.clear_color = color;
    }

    /// Draws a filled rectangle.
    pub fn fill_rect(&mut self, rect: Rect, color: Color) {
        self.quads.push(Quad {
            rect,
            uv: Rect::new(0.0, 0.0, 1.0, 1.0),
            color,
            texture: None,
        });
    }

    /// Draws the whole `texture` stretched to fill `dest`.
    pub fn draw_sprite(&mut self, texture: &Texture, dest: Rect) {
        self.draw_sprite_with(texture, dest, SpriteParams::default());
    }

    /// Draws `text` in the built-in font at scale 1, with its top-left corner
    /// at `pos`. `\n` starts a new line.
    pub fn draw_text(&mut self, text: &str, pos: Vec2, color: Color) {
        let params = TextParams {
            color,
            ..TextParams::default()
        };
        self.draw_text_with(&Font::builtin(), text, pos, params);
    }

    /// Draws `text` in `font` with the given color and scale, with its
    /// top-left corner at `pos`. Use [`Font::measure`] to center or
    /// right-align it.
    pub fn draw_text_with(&mut self, font: &Font, text: &str, pos: Vec2, params: TextParams) {
        let texture = font.texture();
        font.layout(text, pos, params.scale, |source, dest| {
            let sprite = SpriteParams {
                source: Some(source),
                tint: params.color,
                ..SpriteParams::default()
            };
            self.draw_sprite_with(texture, dest, sprite);
        });
    }

    /// Draws `texture` into `dest` with a source region, tint or flip.
    pub fn draw_sprite_with(&mut self, texture: &Texture, dest: Rect, params: SpriteParams) {
        let source =
            params
                .source
                .unwrap_or(Rect::new(0.0, 0.0, texture.size().x, texture.size().y));
        let mut uv = Rect {
            pos: Vec2::new(
                source.pos.x / texture.size().x,
                source.pos.y / texture.size().y,
            ),
            size: Vec2::new(
                source.size.x / texture.size().x,
                source.size.y / texture.size().y,
            ),
        };
        if params.flip_x {
            uv.pos.x += uv.size.x;
            uv.size.x = -uv.size.x;
        }
        if params.flip_y {
            uv.pos.y += uv.size.y;
            uv.size.y = -uv.size.y;
        }
        self.quads.push(Quad {
            rect: dest,
            uv,
            color: params.tint,
            texture: Some(texture.clone()),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VIEW: Vec2 = Vec2::new(320.0, 180.0);

    #[test]
    fn dt_is_step_in_seconds() {
        let ctx = Context::new(
            Duration::from_millis(20),
            Viewport::fit(VIEW, 640, 360, Scaling::Fit),
        );
        assert!((ctx.dt() - 0.02).abs() < 1e-6);
    }

    #[test]
    fn scaling_starts_from_viewport_and_can_change() {
        let viewport = Viewport::fit(VIEW, 640, 360, Scaling::PixelPerfect);
        let mut ctx = Context::new(Duration::from_millis(20), viewport);
        assert_eq!(ctx.scaling(), Scaling::PixelPerfect);
        ctx.set_scaling(Scaling::Fit);
        assert_eq!(ctx.scaling(), Scaling::Fit);
    }

    #[test]
    fn quit_sets_flag() {
        let mut ctx = Context::new(
            Duration::from_millis(20),
            Viewport::fit(VIEW, 640, 360, Scaling::Fit),
        );
        ctx.quit();
        assert!(ctx.quit_requested);
    }

    #[test]
    fn frame_defaults_to_black() {
        assert_eq!(Frame::new(VIEW).clear_color, Color::BLACK);
    }

    #[test]
    fn frame_collects_rects_in_order_and_resets() {
        let mut frame = Frame::new(VIEW);
        frame.clear(Color::WHITE);
        frame.fill_rect(Rect::new(0.0, 0.0, 1.0, 1.0), Color::RED);
        frame.fill_rect(Rect::new(1.0, 1.0, 1.0, 1.0), Color::BLUE);
        let colors: Vec<_> = frame.quads.iter().map(|q| q.color).collect();
        assert_eq!(colors, [Color::RED, Color::BLUE]);

        frame.reset();
        assert!(frame.quads.is_empty());
        assert_eq!(frame.clear_color, Color::BLACK);
        assert_eq!(frame.view_size(), VIEW);
    }

    fn sprite_uv(params: SpriteParams) -> Rect {
        let texture = Texture::from_rgba8(64, 16, vec![0; 64 * 16 * 4]).unwrap();
        let mut frame = Frame::new(VIEW);
        frame.draw_sprite_with(&texture, Rect::new(0.0, 0.0, 16.0, 16.0), params);
        frame.quads[0].uv
    }

    #[test]
    fn text_becomes_one_tinted_quad_per_visible_char() {
        let mut frame = Frame::new(VIEW);
        frame.draw_text("Hi !", Vec2::new(4.0, 4.0), Color::RED);
        assert_eq!(frame.quads.len(), 3);
        assert!(frame.quads.iter().all(|q| q.color == Color::RED));
        assert!(frame.quads.iter().all(|q| q.texture.is_some()));
        assert_eq!(frame.quads[2].rect.pos, Vec2::new(4.0 + 3.0 * 6.0, 4.0));
    }

    #[test]
    fn whole_texture_uses_full_uv_range() {
        assert_eq!(
            sprite_uv(SpriteParams::default()),
            Rect::new(0.0, 0.0, 1.0, 1.0)
        );
    }

    #[test]
    fn source_region_maps_to_uv() {
        let uv = sprite_uv(SpriteParams {
            source: Some(Rect::new(16.0, 0.0, 16.0, 16.0)),
            ..Default::default()
        });
        assert_eq!(uv, Rect::new(0.25, 0.0, 0.25, 1.0));
    }

    #[test]
    fn flips_swap_uv_edges() {
        let uv = sprite_uv(SpriteParams {
            source: Some(Rect::new(16.0, 0.0, 16.0, 16.0)),
            flip_x: true,
            flip_y: true,
            ..Default::default()
        });
        assert_eq!(uv, Rect::new(0.5, 1.0, -0.25, -1.0));
    }
}
