fn main() {
    println!("cargo:rerun-if-changed=assets/icone.ico");
    // échec : message de Cargo et arrêt de la construction (sans `expect`, interdit hors tests)
    #[cfg(windows)]
    if let Err(e) = winresource::WindowsResource::new().set_icon("assets/icone.ico").compile() {
        println!("cargo:warning=Impossible d'intégrer l'icône Windows : {e}");
        std::process::exit(1);
    }
}
