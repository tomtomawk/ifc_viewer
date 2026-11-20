//! Couleurs : un `IfcSurfaceStyle` par matériau du projet, réutilisé, posé par
//! `IfcStyledItem` sur les faces tessellées de ce matériau.
use super::step::{reel, texte, Step, Tri};
use modele::Projet;
use std::collections::HashMap;

#[derive(Default)]
pub(super) struct Styles {
    par_materiau: HashMap<String, usize>,
}

impl Styles {
    /// Style du matériau nommé `nom` : celui du projet, sinon celui du gabarit de même
    /// nom (comme la vue 3D). Couleur de surface et transparence de l'apparence.
    fn style(&mut self, w: &mut Step, p: &Projet, nom: &str) -> usize {
        if let Some(&s) = self.par_materiau.get(nom) {
            return s;
        }
        let m = p.materiau(nom).cloned().unwrap_or_else(|| p.materiau_par_defaut(nom));
        let [r, g, b] = m.couleur.map(|c| reel(c as f64 / 255.0));
        let couleur = w.add(format!("IFCCOLOURRGB($,{r},{g},{b})"));
        let t = m.apparence.transparence as f64;
        let transparence = if t > 0.0 && t <= 1.0 { reel(t) } else { "$".into() };
        let ombrage = w.add(format!("IFCSURFACESTYLESHADING(#{couleur},{transparence})"));
        let s = w.add(format!("IFCSURFACESTYLE({},.BOTH.,(#{ombrage}))", texte(nom)));
        self.par_materiau.insert(nom.to_string(), s);
        s
    }

    /// Forme d'un produit faite de parts (matériau éventuel, triangles en mm) : un jeu de
    /// faces par part non vide, coloré si la part a un matériau. `autres` : autres
    /// représentations (axe). 0 s'il n'y a rien.
    pub(super) fn forme(&mut self, w: &mut Step, p: &Projet, contexte: usize, parts: &[(Option<&str>, &[Tri])], autres: &[usize]) -> usize {
        let mut faces = Vec::new();
        for (materiau, tris) in parts {
            let Some(f) = w.faces(tris) else { continue };
            if let Some(nom) = materiau {
                let s = self.style(w, p, nom);
                w.add(format!("IFCSTYLEDITEM(#{f},(#{s}),$)"));
            }
            faces.push(f);
        }
        w.forme(contexte, &faces, autres)
    }
}
