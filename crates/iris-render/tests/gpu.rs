//! Shader validation always runs. Numeric tests require an explicitly available
//! hardware or software adapter, are ignored by default, and FAIL if unavailable.
//! CI: install Mesa lavapipe, run `cargo test -p iris-render --test gpu -- --ignored`.
//! Set IRIS_GPU_TEST_ADAPTER=hardware to allow the default hardware-first selection.
//! These are correctness tests, not GPU speed or broad acceleration claims.
#[test]
fn exposure_shader_parses_and_validates_without_a_gpu() {
    let shader = include_str!("../shaders/exposure.wgsl");
    let module = naga::front::wgsl::parse_str(shader).expect("WGSL parses");
    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::empty(),
    )
    .validate(&module)
    .expect("WGSL validates");
    assert_eq!(module.entry_points.len(), 1);
    assert_eq!(module.entry_points[0].workgroup_size, [64, 1, 1]);
}

#[cfg(feature = "gpu")]
mod enabled {
    use iris_render::{
        adjustments::{self, Adjustments},
        gpu::{GpuOperation, GpuRenderer},
        Frame, RenderContext,
    };
    fn renderer() -> GpuRenderer {
        let result = if std::env::var("IRIS_GPU_TEST_ADAPTER").as_deref() == Ok("hardware") {
            GpuRenderer::new()
        } else {
            GpuRenderer::new_software()
        };
        result.expect(
            "GPU numeric validation unavailable: provision the CI adapter; this is not a pass/skip",
        )
    }
    #[test]
    fn unsupported_parameters_require_explicit_cpu_fallback() {
        assert!(GpuRenderer::supports(&Adjustments::default()));
        assert!(GpuRenderer::supports(&Adjustments {
            exposure: 1.234,
            ..Default::default()
        }));
        assert!(!GpuRenderer::supports(&Adjustments {
            contrast: 0.01,
            ..Default::default()
        }));
        assert!(!GpuRenderer::supports(&Adjustments {
            exposure: f32::NAN,
            ..Default::default()
        }));
    }
    #[test]
    #[ignore = "requires an explicitly provisioned GPU/software compute adapter"]
    fn gpu_exposure_matches_cpu_across_tiles_signed_hdr_and_alpha() {
        let renderer = renderer();
        let status = renderer.status();
        assert!(status.available);
        assert_eq!(status.operations, vec![GpuOperation::Exposure]);
        assert!(status.maximum_tile_pixels > 0);
        let n = status.maximum_tile_pixels + 17;
        let frame = Frame::new(
            n,
            1,
            (0..n)
                .map(|i| {
                    let t = (i % 257) as f32 / 256.0;
                    [
                        t * 2.0 - 0.5,
                        t * 0.18,
                        t * 3.0,
                        if i % 13 == 0 { 0.0 } else { 0.6 },
                    ]
                })
                .collect(),
        )
        .unwrap();
        for exposure in [-3.2, 0.0, 1.0, 2.3] {
            let p = Adjustments {
                exposure,
                ..Default::default()
            };
            let ctx = RenderContext::default();
            let cpu = adjustments::apply(&frame, &p, &ctx).unwrap();
            let gpu = renderer.apply(&frame, &p, &ctx).unwrap();
            for ((a, b), original) in cpu.pixels.iter().zip(&gpu.pixels).zip(&frame.pixels) {
                assert_eq!(b[3].to_bits(), original[3].to_bits());
                if original[3] == 0.0 || exposure == 0.0 {
                    for c in 0..4 {
                        assert_eq!(a[c].to_bits(), b[c].to_bits());
                    }
                } else {
                    for c in 0..3 {
                        assert!(
                            (a[c] - b[c]).abs() <= a[c].abs().max(1.0) * 4.0 * f32::EPSILON,
                            "CPU {} vs GPU {}",
                            a[c],
                            b[c]
                        );
                    }
                }
            }
        }
    }
    #[test]
    #[ignore = "requires an explicitly provisioned GPU/software compute adapter"]
    fn gpu_validates_memory_overflow_cancellation_and_subnormals() {
        let renderer = renderer();
        let p = Adjustments {
            exposure: 1.0,
            ..Default::default()
        };
        let frame = Frame::new(1, 1, vec![[0.25, 0.5, 1.0, 1.0]]).unwrap();
        assert!(renderer.apply(&frame, &p, &RenderContext::new(1)).is_err());
        let ctx = RenderContext::default();
        ctx.cancel();
        assert!(renderer.apply(&frame, &p, &ctx).is_err());
        assert!(renderer
            .apply(
                &frame,
                &Adjustments {
                    contrast: 1.0,
                    ..Default::default()
                },
                &RenderContext::default()
            )
            .is_err());
        let huge = Frame::new(1, 1, vec![[f32::MAX, 1.0, 1.0, 1.0]]).unwrap();
        assert!(renderer
            .apply(&huge, &p, &RenderContext::default())
            .is_err());
        let tiny = Frame::new(
            1,
            1,
            vec![[-0.0, f32::from_bits(1), -f32::from_bits(1), 1.0]],
        )
        .unwrap();
        let cpu = adjustments::apply(&tiny, &p, &RenderContext::default()).unwrap();
        let gpu = renderer
            .apply(&tiny, &p, &RenderContext::default())
            .unwrap();
        for c in 0..4 {
            assert_eq!(cpu.pixels[0][c].to_bits(), gpu.pixels[0][c].to_bits());
        }
        let tiny_alpha = Frame::new(1, 1, vec![[0.2, 0.3, 0.4, f32::from_bits(1)]]).unwrap();
        let out = renderer
            .apply(&tiny_alpha, &p, &RenderContext::default())
            .unwrap();
        assert!((out.pixels[0][0] - 0.4).abs() < 1e-6);
        assert_eq!(out.pixels[0][3].to_bits(), 1);
        // Failures before submission must leave the reusable renderer operational.
        assert!(renderer.status().available);
        assert!(renderer
            .apply(&frame, &p, &RenderContext::default())
            .is_ok());
    }
}
