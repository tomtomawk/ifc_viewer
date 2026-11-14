//! Administration reproductible ; mêmes sessions et mêmes transactions que l'UI.
use crate::edition::Execution;
use super::*;
use std::path::{Path, PathBuf};
use std::time::Instant;

pub fn executer(args: &[String]) -> Option<Resultat<()>> {
    if args.first().map(String::as_str) != Some("--collaboration") {
        return None;
    }
    Some((|| {
        let action=args.get(1).ok_or("Action : associer, synchroniser, etat, editer, reserver, reprendre, liberer, instantane, recuperer, auditer, mesures")?;
        let chemin = PathBuf::from(args.get(2).ok_or("Chemin local requis")?);
        if action == "sauvegarde" {
            let destination =
                PathBuf::from(args.get(3).ok_or("Dossier de sauvegarde neuf requis")?);
            if destination.exists() {
                return Err("La destination doit être un nouveau dossier".into());
            }
            let depot = stockage::Depot::nouveau(&chemin);
            let mut a = depot.proteger()?;
            a.lire()?.ok_or("Dépôt vide")?;
            std::fs::create_dir_all(&destination).map_err(|e| e.to_string())?;
            for categorie in ["ressources", "paquets", "instantanes"] {
                std::fs::create_dir_all(destination.join(categorie)).map_err(|e| e.to_string())?;
                for f in std::fs::read_dir(chemin.join(categorie)).map_err(|e| e.to_string())? {
                    let f = f.map_err(|e| e.to_string())?;
                    let nom = f.file_name();
                    let b = depot.lire(categorie, &nom.to_string_lossy())?;
                    crate::echanges::enregistrement::ecrire_atomique(
                        &destination.join(categorie).join(nom),
                        &b,
                    )?;
                }
            }
            for nom in ["controle", "repere-instantane.json"] {
                let source = chemin.join(nom);
                if source.exists() {
                    let b = if nom == "controle" {
                        a.copier_registre()?
                    } else {
                        std::fs::read(source).map_err(|e| e.to_string())?
                    };
                    crate::echanges::enregistrement::ecrire_atomique(&destination.join(nom), &b)?;
                }
            }
            println!("Sauvegarde cohérente : {}", destination.display());
            return Ok(());
        }
        if action == "auditer" {
            let d = stockage::Depot::nouveau(chemin);
            let mut a = d.proteger()?;
            let e = a.lire()?.ok_or("Dépôt vide")?;
            println!(
                "Projet {} r{} : {} transactions acceptées",
                e.projet,
                e.revision,
                e.acceptes.len()
            );
            return Ok(());
        }
        let p = Projet::lire_json(&std::fs::read_to_string(&chemin).map_err(|e| e.to_string())?)?;
        if action == "mesures" {
            let sortie = Path::new(args.get(3).ok_or("Rapport JSON requis")?);
            return mesurer(&p, &chemin, sortie);
        }
        let central = if action == "associer" {
            PathBuf::from(args.get(3).ok_or("Dossier partagé requis")?)
        } else {
            let c = session::Session::lire_copie(&chemin.with_extension("collaboration.json"))?;
            c.central
        };
        let mut s = session::Session::ouvrir(&p, chemin.clone(), central)?;
        match action.as_str() {
            "etat" => {
                println!(
                    "{}",
                    String::from_utf8(encoder(&s.copie)?).map_err(|e| e.to_string())?
                );
                return Ok(());
            }
            "editer" => {
                let commande =
                    std::fs::read_to_string(args.get(3).ok_or("Fichier commande JSON requis")?)
                        .map_err(|e| e.to_string())?;
                let avant = s.travail()?;
                let (apres, ids) =
                    avant.executer(serde_json::from_str(&commande).map_err(|e| e.to_string())?)?;
                maillage::depuis_natif(&apres, &chemin)?;
                s.editer(&avant, &apres)?;
                crate::echanges::enregistrement::enregistrer_natif(&apres, &chemin)?;
                println!("Enregistré localement — en attente : {ids:?}");
                return Ok(());
            }
            "recuperer" => {
                let sortie = Path::new(args.get(3).ok_or("Sortie JSON requise")?);
                crate::echanges::enregistrement::enregistrer_natif(&s.affichage()?, sortie)?;
                return Ok(());
            }
            "texte-recuperer" => {
                let id = args.get(3).ok_or("Identifiant annotation requis")?;
                let sortie = Path::new(args.get(4).ok_or("Fichier texte de sortie requis")?);
                crate::echanges::enregistrement::ecrire_atomique(
                    sortie,
                    s.texte_local(id)?.contenu()?.as_bytes(),
                )?;
                return Ok(());
            }
            "associer" | "synchroniser" | "instantane" => {
                s.synchroniser(None, action == "instantane")?
            }
            "reserver" | "reprendre" | "liberer" => {
                let cles = args[3..].to_vec();
                if cles.is_empty() {
                    return Err("Clés requises : collection/identifiant".into());
                }
                s.synchroniser(
                    Some((cles, action == "reprendre", action == "liberer")),
                    false,
                )?;
            }
            _ => return Err("Action inconnue".into()),
        }
        let limite = Instant::now() + std::time::Duration::from_secs(120);
        loop {
            if s.recevoir()?.is_some() {
                crate::echanges::enregistrement::enregistrer_natif(&s.affichage()?, &chemin)?;
                println!("{} · r{}", s.etat, s.copie.base.revision);
                break;
            }
            if s.etat.starts_with("Hors connexion") {
                return Err(s.etat.clone());
            }
            if Instant::now() > limite {
                return Err("Délai dépassé ; état local conservé".into());
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        if !s.copie.conflits.is_empty() {
            return Err("Conflits conservés : ouvrir la copie dans BIMedit".into());
        }
        Ok(())
    })())
}

fn memoire_residente() -> Option<usize> {
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::System::{
            ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS},
            Threading::GetCurrentProcess,
        };
        let mut compteurs: PROCESS_MEMORY_COUNTERS = std::mem::zeroed();
        compteurs.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
        if GetProcessMemoryInfo(
            GetCurrentProcess(),
            &mut compteurs,
            std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
        ) != 0
        {
            return Some(compteurs.WorkingSetSize);
        }
    }
    None
}
fn mesurer(p: &Projet, chemin: &Path, sortie: &Path) -> Resultat<()> {
    let racine = std::env::temp_dir().join(format!("bimedit-mesures-{}", uuid()));
    let d = stockage::Depot::nouveau(&racine);
    d.preparer()?;
    let debut = Instant::now();
    let initial = stockage::Etat::initial(p)?;
    let initial_ms = debut.elapsed().as_secs_f64() * 1000.0;
    let mut apres = p.clone();
    apres.nom.push_str(" — mesure");
    let debut = Instant::now();
    let mut t = Transaction::entre(p, &apres, "mesure", 0)?;
    let comparaison_ms = debut.elapsed().as_secs_f64() * 1000.0;
    let debut = Instant::now();
    let mut c = session::Session::ouvrir(p, racine.join("locale.json"), racine.join("central"))?;
    c.editer(p, &apres)?;
    let validation_commit_local_ms = debut.elapsed().as_secs_f64() * 1000.0;
    let debut = Instant::now();
    let scene = maillage::depuis_natif(&apres, chemin)?;
    let geometrie_ms = debut.elapsed().as_secs_f64() * 1000.0;
    stockage::reinitialiser_mesures();
    {
        let mut a = d.proteger()?;
        let mut e = a.initialiser(p)?;
        let cles: Vec<_> = t.cles_reservation(&e.objets).into_iter().collect();
        a.reserver(&mut e, &cles, "mesure", false, false)?;
        for k in cles {
            t.reservations.insert(k.clone(), e.baux[&k].generation);
        }
        let conflits = a.publier(&mut e, &t)?;
        if !conflits.is_empty() {
            return Err(format!("Mesure refusée : {conflits:?}"));
        }
    }
    let fichiers = stockage::mesures();
    let debut = Instant::now();
    {
        d.proteger()?.lire()?;
    }
    let rattrapage_ms = debut.elapsed().as_secs_f64() * 1000.0;
    {
        let mut a = d.proteger()?;
        let e = a.lire()?.ok_or("État absent")?;
        a.instantane(&e)?;
    }
    let debut = Instant::now();
    {
        d.proteger()?.lire()?;
    }
    let instantane_queue_ms = debut.elapsed().as_secs_f64() * 1000.0;
    let rapport = serde_json::json!({"profil":if cfg!(debug_assertions){"debug"}else{"release"},"materiel":"disque local Windows ; NAS non qualifié","initialisation_ms":initial_ms,"comparaison_ms":comparaison_ms,"validation_et_commit_local_ms":validation_commit_local_ms,"reconstruction_geometrique_ms":geometrie_ms,"transfert_gpu_ms":null,"latence_evenement_interface_ms":null,"memoire_residente_octets":memoire_residente(),"taille_etat_serialise_octets":encoder(&initial)?.len(),"sommets":scene.vertices.len(),"ouverture_journal_ms":rattrapage_ms,"ouverture_instantane_et_queue_ms":instantane_queue_ms,"fichiers_initialisation_et_publication":fichiers,"donnees_essai":racine});
    crate::echanges::enregistrement::ecrire_atomique(
        sortie,
        &serde_json::to_vec_pretty(&rapport).map_err(|e| e.to_string())?,
    )?;
    println!("{}", sortie.display());
    Ok(())
}
