#import bevy_pbr::mesh_functions
#import bevy_pbr::mesh_view_bindings::view
#ifdef DEPTH_PREPASS
#import bevy_pbr::prepass_utils::prepass_depth
#endif
#ifdef TONEMAP_IN_SHADER
#import bevy_core_pipeline::tonemapping::tone_mapping
#endif

struct Params { flags: vec4<f32>, card: vec4<f32>, depth: vec4<f32> };
@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> params: Params;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var base: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var base_sampler: sampler;
struct Vertex {
    @builtin(instance_index) instance: u32,
    @location(0) position: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) uv_next: vec2<f32>,
    @location(3) color: vec4<f32>,
    @location(4) blend: f32,
};
struct Output {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) uv_next: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) blend: f32,
    @location(4) depth: f32,
};
@vertex
fn vertex(v: Vertex) -> Output {
    let world = mesh_functions::get_world_from_local(v.instance) * vec4(v.position, 1.0);
    var out: Output;
    out.position = view.clip_from_world * world;
    out.uv = v.uv;
    out.uv_next = v.uv_next;
    out.blend = v.blend;
    out.color = v.color;
    if params.flags.z > 0.5 { out.color = vec4(pow(max(v.color.rgb, vec3(0.0)), vec3(2.2)), v.color.a); }
    out.depth = -(view.view_from_world * world).z / 0.0254;
    return out;
}
@fragment
fn fragment(in: Output) -> @location(0) vec4<f32> {
    let a = textureSample(base, base_sampler, in.uv);
    let b = textureSample(base, base_sampler, in.uv_next);
    let t = mix(a, b, in.blend * params.card.w);
    var va = mix(1.0, in.color.a, params.flags.y);
#ifdef DEPTH_PREPASS
    if params.depth.x > 0.5 {
        let depth = prepass_depth(in.position, 0u);
        let xy = (in.position.xy - view.viewport.xy) / view.viewport.zw;
        let p = view.view_from_clip * vec4(xy.x * 2.0 - 1.0, 1.0 - xy.y * 2.0, depth, 1.0);
        let scene = -p.z / p.w / 0.0254;
        let e = saturate((scene / 192.0 - 0.75) * 4.0);
        va *= saturate(max(e * e * (3.0 - 2.0 * e), abs(scene - in.depth) / params.depth.y));
    }
#endif
    let alpha = t.a * va;
    if alpha <= params.card.z { discard; }
    var rgb = t.rgb * params.card.x;
    if params.card.y != 0.0 { rgb *= alpha * (1.0 + params.card.y * params.card.x * va); }
    rgb *= mix(vec3(1.0), in.color.rgb, params.flags.x);
    if params.flags.w > 0.5 { rgb = pow(max(rgb, vec3(0.0)), vec3(2.2)); }
#ifdef TONEMAP_IN_SHADER
    rgb = tone_mapping(vec4(rgb, alpha), view.color_grading).rgb;
#endif
    return vec4(rgb, alpha);
}
