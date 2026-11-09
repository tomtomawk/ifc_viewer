//! Nomenclatures : calcul des tableaux et des quantités.
use super::*;

/// Quantités nettes : murs percés, dalles avec trémies. Pas de double compte des assemblages.
pub fn tableau(p: &Projet, n: &Nomenclature) -> Result<Vec<Vec<String>>, String> {
    if !n.colonnes.is_empty() || !n.parametres.is_empty() {
        return tableau_colonnes(p, n);
    }
    let mut groupes: BTreeMap<(String, String), (usize, f64)> = BTreeMap::new();
    let jonctions = jonctions::coupes(p);
    let mut ajouter = |cat: &str, nom: String, v: f64| {
        if !n.categorie.is_empty() && !cat.to_lowercase().contains(&n.categorie.to_lowercase()) {
            return;
        }
        let e = groupes.entry((cat.into(), nom)).or_default();
        e.0 += 1;
        e.1 += v;
    };
    for m in &p.murs {
        ajouter(
            "Murs",
            if n.regrouper {
                p.type_mur(&m.type_mur).unwrap().nom.clone()
            } else {
                format!("{} ({})", m.nom, m.id)
            },
            geometrie::volume(&solides::mur(p, m, &jonctions)?) / 1e9,
        );
    }
    for d in &p.dalles {
        ajouter(
            d.sorte.nom(),
            if n.regrouper {
                p.type_dalle(&d.type_dalle).unwrap().nom.clone()
            } else {
                format!("{} ({})", d.nom, d.id)
            },
            p.volume_dalle(d).unwrap_or(0.0) / 1e9,
        );
    }
    for b in &p.barres {
        let categorie = match b.genre {
            GenreBarre::Poteau => "Poteaux",
            GenreBarre::Poutre => "Poutres",
        };
        ajouter(
            categorie,
            if n.regrouper {
                b.nom.clone()
            } else {
                format!("{} ({})", b.nom, b.id)
            },
            geometrie::volume(&solides::barre(p, b)?) / 1e9,
        );
    }
    for c in &p.composants {
        ajouter("Composants", if n.regrouper { c.nom.clone() } else { format!("{} ({})", c.nom, c.id) }, 0.0);
    }
    for x in &p.pieces {
        ajouter("Pièces", if n.regrouper { x.nom.clone() } else { format!("{} {} ({})", x.numero, x.nom, x.id) }, x.aire() / 1e6);
    }
    for o in &p.ouvertures {
        ajouter(
            match o.genre {
                GenreOuverture::Porte => "Portes",
                GenreOuverture::Fenetre => "Fenêtres",
                GenreOuverture::Libre => "Ouvertures libres",
            },
            o.nom.clone(),
            0.0,
        );
    }
    for e in &p.importes {
        ajouter(
            &e.categorie,
            if n.regrouper {
                e.nom.clone()
            } else {
                format!("{} ({})", e.nom, e.id)
            },
            geometrie::volume(&solides::maillage(&e.maillage)?) / 1e9,
        );
    }
    let mut lignes = vec![vec![
        "Catégorie".into(),
        "Type / élément".into(),
        "Nombre".into(),
        "Volume net (m³)".into(),
    ]];
    for ((cat, nom), (nb, v)) in groupes {
        lignes.push(vec![cat, nom, nb.to_string(), format!("{v:.6}")]);
    }
    Ok(lignes)
}

