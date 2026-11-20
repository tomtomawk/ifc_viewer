//! Sélecteur de type d'une sélection multiple : un sélecteur par sorte d'élément présente
//! (murs, murs composés, éléments horizontaux, portes et fenêtres, ouvertures libres,
//! composants, terrains, garde-corps, arbres), même quand la sélection mélange plusieurs
//! sortes. Le type choisi s'applique à tous les éléments de cette sorte ; « Types variés »
//! quand ils diffèrent. Les dessins 2D ont leur sélecteur de type de trait
//! (`proprietes_formes_2d`).
use crate::*;
use commandes::{Commande, Execution};
use modele::Projet;

/// Type à appliquer à des éléments ; `None` : type « Standard » (garde-corps).
type Appliquer = fn(&mut Projet, &[String], Option<&str>) -> Result<(), String>;

/// Une sorte d'élément de la sélection et ses types interchangeables.
struct Groupe {
    libelle: &'static str,
    cibles: Vec<String>,
    actuels: Vec<Option<String>>,
    types: Vec<(Option<String>, String)>,
    appliquer: Appliquer,
    /// Le changement touche les murs : menuiseries recalées après coup.
    murs: bool,
}

fn types_murs(p: &Projet, composes: bool) -> Vec<(Option<String>, String)> {
    p.types_murs.iter().filter(|t| (t.couches.len() > 1) == composes).map(|t| (Some(t.id.clone()), t.nom.clone())).collect()
}

fn appliquer_murs(q: &mut Projet, ids: &[String], t: Option<&str>) -> Result<(), String> {
    let t = t.ok_or("Type de mur manquant.")?;
    for id in ids {
        if let Some(m) = q.murs.iter_mut().find(|m| &m.id == id) {
            m.type_mur = t.into();
        }
        q.eclater_mur(id)?;
    }
    Ok(())
}

/// Murs composés : couches reconstruites d'après le type, première couche recalée sur
/// l'axe de l'assemblage (comme pour un mur composé sélectionné seul).
fn appliquer_assemblages(q: &mut Projet, ids: &[String], t: Option<&str>) -> Result<(), String> {
    let t = t.ok_or("Type de mur manquant.")?;
    for a in ids {
        let (axe, _, _) = modele::volumes::solides::hote(q, a)?;
        let decalage = q.type_mur(t).and_then(|x| x.decalages_couches().first().copied()).unwrap_or(0.0);
        let premiere = q.assemblage(a).and_then(|x| x.couches.first()).cloned();
        if let Some(x) = q.assemblages.iter_mut().find(|x| &x.id == a) {
            x.type_mur = t.into();
        }
        if let Some(m) = q.murs.iter_mut().find(|m| Some(&m.id) == premiere.as_ref()) {
            m.axe = modele::murs::assemblage::decaler_axe(&axe, decalage);
        }
        *q = q.executer(Commande::ActualiserType { assemblage: a.clone() })?.0;
    }
    Ok(())
}

fn appliquer_dalles(q: &mut Projet, ids: &[String], t: Option<&str>) -> Result<(), String> {
    let t = t.ok_or("Type manquant.")?;
    q.dalles.iter_mut().filter(|d| ids.contains(&d.id)).for_each(|d| d.type_dalle = t.into());
    Ok(())
}

fn appliquer_ouvertures(q: &mut Projet, ids: &[String], t: Option<&str>) -> Result<(), String> {
    let t = q.types_ouvertures.iter().find(|x| Some(x.id.as_str()) == t).cloned().ok_or("Type d'ouverture introuvable.")?;
    for o in q.ouvertures.iter_mut().filter(|o| ids.contains(&o.id)) {
        o.type_ouverture = Some(t.id.clone());
        o.representation = None;
        (o.largeur, o.hauteur, o.genre) = (t.largeur, t.hauteur, t.genre);
    }
    Ok(())
}

fn appliquer_composants(q: &mut Projet, ids: &[String], t: Option<&str>) -> Result<(), String> {
    let t = t.ok_or("Type manquant.")?;
    q.composants.iter_mut().filter(|c| ids.contains(&c.id)).for_each(|c| c.type_composant = t.into());
    Ok(())
}

fn appliquer_terrains(q: &mut Projet, ids: &[String], t: Option<&str>) -> Result<(), String> {
    // le type par défaut devient un type du projet dès qu'on l'affecte
    q.fixer_types_terrains();
    q.terrains.iter_mut().filter(|x| ids.contains(&x.id)).for_each(|x| x.type_terrain = t.map(str::to_string));
    Ok(())
}

