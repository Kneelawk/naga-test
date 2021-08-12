fn complex_add(a: vec2<f32>, b: vec2<f32>) -> vec2<f32> {
    return a + b;
}

fn complex_multiply(a: vec2<f32>, b: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(a.x * b.x - a.y * b.y, a.x * b.y + a.y * b.x);
}

struct FragmentData {
    [[builtin(position)]] position: vec4<f32>;
};

var indexable: array<vec2<f32>,6u> = array<vec2<f32>,6u>(
    vec2<f32>(1.0, 1.0), vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, -1.0),
    vec2<f32>(1.0, 1.0), vec2<f32>(-1.0, 1.0), vec2<f32>(-1.0, -1.0)
);

[[stage(vertex)]]
fn vert_main([[builtin(vertex_index)]] vert_index: u32) -> FragmentData {
    var data: FragmentData;
    let xy = indexable[vert_index];
    data.position = vec4<f32>(xy.x, xy.y, 0.0, 1.0);
    return data;
}

[[stage(fragment)]]
fn frag_main(data: FragmentData) -> [[location(0)]] vec4<f32> {
    return vec4<f32>(0.0, 0.1, 0.2, 1.0);
}
