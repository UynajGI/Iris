//! Diagnostic only: same approved model, varying session options, no application changes.
use anyhow::Result;
use ort::{
    execution_providers::DirectMLExecutionProvider,
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
    ort::init_from(gpu.join("onnxruntime.dll").to_string_lossy()).commit()?;
    for fixed in [false, true] {
        for level in [
            GraphOptimizationLevel::Disable,
            GraphOptimizationLevel::Level1,
            GraphOptimizationLevel::Level2,
            GraphOptimizationLevel::Level3,
        ] {
            let label = format!("fixed={fixed} level={level:?}");
            let result = (|| -> Result<_> {
                let mut b = Session::builder()?
                    .with_intra_threads(1)?
                    .with_inter_threads(1)?
                    .with_memory_pattern(false)?
                    .with_parallel_execution(false)?
                    .with_optimization_level(level)?
                    .with_execution_providers([DirectMLExecutionProvider::default()
                        .with_device_id(0)
                        .build()
                        .error_on_failure()])?;
                if fixed {
                    for (n, v) in [("s99", 1), ("s100", 224), ("s4", 224)] {
                        b = b.with_dimension_override(n, v)?;
                    }
                }
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
            })();
            println!("{label}: {result:?}");
        }
    }
    Ok(())
}