/// Tableau d'une nomenclature à colonnes choisies : une ligne par élément, ou par
/// groupe (valeurs des colonnes non totalisées) si elle regroupe.
pub(super) fn tableau_colonnes(p: &Projet, n: &Nomenclature) -> Result<Vec<Vec<String>>, String> {
    let jonctions = jonctions::coupes(p);
    let nom_niveau = |id: &str| p.niveau(id).map_or(String::new(), |x| x.nom.clone());
    let mut lignes: Vec<LigneQuantite> = Vec::new();
    for m in &p.murs {
        let t = p.type_mur(&m.type_mur);
        let l=p.trajectoire_justification(m.assemblage.as_deref().unwrap_or(&m.id)).map_or_else(||m.axe.longueur(),|(axe,_)|axe.longueur());
        let h=p.bas_haut(m).map(|(a,b)|b-a);
        lignes.push(LigneQuantite {
            categorie: "Murs".into(),
            type_nom: t.map_or(String::new(), |t| t.nom.clone()),
            element: m.nom.clone(),
            niveau: nom_niveau(&m.niveau),
            longueur: Some(l),
            hauteur: h,
            largeur: None,
            epaisseur: t.map(|t| t.epaisseur()),
            surface: h.map(|h| l * h / 1e6),
            volume: Some(geometrie::volume(&solides::mur(p, m, &jonctions)?) / 1e9),
            parametres: BTreeMap::new(),
        });
    }
    for d in &p.dalles {
        let t = p.type_dalle(&d.type_dalle);
        lignes.push(LigneQuantite {
            categorie: d.sorte.nom().into(),
            type_nom: t.map_or(String::new(), |t| t.nom.clone()),
            element: d.nom.clone(),
            niveau: nom_niveau(&d.niveau),
            longueur: None,
            hauteur: None,
            largeur: None,
            epaisseur: t.map(|t| t.epaisseur()),
            surface: Some(geometrie::aire_pair_impair(&d.contours) / 1e6),
            volume: p.volume_dalle(d).map(|v| v / 1e9),
            parametres: BTreeMap::new(),
        });
    }
    for b in &p.barres {
        let l = DVec3::from_array(b.fin).distance(DVec3::from_array(b.debut));
        lignes.push(LigneQuantite {
            categorie: match b.genre {
                GenreBarre::Poteau => "Poteaux",
                GenreBarre::Poutre => "Poutres",
            }
            .into(),
            type_nom: b.nom.clone(),
            element: format!("{} ({})", b.nom, b.id),
            niveau: nom_niveau(&b.niveau),
            longueur: Some(l),
            hauteur: Some(b.hauteur),
            largeur: Some(b.largeur),
            epaisseur: None,
            surface: None,
            volume: Some(geometrie::volume(&solides::barre(p, b)?) / 1e9),
            parametres: BTreeMap::new(),
        });
    }
    for c in &p.composants {
        let t = p.types_composants.iter().find(|t| t.id == c.type_composant);
        lignes.push(LigneQuantite {
            categorie: "Composants".into(),
            type_nom: t.map_or(String::new(), |t| t.nom.clone()),
            element: format!("{} ({})", c.nom, c.id),
            niveau: nom_niveau(&c.niveau),
            longueur: t.map(|t| t.largeur),
            hauteur: t.map(|t| t.hauteur),
            largeur: t.map(|t| t.profondeur),
            epaisseur: None,
            surface: None,
            volume: None,
            parametres: BTreeMap::new(),
        });
    }
    for x in &p.pieces {
        lignes.push(LigneQuantite {
            categorie: "Pièces".into(),
            type_nom: x.nom.clone(),
            element: format!("{} {} ({})", x.numero, x.nom, x.id),
            niveau: nom_niveau(&x.niveau),
            longueur: Some(x.perimetre()),
            hauteur: None,
            largeur: None,
            epaisseur: None,
            surface: Some(x.aire() / 1e6),
            volume: None,
            parametres: BTreeMap::new(),
        });
    }
    for o in &p.ouvertures {
        // niveau du mur hôte (ou de la première couche de son assemblage)
        let hote = p.murs.iter().find(|m| m.id == o.mur || m.assemblage.as_deref() == Some(o.mur.as_str()));
        lignes.push(LigneQuantite {
            categorie: match o.genre {
                GenreOuverture::Porte => "Portes",
                GenreOuverture::Fenetre => "Fenêtres",
                GenreOuverture::Libre => "Ouvertures libres",
            }
            .into(),
            type_nom: o.type_ouverture.as_ref().and_then(|t| p.types_ouvertures.iter().find(|x| &x.id == t)).map_or(o.nom.clone(), |t| t.nom.clone()),
            element: o.nom.clone(),
            niveau: hote.map_or(String::new(), |m| nom_niveau(&m.niveau)),
            longueur: None,
            hauteur: Some(o.hauteur),
            largeur: Some(o.largeur),
            epaisseur: None,
            surface: Some(o.largeur * o.hauteur / 1e6),
            volume: None,
            parametres: solides::parametres_ouverture(p, o),
        });
    }
    for e in &p.escaliers {
        let c = e.calcul(p).ok();
        lignes.push(LigneQuantite {
            categorie: "Escaliers".into(),
            type_nom: e.nom.clone(),
            element: e.nom.clone(),
            niveau: nom_niveau(&e.niveau),
            longueur: c.map(|c| c.volees),
            hauteur: c.map(|c| c.hauteur),
            largeur: Some(e.largeur),
            epaisseur: None,
            surface: None,
            volume: Some(geometrie::volume(&e.solide(p)?) / 1e9),
            parametres: BTreeMap::new(),
        });
    }
    for g in &p.garde_corps {
        let type_nom = g.type_garde_corps.as_ref().and_then(|id| p.types_garde_corps.iter().find(|t| &t.id == id))
            .map_or_else(|| format!("Garde-corps {} · {}", g.remplissage.nom(), crate::config::options::unite().afficher(g.hauteur)),
                |t| t.nom.clone());
        lignes.push(LigneQuantite {
            categorie: "Garde-corps".into(),
            type_nom,
            element: g.nom.clone(),
            niveau: nom_niveau(&g.niveau),
            longueur: Some(g.longueur()),
            hauteur: Some(g.hauteur),
            largeur: None,
            epaisseur: None,
            surface: None,
            volume: None,
            parametres: BTreeMap::new(),
        });
    }
    for e in &p.importes {
        lignes.push(LigneQuantite {
            categorie: e.categorie.clone(),
            type_nom: e.nom.clone(),
            element: format!("{} ({})", e.nom, e.id),
            niveau: e.niveau.as_deref().map_or(String::new(), nom_niveau),
            longueur: None,
            hauteur: None,
            largeur: None,
            epaisseur: None,
            surface: None,
            volume: Some(geometrie::volume(&solides::maillage(&e.maillage)?) / 1e9),
            parametres: BTreeMap::new(),
        });
    }
    let filtre = n.categorie.to_lowercase();
    lignes.retain(|l| filtre.is_empty() || l.categorie.to_lowercase().contains(&filtre));
    // valeur affichée d'une colonne non totalisée, et valeur d'une colonne totalisée
    let texte = |l: &LigneQuantite, c: Colonne| -> String {
        let mm = |v: Option<f64>| v.map_or(String::new(), |v| format!("{v:.0}"));
        match c {
            Colonne::Categorie => l.categorie.clone(),
            Colonne::Type => l.type_nom.clone(),
            Colonne::Element => l.element.clone(),
            Colonne::Niveau => l.niveau.clone(),
            Colonne::Hauteur => mm(l.hauteur),
            Colonne::Largeur => mm(l.largeur),
            Colonne::Epaisseur => mm(l.epaisseur),
            _ => String::new(),
        }
    };
    let somme = |l: &LigneQuantite, c: Colonne| -> Option<f64> {
        match c {
            Colonne::Nombre => Some(1.0),
            Colonne::Longueur => l.longueur,
            Colonne::Surface => l.surface,
            Colonne::Volume => l.volume,
            _ => None,
        }
    };
    let format_somme = |c: Colonne, v: Option<f64>| match (c, v) {
        (_, None) => String::new(),
        (Colonne::Nombre, Some(v)) => format!("{v:.0}"),
        (Colonne::Longueur, Some(v)) => format!("{v:.0}"),
        (Colonne::Surface, Some(v)) => format!("{v:.2}"),
        (_, Some(v)) => format!("{v:.3}"),
    };
    // sans colonnes choisies : catégorie, type ou élément, nombre, volume
    let defaut = [Colonne::Categorie, if n.regrouper { Colonne::Type } else { Colonne::Element }, Colonne::Nombre, Colonne::Volume];
    let cols: &[Colonne] = if n.colonnes.is_empty() { &defaut } else { &n.colonnes };
    let valeur_parametre = |l: &LigneQuantite, nom: &String| l.parametres.get(nom).cloned().unwrap_or_default();
    // regroupement : clé = valeurs des colonnes non totalisées (et l'élément si on ne
    // regroupe pas)
    let mut groupes: BTreeMap<Vec<String>, Vec<Option<f64>>> = BTreeMap::new();
    for (k, l) in lignes.iter().enumerate() {
        let mut cle: Vec<String> = cols.iter().filter(|c| !c.totalisee()).map(|&c| texte(l, c)).collect();
        cle.extend(n.parametres.iter().map(|nom| valeur_parametre(l, nom)));
        if !n.regrouper {
            cle.push(format!("{:08}", k));
        }
        let e = groupes.entry(cle).or_insert_with(|| vec![None; cols.len()]);
        for (i, &c) in cols.iter().enumerate() {
            if let Some(v) = somme(l, c) {
                e[i] = Some(e[i].unwrap_or(0.0) + v);
            }
        }
    }
    let mut sortie = vec![cols.iter().map(|c| c.titre().to_string()).chain(n.parametres.iter().cloned()).collect::<Vec<_>>()];
    for (cle, sommes) in groupes {
        let mut fixes = cle.into_iter();
        let mut ligne: Vec<String> = cols.iter().enumerate().map(|(i, &c)| if c.totalisee() { format_somme(c, sommes[i]) } else { fixes.next().unwrap_or_default() }).collect();
        // les valeurs des paramètres suivent les colonnes fixes dans la clé
        ligne.extend(fixes.take(n.parametres.len()));
        sortie.push(ligne);
    }
    Ok(sortie)
}
