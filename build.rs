fn main() {
    println!("cargo:rerun-if-changed=assets/mori.rc");
    println!("cargo:rerun-if-changed=assets/mori.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    if let Err(error) =
        embed_resource::compile("assets/mori.rc", embed_resource::NONE).manifest_required()
    {
        // MinGW windres can produce a .res that the MSVC linker also accepts.
        let output =
            std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap()).join("mori.res");
        let status = std::process::Command::new("windres")
            .args([
                "--input",
                "assets/mori.rc",
                "--output-format=res",
                "--output",
            ])
            .arg(&output)
            .status()
            .unwrap_or_else(|_| panic!("Install Windows SDK rc.exe or MinGW windres: {error}"));
        assert!(status.success(), "Unable to compile Mori icon resources");
        println!("cargo:rustc-link-arg-bin=mori={}", output.display());
    }
}
