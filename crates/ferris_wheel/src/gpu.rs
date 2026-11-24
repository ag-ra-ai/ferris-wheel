//! GPU setup and per-frame presentation with wgpu.

use std::sync::Arc;

use winit::window::Window;

use crate::{Color, Error};

/// Everything needed to draw into the window.
pub(crate) struct Gpu {
    instance: wgpu::Instance,
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    window: Arc<Window>,
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

        Ok(Self {
            instance,
            device,
            queue,
            surface,
            config,
            window,
        })
    }

    pub(crate) fn window(&self) -> &Window {
        &self.window
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

    /// Clears the window to `clear` and shows the result.
    pub(crate) fn render(&mut self, clear: Color) {
        let size = self.window.inner_size();
        if size.width == 0 || size.height == 0 {
            return;
        }

        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame) => frame,
            wgpu::CurrentSurfaceTexture::Suboptimal(frame) => {
                // Draw this frame, then reconfigure for the next one.
                self.draw_and_present(frame, clear);
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
        self.draw_and_present(frame, clear);
    }

    fn draw_and_present(&self, frame: wgpu::SurfaceTexture, clear: Color) {
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        // An sRGB surface expects linear values and converts them on write.
        let clear = if self.config.format.is_srgb() {
            clear.to_linear()
        } else {
            clear
        };

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("frame encoder"),
            });
        {
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("clear pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(clear.into()),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
        }

        self.queue.submit([encoder.finish()]);
        self.window.pre_present_notify();
        self.queue.present(frame);
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
