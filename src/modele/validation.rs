//! Vérification des références avant affichage, mutation et sauvegarde.
use super::*;
impl Projet {
    pub fn verifier(&self) -> Result<(), String> {
        let mut ids = std::collections::HashSet::new();
        for id in self
            .niveaux
            .iter()
            .map(|x| &x.id)
            .chain(self.murs.iter().map(|x| &x.id))
            .chain(self.dalles.iter().map(|x| &x.id))
            .chain(self.barres.iter().map(|x| &x.id))
            .chain(self.escaliers.iter().map(|x| &x.id))
            .chain(self.garde_corps.iter().map(|x| &x.id))
            .chain(self.assemblages.iter().map(|x| &x.id))
            .chain(self.ouvertures.iter().map(|x| &x.id))
            .chain(self.importes.iter().map(|x| &x.id))
            .chain(self.vues.iter().map(|x| &x.id))
            .chain(self.annotations.iter().map(|x| &x.id))
            .chain(self.quadrillages.iter().map(|x| &x.id))
            .chain(self.feuilles.iter().map(|x| &x.id))
            .chain(self.nomenclatures.iter().map(|x| &x.id))
        {
            if id.is_empty() || !ids.insert(id) {
                return Err(format!("Identifiant vide ou dupliqué : {id}"));
            }
        }
        if self.contextes.keys().any(|id| !ids.contains(id)) {
            return Err("Contexte d'élément sans élément associé.".into());
        }
        for n in &self.niveaux {
            if !n.altitude.is_finite() {
                return Err("Altitude non finie.".into());
            }
        }
        for t in self.types_murs.iter().chain(&self.types_dalles) {
            if t.couches.is_empty()
                || t.couches
                    .iter()
                    .any(|c| !c.epaisseur.is_finite() || c.epaisseur <= 0.0)
            {
                return Err(format!("{} : épaisseur non valide.", t.nom));
            }
        }
        for a in &self.assemblages {
            if self.type_mur(&a.type_mur).is_none() || a.couches.is_empty() {
                return Err("Assemblage sans type ou sans couche.".into());
            }
            let mut membres = std::collections::HashSet::new();
            for id in &a.couches {
                if !membres.insert(id)
                    || !self
                        .murs
                        .iter()
                        .any(|m| &m.id == id && m.assemblage.as_ref() == Some(&a.id))
                {
                    return Err("Relation d'assemblage incohérente.".into());
                }
            }
        }
        for m in &self.murs {
            if self.type_mur(&m.type_mur).is_none() || self.bas_haut(m).is_none() {
                return Err(format!("{} : type ou niveau absent.", m.nom));
            }
            if let Some(a) = &m.assemblage {
                if !self
                    .assemblage(a)
                    .is_some_and(|a| a.couches.contains(&m.id))
                {
                    return Err("Couche sans assemblage.".into());
                }
            }
        }
        for t in &self.types_ouvertures {
            if let Some(f) = &t.famille {
                f.valider(t.largeur, t.hauteur).map_err(|e| format!("{} : {e}", t.nom))?;
            }
            // famille modélisée : contrôlée dans un mur d'essai de 300 mm
            if let Some(f) = &t.modele {
                f.valider(t.largeur, t.hauteur, 300.0).map_err(|e| format!("{} : {e}", t.nom))?;
            }
        }
        for o in &self.ouvertures {
            solides::hote(self, &o.mur)?;
            if ![o.largeur, o.hauteur, o.abscisse, o.allege]
                .iter()
                .all(|x| x.is_finite())
                || o.largeur <= 0.0
                || o.hauteur <= 0.0
            {
                return Err("Ouverture : dimensions non valides.".into());
            }
            if !o.valeurs.values().all(|v| v.is_finite()) || !self.parametres_partages.values().all(|v| v.is_finite()) {
                return Err("Ouverture : valeur de paramètre non finie.".into());
            }
            if let Some(t) = &o.type_ouverture {
                if !self.types_ouvertures.iter().any(|x| &x.id == t) {
                    return Err("Type d'ouverture absent.".into());
                }
            }
            if let Some(m) = &o.representation {
                solides::maillage(m)?;
            }
        }
        for d in &self.dalles {
            if self.niveau(&d.niveau).is_none()
                || self.type_dalle(&d.type_dalle).is_none()
                || !d.decalage.is_finite()
                || d.contours.is_empty()
            {
                return Err("Dalle : niveau, type ou contour absent.".into());
            }
            for c in &d.contours {
                edition::contour_valide(c)?;
            }
            pente::verifier_forme(d)?;
            if d.pentes
                .iter()
                .flatten()
                .flatten()
                .any(|v| !v.is_finite() || *v <= 0.0 || *v >= 89.0)
            {
                return Err("Pente hors de l'intervalle ]0, 89[ degrés.".into());
            }
        }
        for b in &self.barres {
            solides::barre(self, b)?;
        }
        for e in &self.escaliers {
            e.calcul(self).map_err(|x| format!("{} : {x}", e.nom))?;
        }
        for g in &self.garde_corps {
            if self.niveau(&g.niveau).is_none() || g.escalier.as_ref().is_some_and(|e| !self.escaliers.iter().any(|x| &x.id == e)) {
                return Err(format!("{} : niveau ou escalier hôte absent.", g.nom));
            }
            g.solides(self).map_err(|x| format!("{} : {x}", g.nom))?;
        }
        for e in &self.importes {
            if e.niveau.as_ref().is_some_and(|n| self.niveau(n).is_none()) {
                return Err("Élément importé : niveau absent.".into());
            }
            solides::maillage(&e.maillage)?;
        }
        for v in &self.vues {
            if !v.echelle.is_finite() || v.echelle <= 0.0 {
                return Err("Échelle non valide.".into());
            }
            match &v.genre {
                GenreVue::Plan { niveau, coupe }
                    if self.niveau(niveau).is_none() || !coupe.is_finite() =>
                {
                    return Err("Vue : niveau ou coupe non valide.".into());
                }
                GenreVue::Coupe {
                    debut,
                    fin,
                    profondeur,
                    ..
                } if !debut
                    .iter()
                    .chain(fin)
                    .chain([profondeur])
                    .all(|x| x.is_finite())
                    || *profondeur <= 0.0
                    || debut == fin =>
                {
                    return Err("Vue de coupe non valide.".into());
                }
                _ => {}
            }
        }
        for (id, toit) in &self.attaches_toitures {
            if !self.murs.iter().any(|m| &m.id == id) && self.assemblage(id).is_none() {
                return Err("Mur attaché absent.".into());
            }
            if !self
                .dalles
                .iter()
                .any(|d| &d.id == toit && d.sorte == SorteDalle::Toiture)
            {
                return Err("Toiture d'attachement absente.".into());
            }
        }
        for g in &self.quadrillages {
            if g.nom.trim().is_empty()
                || g.debut == g.fin
                || g.debut.iter().chain(&g.fin).any(|v| !v.is_finite())
            {
                return Err("Quadrillage non valide.".into());
            }
        }
        for a in &self.annotations {
            use documentation::{AnnotationForme as F, PointCote};
            if !self.vues.iter().any(|v| v.id == a.vue)
                || !a.hauteur.is_finite()
                || a.hauteur <= 0.0
            {
                return Err("Annotation : vue ou hauteur non valide.".into());
            }
            let point = |p: &[f64; 2]| p.iter().all(|x| x.is_finite());
            let cote = |p: &PointCote| match p {
                PointCote::Fixe { point: p } => point(p),
                PointCote::Mur { .. } => true,
            };
            let valide = match &a.forme {
                F::Texte { point: p, .. } | F::Etiquette { point: p, .. } => point(p),
                F::Ligne { debut, fin } => point(debut) && point(fin) && debut != fin,
                F::Zone { contour } => edition::contour_valide(contour).is_ok(),
                F::Polyligne { points, .. } | F::Spline { points, .. } => points.len() >= 2 && points.iter().all(point),
                F::Cercle { centre, rayon } => point(centre) && rayon.is_finite() && *rayon > 0.0,
                F::Arc { points } => points.iter().all(point) && crate::dessin2d::arc_3_points(points[0].into(), points[1].into(), points[2].into()).is_some(),
                F::Cote {
                    debut,
                    fin,
                    decalage,
                } => cote(debut) && cote(fin) && decalage.is_finite(),
            };
            if !valide {
                return Err("Coordonnées d'annotation non valides.".into());
            }
        }
        for f in &self.feuilles {
            if ![f.largeur, f.hauteur]
                .iter()
                .all(|x| x.is_finite() && *x > 0.0 && *x <= 5000.0)
            {
                return Err("Format de feuille non valide (maximum 5 m).".into());
            }
            for v in &f.vues {
                if !self.vues.iter().any(|x| x.id == v.vue)
                    || !v.echelle.is_finite()
                    || v.echelle <= 0.0
                    || v.position.iter().any(|x| !x.is_finite())
                {
                    return Err("Vue placée non valide.".into());
                }
            }
            for n in &f.nomenclatures {
                if !self.nomenclatures.iter().any(|x| x.id == n.nomenclature) || n.position.iter().any(|x| !x.is_finite()) {
                    return Err("Nomenclature placée non valide.".into());
                }
            }
        }
        Ok(())
    }
}
