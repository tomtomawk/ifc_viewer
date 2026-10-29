//! Enregistrement v0 d'une source RVT ouverte, sans modifier le fichier Revit.
use super::Projet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Instant, SystemTime};

#[derive(Clone)]
pub struct SourceRvt {
    pub chemin: PathBuf,
    octets: u64,
    modifie: SystemTime,
}

impl SourceRvt {
    pub fn ouvrir(chemin: &Path) -> Result<Self, String> {
        let chemin = chemin.canonicalize().map_err(|e| format!("Source RVT inaccessible : {e}"))?;
        let meta = chemin.metadata().map_err(|e| e.to_string())?;
        let modifie = meta.modified().map_err(|e| e.to_string())?;
        Ok(Self { chemin, octets: meta.len(), modifie })
    }

    fn verifier(&self) -> Result<(), String> {
        let actuel = Self::ouvrir(&self.chemin)?;
        if actuel.octets != self.octets || actuel.modifie != self.modifie {
            return Err("Le fichier RVT a changé sur disque depuis son ouverture. Rouvrez-le avant de l’enregistrer en v0.".into());
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct Bilan {
    pub chemin: PathBuf,
    pub rapport: Result<PathBuf, String>,
    pub murs: usize,
    pub importes: usize,
}

/// Prépare intégralement le fichier avant de remplacer une éventuelle sauvegarde.
pub fn ecrire_atomique(chemin: &Path, contenu: &[u8]) -> Result<(), String> {
    use std::io::Write;
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let nom = chemin.file_name().ok_or("Nom du fichier de destination invalide.")?.to_string_lossy();
    let temporaire = chemin.with_file_name(format!(".{nom}.{}-{}.tmp", std::process::id(), SEQUENCE.fetch_add(1,Ordering::Relaxed)));
    let mut fichier = std::fs::OpenOptions::new().write(true).create_new(true).open(&temporaire).map_err(|e| format!("Création impossible de {} : {e}", temporaire.display()))?;
    let resultat = fichier.write_all(contenu).and_then(|_| fichier.sync_all());
    drop(fichier);
    if let Err(e) = resultat {
        let _ = std::fs::remove_file(&temporaire);
        return Err(format!("Écriture impossible : {e}"));
    }
    if let Err(e) = std::fs::rename(&temporaire,chemin) {
        let _ = std::fs::remove_file(&temporaire);
        return Err(format!("Remplacement impossible de {} : {e}", chemin.display()));
    }
    Ok(())
}

pub fn enregistrer_rvt(source: &SourceRvt, sortie: &Path, progress: &dyn Fn(String)) -> Result<Bilan, String> {
    if !sortie.extension().is_some_and(|e| e.eq_ignore_ascii_case("json")) {
        return Err("La destination doit être un fichier .modele.json ou .json.".into());
    }
    if sortie.canonicalize().ok().as_ref() == Some(&source.chemin) {
        return Err("Le fichier Revit source ne peut pas être remplacé.".into());
    }
    source.verifier()?;
    let debut = Instant::now();
    progress("Conversion du projet Revit vers le modèle natif v0…".into());
    let (projet, mesures) = super::depuis_rvt::convertir(&source.chemin,progress)?;
    source.verifier()?;
    progress("Préparation du fichier natif v0…".into());
    let json = projet.json()?;
    let relu = Projet::lire_json(&json)?;
    if relu.murs.len() != projet.murs.len() || relu.importes.len() != projet.importes.len() {
        return Err("Le contrôle du fichier natif a échoué ; la destination est conservée.".into());
    }
    let rapport = super::rapport::markdown(&projet,&mesures,debut.elapsed().as_secs_f64());
    progress(format!("Enregistrement de {}…",sortie.display()));
    ecrire_atomique(sortie,json.as_bytes())?;
    let chemin_rapport = sortie.with_extension("rapport.md");
    let rapport = ecrire_atomique(&chemin_rapport,rapport.as_bytes()).map(|_| chemin_rapport);
    Ok(Bilan { chemin: sortie.to_path_buf(), rapport, murs: projet.murs.len(), importes: projet.importes.len() })
}

#[cfg(test)]
mod tests {
    use super::*;
    static TEST: AtomicU64 = AtomicU64::new(0);
    fn dossier() -> PathBuf {
        let p = std::env::temp_dir().join(format!("ifc-v0-save-{}-{}",std::process::id(),TEST.fetch_add(1,Ordering::Relaxed)));
        std::fs::create_dir(&p).unwrap(); p
    }
    #[test]
    fn sauvegarde_rvt_source_modifiee_ou_destination_invalide() {
        let p = dossier();
        let entree = p.join("source.rvt");
        std::fs::write(&entree,b"source initiale").unwrap();
        let source = SourceRvt::ouvrir(&entree).unwrap();
        assert!(enregistrer_rvt(&source,&entree,&|_|{}).is_err());
        std::fs::write(&entree,b"source modifiee sur disque").unwrap();
        let sortie = p.join("projet.modele.json");
        std::fs::write(&sortie,b"ancienne sauvegarde").unwrap();
        assert!(enregistrer_rvt(&source,&sortie,&|_|{}).unwrap_err().contains("changé"));
        assert_eq!(std::fs::read(&sortie).unwrap(),b"ancienne sauvegarde");
        std::fs::remove_file(sortie).unwrap(); std::fs::remove_file(entree).unwrap(); std::fs::remove_dir(p).unwrap();
    }
    #[test]
    fn sauvegarde_rvt_v0_aller_retour_et_remplacement() {
        let base = std::env::var_os("RVT_ESSAIS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(r"C:\Users\theba\Projets code\Decodage RVT"));
        let entree = base.join(r"essais\2026\E04_mur.rvt");
        if !entree.exists() { eprintln!("fichier d'essai absent : {}",entree.display()); return; }
        let source = SourceRvt::ouvrir(&entree).unwrap();
        let p = dossier();
        let sortie = p.join("projet.modele.json");
        std::fs::write(&sortie,b"ancienne sauvegarde").unwrap();
        let bilan = enregistrer_rvt(&source,&sortie,&|_|{}).unwrap();
        assert_eq!(bilan.murs,1);
        let relu = Projet::lire_json(&std::fs::read_to_string(&sortie).unwrap()).unwrap();
        assert_eq!(relu.version,0); assert_eq!(relu.murs.len(),1);
        assert_eq!(relu.sources[0].format,"RVT");
        let rapport = bilan.rapport.unwrap(); assert!(std::fs::read_to_string(&rapport).unwrap().contains("Conversion"));
        source.verifier().unwrap();
        std::fs::remove_file(sortie).unwrap(); std::fs::remove_file(rapport).unwrap(); std::fs::remove_dir(p).unwrap();
    }
}

/// Valide et remplace un fichier natif sans écriture partielle.
pub fn enregistrer_natif(p: &Projet, chemin: &Path) -> Result<(), String> {
    if !chemin.extension().is_some_and(|e|e.eq_ignore_ascii_case("json")) {return Err("Un projet natif doit être enregistré dans un fichier .json.".into());}
    p.verifier()?;
    let json=p.json()?;
    Projet::lire_json(&json)?;
    ecrire_atomique(chemin,json.as_bytes())
}
pub fn chemin_recuperation(chemin: &Path) -> PathBuf { chemin.with_extension("recuperation.json") }
pub fn recuperer_plus_tard(p: &Projet, chemin: &Path) -> Result<(),String> { enregistrer_natif(p,&chemin_recuperation(chemin)) }
pub fn effacer_recuperation(chemin: &Path) { let _=std::fs::remove_file(chemin_recuperation(chemin)); }