fn appliquer_garde_corps(q: &mut Projet, ids: &[String], t: Option<&str>) -> Result<(), String> {
    let type_gc = t.and_then(|id| q.types_garde_corps.iter().find(|x| x.id == id)).cloned();
    for g in q.garde_corps.iter_mut().filter(|g| ids.contains(&g.id)) {
        g.type_garde_corps = t.map(str::to_string);
        if let Some(x) = &type_gc {
            (g.hauteur, g.remplissage) = (x.hauteur, x.remplissage);
        }
    }
    Ok(())
}

fn appliquer_arbres(q: &mut Projet, ids: &[String], t: Option<&str>) -> Result<(), String> {
    use modele::elements::arbre::Essence;
    let essence = Essence::TOUTES.into_iter().find(|e| Some(e.nom()) == t).ok_or("Essence inconnue.")?;
    q.arbres.iter_mut().filter(|a| ids.contains(&a.id)).for_each(|a| a.essence = essence);
    Ok(())
}

/// Sortes d'éléments de la sélection qui ont des types, dans un ordre fixe.
fn groupes(p: &Projet, ids: &[String]) -> Vec<Groupe> {
    use modele::GenreOuverture as G;
    let mut murs = Vec::new();
    let mut assemblages = Vec::new();
    for id in ids {
        let mur = p.murs.iter().find(|m| &m.id == id);
        // un mur composé, ou l'une de ses couches : le mur composé
        if let Some(a) = p.assemblage(id).or_else(|| mur.and_then(|m| m.assemblage.as_deref()).and_then(|a| p.assemblage(a))) {
            if !assemblages.contains(&a.id) { assemblages.push(a.id.clone()); }
        } else if let Some(m) = mur {
            murs.push(m.id.clone());
        }
    }
    let mut out = Vec::new();
    // une sorte sans aucun type dans le projet n'a rien à proposer
    let mut ajouter = |libelle, cibles: Vec<String>, actuels: Vec<Option<String>>, types: Vec<(Option<String>, String)>, appliquer, murs| {
        if !cibles.is_empty() && !types.is_empty() {
            out.push(Groupe { libelle, cibles, actuels, types, appliquer, murs });
        }
    };
    let actuels = murs.iter().filter_map(|id| p.murs.iter().find(|m| &m.id == id)).map(|m| Some(m.type_mur.clone())).collect();
    ajouter("Murs", murs, actuels, types_murs(p, false), appliquer_murs as Appliquer, true);
    let actuels = assemblages.iter().filter_map(|id| p.assemblage(id)).map(|a| Some(a.type_mur.clone())).collect();
    ajouter("Murs composés", assemblages, actuels, types_murs(p, true), appliquer_assemblages, true);
    let dalles: Vec<_> = p.dalles.iter().filter(|d| ids.contains(&d.id)).collect();
    let types = p.types_dalles.iter().map(|t| (Some(t.id.clone()), t.nom.clone())).collect();
    ajouter("Éléments horizontaux", dalles.iter().map(|d| d.id.clone()).collect(), dalles.iter().map(|d| Some(d.type_dalle.clone())).collect(), types, appliquer_dalles, false);
    for (libelle, libre) in [("Portes et fenêtres", false), ("Ouvertures libres", true)] {
        let ouvertures: Vec<_> = p.ouvertures.iter().filter(|o| ids.contains(&o.id) && (o.genre == G::Libre) == libre).collect();
        let types = p.types_ouvertures.iter().filter(|t| (t.genre == G::Libre) == libre).map(|t| (Some(t.id.clone()), t.nom.clone())).collect();
        ajouter(libelle, ouvertures.iter().map(|o| o.id.clone()).collect(), ouvertures.iter().map(|o| o.type_ouverture.clone()).collect(), types, appliquer_ouvertures, false);
    }
    let composants: Vec<_> = p.composants.iter().filter(|c| ids.contains(&c.id)).collect();
    let types = p.types_composants.iter().map(|t| (Some(t.id.clone()), t.nom.clone())).collect();
    ajouter("Composants", composants.iter().map(|c| c.id.clone()).collect(), composants.iter().map(|c| Some(c.type_composant.clone())).collect(), types, appliquer_composants, false);
    let terrains: Vec<_> = p.terrains.iter().filter(|t| ids.contains(&t.id)).collect();
    let types = p.types_terrains_disponibles().into_iter().map(|t| (Some(t.id), t.nom)).collect();
    ajouter("Terrains", terrains.iter().map(|t| t.id.clone()).collect(), terrains.iter().map(|t| Some(p.type_terrain(t).id)).collect(), types, appliquer_terrains, false);
    let garde_corps: Vec<_> = p.garde_corps.iter().filter(|g| ids.contains(&g.id)).collect();
    let mut types: Vec<(Option<String>, String)> = vec![(None, "Standard".into())];
    types.extend(p.types_garde_corps.iter().map(|t| (Some(t.id.clone()), t.nom.clone())));
    ajouter("Garde-corps", garde_corps.iter().map(|g| g.id.clone()).collect(), garde_corps.iter().map(|g| g.type_garde_corps.clone()).collect(), types, appliquer_garde_corps, false);
    let arbres: Vec<_> = p.arbres.iter().filter(|a| ids.contains(&a.id)).collect();
    let types = modele::elements::arbre::Essence::TOUTES.into_iter().map(|e| (Some(e.nom().to_string()), e.nom().to_string())).collect();
    ajouter("Arbres", arbres.iter().map(|a| a.id.clone()).collect(), arbres.iter().map(|a| Some(a.essence.nom().to_string())).collect(), types, appliquer_arbres, false);
    out
}

