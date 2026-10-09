//! Release-tool helper: verify only. Never executes or installs the artifact.
fn main() -> anyhow::Result<()> {
    let arguments: Vec<_> = std::env::args_os().skip(1).collect();
    anyhow::ensure!(
        arguments.len() == 4,
        "usage: verify_update_artifact <artifact> <signature-file> <public-key-file> <version>"
    );
    let bytes = std::fs::read(&arguments[0])?;
    let signature = std::fs::read_to_string(&arguments[1])?;
    let public_key = std::fs::read_to_string(&arguments[2])?;
    let version = arguments[3]
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("version must be UTF-8"))?;
    iris_shell::updater::verify_release_artifact(&bytes, &signature, &public_key, version)?;
    println!("Artifact signature, public key and signed version verified");
    Ok(())
}
