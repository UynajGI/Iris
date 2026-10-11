//! Diagnostic only: same approved model, varying session options, no application changes.
use anyhow::Result;
use ort::{
    ep::DirectML,
    session::{builder::GraphOptimizationLevel, Session},
    value::Tensor,
};
fn main() -> Result<()> {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../");
    let gpu = root.join("models/directml");
    #[cfg(windows)]
    for name in ["DirectML.dll", "onnxruntime_providers_shared.dll"] {
        unsafe {
            let _ = libloading::os::windows::Library::load_with_flags(
                gpu.join(name),
                0x00000100 | 0x00001000,
            )?
            .into_raw();
        }
    }
    ort::init_from(gpu.join("onnxruntime.dll"))?.commit();
    for fixed in [false, true] {
        for level in [
            GraphOptimizationLevel::Disable,
            GraphOptimizationLevel::Level1,
            GraphOptimizationLevel::Level2,
            GraphOptimizationLevel::Level3,
        ] {
            let label = format!("fixed={fixed} level={level:?}");
            let result = probe(&root, fixed, level);
            println!("{label}: {result:?}");
        }
    }
    Ok(())
}

fn probe(
    root: &std::path::Path,
    fixed: bool,
    level: GraphOptimizationLevel,
) -> Result<(usize, bool)> {
    let mut b = configured_builder(fixed, level)?;
    let model = std::env::var_os("IRIS_PROBE_DINO_MODEL")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| root.join("models/optional/dinov3_vits16.onnx"));
    let mut s = b.commit_from_file(model)?;
    let o = s.run(ort::inputs![Tensor::from_array((
        [1usize, 3, 224, 224],
        vec![0f32; 3 * 224 * 224]
    ))?])?;
    let (_, v) = o["pooler_output"].try_extract_tensor::<f32>()?;
    Ok((v.len(), v.iter().all(|x| x.is_finite())))
}

fn configured_builder(
    fixed: bool,
    level: GraphOptimizationLevel,
) -> Result<ort::session::builder::SessionBuilder> {
    let mut b = Session::builder()?
        .with_intra_threads(1)
        .map_err(ort::Error::<()>::from)?
        .with_inter_threads(1)
        .map_err(ort::Error::<()>::from)?
        .with_memory_pattern(false)
        .map_err(ort::Error::<()>::from)?
        .with_parallel_execution(false)
        .map_err(ort::Error::<()>::from)?
        .with_optimization_level(level)
        .map_err(ort::Error::<()>::from)?
        .with_execution_providers([DirectML::default()
            .with_device_id(0)
            .build()
            .error_on_failure()])
        .map_err(ort::Error::<()>::from)?;
    if fixed {
        for (n, v) in [("s99", 1), ("s100", 224), ("s4", 224)] {
            b = b
                .with_dimension_override(n, v)
                .map_err(ort::Error::<()>::from)?;
        }
    }
    Ok(b)
}
