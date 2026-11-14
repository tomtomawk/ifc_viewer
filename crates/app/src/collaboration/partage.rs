//! Point d'entrée partagé et gestion automatique des copies locales.
//! Le fichier .bimpartage est publié en dernier, après l'état central durable.
use super::session::Session;
use super::stockage::{Depot, Etat};
use super::*;
use std::io::Write;
use std::path::{Component, Path, PathBuf};

pub const FORMAT_PARTAGE: &str = "bimedit-projet-partage";
pub const EXTENSION: &str = "bimpartage";

#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct Partage {
    pub format: String,
    pub version: u32,
    pub protocole: u32,
    pub projet: String,
    pub nom: String,
    pub depot: String,
}
impl Partage {
    fn verifier(&self) -> Resultat<()> {
        if self.format != FORMAT_PARTAGE
            || self.version != 1
            || !protocole_lisible(self.protocole)
            || self.projet.is_empty()
        {
            return Err("Fichier partagé incompatible ou incomplet".into());
        }
        let mut components = Path::new(&self.depot).components();
        if !matches!(components.next(), Some(Component::Normal(_)))
            || components.next().is_some()
            || self.depot.contains(['\\', '/'])
        {
            return Err("Le dossier de collaboration doit être voisin du fichier partagé".into());
        }
        Ok(())
    }
    pub fn depot(&self, entree: &Path) -> Resultat<PathBuf> {
        self.verifier()?;
        Ok(entree
            .parent()
            .ok_or("Emplacement partagé invalide")?
            .join(&self.depot))
    }
}
pub fn extension_partage(path: &Path) -> bool {
    path.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case(EXTENSION))
}
pub fn contenu_partage(texte: &str) -> bool {
    serde_json::from_str::<Value>(texte)
        .ok()
        .and_then(|v| {
            v.get("format")
                .and_then(Value::as_str)
                .map(|s| s == FORMAT_PARTAGE)
        })
        .unwrap_or(false)
}
pub fn racine_cache() -> PathBuf {
    std::env::var_os("BIMEDIT_CACHE_COLLABORATION")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::var_os("LOCALAPPDATA")
                .or_else(|| std::env::var_os("XDG_CACHE_HOME"))
                .map(PathBuf::from)
                .unwrap_or_else(|| {
                    std::env::var_os("HOME")
                        .map(PathBuf::from)
                        .unwrap_or_else(std::env::temp_dir)
                        .join(".cache")
                })
                .join("BIMedit")
                .join("Collaboration")
        })
}
fn absolu(path: &Path) -> Resultat<PathBuf> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        Ok(std::env::current_dir()
            .map_err(|e| e.to_string())?
            .join(path))
    }
}
fn emplacement_entree(cache: &Path, entree: &Path) -> PathBuf {
    cache
        .join("entrees")
        .join(format!("{}.json", hash(&entree.to_string_lossy())))
}

