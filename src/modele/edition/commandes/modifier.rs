//! Commandes de modification composées : symétrie, échelle, réseau, décaler, scinder.
//! Elles réutilisent Copier, Déplacer et la transformation affine du plan.
use super::transformations::Affine;
use super::*;

impl Projet {
    pub(super) fn symetrie(&mut self, ids: Vec<String>, a: [f64; 2], b: [f64; 2]) -> Result<Vec<String>, String> {
        let f = Affine::symetrie(a, b).ok_or("Symétrie : l'axe doit avoir deux points distincts.")?;
        let set = self.dependances_avec(&ids, true)?;
        self.transformer_affine(&set, f, 0.0)?;
        Ok(ids)
    }

    pub(super) fn echelle(&mut self, ids: Vec<String>, centre: [f64; 2], facteur: f64) -> Result<Vec<String>, String> {
        if !(facteur.is_finite() && facteur > 1e-6) {
            return Err("Échelle : le facteur doit être positif.".into());
        }
        let set = self.dependances_avec(&ids, true)?;
        self.transformer_affine(&set, Affine::echelle(centre, facteur), 0.0)?;
        Ok(ids)
    }

    /// Réseau : `nombre` éléments au total (original compris) ; la copie k est tournée de
    /// k·`angle` degrés autour de `centre` puis déplacée de k·`delta`.
    pub(super) fn reseau(&mut self, ids: Vec<String>, nombre: usize, delta: [f64; 3], centre: [f64; 2], angle: f64) -> Result<Vec<String>, String> {
        if !(2..=500).contains(&nombre) {
            return Err("Réseau : de 2 à 500 éléments.".into());
        }
        if delta.iter().all(|v| v.abs() < 1e-9) && angle.abs() < 1e-9 {
            return Err("Réseau : le pas est nul.".into());
        }
        let mut toutes = Vec::new();
        for k in 1..nombre {
            let copies = self.executer_interne(Commande::Copier { ids: ids.clone(), delta: [0.0; 3] })?;
            let set = self.dependances_avec(&copies, true)?;
            let k = k as f64;
            self.transformer(&set, [delta[0] * k, delta[1] * k, delta[2] * k], centre, angle * k)?;
            toutes.extend(copies);
        }
        Ok(toutes)
    }

    /// Couches d'un mur (lui-même) ou d'un mur composé, dans l'ordre.
    fn couches_de(&self, id: &str) -> Vec<String> {
        match self.assemblage(id) {
            Some(a) => a.couches.clone(),
            None => vec![id.to_string()],
        }
    }

    /// Décaler : mur droit (ou composé) ou ligne d'annotation, de `distance` mm vers la
    /// gauche de son sens (négatif : vers la droite) ; `copie` garde l'original.
    pub(super) fn decaler(&mut self, id: String, distance: f64, copie: bool) -> Result<Vec<String>, String> {
        if !distance.is_finite() || distance.abs() < 1e-6 {
            return Err("Décaler : distance nulle.".into());
        }
        if self.couches_de(&id).iter().any(|c| self.murs.iter().any(|m| &m.id == c && matches!(m.axe, Axe::Arc { .. }))) {
            return self.decaler_arc(id, distance, copie);
        }
        if self.dalles.iter().any(|d| d.id == id) {
            return self.decaler_contour(id, distance, copie);
        }
        let (a, b) = if let Some((a, b, _)) = self.axe_element(&id) {
            (a, b)
        } else if let Some(x) = self.annotations.iter().find(|x| x.id == id) {
            match x.forme {
                documentation::AnnotationForme::Ligne { debut, fin } => (debut, fin),
                _ => return Err("Décaler : ligne d'annotation seulement.".into()),
            }
        } else {
            return Err("Décaler : choisir un mur droit ou une ligne.".into());
        };
        let u = (DVec2::from_array(b) - DVec2::from_array(a)).try_normalize().ok_or("Décaler : élément de longueur nulle.")?;
        let v = u.perp() * distance;
        let delta = [v.x, v.y, 0.0];
        let ids = vec![id];
        if copie {
            self.executer_interne(Commande::Copier { ids, delta })
        } else {
            self.executer_interne(Commande::Deplacer { ids: ids.clone(), delta })?;
            Ok(ids)
        }
    }

