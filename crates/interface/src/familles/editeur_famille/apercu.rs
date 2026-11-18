//! Visibilité des éléments dans l'éditeur de familles (F9) : tous montrés pour l'édition,
//! ceux qu'une vue du projet cacherait (niveau de détail, type de vue, « Visible si »)
//! signalés dans l'arborescence ; l'aperçu « comme dans le projet » les retire de la vue,
//! exactement comme une occurrence placée. Réglages de visibilité d'un élément.
use super::*;
use modele::famille::modelee::{Affichage, GenreParametre, NiveauDetail, Parametre, TypeVue, VisibiliteElement};

/// Élément de la famille désigné par son genre et son indice.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum ElementVisible {
    Volume(usize),
    Forme(usize),
    Element2D(usize),
}

impl ElementVisible {
    pub(crate) fn depuis_cle(genre: &str, k: usize) -> Option<Self> {
        Some(match genre {
            "volume" => Self::Volume(k),
            "extrusion" | "forme" => Self::Forme(k),
            "element_2d" => Self::Element2D(k),
            _ => return None,
        })
    }
}

impl EditeurFamille {
    /// Affichage de la vue de famille ouverte : niveau de détail de la barre de vue, type
    /// de la vue (plan, coupe ou élévation, 3D).
    pub(super) fn affichage(&self) -> Affichage {
        Affichage { detail: self.detail, vue: if self.vue_3d { TypeVue::Vue3d } else { TypeVue::depuis_esquisse(self.vue.esquisse()) } }
    }

    /// Visibilité paramétrique et réglages de visibilité d'un élément.
    pub(crate) fn visibilite(&self, e: ElementVisible) -> Option<(&Option<String>, &VisibiliteElement)> {
        match e {
            ElementVisible::Volume(k) => self.famille.volumes.get(k).map(|v| (&v.visible, &v.visibilite)),
            ElementVisible::Forme(k) => self.famille.extrusions.get(k).map(|v| (&v.visible, &v.visibilite)),
            ElementVisible::Element2D(k) => self.famille.elements_2d.get(k).map(|v| (&v.visible, &v.visibilite)),
        }
    }

    pub(crate) fn visibilite_mut(&mut self, e: ElementVisible) -> Option<(&mut Option<String>, &mut VisibiliteElement)> {
        match e {
            ElementVisible::Volume(k) => self.famille.volumes.get_mut(k).map(|v| (&mut v.visible, &mut v.visibilite)),
            ElementVisible::Forme(k) => self.famille.extrusions.get_mut(k).map(|v| (&mut v.visible, &mut v.visibilite)),
            ElementVisible::Element2D(k) => self.famille.elements_2d.get_mut(k).map(|v| (&mut v.visible, &mut v.visibilite)),
        }
    }

    /// L'élément serait caché par une vue du projet de même niveau de détail et de même
    /// type que la vue ouverte, avec les valeurs courantes des paramètres.
    pub(super) fn serait_cache(&self, r: &Resolution, e: ElementVisible) -> bool {
        self.visibilite(e).is_some_and(|(parametre, v)| !self.famille.visible(r, parametre) || !v.montre(self.affichage()))
    }

    /// Solide (volumes puis formes à profil, indice de `FamilleModelee::solides`) montré
    /// par la vue de l'éditeur : tous, sauf ceux que l'aperçu retire.
    pub(super) fn montre_solide(&self, s: &modele::famille::modelee::SolideFamille) -> bool {
        !self.apercu || (s.visible && s.visibilite.montre(self.affichage()))
    }

    /// Élément 2D montré par la vue de l'éditeur.
    pub(super) fn montre_element_2d(&self, r: &Resolution, k: usize) -> bool {
        !self.apercu || !self.serait_cache(r, ElementVisible::Element2D(k))
    }

    /// Vide qui perce le mur générique de la vue : tous, sauf ceux masqués par leur
    /// paramètre en aperçu (un vide perce toujours, quel que soit le niveau de détail).
    pub(super) fn vide_actif(&self, r: &Resolution, k: usize) -> bool {
        !self.apercu || self.famille.visible(r, &self.famille.volumes[k].visible)
    }

    /// Libellé d'un élément de l'arborescence : texte faible et italique s'il serait caché.
    pub(super) fn libelle_arbre(&self, r: &Resolution, e: ElementVisible, texte: String) -> RichText {
        let t = RichText::new(texte);
        if self.serait_cache(r, e) { t.weak().italics() } else { t }
    }

