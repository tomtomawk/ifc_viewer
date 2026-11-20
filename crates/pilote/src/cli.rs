//! Commandes non graphiques du modèle natif (utilisables aussi en CI).
use commandes::Execution;
use modele::{self, Projet};
use std::path::Path;
pub fn executer(args: &[String]) -> Option<Result<(), String>> {
    let commande = args.first()?.as_str();
    if !matches!(
        commande,
        "--editer" | "--export-ifc" | "--export-vue" | "--export-feuille" | "--quantites" | "--mesurer-scene"
    ) {
        return None;
    }
    Some((|| {
        let entree = args.get(1).ok_or("Fichier .modele.json requis.")?;
        let p = Projet::lire_json(&std::fs::read_to_string(entree).map_err(|e| e.to_string())?)?;
        p.verifier()?;
        if commande == "--mesurer-scene" {
            // construction complète, puis reconstruction après le déplacement d'un mur ;
            // résultats sur la sortie d'erreur avec les étapes (BIMEDIT_CHRONO)
            let chemin = Path::new(entree);
            let debut = std::time::Instant::now();
            maillage::depuis_natif(&p, chemin)?;
            eprintln!("scène complète : {:.0} ms", debut.elapsed().as_secs_f64() * 1000.0);
            let repetitions: usize = args.get(2).and_then(|n| n.parse().ok()).unwrap_or(1);
            let mut courant = p;
            for k in 0..repetitions {
                let Some(id) = courant.murs.get(k * 7 % courant.murs.len().max(1)).map(|m| m.assemblage.clone().unwrap_or_else(|| m.id.clone())) else { break };
                let debut = std::time::Instant::now();
                let (suivant, _) = courant.executer(commandes::Commande::Deplacer { ids: vec![id], delta: [0.0, 50.0, 0.0] })?;
                let t_commande = debut.elapsed().as_secs_f64() * 1000.0;
                let debut = std::time::Instant::now();
                maillage::depuis_natif(&suivant, chemin)?;
                eprintln!("mur déplacé : commande {:.0} ms, scène {:.0} ms", t_commande, debut.elapsed().as_secs_f64() * 1000.0);
                courant = suivant;
            }
            return Ok(());
        }
        let arg = args.get(2).ok_or("Argument manquant.")?;
        match commande {
            "--editer" => {
                if Path::new(entree).with_extension("collaboration.json").exists() {
                    if args.get(3).is_some_and(|s| s != entree) { return Err("En collaboration, utilisez une copie locale distincte pour Enregistrer sous".into()); }
                    return collaboration::cli::executer(&["--collaboration".into(),"editer".into(),entree.clone(),arg.clone()]).ok_or("Routage collaboratif absent")?;
                }
                let json = std::fs::read_to_string(arg).map_err(|e| e.to_string())?;
                let c = serde_json::from_str(&json).map_err(|e| e.to_string())?;
                let (p, ids) = p.executer(c)?;
                maillage::depuis_natif(&p, Path::new(entree))?;
                let sortie = args.get(3).map_or(entree, |s| s);
                echanges::enregistrement::enregistrer_natif(&p, Path::new(sortie))?;
                println!("{}", serde_json::to_string(&ids).map_err(|e| e.to_string())?);
            }
            "--export-ifc" => echanges::enregistrement::ecrire_atomique(
                Path::new(arg),
                echanges::export_ifc::exporter_fichier(&p, &Path::new(arg).file_name().map_or_else(|| arg.clone(), |n| n.to_string_lossy().into_owned()))?.as_bytes(),
            )?,
            "--quantites" => {
                let n = modele::documentation::Nomenclature {
                    id: "rapport".into(),
                    nom: "Quantités".into(),
                    categorie: args.get(3).cloned().unwrap_or_default(),
                    regrouper: true,
                    colonnes: Vec::new(),
                    parametres: Vec::new(),
                };
                let lignes = modele::documentation::tableau(&p, &n)?;
                echanges::enregistrement::ecrire_atomique(
                    Path::new(arg),
                    modele::documentation::csv(&lignes).as_bytes(),
                )?;
            }
            _ => {
                let sortie = Path::new(args.get(3).ok_or("Chemin de sortie SVG ou PDF requis.")?);
                let svg = if commande == "--export-feuille" {
                    let f = p
                        .feuilles
                        .iter()
                        .find(|f| f.id == *arg || f.numero == *arg)
                        .ok_or("Feuille absente.")?;
                    moteur2d::svg_feuille(&p, f)?
                } else {
                    moteur2d::dessin_vue(&p, arg)?.svg(arg)
                };
                let b = match sortie.extension().and_then(|s| s.to_str()) {
                    Some("pdf") => moteur2d::pdf(&svg)?,
                    Some("svg") => svg.into_bytes(),
                    _ => return Err("Extension attendue : .svg ou .pdf".into()),
                };
                echanges::enregistrement::ecrire_atomique(sortie, &b)?;
            }
        };
        Ok(())
    })())
}

