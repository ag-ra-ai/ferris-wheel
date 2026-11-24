//! Keyboard and mouse state.

use std::collections::HashSet;
use std::hash::Hash;

use winit::dpi::PhysicalPosition;
use winit::event::{ElementState, WindowEvent};
use winit::keyboard::PhysicalKey;

/// A mouse button.
pub use winit::event::MouseButton;
/// A physical key, named by its position on a US keyboard
/// (so `Key::KeyW` is the same physical key on AZERTY layouts).
pub use winit::keyboard::KeyCode as Key;

/// Keyboard and mouse state, as seen by [`crate::Game::update`].
///
/// "Pressed" and "released" are true for exactly one update after the event,
/// even if several frames pass without an update or several updates run in
/// one frame.
#[derive(Debug, Default)]
pub struct Input {
    keys: Buttons<Key>,
    mouse_buttons: Buttons<MouseButton>,
    mouse_position: Option<(f32, f32)>,
}

impl Input {
    /// True while the key is held down.
    pub fn key_held(&self, key: Key) -> bool {
        self.keys.held.contains(&key)
    }

    /// True for the one update after the key went down.
    pub fn key_pressed(&self, key: Key) -> bool {
        self.keys.pressed.contains(&key)
    }

    /// True for the one update after the key went up.
    pub fn key_released(&self, key: Key) -> bool {
        self.keys.released.contains(&key)
    }

    /// True while the mouse button is held down.
    pub fn mouse_held(&self, button: MouseButton) -> bool {
        self.mouse_buttons.held.contains(&button)
    }

    /// True for the one update after the mouse button went down.
    pub fn mouse_pressed(&self, button: MouseButton) -> bool {
        self.mouse_buttons.pressed.contains(&button)
    }

    /// True for the one update after the mouse button went up.
    pub fn mouse_released(&self, button: MouseButton) -> bool {
        self.mouse_buttons.released.contains(&button)
    }

    /// Cursor position in logical pixels from the window's top-left corner,
    /// or `None` when the cursor is outside the window.
    pub fn mouse_position(&self) -> Option<(f32, f32)> {
        self.mouse_position
    }

    /// Updates state from a window event. `scale_factor` converts physical
    /// pixels to logical pixels.
    pub(crate) fn handle_event(&mut self, event: &WindowEvent, scale_factor: f64) {
        match event {
            WindowEvent::KeyboardInput { event, .. } => {
                let PhysicalKey::Code(key) = event.physical_key else {
                    return;
                };
                match event.state {
                    // OS key repeat is not a new press.
                    ElementState::Pressed if !event.repeat => self.keys.down(key),
                    ElementState::Pressed => {}
                    ElementState::Released => self.keys.up(key),
                }
            }
            WindowEvent::MouseInput { state, button, .. } => match state {
                ElementState::Pressed => self.mouse_buttons.down(*button),
                ElementState::Released => self.mouse_buttons.up(*button),
            },
            WindowEvent::CursorMoved { position, .. } => {
                self.mouse_position = Some(to_logical(*position, scale_factor));
            }
            WindowEvent::CursorLeft { .. } => self.mouse_position = None,
            // We won't see the key-up events while unfocused, so let go of
            // everything now rather than leave keys stuck down.
            WindowEvent::Focused(false) => self.release_all(),
            _ => {}
        }
    }

    /// Called after each update so "pressed"/"released" last one update.
    pub(crate) fn end_update(&mut self) {
        self.keys.end_update();
        self.mouse_buttons.end_update();
    }

    fn release_all(&mut self) {
        self.keys.release_all();
        self.mouse_buttons.release_all();
    }
}

fn to_logical(position: PhysicalPosition<f64>, scale_factor: f64) -> (f32, f32) {
    let logical = position.to_logical::<f64>(scale_factor);
    (logical.x as f32, logical.y as f32)
}

/// Held / pressed / released tracking for one kind of button.
#[derive(Debug)]
struct Buttons<T> {
    held: HashSet<T>,
    pressed: HashSet<T>,
    released: HashSet<T>,
}

// Manual impl: derive would wrongly require `T: Default`.
impl<T> Default for Buttons<T> {
    fn default() -> Self {
        Self {
            held: HashSet::new(),
            pressed: HashSet::new(),
            released: HashSet::new(),
        }
    }
}

impl<T: Copy + Eq + Hash> Buttons<T> {
    fn down(&mut self, button: T) {
        if self.held.insert(button) {
            self.pressed.insert(button);
        }
    }

    fn up(&mut self, button: T) {
        if self.held.remove(&button) {
            self.released.insert(button);
        }
    }

    fn release_all(&mut self) {
        self.released.extend(self.held.drain());
    }

    fn end_update(&mut self) {
        self.pressed.clear();
        self.released.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn press_lasts_one_update_hold_persists() {
        let mut input = Input::default();
        input.keys.down(Key::Space);
        assert!(input.key_pressed(Key::Space));
        assert!(input.key_held(Key::Space));

        input.end_update();
        assert!(!input.key_pressed(Key::Space));
        assert!(input.key_held(Key::Space));
    }

    #[test]
    fn release_lasts_one_update() {
        let mut input = Input::default();
        input.keys.down(Key::KeyA);
        input.end_update();
        input.keys.up(Key::KeyA);
        assert!(input.key_released(Key::KeyA));
        assert!(!input.key_held(Key::KeyA));

        input.end_update();
        assert!(!input.key_released(Key::KeyA));
    }

    #[test]
    fn tap_between_updates_is_not_lost() {
        let mut input = Input::default();
        input.keys.down(Key::Enter);
        input.keys.up(Key::Enter);
        assert!(input.key_pressed(Key::Enter));
        assert!(input.key_released(Key::Enter));
        assert!(!input.key_held(Key::Enter));
    }

    #[test]
    fn duplicate_down_is_not_a_new_press() {
        let mut input = Input::default();
        input.keys.down(Key::KeyW);
        input.end_update();
        input.keys.down(Key::KeyW);
        assert!(!input.key_pressed(Key::KeyW));
    }

    #[test]
    fn release_without_press_is_ignored() {
        let mut input = Input::default();
        input.keys.up(Key::KeyQ);
        assert!(!input.key_released(Key::KeyQ));
    }

    #[test]
    fn focus_loss_releases_everything() {
        let mut input = Input::default();
        input.keys.down(Key::ArrowLeft);
        input.mouse_buttons.down(MouseButton::Left);
        input.end_update();

        input.handle_event(&WindowEvent::Focused(false), 1.0);
        assert!(!input.key_held(Key::ArrowLeft));
        assert!(input.key_released(Key::ArrowLeft));
        assert!(!input.mouse_held(MouseButton::Left));
        assert!(input.mouse_released(MouseButton::Left));
    }

    #[test]
    fn mouse_position_is_in_logical_pixels() {
        assert_eq!(
            to_logical(PhysicalPosition::new(200.0, 100.0), 2.0),
            (100.0, 50.0)
        );
    }
}
