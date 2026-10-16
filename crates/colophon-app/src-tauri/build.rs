fn main() {
    // Le pont PhotoKit n'existe que sur macOS. On lit la cible plutôt que
    // `cfg!(target_os)`, qui dans un script de construction parle de la
    // machine qui compile, pas de celle pour qui on compile.
    let macos = std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos");
    // La saveur App Store, lue de la même façon : une feature arrive au
    // script de construction par l'environnement, jamais par `cfg!`.
    let mas = std::env::var_os("CARGO_FEATURE_MAS").is_some();
    if macos {
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
    if macos && mas {
        // Les marque-pages de sécurité et NSWorkspace, que seul le bac à
        // sable demande : le DMG n'en compile pas une ligne.
        cc::Build::new()
            .file("src/signet.m")
            .flag("-fobjc-arc")
            .compile("colophon_signet");
        println!("cargo:rustc-link-lib=framework=AppKit");
        println!("cargo:rerun-if-changed=src/signet.m");
    }
    // La capacité de l'updater vit à part (`capabilities/maj.json`), et la
    // saveur App Store ne la charge pas : la règle 2.4.5(vii) interdit à une
    // app du Store de se mettre à jour elle-même.
    let attributs = tauri_build::Attributes::new();
    let attributs = if mas {
        println!("cargo:rerun-if-changed=capabilities");
        attributs.capabilities_path_pattern("./capabilities/default.json")
    } else {
        attributs
    };
    tauri_build::try_build(attributs).expect("tauri-build");
}
