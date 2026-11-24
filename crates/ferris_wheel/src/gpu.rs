//! GPU setup and per-frame presentation with wgpu.

use std::sync::Arc;

use winit::window::Window;

use crate::renderer::QuadRenderer;
use crate::viewport::Viewport;
use crate::{Error, Frame};

/// Everything needed to draw into the window.
pub(crate) struct Gpu {
    instance: wgpu::Instance,
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    window: Arc<Window>,
    renderer: QuadRenderer,
}

impl Gpu {
    pub(crate) fn new(instance: wgpu::Instance, window: Arc<Window>) -> Result<Self, Error> {
        let surface = instance
            .create_surface(window.clone())
            .map_err(|e| Error::Gpu(e.to_string()))?;

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::default(),
            force_fallback_adapter: false,
            compatible_surface: Some(&surface),
            // Bucketing hides exact GPU limits from untrusted code; not needed here.
            apply_limit_buckets: false,
        }))
        .map_err(|e| Error::Gpu(e.to_string()))?;

        let info = adapter.get_info();
        log::info!("using GPU: {} ({:?})", info.name, info.backend);

        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("ferris_wheel device"),
            // Downlevel limits keep us compatible with older GPUs and GLES.
            required_limits:
                wgpu::Limits::downlevel_webgl2_defaults().using_resolution(adapter.limits()),
            ..Default::default()
        }))
        .map_err(|e| Error::Gpu(e.to_string()))?;

        let size = window.inner_size();
        let mut config = surface
            .get_default_config(&adapter, size.width.max(1), size.height.max(1))
            .ok_or_else(|| Error::Gpu("the GPU cannot draw to this window".into()))?;

        // Prefer an sRGB format so the GPU handles gamma for us.
        let caps = surface.get_capabilities(&adapter);
        if let Some(srgb) = caps.formats.iter().copied().find(|f| f.is_srgb()) {
            config.format = srgb;
        }
        // Fifo (vsync) is the one present mode every platform must support.
        config.present_mode = wgpu::PresentMode::Fifo;
        surface.configure(&device, &config);
        let renderer = QuadRenderer::new(&device, &queue, config.format);

        Ok(Self {
            instance,
            device,
            queue,
            surface,
            config,
            window,
            renderer,
        })
    }

    pub(crate) fn window(&self) -> &Window {
        &self.window
    }

    /// Current drawing surface size in physical pixels.
    pub(crate) fn size(&self) -> (u32, u32) {
        (self.config.width, self.config.height)
    }

    /// Updates the drawing surface to match a new window size.
    pub(crate) fn resize(&mut self, width: u32, height: u32) {
        // A minimized window can report 0x0, which the surface cannot use.
        if width == 0 || height == 0 {
            return;
        }
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
    }

    /// Draws `frame` into the window and shows the result.
    pub(crate) fn render(&mut self, frame: &Frame, viewport: &Viewport) {
        let size = self.window.inner_size();
        if size.width == 0 || size.height == 0 {
            return;
        }

        let target = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(target) => target,
            wgpu::CurrentSurfaceTexture::Suboptimal(target) => {
                // Draw this frame, then reconfigure for the next one.
                self.draw_and_present(target, frame, viewport);
                self.surface.configure(&self.device, &self.config);
                return;
            }
            wgpu::CurrentSurfaceTexture::Outdated => {
                self.surface.configure(&self.device, &self.config);
                return;
            }
            wgpu::CurrentSurfaceTexture::Lost => {
                self.recreate_surface();
                return;
            }
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                return;
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                log::error!("validation error while getting the next frame");
                return;
            }
        };
        self.draw_and_present(target, frame, viewport);
    }

    fn draw_and_present(
        &mut self,
        target: wgpu::SurfaceTexture,
        frame: &Frame,
        viewport: &Viewport,
    ) {
        let view = target
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        self.renderer
            .prepare(&self.device, &self.queue, frame, viewport);

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("frame encoder"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("main pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        // Black fills the letterbox bars; the view itself is
                        // covered by the frame's clear color.
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            self.renderer.draw(&mut pass, viewport);
        }

        self.queue.submit([encoder.finish()]);
        self.window.pre_present_notify();
        self.queue.present(target);
    }

    fn recreate_surface(&mut self) {
        log::warn!("surface lost, recreating it");
        match self.instance.create_surface(self.window.clone()) {
            Ok(surface) => {
                self.surface = surface;
                self.surface.configure(&self.device, &self.config);
            }
            Err(e) => log::error!("could not recreate surface: {e}"),
        }
    }
}
