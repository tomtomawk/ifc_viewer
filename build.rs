fn main() {
    println!("cargo:rerun-if-changed=assets/icone.ico");
    #[cfg(windows)]
    winresource::WindowsResource::new()
        .set_icon("assets/icone.ico")
        .compile()
        .expect("Impossible d'intégrer l'icône Windows");
}