    /// Scinder un mur droit (ou composé) au point projeté de `point` ; `ecart` (mm)
    /// laisse un vide entre les deux parties. Les ouvertures suivent leur partie.
    pub(super) fn scinder(&mut self, id: String, point: [f64; 2], ecart: f64) -> Result<Vec<String>, String> {
        if !(ecart.is_finite() && ecart >= 0.0) {
            return Err("Scinder : écart négatif.".into());
        }
        if self.barres.iter().any(|b| b.id == id && b.genre == GenreBarre::Poutre) {
            return self.scinder_poutre(id, point, ecart);
        }
        if self.annotations.iter().any(|a| a.id == id) {
            return self.scinder_ligne(id, point, ecart);
        }
        let couches = self.couches_de(&id);
        if couches.is_empty() || !couches.iter().all(|c| self.murs.iter().any(|m| &m.id == c && matches!(m.axe, Axe::Droit { .. }))) {
            return Err("Scinder : mur droit, poutre ou ligne d'annotation seulement.".into());
        }
        let (axe, _, _) = solides::hote(self, &id)?;
        let longueur = axe.longueur();
        let s = axe.abscisse(point);
        if s - ecart / 2.0 < 1.0 || s + ecart / 2.0 > longueur - 1.0 {
            return Err("Scinder : le point de coupure doit être à l'intérieur du mur.".into());
        }
        let (debut_coupe, fin_coupe) = (s - ecart / 2.0, s + ecart / 2.0);
        let hote_de = |p: &Projet, o: &Ouverture, couches: &[String], id: &str| o.mur == id || couches.contains(&o.mur) && p.assemblage(id).is_some();
        for o in self.ouvertures.iter().filter(|o| hote_de(self, o, &couches, &id)) {
            if o.abscisse - o.largeur / 2.0 < fin_coupe && o.abscisse + o.largeur / 2.0 > debut_coupe {
                return Err(format!("Scinder : l'ouverture « {} » traverse le point de coupure.", o.nom));
            }
        }
        let copie = self.executer_interne(Commande::Copier { ids: vec![id.clone()], delta: [0.0; 3] })?.remove(0);
        let couches_copie = self.couches_de(&copie);
        let p = DVec2::from_array(point);
        for (c, c2) in couches.iter().zip(&couches_copie) {
            let Some(Axe::Droit { debut, fin }) = self.murs.iter().find(|m| &m.id == c).map(|m| m.axe.clone()) else { continue };
            let (a, b) = (DVec2::from_array(debut), DVec2::from_array(fin));
            let v = (b - a).normalize();
            let q = a + v * (p - a).dot(v);
            let (q1, q2) = ((q - v * ecart / 2.0).to_array(), (q + v * ecart / 2.0).to_array());
            if let Some(m) = self.murs.iter_mut().find(|m| &m.id == c) {
                m.axe = Axe::Droit { debut, fin: q1 };
            }
            if let Some(m) = self.murs.iter_mut().find(|m| &m.id == c2) {
                m.axe = Axe::Droit { debut: q2, fin };
            }
        }
        // ouvertures : chacune reste dans la partie qui la contient
        let mut a_supprimer = Vec::new();
        for o in &mut self.ouvertures {
            let original = o.mur == id || couches.contains(&o.mur) && o.mur != copie;
            let copiee = o.mur == copie || couches_copie.contains(&o.mur);
            if copiee {
                if o.abscisse < s {
                    a_supprimer.push(o.id.clone());
                } else {
                    o.abscisse -= fin_coupe;
                }
            } else if original && o.abscisse > s {
                a_supprimer.push(o.id.clone());
            }
        }
        if !a_supprimer.is_empty() {
            self.executer_interne(Commande::Supprimer { ids: a_supprimer })?;
        }
        Ok(vec![id, copie])
    }
}

