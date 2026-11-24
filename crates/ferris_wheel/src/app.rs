//! The winit event loop that drives the engine.

use std::sync::Arc;
use std::time::Instant;

use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowId};

use crate::game::{Context, Frame};
use crate::gpu::Gpu;
use crate::time::FixedTimestep;
use crate::{Config, Error, Game};

pub(crate) fn run<G: Game>(config: Config, game: G) -> Result<(), Error> {
    let event_loop = EventLoop::new()?;
    let timestep = FixedTimestep::new(config.updates_per_second);
    let mut app = App {
        ctx: Context::new(timestep.step()),
        timestep,
        last_frame: Instant::now(),
        config,
        game,
        gpu: None,
        error: None,
    };
    event_loop.run_app(&mut app)?;
    match app.error {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

struct App<G> {
    config: Config,
    game: G,
    ctx: Context,
    timestep: FixedTimestep,
    last_frame: Instant,
    gpu: Option<Gpu>,
    /// A startup failure, returned from [`run`] after the loop exits.
    error: Option<Error>,
}

impl<G: Game> App<G> {
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

    /// Runs any due updates, then draws one frame.
    fn frame(&mut self, event_loop: &ActiveEventLoop) {
        let now = Instant::now();
        let steps = self.timestep.advance(now - self.last_frame);
        self.last_frame = now;

        for _ in 0..steps {
            self.game.update(&mut self.ctx);
            self.ctx.input.end_update();
            if self.ctx.quit_requested {
                event_loop.exit();
                return;
            }
        }

        let mut frame = Frame::new();
        self.game.draw(&mut frame);
        if let Some(gpu) = self.gpu.as_mut() {
            gpu.render(frame.clear_color);
            // Ask for the next frame right away; vsync paces the loop.
            gpu.window().request_redraw();
        }
    }
}

impl<G: Game> ApplicationHandler for App<G> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        // Desktop platforms call this once at startup; only set up the first time.
        if self.gpu.is_some() {
            return;
        }
        match self.init(event_loop) {
            Ok(gpu) => {
                gpu.window().request_redraw();
                self.gpu = Some(gpu);
                // Don't count window/GPU setup time as game time.
                self.last_frame = Instant::now();
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
        self.ctx
            .input
            .handle_event(&event, gpu.window().scale_factor());
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => gpu.resize(size.width, size.height),
            WindowEvent::RedrawRequested => self.frame(event_loop),
            _ => {}
        }
    }
}
