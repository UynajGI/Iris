// SPDX-License-Identifier: GPL-3.0-or-later
// Iris exposure v1. Straight-alpha linear Rec.2020, no tone map or gamut clip.
// The host checks finite input/output domain before dispatch. Gain is computed
// once on the CPU; FP32 GPU results can differ from CPU f64 intermediates by ULPs.
struct Parameters {
    gain: f32,
    count: u32,
    padding: vec2<u32>,
}
@group(0) @binding(0) var<storage, read> source: array<vec4<f32>>;
@group(0) @binding(1) var<storage, read_write> destination: array<vec4<f32>>;
@group(0) @binding(2) var<uniform> parameters: Parameters;

@compute @workgroup_size(64)
fn exposure(@builtin(global_invocation_id) invocation: vec3<u32>) {
    let index = invocation.x;
    if index >= parameters.count { return; }
    let pixel = source[index];
    // Integer inspection avoids treating a positive subnormal alpha as zero on
    // hardware which flushes FP32 arithmetic denormals. Host copies alpha exactly.
    if (bitcast<u32>(pixel.a) & 0x7fffffffu) == 0u {
        destination[index] = pixel;
    } else {
        destination[index] = vec4<f32>(pixel.rgb * parameters.gain, pixel.a);
    }
}