impl Projet {
    /// Décaler un mur courbe (ou composé courbe) : arc concentrique, de `distance` vers la
    /// gauche du sens de l'arc ; les ouvertures gardent leur position angulaire.
    fn decaler_arc(&mut self, id: String, distance: f64, copie: bool) -> Result<Vec<String>, String> {
        let cible = if copie { self.executer_interne(Commande::Copier { ids: vec![id.clone()], delta: [0.0; 3] })?.remove(0) } else { id.clone() };
        let couches = self.couches_de(&cible);
        let mut rapport = None;
        for c in &couches {
            let m = self.murs.iter_mut().find(|m| &m.id == c).ok_or("Couche introuvable.")?;
            let Axe::Arc { debut, fin, centre, direct } = &mut m.axe else { return Err("Décaler : mur composé mêlant arcs et droites.".into()) };
            let r = (debut[0] - centre[0]).hypot(debut[1] - centre[1]);
            // à gauche du sens : vers le centre pour un arc trigonométrique
            let nouveau = if *direct { r - distance } else { r + distance };
            if nouveau < 10.0 {
                return Err("Décaler : le rayon deviendrait nul.".into());
            }
            let k = nouveau / r;
            for p in [&mut *debut, &mut *fin] {
                *p = [centre[0] + (p[0] - centre[0]) * k, centre[1] + (p[1] - centre[1]) * k];
            }
            rapport.get_or_insert(k);
        }
        let k = rapport.unwrap_or(1.0);
        for o in self.ouvertures.iter_mut().filter(|o| o.mur == cible || couches.contains(&o.mur)) {
            o.abscisse *= k;
        }
        Ok(vec![cible])
    }

    /// Décaler le contour extérieur d'une dalle : `distance` positive vers l'extérieur.
    fn decaler_contour(&mut self, id: String, distance: f64, copie: bool) -> Result<Vec<String>, String> {
        let cible = if copie { self.executer_interne(Commande::Copier { ids: vec![id.clone()], delta: [0.0; 3] })?.remove(0) } else { id.clone() };
        let d = self.dalles.iter_mut().find(|d| d.id == cible).ok_or("Dalle introuvable.")?;
        let contour = d.contours.first_mut().ok_or("Dalle sans contour.")?;
        *contour = decaler_polygone(contour, distance).ok_or("Décaler : contour dégénéré après décalage.")?;
        Ok(vec![cible])
    }

    /// Scinder une poutre au point projeté (hauteur interpolée), avec un écart.
    fn scinder_poutre(&mut self, id: String, point: [f64; 2], ecart: f64) -> Result<Vec<String>, String> {
        let b = self.barres.iter().find(|b| b.id == id).ok_or("Poutre introuvable.")?;
        let (a, f) = (glam::DVec3::from_array(b.debut), glam::DVec3::from_array(b.fin));
        let l = (f - a).truncate().length();
        if l < 1.0 {
            return Err("Scinder : poutre de longueur nulle.".into());
        }
        let u = (f - a).truncate() / l;
        let s = (DVec2::from_array(point) - a.truncate()).dot(u);
        if s - ecart / 2.0 < 1.0 || s + ecart / 2.0 > l - 1.0 {
            return Err("Scinder : le point de coupure doit être à l'intérieur de la poutre.".into());
        }
        let p = |t: f64| (a + (f - a) * (t / l)).to_array();
        let (q1, q2) = (p(s - ecart / 2.0), p(s + ecart / 2.0));
        let copie = self.executer_interne(Commande::Copier { ids: vec![id.clone()], delta: [0.0; 3] })?.remove(0);
        if let Some(b) = self.barres.iter_mut().find(|b| b.id == id) { b.fin = q1; }
        if let Some(b) = self.barres.iter_mut().find(|b| b.id == copie) { b.debut = q2; }
        Ok(vec![id, copie])
    }

