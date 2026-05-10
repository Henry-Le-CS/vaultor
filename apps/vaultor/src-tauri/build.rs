fn main() {
    #[cfg(target_os = "macos")]
    {
        cc::Build::new()
            .file("src/touchid.m")
            .flag("-fobjc-arc")
            .compile("vaultor_touchid");

        println!("cargo:rustc-link-lib=framework=LocalAuthentication");
        println!("cargo:rustc-link-lib=framework=Foundation");
    }

    tauri_build::build()
}
