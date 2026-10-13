// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

#import bevy_pbr::{
    forward_io::{VertexOutput, FragmentOutput},
    mesh_view_bindings as view_bindings,
    mesh_view_types,
    mesh_types::MESH_FLAGS_SHADOW_RECEIVER_BIT,
    pbr_bindings,
    pbr_types::PbrInput,
    clustered_forward as clustering,
    shadows,
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{alpha_discard, apply_pbr_lighting, main_pass_post_lighting_processing},
}

@group(#{MATERIAL_BIND_GROUP}) @binding(100) var<uniform> plane: vec4<f32>;

@group(#{MATERIAL_BIND_GROUP}) @binding(101) var<uniform> phong: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(102) var<uniform> model_flags: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(103) var exponent_map: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(104) var exponent_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(105) var lightwarp: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(106) var lightwarp_sampler: sampler;

fn model_lighting(pbr: PbrInput, uv: vec2<f32>, base_alpha: f32) -> vec4<f32> {
    let exponent_sample = textureSample(exponent_map, exponent_sampler, uv);
    let exponent = 1.0 + 149.0 * exponent_sample.r;
    var mask = 1.0;
#ifdef STANDARD_MATERIAL_NORMAL_MAP
    mask = textureSample(pbr_bindings::normal_map_texture, pbr_bindings::normal_map_sampler, uv).a;
#endif
    let facing = clamp(1.0 - dot(pbr.N, pbr.V), 0.0, 1.0);
    let f = facing * facing;
    let fresnel = select(mix(phong.x, phong.y, 2.0 * f),
        mix(phong.y, phong.z, 2.0 * f - 1.0), f > 0.5);
    let reflected = reflect(-pbr.V, pbr.N);
    let view_z = (view_bindings::view.view_from_world * pbr.world_position).z;
    let cluster = clustering::view_fragment_cluster_index(pbr.frag_coord.xy, view_z, pbr.is_orthographic);
    let indices = clustering::unpack_clusterable_object_index_ranges(cluster);
    var diffuse = view_bindings::lights.ambient_color.rgb;
    var specular = vec3<f32>(0.0);
    for (var i = indices.first_point_light_index_offset; i < indices.first_spot_light_index_offset; i += 1u) {
        let id = clustering::get_clusterable_object_id(i);
        let light = view_bindings::clustered_lights.data[id];
        let delta = light.position_radius.xyz - pbr.world_position.xyz;
        let d2 = max(dot(delta, delta), 0.0001);
        let direction = delta * inverseSqrt(d2);
        let cutoff = max(1.0 - pow(d2 * light.color_inverse_square_range.w, 2.0), 0.0);
        var shadow = 1.0;
        if ((pbr.flags & MESH_FLAGS_SHADOW_RECEIVER_BIT) != 0u &&
            (light.flags & mesh_view_types::POINT_LIGHT_FLAGS_SHADOWS_ENABLED_BIT) != 0u) {
            shadow = shadows::fetch_point_shadow(id, pbr.world_position, pbr.world_normal, pbr.frag_coord.xy);
        }
        let irradiance = light.color_inverse_square_range.rgb * cutoff * cutoff / d2 * shadow;
        let ndotl = dot(pbr.N, direction);
        let half_lambert = clamp(ndotl * 0.5 + 0.5, 0.0, 1.0);
        var response = vec3<f32>(half_lambert * half_lambert);
        if model_flags.y > 0.0 {
            response = 2.0 * textureSampleLevel(lightwarp, lightwarp_sampler, vec2<f32>(half_lambert), 0.0).rgb;
        }
        diffuse += irradiance * response;
        specular += irradiance * max(ndotl, 0.0) * pow(max(dot(reflected, direction), 0.0), exponent);
    }
    var color = diffuse * pbr.material.base_color.rgb * view_bindings::view.exposure;
    if model_flags.z > 0.0 {
        color = mix(color, pbr.material.base_color.rgb, base_alpha);
    }
    color += specular * mask * fresnel * phong.w * view_bindings::view.exposure * mix(vec3<f32>(1.0), pbr.material.base_color.rgb, exponent_sample.g);
    return vec4<f32>(color, pbr.material.base_color.a);
}

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) front: bool) -> FragmentOutput {
    if dot(plane, in.world_position) < 0.0 { discard; }
    var pbr = pbr_input_from_standard_material(in, front);
    let base_alpha = pbr.material.base_color.a;
    pbr.material.base_color = alpha_discard(pbr.material, pbr.material.base_color);
    var out: FragmentOutput;
    var color = apply_pbr_lighting(pbr);
#ifdef VERTEX_UVS_A
    if model_flags.x > 0.0 { color = model_lighting(pbr, in.uv, base_alpha); }
#endif
    out.color = main_pass_post_lighting_processing(pbr, color);
    return out;
}
