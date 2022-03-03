struct QuadDrawInfo {
    color: vec4<f32>;
    scale: vec2<f32>;
    position: vec2<f32>;
    orientation: vec2<f32>;
};

struct QuadDrawInfos{
    draw_infos: array<QuadDrawInfo, 1024>;
};

[[group(0), binding(0)]]
var<uniform> quad_draw_infos: QuadDrawInfos;

struct VertexOutput {
    [[builtin(position)]] clip_position: vec4<f32>;
    [[location(0)]] color: vec4<f32>;
};

fn vertex_index_to_corner(index: u32) -> vec2<f32> {
    let i: u32 = index & u32(3);

    switch (i) {
        case 0: { return vec2<f32>(-0.5, -0.5); }
        case 1: { return vec2<f32>( 0.5, -0.5); }
        case 2: { return vec2<f32>(-0.5,  0.5); }
        case 3: { return vec2<f32>( 0.5,  0.5); }
        default: { return vec2<f32>(0.0,0.0); }
    }
}

[[stage(vertex)]]
fn vs_main(
    [[builtin(vertex_index)]] in_vertex_index: u32, 
) -> VertexOutput {
    let info_index = in_vertex_index / u32(4);
    let info: QuadDrawInfo = quad_draw_infos.draw_infos[info_index];

    var out: VertexOutput;
    let pos_2d = vertex_index_to_corner(in_vertex_index);
    out.clip_position.x = pos_2d.x * info.scale.x + info.position.x;
    out.clip_position.y = pos_2d.y * info.scale.x + info.position.y;
    out.clip_position.z = 0.5;
    out.clip_position.w = 1.0;
    out.color = info.color;
    
    return out;
}

[[stage(fragment)]]
fn fs_main(in: VertexOutput) -> [[location(0)]] vec4<f32> {
    return in.color;
}