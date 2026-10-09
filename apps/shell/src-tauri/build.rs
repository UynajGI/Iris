fn main() {
    for key in [
        "IRIS_DISTRIBUTION",
        "IRIS_UPDATE_ENDPOINT",
        "IRIS_UPDATE_PUBLIC_KEY",
    ] {
        println!("cargo:rerun-if-env-changed={key}");
    }
    let endpoint = std::env::var_os("IRIS_UPDATE_ENDPOINT");
    let public_key = std::env::var_os("IRIS_UPDATE_PUBLIC_KEY");
    assert_eq!(
        endpoint.is_some(),
        public_key.is_some(),
        "IRIS_UPDATE_ENDPOINT and IRIS_UPDATE_PUBLIC_KEY must be supplied together"
    );
    if endpoint.is_some() {
        assert_eq!(
            std::env::var("IRIS_DISTRIBUTION").as_deref(),
            Ok("nsis"),
            "updater configuration requires IRIS_DISTRIBUTION=nsis"
        );
    }
    #[cfg(feature = "desktop")]
    {
        // Tauri's Unix context also requires a PNG. This transparent build
        // resource deliberately does not introduce a product logo.
        let png = std::path::PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap())
            .join("icons/icon.png");
        if !png.exists() {
            std::fs::create_dir_all(png.parent().unwrap()).unwrap();
            std::fs::write(
                &png,
                [
                    137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0,
                    0, 1, 8, 6, 0, 0, 0, 31, 21, 196, 137, 0, 0, 0, 13, 73, 68, 65, 84, 120, 156,
                    99, 96, 96, 96, 96, 0, 0, 0, 5, 0, 1, 165, 246, 69, 64, 0, 0, 0, 0, 73, 69, 78,
                    68, 174, 66, 96, 130,
                ],
            )
            .unwrap();
        }
        // Windows requires an icon resource even for a hidden headless host.
        // Transparent 1px ICO is a build resource, not a product visual design.
        let icon = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap()).join("host.ico");
        let mut bytes = vec![
            0, 0, 1, 0, 1, 0, 1, 1, 0, 0, 1, 0, 32, 0, 48, 0, 0, 0, 22, 0, 0, 0,
        ];
        bytes.extend_from_slice(&40u32.to_le_bytes());
        bytes.extend_from_slice(&1u32.to_le_bytes());
        bytes.extend_from_slice(&2u32.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&32u16.to_le_bytes());
        bytes.extend_from_slice(&[0; 24]);
        bytes.extend_from_slice(&[0; 4]);
        bytes.extend_from_slice(&[255; 4]);
        std::fs::write(&icon, bytes).unwrap();
        tauri_build::try_build(
            tauri_build::Attributes::new()
                .windows_attributes(tauri_build::WindowsAttributes::new().window_icon_path(icon)),
        )
        .expect("build desktop host resources");
    }
}
