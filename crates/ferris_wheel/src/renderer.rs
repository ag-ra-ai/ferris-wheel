//! Batched rectangle drawing: every rectangle in a frame goes into one
//! instance buffer and is drawn with a single draw call.

use crate::game::Quad;
use crate::viewport::Viewport;
use crate::{Color, Frame, Rect};

/// Bytes per instance: pos (2 f32) + size (2 f32) + color (4 f32).
const INSTANCE_SIZE: u64 = 8 * 4;
/// Instance capacity allocated up front; the buffer grows when needed.
const INITIAL_CAPACITY: u64 = 1024;

pub(crate) struct QuadRenderer {
    pipeline: wgpu::RenderPipeline,
    globals: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    instances: wgpu::Buffer,
    /// Capacity of `instances`, in instances.
    capacity: u64,
    /// CPU-side staging for instance data, reused every frame.
    staging: Vec<u8>,
    /// Number of instances written by the last `prepare`.
    count: u32,
    /// True when the target expects linear colors (sRGB surface).
    linear_colors: bool,
}

impl QuadRenderer {
    pub(crate) fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("quad shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/quad.wgsl").into()),
        });

        let globals = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("quad globals"),
            size: 16, // vec2 view_size + vec2 padding
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("quad globals layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("quad globals"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: globals.as_entire_binding(),
            }],
        });

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("quad pipeline layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("quad pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: INSTANCE_SIZE,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &wgpu::vertex_attr_array![
                        0 => Float32x2, // pos
                        1 => Float32x2, // size
                        2 => Float32x4, // color
                    ],
                })],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        Self {
            pipeline,
            globals,
            bind_group,
            instances: create_instance_buffer(device, INITIAL_CAPACITY),
            capacity: INITIAL_CAPACITY,
            staging: Vec::new(),
            count: 0,
            linear_colors: format.is_srgb(),
        }
    }

    /// Uploads this frame's rectangles to the GPU. Call before the render pass.
    pub(crate) fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        frame: &Frame,
        viewport: &Viewport,
    ) {
        let view = viewport.view_size;
        let mut globals = [0u8; 16];
        globals[0..4].copy_from_slice(&view.x.to_le_bytes());
        globals[4..8].copy_from_slice(&view.y.to_le_bytes());
        queue.write_buffer(&self.globals, 0, &globals);

        self.staging.clear();
        // The clear color is drawn as a rectangle covering the whole view,
        // so the letterbox bars around it stay black.
        let background = Quad {
            rect: Rect::new(0.0, 0.0, view.x, view.y),
            color: frame.clear_color,
        };
        for quad in std::iter::once(&background).chain(&frame.quads) {
            let color = if self.linear_colors {
                quad.color.to_linear()
            } else {
                quad.color
            };
            write_instance(&mut self.staging, quad.rect, color);
        }

        let count = self.staging.len() as u64 / INSTANCE_SIZE;
        if count > self.capacity {
            self.capacity = count.next_power_of_two();
            self.instances = create_instance_buffer(device, self.capacity);
        }
        queue.write_buffer(&self.instances, 0, &self.staging);
        self.count = count as u32;
    }

    /// Records the draw call into `pass`.
    pub(crate) fn draw(&self, pass: &mut wgpu::RenderPass<'_>, viewport: &Viewport) {
        let size = viewport.physical_size();
        pass.set_viewport(
            viewport.offset.x,
            viewport.offset.y,
            size.x,
            size.y,
            0.0,
            1.0,
        );
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.set_vertex_buffer(0, self.instances.slice(..));
        pass.draw(0..6, 0..self.count);
    }
}

fn create_instance_buffer(device: &wgpu::Device, capacity: u64) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("quad instances"),
        size: capacity * INSTANCE_SIZE,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

