//! Batched sprite and rectangle drawing.
//!
//! Every rectangle in a frame goes into one instance buffer. Consecutive
//! rectangles that use the same texture are drawn with one draw call; plain
//! colored rectangles use a shared 1x1 white texture, so they batch too.
//! Draw order is always preserved.

use std::collections::{HashMap, HashSet};
use std::ops::Range;
use std::sync::{Arc, Weak};

use crate::game::Quad;
use crate::texture::TextureData;
use crate::viewport::Viewport;
use crate::{Color, Frame, Rect};

/// Bytes per instance: pos, size, uv_pos, uv_size (2 f32 each) + color (4 f32).
const INSTANCE_SIZE: u64 = 12 * 4;
/// Instance capacity allocated up front; the buffer grows when needed.
const INITIAL_CAPACITY: u64 = 1024;

pub(crate) struct QuadRenderer {
    pipeline: wgpu::RenderPipeline,
    globals: wgpu::Buffer,
    globals_bind_group: wgpu::BindGroup,
    texture_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    /// Used for plain colored rectangles.
    white: GpuTexture,
    /// GPU copies of [`crate::Texture`]s, by texture id.
    textures: HashMap<u64, GpuTexture>,
    /// Textures that could not be uploaded; logged once, then skipped.
    failed: HashSet<u64>,
    instances: wgpu::Buffer,
    /// Capacity of `instances`, in instances.
    capacity: u64,
    /// CPU-side staging for instance data, reused every frame.
    staging: Vec<u8>,
    /// Draw calls for the current frame.
    batches: Vec<Batch>,
    /// True when the target expects linear colors (sRGB surface).
    linear_colors: bool,
}

struct GpuTexture {
    /// The engine texture this was made from; once it is dropped, so is this.
    owner: Weak<TextureData>,
    bind_group: wgpu::BindGroup,
}

/// A run of instances that share a texture (`None` = white).
#[derive(Debug, PartialEq)]
struct Batch {
    texture: Option<u64>,
    instances: Range<u32>,
}

impl QuadRenderer {
    pub(crate) fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
    ) -> Self {
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

        let globals_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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

        let globals_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("quad globals"),
            layout: &globals_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: globals.as_entire_binding(),
            }],
        });

        let texture_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("sprite texture layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        // Nearest-neighbour sampling keeps pixel art crisp when scaled up.
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("sprite sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("quad pipeline layout"),
            bind_group_layouts: &[Some(&globals_layout), Some(&texture_layout)],
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
                        2 => Float32x2, // uv_pos
                        3 => Float32x2, // uv_size
                        4 => Float32x4, // color
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

        let white = upload_texture(
            device,
            queue,
            &texture_layout,
            &sampler,
            Weak::new(),
            1,
            1,
            &[255; 4],
        );

        Self {
            pipeline,
            globals,
            globals_bind_group,
            texture_layout,
            sampler,
            white,
            textures: HashMap::new(),
            failed: HashSet::new(),
            instances: create_instance_buffer(device, INITIAL_CAPACITY),
            capacity: INITIAL_CAPACITY,
            staging: Vec::new(),
            batches: Vec::new(),
            linear_colors: format.is_srgb(),
        }
    }

    /// Uploads this frame's rectangles (and any new textures) to the GPU.
    /// Call before the render pass.
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

        // Free GPU textures whose engine texture has been dropped.
        self.textures.retain(|_, t| t.owner.strong_count() > 0);

        self.staging.clear();
        self.batches.clear();
        // The clear color is drawn as a rectangle covering the whole view,
        // so the letterbox bars around it stay black.
        let background = Quad {
            rect: Rect::new(0.0, 0.0, view.x, view.y),
            uv: Rect::new(0.0, 0.0, 1.0, 1.0),
            color: frame.clear_color,
            texture: None,
        };
        let mut count = 0;
        for quad in std::iter::once(&background).chain(&frame.quads) {
            let key = match &quad.texture {
                None => None,
                Some(texture) => {
                    if !self.ensure_uploaded(device, queue, texture.data()) {
                        continue;
                    }
                    Some(texture.id())
                }
            };
            let color = if self.linear_colors {
                quad.color.to_linear()
            } else {
                quad.color
            };
            write_instance(&mut self.staging, quad.rect, quad.uv, color);
            push_to_batch(&mut self.batches, key, count);
            count += 1;
        }

        let count = u64::from(count);
        if count > self.capacity {
            self.capacity = count.next_power_of_two();
            self.instances = create_instance_buffer(device, self.capacity);
        }
        queue.write_buffer(&self.instances, 0, &self.staging);
    }

    /// Records the draw calls into `pass`.
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
        pass.set_bind_group(0, &self.globals_bind_group, &[]);
        pass.set_vertex_buffer(0, self.instances.slice(..));
        for batch in &self.batches {
            let texture = match batch.texture {
                None => &self.white,
                // Always present: `prepare` uploaded it and nothing removes
                // textures between `prepare` and `draw`.
                Some(id) => &self.textures[&id],
            };
            pass.set_bind_group(1, &texture.bind_group, &[]);
            pass.draw(0..6, batch.instances.clone());
        }
    }

    /// Uploads `data` if it isn't on the GPU yet. Returns false if it can't be
    /// (too large for this GPU).
    fn ensure_uploaded(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        data: &Arc<TextureData>,
    ) -> bool {
        if self.textures.contains_key(&data.id) {
            return true;
        }
        if self.failed.contains(&data.id) {
            return false;
        }
        let max = device.limits().max_texture_dimension_2d;
        if data.width > max || data.height > max {
            log::error!(
                "texture is {}x{}, larger than this GPU's limit of {max}x{max}; not drawing it",
                data.width,
                data.height
            );
            self.failed.insert(data.id);
            return false;
        }
        let gpu = upload_texture(
            device,
            queue,
            &self.texture_layout,
            &self.sampler,
            Arc::downgrade(data),
            data.width,
            data.height,
            &data.pixels,
        );
        self.textures.insert(data.id, gpu);
        true
    }
}

