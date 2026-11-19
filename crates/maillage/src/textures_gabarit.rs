//! Textures livrées avec le gabarit (`gabarits/textures`), embarquées dans le logiciel :
//! un chemin `gabarit:textures/<fichier>` d'une apparence de matériau est remplacé par le
//! fichier écrit une fois dans le dossier temporaire (le rendu lit les textures sur disque).
use std::path::PathBuf;

const TEXTURES: &[(&str, &[u8])] = &[
    ("herbe_diff.jpg", include_bytes!("../../../gabarits/textures/herbe_diff.jpg")),
    ("herbe_nor_gl.jpg", include_bytes!("../../../gabarits/textures/herbe_nor_gl.jpg")),
    ("herbe_disp.jpg", include_bytes!("../../../gabarits/textures/herbe_disp.jpg")),
    ("herbe_ao.jpg", include_bytes!("../../../gabarits/textures/herbe_ao.jpg")),
    ("feuilles.png", include_bytes!("../../../gabarits/textures/feuilles.png")),
    ("feuilles_masque.png", include_bytes!("../../../gabarits/textures/feuilles_masque.png")),
    ("ecorce.png", include_bytes!("../../../gabarits/textures/ecorce.png")),
    ("ecorce_relief.png", include_bytes!("../../../gabarits/textures/ecorce_relief.png")),
];

/// Fichier d'une texture du gabarit (`gabarit:textures/herbe_diff.jpg`), écrit au besoin ;
/// None si le chemin n'en désigne pas une.
pub fn chemin(chemin: &str) -> Option<PathBuf> {
    let nom = chemin.strip_prefix(modele::gabarit::PREFIXE_TEXTURE)?.strip_prefix("textures/")?;
    let (_, octets) = TEXTURES.iter().find(|(n, _)| *n == nom)?;
    let dossier = std::env::temp_dir().join("bimedit-textures-gabarit");
    let fichier = dossier.join(nom);
    if std::fs::metadata(&fichier).map_or(true, |m| m.len() != octets.len() as u64) {
        std::fs::create_dir_all(&dossier).ok()?;
        // écrit sous un nom provisoire puis renommé : deux instances ne lisent jamais un fichier partiel
        let provisoire = dossier.join(format!("{nom}.{}", std::process::id()));
        std::fs::write(&provisoire, octets).ok()?;
        std::fs::rename(&provisoire, &fichier).ok()?;
    }
    Some(fichier)
}

#[cfg(test)]
mod tests {
    #[test]
    fn texture_du_gabarit_ecrite() {
        let f = super::chemin("gabarit:textures/feuilles_masque.png").unwrap();
        assert!(f.is_file());
        assert!(super::chemin("gabarit:textures/absente.png").is_none());
        assert!(super::chemin("textures/feuilles.png").is_none());
    }
}