    /// Scinder une ligne d'annotation au point projeté, avec un écart.
    fn scinder_ligne(&mut self, id: String, point: [f64; 2], ecart: f64) -> Result<Vec<String>, String> {
        use documentation::AnnotationForme as F;
        let x = self.annotations.iter().find(|x| x.id == id).ok_or("Annotation introuvable.")?;
        let F::Ligne { debut, fin } = x.forme else { return Err("Scinder : ligne d'annotation seulement.".into()) };
        let (a, f) = (DVec2::from_array(debut), DVec2::from_array(fin));
        let l = (f - a).length();
        let u = (f - a).try_normalize().ok_or("Scinder : ligne de longueur nulle.")?;
        let s = (DVec2::from_array(point) - a).dot(u);
        if s - ecart / 2.0 < 0.01 || s + ecart / 2.0 > l - 0.01 {
            return Err("Scinder : le point de coupure doit être sur la ligne.".into());
        }
        let copie = self.executer_interne(Commande::Copier { ids: vec![id.clone()], delta: [0.0; 3] })?.remove(0);
        for (cle, d, fi) in [(&id, debut, (a + u * (s - ecart / 2.0)).to_array()), (&copie, (a + u * (s + ecart / 2.0)).to_array(), fin)] {
            if let Some(x) = self.annotations.iter_mut().find(|x| &x.id == cle) {
                x.forme = F::Ligne { debut: d, fin: fi };
            }
        }
        Ok(vec![id, copie])
    }

    /// Phase de création d'un élément (champ propre ou contexte).
    pub fn phase_element(&self, id: &str) -> Option<String> {
        self.murs.iter().find(|m| m.id == id).and_then(|m| m.phase.clone())
            .or_else(|| self.dalles.iter().find(|d| d.id == id).and_then(|d| d.phase.clone()))
            .or_else(|| self.assemblage(id).and_then(|a| self.couches_assemblage(&a.id).first().and_then(|m| m.phase.clone())))
            .or_else(|| self.contextes.get(id).and_then(|c| c.phase.clone()))
    }

    /// Démolir (ou annuler la démolition si `phase` est absente) : phase de démolition.
    pub(super) fn demolir(&mut self, ids: Vec<String>, phase: Option<String>) -> Result<Vec<String>, String> {
        let connus = self.identifiants();
        if let Some(ph) = &phase {
            let rang = self.phases.iter().position(|p| p == ph).ok_or_else(|| format!("Phase inconnue : {ph}"))?;
            for id in &ids {
                if let Some(creation) = self.phase_element(id) {
                    if self.phases.iter().position(|p| p == &creation).is_some_and(|c| c > rang) {
                        return Err(format!("Démolir : un élément créé en phase {creation} ne peut pas être démoli en phase {ph}."));
                    }
                }
            }
        }
        for id in &ids {
            if !connus.contains(id) {
                return Err(format!("Élément introuvable : {id}"));
            }
            match &phase {
                Some(ph) => { self.demolitions.insert(id.clone(), ph.clone()); }
                None => { self.demolitions.remove(id); }
            }
        }
        Ok(ids)
    }

    /// Reproduire les propriétés de `source` (type et paramètres, sans la géométrie) sur
    /// des éléments de la même catégorie.
    pub(super) fn reproduire(&mut self, source: String, cibles: Vec<String>) -> Result<Vec<String>, String> {
        if let Some(m) = self.murs.iter().find(|m| m.id == source && m.assemblage.is_none()).cloned() {
            for c in &cibles {
                let x = self.murs.iter_mut().find(|x| &x.id == c && x.assemblage.is_none()).ok_or("Reproduire : la cible doit être un mur simple, comme la source.")?;
                x.type_mur = m.type_mur.clone();
                x.decalage_bas = m.decalage_bas;
                x.haut = m.haut.clone();
                x.phase = m.phase.clone();
            }
        } else if let Some(d) = self.dalles.iter().find(|d| d.id == source).cloned() {
            for c in &cibles {
                let x = self.dalles.iter_mut().find(|x| &x.id == c).ok_or("Reproduire : la cible doit être une dalle, comme la source.")?;
                x.type_dalle = d.type_dalle.clone();
                x.decalage = d.decalage;
                x.sorte = d.sorte.clone();
                x.phase = d.phase.clone();
            }
        } else if let Some(o) = self.ouvertures.iter().find(|o| o.id == source).cloned() {
            for c in &cibles {
                let x = self.ouvertures.iter_mut().find(|x| &x.id == c).ok_or("Reproduire : la cible doit être une ouverture, comme la source.")?;
                x.genre = o.genre.clone();
                x.type_ouverture = o.type_ouverture.clone();
                x.largeur = o.largeur;
                x.hauteur = o.hauteur;
                x.allege = o.allege;
                x.valeurs = o.valeurs.clone();
                x.materiaux = o.materiaux.clone();
            }
        } else if let Some(b) = self.barres.iter().find(|b| b.id == source).cloned() {
            for c in &cibles {
                let x = self.barres.iter_mut().find(|x| &x.id == c && x.genre == b.genre).ok_or("Reproduire : la cible doit être une barre du même genre.")?;
                x.largeur = b.largeur;
                x.hauteur = b.hauteur;
            }
        } else {
            return Err("Reproduire : source non prise en charge (mur simple, dalle, ouverture, poteau ou poutre).".into());
        }
        if let Some(ctx) = self.contextes.get(&source).cloned() {
            for c in &cibles {
                if self.contextes.contains_key(c) {
                    self.contextes.insert(c.clone(), ctx.clone());
                }
            }
        }
        Ok(cibles)
    }

