//! The winit event loop that drives the engine.

use std::sync::Arc;

use web_time::Instant;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy};
use winit::window::{Window, WindowId};

use crate::game::{Context, Frame};
use crate::gpu::Gpu;
use crate::time::FixedTimestep;
use crate::viewport::Viewport;
use crate::{Config, Error, Game, Vec2};

/// Sent to the event loop when GPU setup finishes. In browsers setup runs in
/// the background, so the result arrives as an event.
type GpuReady = Result<Gpu, Error>;

pub(crate) fn run<G: Game + 'static>(config: Config, game: G) -> Result<(), Error> {
    let event_loop = EventLoop::<GpuReady>::with_user_event().build()?;
    let timestep = FixedTimestep::new(config.updates_per_second);
    let (w, h) = config.resolution;
    let view_size = Vec2::new(w.max(1) as f32, h.max(1) as f32);
    // Placeholder until the window exists and reports its real size.
    let viewport = Viewport::fit(view_size, w.max(1), h.max(1));
    #[cfg_attr(target_arch = "wasm32", allow(unused_mut))]
    let mut app = App {
        ctx: Context::new(timestep.step(), viewport),
        frame: Frame::new(view_size),
        viewport,
        timestep,
        last_frame: Instant::now(),
        config,
        game,
        proxy: event_loop.create_proxy(),
        started: false,
        gpu: None,
        error: None,
    };

    #[cfg(not(target_arch = "wasm32"))]
    {
        event_loop.run_app(&mut app)?;
        match app.error {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }

    // Browsers own the event loop: hand the app over and return right away.
    #[cfg(target_arch = "wasm32")]
    {
        use winit::platform::web::EventLoopExtWebSys;
        std::panic::set_hook(Box::new(console_error_panic_hook::hook));
        event_loop.spawn_app(app);
        Ok(())
    }
}

struct App<G> {
    config: Config,
    game: G,
    ctx: Context,
    /// Reused every frame so its rectangle list keeps its allocation.
    frame: Frame,
    viewport: Viewport,
    timestep: FixedTimestep,
    last_frame: Instant,
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    proxy: EventLoopProxy<GpuReady>,
    /// True once the window was created and GPU setup started.
    started: bool,
    gpu: Option<Gpu>,
    /// A startup failure, returned from [`run`] after the loop exits.
    error: Option<Error>,
}

impl<G: Game> App<G> {
    fn create_window(&self, event_loop: &ActiveEventLoop) -> Result<Arc<Window>, Error> {
        let attrs = Window::default_attributes().with_title(&self.config.title);

        #[cfg(not(target_arch = "wasm32"))]
        let attrs = attrs.with_inner_size(winit::dpi::LogicalSize::new(
            self.config.width,
            self.config.height,
        ));

        // In browsers, add a canvas to the page. Its size comes from the
        // page's CSS, so the window size in Config is not used.
        #[cfg(target_arch = "wasm32")]
        let attrs = {
            use winit::platform::web::WindowAttributesExtWebSys;
            attrs.with_append(true)
        };

        Ok(Arc::new(event_loop.create_window(attrs)?))
    }

    /// Starts GPU setup. On desktop this finishes before returning; in
    /// browsers the result arrives later through [`ApplicationHandler::user_event`].
    fn start_gpu(&mut self, event_loop: &ActiveEventLoop, window: Arc<Window>) {
        let setup = Gpu::new(window, event_loop.owned_display_handle());

        #[cfg(not(target_arch = "wasm32"))]
        self.gpu_ready(event_loop, pollster::block_on(setup));

        #[cfg(target_arch = "wasm32")]
        {
            let proxy = self.proxy.clone();
            wasm_bindgen_futures::spawn_local(async move {
                // Fails only if the event loop is gone, and then nobody needs it.
                let _ = proxy.send_event(setup.await);
            });
        }
    }

    fn gpu_ready(&mut self, event_loop: &ActiveEventLoop, result: GpuReady) {
        match result {
            Ok(mut gpu) => {
                // The window may have been resized while setup ran.
                let size = gpu.window().inner_size();
                gpu.resize(size.width, size.height);
                let (w, h) = gpu.size();
                self.set_viewport(w, h);
                gpu.window().request_redraw();
                self.gpu = Some(gpu);
                // Don't count window/GPU setup time as game time.
                self.last_frame = Instant::now();
            }
            Err(e) => {
                // In browsers `run` has already returned, so the log is the
                // only place this error can go.
                log::error!("{e}");
                self.error = Some(e);
                event_loop.exit();
            }
        }
    }

    fn set_viewport(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return; // minimized; keep the last good viewport
        }
        self.viewport = Viewport::fit(self.ctx.view_size, width, height);
        self.ctx.input.set_viewport(self.viewport);
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

        self.frame.reset();
        self.frame.interpolation = self.timestep.interpolation();
        self.game.draw(&mut self.frame);
        if let Some(gpu) = self.gpu.as_mut() {
            gpu.render(&self.frame, &self.viewport);
            // Ask for the next frame right away; vsync paces the loop.
            gpu.window().request_redraw();
        }
    }
}

impl<G: Game> ApplicationHandler<GpuReady> for App<G> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        // Desktop and web call this once at startup; only set up the first time.
        if self.started {
            return;
        }
        self.started = true;
        match self.create_window(event_loop) {
            Ok(window) => self.start_gpu(event_loop, window),
            Err(e) => self.gpu_ready(event_loop, Err(e)),
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, result: GpuReady) {
        self.gpu_ready(event_loop, result);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let Some(gpu) = self.gpu.as_mut() else {
            return;
        };
        self.ctx.input.handle_event(&event);
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                gpu.resize(size.width, size.height);
                self.set_viewport(size.width, size.height);
            }
            WindowEvent::RedrawRequested => self.frame(event_loop),
            _ => {}
        }
    }
}
