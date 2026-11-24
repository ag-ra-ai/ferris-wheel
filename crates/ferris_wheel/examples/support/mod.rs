//! Helpers shared by the examples. (A folder without `main.rs`, so Cargo does
//! not treat it as an example of its own.)

/// Sends log messages somewhere visible: stderr on desktop (filtered by
/// `RUST_LOG`, e.g. `RUST_LOG=info`), the browser console on the web.
pub fn init_logging() {
    #[cfg(not(target_arch = "wasm32"))]
    env_logger::init();

    #[cfg(target_arch = "wasm32")]
    console_log::init_with_level(log::Level::Info).expect("logger already set");
}
