// Draws axis-aligned colored rectangles, one instance per rectangle.
// Each instance expands to 6 vertices (two triangles) in the vertex shader,
// so no vertex buffer is needed, only the per-instance data.

struct Globals {
    // Size of the game view in world units.
    view_size: vec2<f32>,
    _padding: vec2<f32>,
};

@group(0) @binding(0)
var<uniform> globals: Globals;

struct Instance {
    @location(0) pos: vec2<f32>,   // top-left corner, world units
    @location(1) size: vec2<f32>,  // world units
    @location(2) color: vec4<f32>, // linear RGBA
};

struct VertexOut {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vertex: u32, instance: Instance) -> VertexOut {
    var corners = array<vec2<f32>, 6>(
        vec2(0.0, 0.0), vec2(1.0, 0.0), vec2(0.0, 1.0),
        vec2(0.0, 1.0), vec2(1.0, 0.0), vec2(1.0, 1.0),
    );
    let world = instance.pos + corners[vertex] * instance.size;

    // World (0,0)..view_size, y down  ->  clip space (-1,1)..(1,-1), y up.
    let unit = world / globals.view_size;
    let clip = vec2(unit.x * 2.0 - 1.0, 1.0 - unit.y * 2.0);

    var out: VertexOut;
    out.clip_position = vec4(clip, 0.0, 1.0);
    out.color = instance.color;
    return out;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    return in.color;
}