/// Applique le type `t` aux éléments du groupe `g`, sur une copie du projet.
fn changer_type(p: &Projet, g: &Groupe, t: Option<&str>) -> Result<Projet, String> {
    let mut q = p.clone();
    (g.appliquer)(&mut q, &g.cibles, t)?;
    if g.murs {
        q.recaler_menuiseries(p)?;
    }
    q.verifier()?;
    Ok(q)
}

/// Sorte d'un élément pour le report d'une modification : sa collection ; un mur seul
/// et une couche de mur composé diffèrent, comme deux formes d'annotation.
fn sorte(p: &Projet, id: &str) -> Option<String> {
    if let Some(m) = p.murs.iter().find(|m| m.id == id) {
        return Some(format!("mur:{}", m.assemblage.is_some()));
    }
    if let Some(a) = p.annotations.iter().find(|a| a.id == id) {
        return Some(format!("annotation:{:?}", std::mem::discriminant(&a.forme)));
    }
    // niveaux : chacun a son altitude, rien à reporter de l'un à l'autre
    let genre = crate::socle::document_bim::elements_natifs(p).find(|(_, c)| c.as_str() == id).map(|(g, _)| g).filter(|g| *g != "niveau")
        .or_else(|| p.pieces.iter().any(|x| x.id == id).then_some("piece"))
        .or_else(|| p.quadrillages.iter().any(|x| x.id == id).then_some("quadrillage"))
        .or_else(|| p.vues.iter().any(|x| x.id == id).then_some("vue"))?;
    Some(genre.to_string())
}

/// Élément principal (le premier) et éléments sélectionnés de même sorte, principal
/// compris ; None sans pareil (sélection d'un seul élément de sa sorte).
pub(crate) fn pareils(p: &Projet, ids: &[String]) -> Option<(String, Vec<String>)> {
    let principal = ids.first()?;
    let s = sorte(p, principal)?;
    let memes: Vec<String> = ids.iter().filter(|id| sorte(p, id).as_ref() == Some(&s)).cloned().collect();
    (memes.len() > 1).then(|| (principal.clone(), memes))
}

