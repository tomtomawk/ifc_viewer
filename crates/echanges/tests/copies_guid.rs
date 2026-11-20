//! Copies d'éléments importés (X7) : une copie ne garde pas l'`origine` de l'original,
//! donc pas son GUID IFC ; après copie, tous les GlobalId du fichier exporté sont uniques.
#![allow(clippy::unwrap_used, reason = "tests d'intégration : les fonctions d'aide hors #[test] échouent par panique, comme les tests")]
use commandes::{Commande, Execution};
use echanges::{depuis_ifc, export_ifc};
use modele::Projet;
use std::collections::HashMap;
use std::path::Path;

fn importer(nom: &str) -> Projet {
    let chemin = format!("{}/../../samples/{nom}", env!("CARGO_MANIFEST_DIR"));
    depuis_ifc::convertir(Path::new(&chemin), &|_| {}).unwrap().0
}

/// GlobalId des entités `IfcRoot` : premier argument de 22 caractères suivi de
/// l'`OwnerHistory` (référence ou `$`).
fn guids_en_double(texte: &str) -> Vec<(String, usize)> {
    let mut compte: HashMap<&str, usize> = HashMap::new();
    for ligne in texte.lines() {
        let Some(i) = ligne.find("('") else { continue };
        let reste = &ligne[i + 2..];
        if reste.len() > 24 && &reste[22..24] == "'," && matches!(reste.as_bytes().get(24), Some(b'#' | b'$')) {
            *compte.entry(&reste[..22]).or_default() += 1;
        }
    }
    let mut v: Vec<_> = compte.into_iter().filter(|(_, n)| *n > 1).map(|(g, n)| (g.to_string(), n)).collect();
    v.sort();
    v
}

/// Identifiants des éléments du modèle qui portent une origine.
fn avec_origine(p: &Projet) -> Vec<String> {
    let mut ids = Vec::new();
    ids.extend(p.murs.iter().filter(|x| x.origine.is_some() && x.assemblage.is_none()).map(|x| x.id.clone()));
    ids.extend(p.assemblages.iter().filter(|x| x.origine.is_some()).map(|x| x.id.clone()));
    ids.extend(p.dalles.iter().filter(|x| x.origine.is_some()).map(|x| x.id.clone()));
    ids.extend(p.barres.iter().filter(|x| x.origine.is_some()).map(|x| x.id.clone()));
    ids.extend(p.importes.iter().filter(|x| x.origine.is_some()).map(|x| x.id.clone()));
    ids.extend(p.pieces.iter().filter(|x| x.origine.is_some()).map(|x| x.id.clone()));
    ids
}

#[test]
fn copies_sans_guid_de_l_original() {
    for echantillon in ["test_ifc4_murs.ifc", "test_ifc4_espaces.ifc", "test_ifc4.ifc", "test_ifc4_proprietes.ifc"] {
        let p = importer(echantillon);
        let ids = avec_origine(&p);
        assert!(!ids.is_empty(), "{echantillon} : aucun élément d'origine IFC");
        assert!(guids_en_double(&export_ifc::exporter(&p).unwrap()).is_empty(), "{echantillon} : export de départ");
        // Copier (avec ouvertures), puis Réseau qui passe par Copier
        let (q, copies) = p.executer(Commande::Copier { ids: ids.clone(), delta: [20_000.0, 0.0, 0.0] }).unwrap();
        let (q, _) = q.executer(Commande::Reseau { ids: copies, nombre: 3, delta: [0.0, 30_000.0, 0.0], centre: [0.0, 0.0], angle: 0.0 }).unwrap();
        let texte = export_ifc::exporter(&q).unwrap();
        assert_eq!(guids_en_double(&texte), Vec::<(String, usize)>::new(), "{echantillon}");
        let origines = |q: &Projet| avec_origine(q).len();
        assert_eq!(origines(&q), ids.len(), "{echantillon} : seules les originales gardent leur origine");
    }
}
