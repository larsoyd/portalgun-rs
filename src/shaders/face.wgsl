// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

// portal_refract_vs20 and the ps20b stage 2, kept in step with the portal crate's face math
#define_import_path portalgun::face

struct Overlay {
    u: f32,
    alpha: f32,
};

fn ease(x: f32) -> f32 {
    return (x * -2.0 + 3.0) * (x * x);
}

fn ellipse(uv: vec2<f32>) -> vec2<f32> {
    return uv * 1.075 - 0.0375;
}

fn radius(t0: vec2<f32>) -> f32 {
    let p = 2.0 * t0 - 1.0;
    return sqrt(p.x * p.x + p.y * p.y);
}

fn opening(open: f32) -> f32 {
    return ease(saturate(open));
}

fn noise_coords(uv: vec2<f32>, open: f32, time: f32) -> vec4<f32> {
    let k = 1.0 / clamp(open + 0.001, 0.0, 1.0);
    let q = ((uv - 0.5) * k + 0.5) * 0.3;
    return vec4<f32>(q.x + 0.0275 * time, q.y, q.x - 0.0275 * time, q.y);
}

fn overlay(open: f32, keep_in: f32, t0: vec2<f32>, t5: vec4<f32>, noise: texture_2d<f32>, noise_sampler: sampler) -> Overlay {
    let n0 = textureSample(noise, noise_sampler, t5.xy).xy;
    let s = opening(open);
    let s2 = s * s;
    let d = radius(t0);
    let inside = select(0.0, 1.0, s2 - d >= 0.0);
    let lo = s2 - 0.3;
    let edge_in = saturate((d - lo) * (1.0 / (s2 - lo)));
    let edge_out = 1.0 - saturate((d - s2) * 13.333333);
    let edge = select(edge_out, edge_in, inside == 1.0);
    let keep = saturate(keep_in);
    let m = edge * max(saturate(s * 2.5), 1.0 - keep);
    let n1 = textureSample(noise, noise_sampler, t5.wz - n0 * 0.02).xy;
    let n2 = textureSample(noise, noise_sampler, t5.xy - n1 * 0.02).xy;
    let h = (n1.y + n2.y) * 0.5;
    let x = saturate((h - (m - 0.875)) * (1.0 / ((m + 0.875) - (m - 0.875))));
    let sx = 1.0 - ease(x);
    let y = saturate(keep * (1.0 / h));
    let sy = 1.0 - ease(y);
    let alpha = saturate(m * sx + inside * sy) * 1.5;
    let u = alpha * sqrt(sx) * (pow(abs(t0.y), 1.5) * 0.8 + 0.2);
    return Overlay(u, alpha);
}

fn coverage(alpha: f32) -> f32 {
    return select(0.0, min(alpha, 1.0), alpha > 1.0 / 255.0);
}