#[allow(clippy::too_many_arguments)]
fn upload_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
    sampler: &wgpu::Sampler,
    owner: Weak<TextureData>,
    width: u32,
    height: u32,
    pixels: &[u8],
) -> GpuTexture {
    let size = wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("sprite texture"),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        // Image files store sRGB; the GPU converts to linear when sampling.
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        pixels,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(width * 4),
            rows_per_image: None,
        },
        size,
    );
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    // The bind group keeps the view, and through it the texture, alive.
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("sprite texture"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
        ],
    });
    GpuTexture { owner, bind_group }
}

/// Adds instance `index` to the last batch if it uses the same texture,
/// otherwise starts a new batch.
fn push_to_batch(batches: &mut Vec<Batch>, texture: Option<u64>, index: u32) {
    match batches.last_mut() {
        Some(last) if last.texture == texture && last.instances.end == index => {
            last.instances.end += 1;
        }
        _ => batches.push(Batch {
            texture,
            instances: index..index + 1,
        }),
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
fn write_instance(out: &mut Vec<u8>, rect: Rect, uv: Rect, color: Color) {
    let values = [
        rect.pos.x,
        rect.pos.y,
        rect.size.x,
        rect.size.y,
        uv.pos.x,
        uv.pos.y,
        uv.size.x,
        uv.size.y,
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
    use crate::{Font, Scaling, SpriteParams, TextParams, Texture, Vec2};

    #[test]
    fn instance_layout_matches_stride() {
        let mut out = Vec::new();
        let uv = Rect::new(0.5, 0.0, 0.25, 1.0);
        write_instance(&mut out, Rect::new(1.0, 2.0, 3.0, 4.0), uv, Color::WHITE);
        assert_eq!(out.len() as u64, INSTANCE_SIZE);
        assert_eq!(&out[0..4], &1.0f32.to_le_bytes()); // pos.x
        assert_eq!(&out[12..16], &4.0f32.to_le_bytes()); // size.y
        assert_eq!(&out[16..20], &0.5f32.to_le_bytes()); // uv_pos.x
        assert_eq!(&out[44..48], &1.0f32.to_le_bytes()); // color.a
    }

    #[test]
    fn consecutive_same_texture_draws_share_a_batch() {
        let mut batches = Vec::new();
        for (i, key) in [None, None, Some(1), Some(1), None, Some(2)]
            .into_iter()
            .enumerate()
        {
            push_to_batch(&mut batches, key, i as u32);
        }
        let expected = [(None, 0..2), (Some(1), 2..4), (None, 4..5), (Some(2), 5..6)];
        let expected: Vec<_> = expected
            .into_iter()
            .map(|(texture, instances)| Batch { texture, instances })
            .collect();
        assert_eq!(batches, expected);
    }

    /// A GPU device for tests. Needs a GPU or a software driver (lavapipe).
    fn test_gpu() -> (wgpu::Device, wgpu::Queue) {
        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let adapter = pollster::block_on(instance.request_adapter(&Default::default())).expect(
            "no GPU adapter found; install a Vulkan or GL driver (e.g. mesa-vulkan-drivers)",
        );
        pollster::block_on(adapter.request_device(&Default::default())).unwrap()
    }

    /// Draws `frame` into an offscreen sRGB texture with the real renderer and
    /// returns its RGBA8 pixels. Needs a GPU (or a software one like lavapipe).
    fn render_offscreen(frame: &Frame, viewport: &Viewport, width: u32, height: u32) -> Vec<u8> {
        let (device, queue) = test_gpu();

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

        let mut renderer = QuadRenderer::new(&device, &queue, format);
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
        let viewport = Viewport::fit(Vec2::new(32.0, 32.0), width, height, Scaling::Fit);
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

    #[test]
    fn gpu_draws_sprites_with_nearest_sampling_flip_region_and_tint() {
        let (width, height) = (64, 32);
        let viewport = Viewport::fit(Vec2::new(32.0, 32.0), width, height, Scaling::Fit);
        // 2x2 texture: red, green / blue, white.
        #[rustfmt::skip]
        let pixels = vec![
            255, 0, 0, 255,   0, 255, 0, 255,
            0, 0, 255, 255,   255, 255, 255, 255,
        ];
        let texture = Texture::from_rgba8(2, 2, pixels).unwrap();

        let mut frame = Frame::new(viewport.view_size);
        frame.draw_sprite(&texture, Rect::new(0.0, 0.0, 8.0, 8.0));
        let flipped = SpriteParams {
            flip_x: true,
            ..Default::default()
        };
        frame.draw_sprite_with(&texture, Rect::new(8.0, 0.0, 8.0, 8.0), flipped);
        let white_corner_tinted = SpriteParams {
            source: Some(Rect::new(1.0, 1.0, 1.0, 1.0)),
            tint: Color::hex(0x808080),
            ..Default::default()
        };
        frame.draw_sprite_with(&texture, Rect::new(0.0, 8.0, 8.0, 8.0), white_corner_tinted);
        // A plain rect after sprites still draws (white texture batch).
        frame.fill_rect(Rect::new(8.0, 8.0, 8.0, 8.0), Color::hex(0xebcb8b));

        let px = render_offscreen(&frame, &viewport, width, height);
        let at = |x: u32, y: u32| pixel(&px, width, 16 + x, y);

        // Each texel becomes a crisp 4x4 block.
        assert_rgb(at(1, 1), 0xff0000, "top-left texel");
        assert_rgb(at(6, 1), 0x00ff00, "top-right texel");
        assert_rgb(at(1, 6), 0x0000ff, "bottom-left texel");
        assert_rgb(at(6, 6), 0xffffff, "bottom-right texel");
        assert_rgb(at(3, 1), 0xff0000, "no blending inside a texel");
        assert_rgb(at(9, 1), 0x00ff00, "flipped: green on the left");
        assert_rgb(at(14, 1), 0xff0000, "flipped: red on the right");
        assert_rgb(at(4, 12), 0x808080, "tinted white region");
        assert_rgb(at(12, 12), 0xebcb8b, "rect after sprites");
    }

    #[test]
    fn gpu_draws_builtin_text_pixel_exact() {
        let (width, height) = (64, 32);
        let viewport = Viewport::fit(Vec2::new(32.0, 32.0), width, height, Scaling::Fit);
        let mut frame = Frame::new(viewport.view_size);
        frame.clear(Color::BLACK);
        // "H" at scale 1: columns 0 and 4 lit in every row, row 3 fully lit.
        frame.draw_text("H", Vec2::new(0.0, 0.0), Color::hex(0xebcb8b));
        // "T" at scale 2: top row (2 units tall) fully lit, stem in columns 4-5.
        let big = TextParams {
            color: Color::WHITE,
            scale: 2.0,
        };
        frame.draw_text_with(&Font::builtin(), "T", Vec2::new(8.0, 0.0), big);

        let px = render_offscreen(&frame, &viewport, width, height);
        let at = |x: u32, y: u32| pixel(&px, width, 16 + x, y);

        for y in 0..7 {
            assert_rgb(at(0, y), 0xebcb8b, "H left stem");
            assert_rgb(at(4, y), 0xebcb8b, "H right stem");
        }
        assert_rgb(at(2, 0), 0x000000, "H gap above bar");
        assert_rgb(at(2, 3), 0xebcb8b, "H crossbar");
        assert_rgb(at(2, 7), 0x000000, "H descender row is empty");
        assert_rgb(at(5, 3), 0x000000, "spacing after H");

        for x in 8..18 {
            assert_rgb(at(x, 0), 0xffffff, "T top bar");
            assert_rgb(at(x, 1), 0xffffff, "T top bar, 2nd row at scale 2");
        }
        assert_rgb(at(12, 10), 0xffffff, "T stem");
        assert_rgb(at(13, 10), 0xffffff, "T stem is 2 wide");
        assert_rgb(at(11, 10), 0x000000, "left of T stem");
    }

    #[test]
    fn gpu_pixel_perfect_draws_every_texel_the_same_size() {
        // A 16x16 view in a 64x40 target: Fit would scale 2.5x; pixel-perfect
        // uses 2x, centered at (16, 4).
        let (width, height) = (64, 40);
        let viewport = Viewport::fit(Vec2::new(16.0, 16.0), width, height, Scaling::PixelPerfect);
        assert_eq!(
            (viewport.scale, viewport.offset),
            (2.0, Vec2::new(16.0, 4.0))
        );

        // 4x1 texture: red, green, blue, white, drawn 1 world unit per texel.
        #[rustfmt::skip]
        let pixels = vec![
            255, 0, 0, 255,   0, 255, 0, 255,   0, 0, 255, 255,   255, 255, 255, 255,
        ];
        let texture = Texture::from_rgba8(4, 1, pixels).unwrap();
        let mut frame = Frame::new(viewport.view_size);
        frame.clear(Color::hex(0x2e3440));
        frame.draw_sprite(&texture, Rect::new(0.0, 0.0, 4.0, 1.0));

        let px = render_offscreen(&frame, &viewport, width, height);
        let at = |x: u32, y: u32| pixel(&px, width, x, y);

        assert_rgb(at(15, 4), 0x000000, "left border");
        assert_rgb(at(16, 3), 0x000000, "top border");
        assert_rgb(at(48, 4), 0x000000, "right border");
        let expected = [0xff0000, 0x00ff00, 0x0000ff, 0xffffff];
        for (i, &color) in expected.iter().enumerate() {
            let x0 = 16 + 2 * i as u32;
            // Exactly a 2x2 block of screen pixels per texel.
            for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                assert_rgb(at(x0 + dx, 4 + dy), color, "texel block");
            }
        }
        assert_rgb(at(16, 6), 0x2e3440, "background below the sprite");
    }

    #[test]
    fn gpu_textures_are_freed_after_drop() {
        let (device, queue) = test_gpu();
        let format = wgpu::TextureFormat::Rgba8UnormSrgb;
        let mut renderer = QuadRenderer::new(&device, &queue, format);
        let viewport = Viewport::fit(Vec2::new(32.0, 32.0), 32, 32, Scaling::Fit);

        let texture = Texture::from_rgba8(1, 1, vec![255; 4]).unwrap();
        let mut frame = Frame::new(viewport.view_size);
        frame.draw_sprite(&texture, Rect::new(0.0, 0.0, 4.0, 4.0));
        frame.draw_sprite(&texture, Rect::new(4.0, 0.0, 4.0, 4.0));
        renderer.prepare(&device, &queue, &frame, &viewport);
        assert_eq!(
            renderer.textures.len(),
            1,
            "uploaded once, shared by both draws"
        );

        drop(texture);
        frame.reset(); // the frame held the last clones
        renderer.prepare(&device, &queue, &frame, &viewport);
        assert!(renderer.textures.is_empty());
    }
}
