//! Dialogue de choix de fichier, même écriture sur poste et dans le navigateur.
//!
//! Poste : `rfd::FileDialog` (synchrone). Navigateur (wasm32) : il n'y a ni chemin ni
//! dialogue synchrone ; chaque choix rend `None` et la crate `web` passe par le
//! téléversement et le téléchargement (ROADMAP §10.3, W5). Les appelants écrivent
//! `Fichier::new()` comme ils écrivaient `crate::socle::dialogue::Fichier::new()`.
use std::path::PathBuf;

#[derive(Default)]
pub(crate) struct Fichier {
    #[cfg(not(target_arch = "wasm32"))]
    rfd: rfd::FileDialog,
}

#[cfg(not(target_arch = "wasm32"))]
impl Fichier {
    pub(crate) fn new() -> Self { Self { rfd: rfd::FileDialog::new() } }
    pub(crate) fn set_title(self, titre: impl Into<String>) -> Self { Self { rfd: self.rfd.set_title(titre) } }
    pub(crate) fn add_filter(self, nom: impl Into<String>, extensions: &[impl ToString]) -> Self { Self { rfd: self.rfd.add_filter(nom, extensions) } }
    pub(crate) fn set_file_name(self, nom: impl Into<String>) -> Self { Self { rfd: self.rfd.set_file_name(nom) } }
    pub(crate) fn pick_file(self) -> Option<PathBuf> { self.rfd.pick_file() }
    pub(crate) fn pick_folder(self) -> Option<PathBuf> { self.rfd.pick_folder() }
    pub(crate) fn save_file(self) -> Option<PathBuf> { self.rfd.save_file() }
}

#[cfg(target_arch = "wasm32")]
impl Fichier {
    pub(crate) fn new() -> Self { Self {} }
    pub(crate) fn set_title(self, _titre: impl Into<String>) -> Self { self }
    pub(crate) fn add_filter(self, _nom: impl Into<String>, _extensions: &[impl ToString]) -> Self { self }
    pub(crate) fn set_file_name(self, _nom: impl Into<String>) -> Self { self }
    pub(crate) fn pick_file(self) -> Option<PathBuf> { None }
    pub(crate) fn pick_folder(self) -> Option<PathBuf> { None }
    pub(crate) fn save_file(self) -> Option<PathBuf> { None }
}