    /// Changer d'hôte : l'ouverture passe sur le mur `mur`, centrée à la projection de `point`.
    pub(super) fn changer_hote(&mut self, id: String, mur: String, point: [f64; 2]) -> Result<Vec<String>, String> {
        let (axe, _, _) = solides::hote(self, &mur)?;
        let o = self.ouvertures.iter().find(|o| o.id == id).ok_or("Ouverture introuvable.")?;
        if o.mur == mur {
            return Err("Changer d'hôte : choisir un autre mur.".into());
        }
        let abscisse = axe.abscisse(point);
        if abscisse - o.largeur / 2.0 < 0.0 || abscisse + o.largeur / 2.0 > axe.longueur() {
            return Err("Changer d'hôte : l'ouverture dépasse du nouveau mur.".into());
        }
        let o = self.ouvertures.iter_mut().find(|o| o.id == id).unwrap();
        o.mur = mur;
        o.abscisse = abscisse;
        o.representation = None;
        Ok(vec![id])
    }

    /// Inverser une ouverture : côté d'ouverture (`retournee`) ou sens des paumelles.
    pub(super) fn inverser_ouverture(&mut self, id: String, paumelles: bool) -> Result<Vec<String>, String> {
        let o = self.ouvertures.iter_mut().find(|o| o.id == id).ok_or("Ouverture introuvable.")?;
        if paumelles { o.inversee = !o.inversee; } else { o.retournee = !o.retournee; }
        Ok(vec![id])
    }

    /// Éléments du même type que `id` (même catégorie), `id` compris.
    pub fn identiques(&self, id: &str) -> Vec<String> {
        if let Some(a) = self.assemblage(id) {
            return self.assemblages.iter().filter(|x| x.type_mur == a.type_mur).map(|x| x.id.clone()).collect();
        }
        if let Some(m) = self.murs.iter().find(|m| m.id == id) {
            return self.murs.iter().filter(|x| x.assemblage.is_none() && x.type_mur == m.type_mur).map(|x| x.id.clone()).collect();
        }
        if let Some(d) = self.dalles.iter().find(|d| d.id == id) {
            return self.dalles.iter().filter(|x| x.type_dalle == d.type_dalle && x.sorte == d.sorte).map(|x| x.id.clone()).collect();
        }
        if let Some(o) = self.ouvertures.iter().find(|o| o.id == id) {
            return self.ouvertures.iter().filter(|x| x.genre == o.genre && x.type_ouverture == o.type_ouverture && (o.type_ouverture.is_some() || (x.largeur == o.largeur && x.hauteur == o.hauteur))).map(|x| x.id.clone()).collect();
        }
        if let Some(b) = self.barres.iter().find(|b| b.id == id) {
            return self.barres.iter().filter(|x| x.genre == b.genre && x.largeur == b.largeur && x.hauteur == b.hauteur).map(|x| x.id.clone()).collect();
        }
        for (liste, nom) in [(self.escaliers.iter().map(|x| x.id.clone()).collect::<Vec<_>>(), "escalier"), (self.garde_corps.iter().map(|x| x.id.clone()).collect(), "garde-corps")] {
            let _ = nom;
            if liste.iter().any(|x| x == id) {
                return liste;
            }
        }
        vec![id.to_string()]
    }
}