/// Appends one instance in the layout `quad.wgsl` expects.
fn write_instance(out: &mut Vec<u8>, rect: Rect, color: Color) {
    let values = [
        rect.pos.x,
        rect.pos.y,
        rect.size.x,
        rect.size.y,
        color.r,
        color.g,
        color.b,
        color.a,
    ];
    for v in values {
        out.extend_from_slice(&v.to_le_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Vec2;

    #[test]
    fn instance_layout_matches_stride() {
        let mut out = Vec::new();
        write_instance(&mut out, Rect::new(1.0, 2.0, 3.0, 4.0), Color::WHITE);
        assert_eq!(out.len() as u64, INSTANCE_SIZE);
        assert_eq!(&out[0..4], &1.0f32.to_le_bytes());
        assert_eq!(&out[12..16], &4.0f32.to_le_bytes());
        assert_eq!(&out[28..32], &1.0f32.to_le_bytes());
    }

    /// Draws `frame` into an offscreen sRGB texture with the real renderer and
    /// returns its RGBA8 pixels. Needs a GPU (or a software one like lavapipe).
    fn render_offscreen(frame: &Frame, viewport: &Viewport, width: u32, height: u32) -> Vec<u8> {
        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let adapter = pollster::block_on(instance.request_adapter(&Default::default())).expect(
            "no GPU adapter found; install a Vulkan or GL driver (e.g. mesa-vulkan-drivers)",
        );
        let (device, queue) =
            pollster::block_on(adapter.request_device(&Default::default())).unwrap();

        let format = wgpu::TextureFormat::Rgba8UnormSrgb;
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        // Rows must be 256-byte aligned for the copy; tests use width 64.
        let bytes_per_row = width * 4;
        assert_eq!(bytes_per_row % wgpu::COPY_BYTES_PER_ROW_ALIGNMENT, 0);
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: u64::from(bytes_per_row * height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let mut renderer = QuadRenderer::new(&device, format);
        renderer.prepare(&device, &queue, frame, viewport);
        let mut encoder = device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            renderer.draw(&mut pass, viewport);
        }
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(bytes_per_row),
                    rows_per_image: None,
                },
            },
            size,
        );
        queue.submit([encoder.finish()]);

        readback.map_async(wgpu::MapMode::Read, .., |r| r.unwrap());
        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        readback.get_mapped_range(..).unwrap().to_vec()
    }

    fn pixel(pixels: &[u8], width: u32, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * width + x) * 4) as usize;
        pixels[i..i + 4].try_into().unwrap()
    }

    /// Compares RGB with a tolerance of 1 for GPU rounding.
    fn assert_rgb(actual: [u8; 4], expected: u32, what: &str) {
        let expected = [
            (expected >> 16) as u8,
            (expected >> 8) as u8,
            expected as u8,
        ];
        let close = actual[..3]
            .iter()
            .zip(expected)
            .all(|(a, e)| a.abs_diff(e) <= 1);
        assert!(close, "{what}: got {actual:?}, expected {expected:?}");
    }

    #[test]
    fn gpu_draws_letterbox_background_and_rects() {
        // A 32x32 view in a 64x32 target: scale 1, 16-pixel bars left and right.
        let (width, height) = (64, 32);
        let viewport = Viewport::fit(Vec2::new(32.0, 32.0), width, height);
        let mut frame = Frame::new(viewport.view_size);
        frame.clear(Color::hex(0x2e3440));
        frame.fill_rect(Rect::new(8.0, 8.0, 8.0, 8.0), Color::hex(0xbf616a));
        frame.fill_rect(Rect::new(12.0, 12.0, 8.0, 8.0), Color::hex(0xa3be8c));

        let px = render_offscreen(&frame, &viewport, width, height);
        let at = |x, y| pixel(&px, width, x, y);

        assert_rgb(at(2, 16), 0x000000, "left bar");
        assert_rgb(at(61, 16), 0x000000, "right bar");
        assert_rgb(at(16, 0), 0x2e3440, "view top-left");
        assert_rgb(at(47, 31), 0x2e3440, "view bottom-right");
        assert_rgb(at(16 + 9, 9), 0xbf616a, "first rect");
        // The second rect is drawn later, so it covers the overlap.
        assert_rgb(at(16 + 14, 14), 0xa3be8c, "overlap");
        assert_rgb(at(16 + 18, 18), 0xa3be8c, "second rect");
    }
}
