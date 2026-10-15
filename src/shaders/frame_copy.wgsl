// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

// outside the depth range, so nothing is rasterised
@vertex
fn vertex(@location(0) position: vec3<f32>) -> @builtin(position) vec4<f32> {
    return vec4(0.0, 0.0, -1.0, 1.0);
}
@fragment
fn fragment() -> @location(0) vec4<f32> {
    return vec4(0.0);
}