/// Polygone décalé de `d` vers l'extérieur (négatif : vers l'intérieur), angles en onglet.
pub fn decaler_polygone(pts: &[[f64; 2]], d: f64) -> Option<Vec<[f64; 2]>> {
    let n = pts.len();
    if n < 3 {
        return None;
    }
    let aire: f64 = (0..n).map(|i| { let (a, b) = (pts[i], pts[(i + 1) % n]); a[0] * b[1] - b[0] * a[1] }).sum::<f64>() / 2.0;
    // extérieur à droite du sens pour un contour trigonométrique
    let s = if aire > 0.0 { 1.0 } else { -1.0 };
    let normale = |a: [f64; 2], b: [f64; 2]| -> Option<DVec2> {
        let u = (DVec2::from_array(b) - DVec2::from_array(a)).try_normalize()?;
        Some(DVec2::new(u.y, -u.x) * s)
    };
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let (p0, p1, p2) = (pts[(i + n - 1) % n], pts[i], pts[(i + 1) % n]);
        let (n1, n2) = (normale(p0, p1)?, normale(p1, p2)?);
        let bis = n1 + n2;
        let cos = bis.length() / 2.0;
        let q = if cos < 1e-6 { DVec2::from_array(p1) + n1 * d } else { DVec2::from_array(p1) + bis.normalize() * (d / cos) };
        out.push(q.to_array());
    }
    let aire2: f64 = (0..n).map(|i| { let (a, b) = (out[i], out[(i + 1) % n]); a[0] * b[1] - b[0] * a[1] }).sum::<f64>() / 2.0;
    // une arête retournée signale un contour qui s'est croisé (décalage trop grand)
    let retournee = (0..n).any(|i| {
        let (a, b) = (DVec2::from_array(pts[i]), DVec2::from_array(pts[(i + 1) % n]));
        let (c, e) = (DVec2::from_array(out[i]), DVec2::from_array(out[(i + 1) % n]));
        (b - a).dot(e - c) <= 0.0
    });
    (!retournee && aire2.abs() > 1.0 && aire2.signum() == aire.signum()).then_some(out)
}

#[cfg(test)]
mod tests {
    use crate::modele::edition::commandes::Commande;
    use crate::modele::edition::ParametresMur;
    use crate::modele::{Axe, Projet};

    fn mur(p: &mut Projet, a: [f64; 2], b: [f64; 2]) -> String {
        p.ajouter_mur(a, b, &ParametresMur::default()).unwrap();
        p.murs.last().unwrap().id.clone()
    }
    fn axe(p: &Projet, id: &str) -> ([f64; 2], [f64; 2]) {
        match p.murs.iter().find(|m| m.id == id).unwrap().axe {
            Axe::Droit { debut, fin } => (debut, fin),
            _ => unreachable!(),
        }
    }
    fn proche(a: [f64; 2], b: [f64; 2]) -> bool {
        (a[0] - b[0]).abs() < 1e-6 && (a[1] - b[1]).abs() < 1e-6
    }

    #[test]
    fn symetrie_retourne_le_sens_du_mur_et_garde_la_longueur() {
        let mut p = Projet::nouveau("S");
        let id = mur(&mut p, [1000.0, 0.0], [5000.0, 0.0]);
        let (q, _) = p.executer(Commande::Symetrie { ids: vec![id.clone()], a: [0.0, 0.0], b: [0.0, 1.0] }).unwrap();
        let (d, f) = axe(&q, &id);
        // image de (1000,0)→(5000,0) : (-1000,0)→(-5000,0), sens inversé
        assert!(proche(d, [-5000.0, 0.0]) && proche(f, [-1000.0, 0.0]), "{d:?} {f:?}");
    }