impl Application {
    /// Sélecteurs de type de plusieurs éléments : un par sorte d'élément sélectionnée.
    /// Projet modifié si un type est choisi.
    pub(crate) fn selecteur_types_multiples(&mut self, ui: &mut egui::Ui, ids: &[String]) -> Option<Projet> {
        let (p, _) = self.natif.as_ref()?;
        let groupes = groupes(p, ids);
        if groupes.is_empty() {
            // dessins 2D seuls : leur sélecteur de type de trait suit
            let dessins = ids.iter().all(|id| p.annotations.iter().any(|a| &a.id == id && a.forme.forme_2d().is_some()));
            if !dessins {
                ui.add_enabled(false, egui::Button::new(format!("Sélection : {} éléments", ids.len())).min_size(egui::vec2(ui.available_width(), 24.0)))
                    .on_disabled_hover_text("Ces éléments n'ont pas de types interchangeables.");
                ui.separator();
            }
            return None;
        }
        let mut choix: Option<(usize, Option<String>)> = None;
        for (k, g) in groupes.iter().enumerate() {
            let (commun, mixte) = crate::zones::communes::melange(g.actuels.iter().cloned());
            let commun = commun.flatten().filter(|_| !mixte);
            let nom = if mixte {
                "Types variés".to_string()
            } else {
                g.types.iter().find(|t| t.0 == commun).map_or_else(|| commun.clone().unwrap_or_else(|| "Standard".into()), |t| t.1.clone())
            };
            let lignes: Vec<crate::zones::ruban::selecteur::Ligne> = g.types.iter().map(|(id, nom)| crate::zones::ruban::selecteur::Ligne {
                nom, courant: !mixte && *id == commun, etiquette: None, sous_titre: None, detail: None, actions: &[],
            }).collect();
            let titre = format!("{} ({}) · {nom}", g.libelle, g.cibles.len());
            let c = crate::zones::ruban::selecteur::simple(ui, &titre, None, &format!("{titre}\nLe type choisi s'applique à tous les éléments de cette sorte."), &lignes, None);
            if let Some(i) = c.ligne {
                choix = Some((k, g.types[i].0.clone()));
            }
            ui.add_space(4.0);
        }
        ui.separator();
        let (k, t) = choix?;
        let g = &groupes[k];
        if g.actuels.iter().all(|a| *a == t) {
            return None;
        }
        match changer_type(p, g, t.as_deref()) {
            Ok(q) => Some(q),
            Err(e) => {
                self.error = Some(e);
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn projet() -> Projet {
        let p = modele::Projet::lire_json(include_str!("../../../../samples/maison_native.modele.json")).unwrap();
        p.verifier().unwrap();
        p
    }

    #[test]
    fn selection_melangee_un_groupe_par_sorte() {
        let p = projet();
        let mut ids: Vec<String> = p.murs.iter().filter(|m| m.assemblage.is_none()).take(2).map(|m| m.id.clone()).collect();
        ids.extend(p.dalles.iter().take(1).map(|d| d.id.clone()));
        ids.extend(p.ouvertures.iter().take(1).map(|o| o.id.clone()));
        let g = groupes(&p, &ids);
        let libelles: Vec<_> = g.iter().map(|g| g.libelle).collect();
        assert!(libelles.contains(&"Éléments horizontaux"), "{libelles:?}");
        assert!(g.iter().all(|g| !g.types.is_empty()));
        let nombre = |libelle| g.iter().find(|g| g.libelle == libelle).map_or(0, |g| g.cibles.len());
        assert_eq!((nombre("Murs"), nombre("Éléments horizontaux")), (2, 1));
        // ouverture de l'exemple sans type dans le projet : pas de sélecteur vide
        assert_eq!(nombre("Portes et fenêtres") + nombre("Ouvertures libres"), usize::from(!p.types_ouvertures.is_empty()));
    }

    #[test]
    fn pareils_de_meme_sorte() {
        let p = projet();
        let murs: Vec<String> = p.murs.iter().filter(|m| m.assemblage.is_none()).take(2).map(|m| m.id.clone()).collect();
        let mut ids = murs.clone();
        ids.push(p.dalles[0].id.clone());
        assert_eq!(pareils(&p, &ids), Some((murs[0].clone(), murs.clone())));
        // un seul élément de sa sorte : rien à reporter
        assert_eq!(pareils(&p, &[p.dalles[0].id.clone(), murs[0].clone()]), None);
        // couche d'un mur composé : autre sorte qu'un mur seul
        if let Some(couche) = p.murs.iter().find(|m| m.assemblage.is_some()) {
            assert_eq!(pareils(&p, &[murs[0].clone(), couche.id.clone()]), None);
        }
    }

    #[test]
    fn type_applique_a_toute_la_sorte() {
        let p = projet();
        let ids: Vec<String> = p.dalles.iter().map(|d| d.id.clone()).collect();
        let g = groupes(&p, &ids);
        let dalles = g.iter().find(|g| g.libelle == "Éléments horizontaux").unwrap();
        let t = p.types_dalles.last().unwrap().id.clone();
        let q = changer_type(&p, dalles, Some(&t)).unwrap();
        assert!(q.dalles.iter().all(|d| d.type_dalle == t));
    }
}
