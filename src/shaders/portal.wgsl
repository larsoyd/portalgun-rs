// a portal's rim and static from PortalRefract stage 2, premultiplied
#import bevy_pbr::forward_io::VertexOutput
#import portalgun::face::{ellipse, noise_coords, overlay, overlay_rgb, coverage}

struct PortalParams {
    open: f32,
    // one minus the static amount
    keep: f32,
    scale: f32,
    // seconds wrapped every thousand
    time: f32,
};

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> params: PortalParams;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var noise_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var noise_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(3) var ramp_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(4) var ramp_sampler: sampler;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let t0 = ellipse(in.uv);
    let t5 = noise_coords(in.uv, params.open, params.time);
    let o = overlay(params.open, params.keep, t0, t5, noise_texture, noise_sampler);
    let ramp = textureSampleLevel(ramp_texture, ramp_sampler, vec2<f32>(o.u, o.u), 0.0).rgb;
    let a = coverage(o.alpha);
    return vec4<f32>(overlay_rgb(ramp, params.scale) * a, a);
}
