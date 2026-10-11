// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

// a portal face, the linked view in the stencil hole with PortalRefract stage 2 over it
// at the last level it is the depth doubler instead, the screen copy masked by portal_mask
#import bevy_pbr::forward_io::VertexOutput
#import bevy_pbr::mesh_view_bindings::view
#import portalgun::face::{ellipse, noise_coords, overlay, coverage, opening, radius}
#ifdef TONEMAP_IN_SHADER
#import bevy_core_pipeline::tonemapping::tone_mapping
#endif

struct PortalParams {
    open: f32,
    // one minus the static amount
    keep: f32,
    scale: f32,
    // seconds wrapped every thousand
    time: f32,
    // 1 when the hole shows the linked view
    hole: f32,
    // 1 when the face is the depth doubler
    doubler: f32,
    // 1 for the near plane's cut through the hole, which shows the view
    fix: f32,
    doubler_clip_from_world: mat4x4<f32>,
};

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> params: PortalParams;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var noise_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var noise_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(3) var ramp_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(4) var ramp_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(5) var view_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(6) var view_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(7) var mask_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(8) var mask_sampler: sampler;

// the linked views are drawn untonemapped so a view drawn for the window gets it once
fn seen(uv: vec2<f32>) -> vec3<f32> {
    let c = textureSampleLevel(view_texture, view_sampler, uv, 0.0);
#ifdef TONEMAP_IN_SHADER
    return tone_mapping(c, view.color_grading).rgb;
#else
    return c.rgb;
#endif
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    if params.fix > 0.5 {
        return vec4<f32>(seen(in.position.xy / view.viewport.zw), 1.0);
    }
    if params.doubler > 0.5 {
        let clip = params.doubler_clip_from_world * vec4<f32>(in.world_position.xyz, 1.0);
        let at = saturate(vec2<f32>(clip.x + clip.w, clip.w - clip.y) / (2.0 * clip.w));
        let a = textureSample(mask_texture, mask_sampler, in.uv).a;
        return vec4<f32>(seen(at) * a, a);
    }
    let t0 = ellipse(in.uv);
    let t5 = noise_coords(in.uv, params.open, params.time);
    let o = overlay(params.open, params.keep, t0, t5, noise_texture, noise_sampler);
    let ramp = textureSampleLevel(ramp_texture, ramp_sampler, vec2<f32>(o.u, o.u), 0.0).rgb;
    let a = coverage(o.alpha);
    // stage 2 writes linear through hardware srgb, as the output here does
    let rim = saturate(ramp * params.scale) * a;
    let s = opening(params.open);
    let hole = select(0.0, params.hole, s * s - radius(t0) >= 0.0);
    let behind = seen(in.position.xy / view.viewport.zw) * hole;
    return vec4<f32>(rim + behind * (1.0 - a), a + hole * (1.0 - a));
}
