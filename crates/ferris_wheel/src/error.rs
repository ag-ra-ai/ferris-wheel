use std::fmt;

/// Errors that stop the engine from starting or running.
#[derive(Debug)]
pub enum Error {
    /// The OS event loop could not be created or failed while running.
    EventLoop(winit::error::EventLoopError),
    /// The window could not be created.
    Window(winit::error::OsError),
    /// The GPU could not be set up for drawing to the window.
    Gpu(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EventLoop(e) => write!(f, "event loop error: {e}"),
            Self::Window(e) => write!(f, "could not create window: {e}"),
            Self::Gpu(msg) => write!(f, "GPU setup failed: {msg}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::EventLoop(e) => Some(e),
            Self::Window(e) => Some(e),
            Self::Gpu(_) => None,
        }
    }
}

impl From<winit::error::EventLoopError> for Error {
    fn from(e: winit::error::EventLoopError) -> Self {
        Self::EventLoop(e)
    }
}

impl From<winit::error::OsError> for Error {
    fn from(e: winit::error::OsError) -> Self {
        Self::Window(e)
    }
}