#[cfg(test)]
mod tests {
    use super::*;
    use modele::documentation::{Annotation, AnnotationForme, PointCote};

    /// `--editer {"action":"supprimer"}` : mêmes annotations détachées que dans l'interface.
    #[test]
    fn editer_supprimer_detache_les_annotations() {
        let mut p = Projet::nouveau("Pilote");
        let mur = p.ajouter_mur([0.0, 0.0], [4000.0, 0.0], &modele::creation::ParametresMur::default()).unwrap();
        let ouverture = commandes::Commande::Ouverture { mur: mur.clone(), genre: modele::GenreOuverture::Porte, type_ouverture: None, abscisse: 1000.0, largeur: 900.0, hauteur: 2100.0, allege: 0.0 };
        let mut p = p.executer(ouverture).unwrap().0;
        let vue = p.ajouter_vue_plan(&p.niveaux[0].id.clone(), 1200.0).unwrap();
        for (id, forme) in [
            ("cote", AnnotationForme::Cote { debut: PointCote::Mur { id: mur.clone(), fin: false }, fin: PointCote::Mur { id: mur.clone(), fin: true }, decalage: 500.0 }),
            ("etiquette", AnnotationForme::Etiquette { element: mur.clone(), point: [2000.0, 500.0] }),
        ] {
            p = p.executer(commandes::Commande::Annoter { annotation: Annotation { id: id.into(), vue: vue.clone(), hauteur: 2.5, forme } }).unwrap().0;
        }
        let dossier = std::env::temp_dir().join(format!("bimedit-supprimer-{}", std::process::id()));
        std::fs::create_dir_all(&dossier).unwrap();
        let (entree, commande) = (dossier.join("p.modele.json"), dossier.join("c.json"));
        std::fs::write(&entree, p.json().unwrap()).unwrap();
        std::fs::write(&commande, format!(r#"{{"action":"supprimer","ids":["{mur}"]}}"#)).unwrap();
        let args: Vec<String> = ["--editer".into(), entree.to_string_lossy().into_owned(), commande.to_string_lossy().into_owned()].into();
        executer(&args).unwrap().unwrap();
        let q = Projet::lire_json(&std::fs::read_to_string(&entree).unwrap()).unwrap();
        let _ = std::fs::remove_dir_all(&dossier);
        q.verifier().unwrap();
        assert!(q.murs.is_empty() && q.ouvertures.is_empty());
        let forme = |id: &str| q.annotations.iter().find(|a| a.id == id).map(|a| a.forme.clone());
        assert!(matches!(forme("cote"), Some(AnnotationForme::Cote { debut: PointCote::Fixe { point: [0.0, _] }, fin: PointCote::Fixe { point: [4000.0, _] }, .. })));
        assert!(matches!(forme("etiquette"), Some(AnnotationForme::Texte { .. })));
    }
}
