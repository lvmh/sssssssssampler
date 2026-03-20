struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) glyph_idx: u32,
    @location(2) color: vec4<f32>,
    @location(3) uv_min: vec2<f32>,
    @location(4) uv_max: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
};

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    // Normalize to NDC [-1, 1]
    output.position = vec4<f32>(
        (input.position.x / 960.0) * 2.0 - 1.0,
        (input.position.y / 540.0) * 2.0 - 1.0,
        0.0,
        1.0,
    );
    output.uv = input.uv_min;
    output.color = input.color;
    return output;
}

@group(0) @binding(0)
var glyph_atlas: texture_2d<f32>;

@group(0) @binding(1)
var glyph_sampler: sampler;

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let atlas_color = textureSample(glyph_atlas, glyph_sampler, input.uv);
    // Simple blend: modulate atlas by color
    return atlas_color * input.color;
}