    #[test]
    fn echelle_refusee_pour_les_murs() {
        let mut p = Projet::nouveau("E");
        let id = mur(&mut p, [0.0, 0.0], [4000.0, 0.0]);
        assert!(p.executer(Commande::Echelle { ids: vec![id], centre: [0.0, 0.0], facteur: 2.0 }).is_err());
    }

    #[test]
    fn echelle_d_une_ligne_d_annotation() {
        use crate::modele::documentation::{Annotation, AnnotationForme};
        let mut p = Projet::nouveau("E");
        mur(&mut p, [0.0, 0.0], [1000.0, 0.0]);
        let niveau = p.niveaux[0].id.clone();
        let vue = p.ajouter_vue_plan(&niveau, 1200.0).unwrap();
        let annotation = Annotation { id: String::new(), vue, hauteur: 2.5, forme: AnnotationForme::Ligne { debut: [1000.0, 0.0], fin: [3000.0, 0.0] } };
        let (p, ids) = p.executer(Commande::Annoter { annotation }).unwrap();
        let (q, _) = p.executer(Commande::Echelle { ids: ids.clone(), centre: [0.0, 0.0], facteur: 2.0 }).unwrap();
        let AnnotationForme::Ligne { debut, fin } = q.annotations.iter().find(|a| a.id == ids[0]).unwrap().forme else { unreachable!() };
        assert!(proche(debut, [2000.0, 0.0]) && proche(fin, [6000.0, 0.0]));
    }

    #[test]
    fn reseau_lineaire_et_polaire() {
        let mut p = Projet::nouveau("R");
        let id = mur(&mut p, [0.0, 0.0], [1000.0, 0.0]);
        let (q, ids) = p.executer(Commande::Reseau { ids: vec![id.clone()], nombre: 4, delta: [0.0, 2000.0, 0.0], centre: [0.0, 0.0], angle: 0.0 }).unwrap();
        assert_eq!(ids.len(), 3);
        assert!(proche(axe(&q, &ids[2]).0, [0.0, 6000.0]));
        let (q, ids) = p.executer(Commande::Reseau { ids: vec![id], nombre: 4, delta: [0.0; 3], centre: [0.0, 0.0], angle: 90.0 }).unwrap();
        assert!(proche(axe(&q, &ids[0]).1, [0.0, 1000.0]));
        assert!(proche(axe(&q, &ids[1]).1, [-1000.0, 0.0]));
    }

    #[test]
    fn decaler_vers_la_gauche_avec_ou_sans_copie() {
        let mut p = Projet::nouveau("D");
        let id = mur(&mut p, [0.0, 0.0], [4000.0, 0.0]);
        let (q, _) = p.executer(Commande::Decaler { id: id.clone(), distance: 500.0, copie: false }).unwrap();
        assert!(proche(axe(&q, &id).0, [0.0, 500.0]));
        let (q, ids) = p.executer(Commande::Decaler { id: id.clone(), distance: -300.0, copie: true }).unwrap();
        assert_eq!(q.murs.len(), 2);
        assert!(proche(axe(&q, &id).0, [0.0, 0.0]));
        assert!(proche(axe(&q, &ids[0]).0, [0.0, -300.0]));
    }

    #[test]
    fn decaler_arc_et_contour_de_dalle() {
        let mut p = Projet::nouveau("A");
        p.ajouter_mur([1000.0, 0.0], [-1000.0, 0.0], &ParametresMur::default()).unwrap();
        let id = p.murs[0].id.clone();
        if let Axe::Droit { .. } = p.murs[0].axe {
            p.murs[0].axe = Axe::Arc { debut: [1000.0, 0.0], fin: [-1000.0, 0.0], centre: [0.0, 0.0], direct: true };
        }
        let (q, _) = p.executer(Commande::Decaler { id: id.clone(), distance: 200.0, copie: false }).unwrap();
        let Axe::Arc { debut, .. } = q.murs[0].axe else { unreachable!() };
        assert!(proche(debut, [800.0, 0.0]), "{debut:?}");
        let carre = vec![[0.0, 0.0], [1000.0, 0.0], [1000.0, 1000.0], [0.0, 1000.0]];
        let g = super::decaler_polygone(&carre, 100.0).unwrap();
        assert!(proche(g[0], [-100.0, -100.0]) && proche(g[2], [1100.0, 1100.0]), "{g:?}");
        let g = super::decaler_polygone(&carre, -100.0).unwrap();
        assert!(proche(g[0], [100.0, 100.0]));
        assert!(super::decaler_polygone(&carre, -600.0).is_none());
    }

