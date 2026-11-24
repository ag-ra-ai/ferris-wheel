//! The winit event loop that drives the engine.

use std::sync::Arc;

use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowId};

use crate::gpu::Gpu;
use crate::{Config, Error};

pub(crate) fn run(config: Config) -> Result<(), Error> {
    let event_loop = EventLoop::new()?;
    let mut app = App {
        config,
        gpu: None,
        error: None,
    };
    event_loop.run_app(&mut app)?;
    match app.error {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

struct App {
    config: Config,
    gpu: Option<Gpu>,
    /// A startup failure, returned from [`run`] after the loop exits.
    error: Option<Error>,
}

impl App {
    fn init(&mut self, event_loop: &ActiveEventLoop) -> Result<Gpu, Error> {
        let attrs = Window::default_attributes()
            .with_title(&self.config.title)
            .with_inner_size(LogicalSize::new(self.config.width, self.config.height));
        let window = Arc::new(event_loop.create_window(attrs)?);

        // The display handle is required for GLES on Wayland and harmless elsewhere.
        // `_from_env` lets WGPU_BACKEND etc. override settings when debugging.
        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_with_display_handle_from_env(
                Box::new(event_loop.owned_display_handle()),
            ));
        Gpu::new(instance, window)
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        // Desktop platforms call this once at startup; only set up the first time.
        if self.gpu.is_some() {
            return;
        }
        match self.init(event_loop) {
            Ok(gpu) => {
                gpu.window().request_redraw();
                self.gpu = Some(gpu);
            }
            Err(e) => {
                self.error = Some(e);
                event_loop.exit();
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let Some(gpu) = self.gpu.as_mut() else {
            return;
        };
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => gpu.resize(size.width, size.height),
            WindowEvent::RedrawRequested => {
                gpu.render(self.config.clear_color);
                // Ask for the next frame right away; vsync paces the loop.
                gpu.window().request_redraw();
            }
            _ => {}
        }
    }
}
