fn main() {
    // Le pont PhotoKit n'existe que sur macOS. On lit la cible plutôt que
    // `cfg!(target_os)`, qui dans un script de construction parle de la
    // machine qui compile, pas de celle pour qui on compile.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        cc::Build::new()
            .file("src/photos.m")
            // ARC : le module ne gère aucun compte de références à la main, et
            // c'est ce qui le garde lisible. La seule mémoire qu'il possède
            // sont les chaînes rendues à Rust, libérées par
            // `colophon_photos_liberer`.
            .flag("-fobjc-arc")
            .compile("colophon_photos");
        println!("cargo:rustc-link-lib=framework=Photos");
        println!("cargo:rustc-link-lib=framework=Foundation");
        println!("cargo:rerun-if-changed=src/photos.m");
    }
    tauri_build::build()
}