    #[test]
    fn demolir_reproduire_changer_d_hote_et_identiques() {
        let mut p = Projet::nouveau("P");
        let a = mur(&mut p, [0.0, 0.0], [4000.0, 0.0]);
        let b = mur(&mut p, [0.0, 3000.0], [4000.0, 3000.0]);
        let (p2, _) = p.executer(Commande::Demolir { ids: vec![a.clone()], phase: Some("Projet".into()) }).unwrap();
        assert_eq!(p2.demolitions.get(&a).map(String::as_str), Some("Projet"));
        let (p3, _) = p2.executer(Commande::Demolir { ids: vec![a.clone()], phase: None }).unwrap();
        assert!(p3.demolitions.is_empty());
        assert!(p.executer(Commande::Demolir { ids: vec![a.clone()], phase: Some("Inconnue".into()) }).is_err());
        assert_eq!(p.identiques(&a).len(), 2);
        let mut q = p.clone();
        q.murs[0].decalage_bas = 250.0;
        let (q, _) = q.executer(Commande::Reproduire { source: a.clone(), cibles: vec![b.clone()] }).unwrap();
        assert_eq!(q.murs[1].decalage_bas, 250.0);
        let p = p.executer(Commande::Ouverture { mur: a.clone(), genre: crate::modele::GenreOuverture::Porte, type_ouverture: None, abscisse: 1000.0, largeur: 900.0, hauteur: 2100.0, allege: 0.0 }).unwrap().0;
        let o = p.ouvertures[0].id.clone();
        let (p, _) = p.executer(Commande::ChangerHote { id: o.clone(), mur: b.clone(), point: [2500.0, 3000.0] }).unwrap();
        assert_eq!(p.ouvertures[0].mur, b);
        assert!((p.ouvertures[0].abscisse - 2500.0).abs() < 1e-6);
        assert!(p.executer(Commande::ChangerHote { id: o, mur: a, point: [3900.0, 0.0] }).is_err(), "dépasse du mur");
    }

    #[test]
    fn scinder_avec_ecart_et_ouvertures_reparties() {
        let mut p = Projet::nouveau("C");
        let id = mur(&mut p, [0.0, 0.0], [6000.0, 0.0]);
        for abscisse in [1000.0, 5000.0] {
            p = p.executer(Commande::Ouverture { mur: id.clone(), genre: crate::modele::GenreOuverture::Porte, type_ouverture: None, abscisse, largeur: 600.0, hauteur: 2000.0, allege: 0.0 }).unwrap().0;
        }
        let (q, ids) = p.executer(Commande::Scinder { id: id.clone(), point: [3000.0, 0.0], ecart: 200.0 }).unwrap();
        assert_eq!(ids.len(), 2);
        assert!(proche(axe(&q, &ids[0]).1, [2900.0, 0.0]));
        assert!(proche(axe(&q, &ids[1]).0, [3100.0, 0.0]) && proche(axe(&q, &ids[1]).1, [6000.0, 0.0]));
        let sur = |m: &str| q.ouvertures.iter().filter(|o| o.mur == m).map(|o| o.abscisse).collect::<Vec<_>>();
        assert_eq!(sur(&ids[0]), vec![1000.0]);
        assert_eq!(sur(&ids[1]).len(), 1);
        assert!((sur(&ids[1])[0] - 1900.0).abs() < 1e-6);
        // ouverture à cheval : refus
        assert!(p.executer(Commande::Scinder { id, point: [1000.0, 0.0], ecart: 0.0 }).is_err());
    }
}