/// Création unique : ne remplace ni un projet existant ni un ancien dépôt.
pub fn creer(p: &Projet, entree: &Path) -> Resultat<Partage> {
    let entree = absolu(entree)?;
    if entree.exists() {
        return Err("Un fichier existe déjà à cet emplacement ; choisir un nouveau nom".into());
    }
    let nom = entree
        .file_stem()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty())
        .ok_or("Nom de projet partagé invalide")?;
    let partage = Partage {
        format: FORMAT_PARTAGE.into(),
        version: 1,
        protocole: PROTOCOLE,
        projet: p.identite.clone(),
        nom: p.nom.clone(),
        depot: format!("{nom}.collaboration"),
    };
    let central = partage.depot(&entree)?;
    let mut p = p.clone();
    p.figer_references();
    p.verifier()?;
    std::fs::create_dir(&central)
        .map_err(|e| format!("Création du dossier de collaboration impossible : {e}"))?;
    let d = Depot::nouveau(&central);
    d.preparer()?;
    {
        let mut a = d.proteger()?;
        let e = a.initialiser(&p)?;
        a.instantane(&e)?;
    }
    let bytes = encoder(&partage)?;
    let temp = entree.with_file_name(format!(".partage-{}.tmp", uuid()));
    let mut f = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temp)
        .map_err(|e| e.to_string())?;
    f.write_all(&bytes)
        .and_then(|_| f.sync_all())
        .map_err(|e| e.to_string())?;
    drop(f);
    let resultat = publier_sans_remplacer(&temp, &entree);
    if resultat.is_err() {
        let _ = std::fs::remove_file(&temp);
    }
    resultat?;
    if std::fs::read(&entree).map_err(|e| e.to_string())? != bytes {
        return Err("Vérification du fichier partagé échouée".into());
    }
    Ok(partage)
}
fn publier_sans_remplacer(source: &Path, cible: &Path) -> Resultat<()> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::Storage::FileSystem::{MOVEFILE_WRITE_THROUGH, MoveFileExW};
        let s: Vec<u16> = source.as_os_str().encode_wide().chain(Some(0)).collect();
        let c: Vec<u16> = cible.as_os_str().encode_wide().chain(Some(0)).collect();
        if unsafe { MoveFileExW(s.as_ptr(), c.as_ptr(), MOVEFILE_WRITE_THROUGH) } == 0 {
            return Err(format!(
                "Publication du projet partagé impossible : {}",
                std::io::Error::last_os_error()
            ));
        }
    }
    #[cfg(not(windows))]
    {
        std::fs::hard_link(source, cible).map_err(|e| e.to_string())?;
        std::fs::remove_file(source).map_err(|e| e.to_string())?;
    }
    Ok(())
}
fn lire(entree: &Path, cache: &Path) -> Resultat<Partage> {
    let chemin_cache = emplacement_entree(cache, entree);
    let (bytes, distante) = match std::fs::read(entree) {
        Ok(b) => (b, true),
        Err(e) => (
            std::fs::read(&chemin_cache).map_err(|_| {
                format!("Projet partagé inaccessible et aucune entrée locale : {e}")
            })?,
            false,
        ),
    };
    let p: Partage =
        serde_json::from_slice(&bytes).map_err(|e| format!("Fichier partagé invalide : {e}"))?;
    p.verifier()?;
    if distante {
        stockage::sauver_local(&chemin_cache, &bytes)?;
    }
    Ok(p)
}
fn etat_central(partage: &Partage, entree: &Path) -> Resultat<Etat> {
    let d = Depot::nouveau(partage.depot(entree)?);
    let mut a = d.proteger_attente(false, std::time::Duration::from_secs(2))?;
    let e = a.lire()?.ok_or("Dépôt partagé sans état initial")?;
    if e.projet != partage.projet {
        return Err("Le fichier partagé et son dossier ne désignent pas le même projet".into());
    }
    Ok(e)
}
fn actualiser(s: &mut Session) -> Resultat<Projet> {
    s.reception_seule()?;
    // Un NAS qui ne répond plus ne bloque pas l'ouverture : la copie locale
    // s'affiche et la réponse sera reçue plus tard par l'interface.
    let limite = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        if std::time::Instant::now() >= limite {
            s.etat = "Hors connexion : dépôt central sans réponse".into();
            return s.affichage();
        }
        if s.recevoir()?.is_some() {
            return s.affichage();
        }
        if !s.synchronisation_en_cours() {
            return s.affichage();
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

/// Le verrou de session reste vivant jusqu'au transfert vers l'interface.
pub fn ouvrir(entree: &Path, cache: &Path) -> Resultat<(Projet, PathBuf, Session)> {
    let entree = absolu(entree)?;
    let partage = lire(&entree, cache)?;
    let central = partage.depot(&entree)?;
    let root = cache
        .join("projets")
        .join(hash(&(partage.projet.as_str(), central.to_string_lossy())));
    std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    let mut candidats: Vec<_> = std::fs::read_dir(&root)
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .filter(|e| e.path().is_dir())
        .collect();
    candidats.sort_by_key(|e| std::cmp::Reverse(e.metadata().and_then(|m| m.modified()).ok()));
    for c in candidats {
        let local = c.path().join("projet.modele.json");
        if !local.exists() {
            continue;
        }
        let p = match std::fs::read_to_string(&local)
            .map_err(|e| e.to_string())
            .and_then(|t| Projet::lire_json(&t))
        {
            Ok(p) => p,
            Err(e) => {
                eprintln!("Copie locale ignorée ({}) : {e}", local.display());
                continue;
            }
        };
        match Session::ouvrir(&p, local.clone(), central.clone()) {
            Ok(mut s) => {
                s.copie.partage = Some(entree.clone());
                let p = actualiser(&mut s)?;
                crate::echanges::enregistrement::enregistrer_natif(&p, &local)?;
                return Ok((p, local, s));
            }
            Err(e) if e.starts_with("Copie locale déjà ouverte") => continue,
            Err(e) => {
                eprintln!("Copie locale ignorée ({}) : {e}", local.display());
                continue;
            }
        }
    }
    let e = etat_central(&partage, &entree)?;
    let p = e.projection()?;
    let dir = root.join(uuid());
    std::fs::create_dir(&dir).map_err(|e| e.to_string())?;
    let local = dir.join("projet.modele.json");
    let mut s = Session::ouvrir(&p, local.clone(), central)?;
    s.copie.base = e;
    s.copie.partage = Some(entree.clone());
    s.etat = "Synchronisé".into();
    s.persister()?;
    crate::echanges::enregistrement::enregistrer_natif(&p, &local)?;
    Ok((p, local, s))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn dossier() -> PathBuf {
        let p = std::env::temp_dir().join(format!("bimedit-partage-{}", uuid()));
        std::fs::create_dir(&p).unwrap();
        p
    }
    fn projet_test() -> Projet {
        Projet::lire_json(include_str!("../../../../samples/maison_native.modele.json")).unwrap()
    }
    #[test]
    fn partager_ouvrir_deux_copies_et_reutiliser_sans_ecraser() {
        let root = dossier();
        let entree = root.join("Maison.bimpartage");
        let p = projet_test();
        let partage = creer(&p, &entree).unwrap();
        assert!(partage.depot(&entree).unwrap().join("controle").exists());
        assert!(creer(&p, &entree).is_err());
        let cache = root.join("cache");
        let (a, local_a, mut sa) = ouvrir(&entree, &cache).unwrap();
        let (b, local_b, sb) = ouvrir(&entree, &cache).unwrap();
        assert_ne!(local_a, local_b);
        assert_eq!(a.identite, b.identite);
        assert_ne!(sa.copie.session, sb.copie.session);
        let mut proposition = a.clone();
        proposition.vues[0].nom = "Encore local".into();
        sa.editer(&a, &proposition).unwrap();
        drop(sa);
        let (r, local, s) = ouvrir(&entree, &cache).unwrap();
        assert_eq!(local, local_a);
        assert_eq!(r.vues[0].nom, "Encore local");
        assert_eq!(s.copie.attente.len(), 1);
        assert_eq!(s.copie.base.revision, 0);
    }
    #[test]
    fn entree_incompatible_traversee_et_depot_manquant_refuses() {
        let root = dossier();
        let entree = root.join("Maison.bimpartage");
        let mut p = creer(&projet_test(), &entree).unwrap();
        p.depot = "../ailleurs".into();
        assert!(p.depot(&entree).is_err());
        p.depot = "x".into();
        p.protocole = 999;
        std::fs::write(&entree, encoder(&p).unwrap()).unwrap();
        assert!(ouvrir(&entree, &root.join("cache")).is_err());
    }
    #[test]
    fn ouvrir_ne_recree_jamais_un_controle_central_disparu() {
        let root = dossier();
        let entree = root.join("Maison.bimpartage");
        let partage = creer(&projet_test(), &entree).unwrap();
        let controle = partage.depot(&entree).unwrap().join("controle");
        std::fs::remove_file(&controle).unwrap();
        assert!(ouvrir(&entree, &root.join("cache")).is_err());
        assert!(!controle.exists());
    }
    #[test]
    fn reouverture_conserve_les_conflits_et_les_propositions() {
        let root = dossier();
        let entree = root.join("Maison.bimpartage");
        creer(&projet_test(), &entree).unwrap();
        let cache = root.join("cache");
        let (a, _, mut sa) = ouvrir(&entree, &cache).unwrap();
        let (b, local_b, mut sb) = ouvrir(&entree, &cache).unwrap();
        let mut pa = a.clone();
        pa.vues[0].nom = "Architecte A".into();
        let mut pb = b.clone();
        pb.vues[0].nom = "Architecte B".into();
        sa.editer(&a, &pa).unwrap();
        sb.editer(&b, &pb).unwrap();
        for s in [&mut sa, &mut sb] {
            s.synchroniser(None, false).unwrap();
            let fin = std::time::Instant::now() + std::time::Duration::from_secs(5);
            loop {
                if s.recevoir().unwrap().is_some() {
                    break;
                }
                assert!(std::time::Instant::now() < fin);
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
        }
        assert!(!sb.copie.conflits.is_empty());
        drop(sb);
        let (p, reutilise, s) = ouvrir(&entree, &cache).unwrap();
        assert_eq!(reutilise, local_b);
        assert_eq!(p.vues[0].nom, "Architecte B");
        assert_eq!(s.travail().unwrap().vues[0].nom,"Architecte A");
        assert_eq!(s.copie.attente.len(), 1);
        assert!(!s.copie.conflits.is_empty());
    }
    #[test]
    fn entree_absente_retrouve_copie_hors_connexion() {
        let root = dossier();
        let entree = root.join("Maison.bimpartage");
        let p = projet_test();
        let partage = creer(&p, &entree).unwrap();
        let cache = root.join("cache");
        let (_, local, s) = ouvrir(&entree, &cache).unwrap();
        drop(s);
        std::fs::remove_file(&entree).unwrap();
        let central = partage.depot(&entree).unwrap();
        let arret = root.join("serveur-indisponible");
        std::fs::rename(&central, &arret).unwrap();
        let (r, reutilise, s) = ouvrir(&entree, &cache).unwrap();
        assert_eq!(local, reutilise);
        assert_eq!(r.identite, p.identite);
        assert!(s.etat.starts_with("Hors connexion"));
        assert!(!central.exists());
    }
}