    /// Règle la visibilité d'un élément (pilote) : clés `faible`, `moyen`, `eleve`,
    /// `plan`, `coupe`, `vue_3d` (booléens) et `visible_si` (paramètre oui/non ou null).
    pub(crate) fn regler_visibilite(&mut self, e: ElementVisible, reglage: &serde_json::Value) -> Result<(), String> {
        let objet = reglage.as_object().ok_or("Objet de visibilité attendu")?;
        let oui_non: Vec<String> = self.famille.parametres.iter().filter(|p| p.genre == GenreParametre::OuiNon).map(|p| p.nom.clone()).collect();
        let (parametre, v) = self.visibilite_mut(e).ok_or("Élément introuvable")?;
        let mut nouvelle = *v;
        let mut nouveau_parametre = parametre.clone();
        for (cle, valeur) in objet {
            if cle == "visible_si" {
                nouveau_parametre = match valeur {
                    serde_json::Value::Null => None,
                    serde_json::Value::String(n) if oui_non.contains(n) => Some(n.clone()),
                    _ => return Err("visible_si : nom d'un paramètre oui/non ou null attendu".into()),
                };
                continue;
            }
            let b = valeur.as_bool().ok_or_else(|| format!("{cle} : booléen attendu"))?;
            let case = if let Some(d) = NiveauDetail::depuis_cle(cle) { nouvelle.detail_mut(d) } else {
                match cle.as_str() { "plan" => &mut nouvelle.plan, "coupe" => &mut nouvelle.coupe, "vue_3d" => &mut nouvelle.vue_3d, _ => return Err(format!("Clé de visibilité inconnue : {cle}")) }
            };
            *case = b;
        }
        *v = nouvelle;
        *parametre = nouveau_parametre;
        Ok(())
    }
}

/// Réglages de visibilité d'un élément dans le panneau des propriétés : niveaux de
/// détail, types de vue et paramètre oui/non (« Visible si ») ; vrai au changement.
pub(super) fn visibilite_ui(ui: &mut egui::Ui, parametre: &mut Option<String>, v: &mut VisibiliteElement, parametres: &[Parametre], id: (&str, usize)) -> bool {
    let mut change = false;
    ui.label("Niveau de détail").on_hover_text("Niveaux de détail des vues du projet qui montrent l'élément");
    ui.horizontal(|ui| {
        for d in NiveauDetail::TOUS { change |= ui.checkbox(v.detail_mut(d), d.libelle()).changed(); }
    });
    ui.end_row();
    ui.label("Vues").on_hover_text("Types de vue du projet qui montrent l'élément");
    ui.horizontal(|ui| {
        for t in TypeVue::TOUS { change |= ui.checkbox(v.vue_mut(t), t.libelle()).changed(); }
    });
    ui.end_row();
    ui.label("Visible si").on_hover_text("Paramètre oui/non : non masque l'élément (un vide masqué ne perce plus)");
    egui::ComboBox::from_id_salt(("visible_si", id.0, id.1)).selected_text(parametre.clone().unwrap_or_else(|| "Toujours".into())).show_ui(ui, |ui| {
        change |= ui.selectable_value(parametre, None, "Toujours").changed();
        for p in parametres.iter().filter(|p| p.genre == GenreParametre::OuiNon) {
            change |= ui.selectable_value(parametre, Some(p.nom.clone()), &p.nom).changed();
        }
    });
    ui.end_row();
    change
}

impl ViewerApp {
    /// Pilote de la vue de famille : `famille_vue [etat|detail faible|moyen|eleve|apercu
    /// oui|non]` et `famille_visibilite volume|extrusion|element_2d <indice> <objet>`.
    pub(super) fn piloter_apercu(&mut self, commande: &str, argument: &str) -> Option<Result<serde_json::Value, String>> {
        if !["famille_vue", "famille_visibilite"].contains(&commande) { return None; }
        Some((|| {
            let ed = self.famille.as_mut().ok_or("Éditeur fermé")?;
            if commande == "famille_visibilite" {
                let mut mots = argument.splitn(3, ' ');
                let (genre, indice, reglage) = (mots.next().unwrap_or(""), mots.next().unwrap_or(""), mots.next().unwrap_or(""));
                let usage = "famille_visibilite volume|extrusion|element_2d <indice> {\"faible\":false,…,\"visible_si\":null}";
                let e = indice.parse().ok().and_then(|k| ElementVisible::depuis_cle(genre, k)).ok_or(usage)?;
                let reglage: serde_json::Value = serde_json::from_str(reglage).map_err(|_| usage)?;
                ed.regler_visibilite(e, &reglage)?;
            } else {
                match argument.split_once(' ').unwrap_or((argument, "")) {
                    ("" | "etat", _) => {}
                    ("detail", d) => ed.detail = NiveauDetail::depuis_cle(d).ok_or("detail faible|moyen|eleve")?,
                    ("apercu", "oui") => ed.apercu = true,
                    ("apercu", "non") => ed.apercu = false,
                    _ => return Err("famille_vue etat|detail faible|moyen|eleve|apercu oui|non".into()),
                }
            }
            let r = ed.resoudre();
            let caches = |n: usize, f: fn(usize) -> ElementVisible| (0..n).filter(|k| ed.serait_cache(&r, f(*k))).collect::<Vec<_>>();
            Ok(serde_json::json!({
                "detail": ed.detail.cle(), "apercu": ed.apercu, "type_vue": ed.affichage().vue.libelle(),
                "caches": {"volumes": caches(ed.famille.volumes.len(), ElementVisible::Volume), "extrusions": caches(ed.famille.extrusions.len(), ElementVisible::Forme), "elements_2d": caches(ed.famille.elements_2d.len(), ElementVisible::Element2D)},
            }))
        })())
    }
}
