//! Réseau de l'onglet Modifier de l'éditeur de familles : copies répétées de la
//! sélection par translation (réseau linéaire) ou rotation (réseau polaire).
//!
//! Les points cliqués donnent la transformation d'une copie à la suivante (pas), ou de
//! l'original à la dernière copie (longueur totale). Chaque copie passe par
//! `transformer_selection` : mêmes limites que Déplacer et Tourner (un volume
//! rectangulaire ne tourne que par quarts de tour). Le réseau entier est une étape
//! d'annulation ; un refus sur une copie n'en laisse aucune.
use super::*;
use super::transformer::Transformation;

/// Options des outils Réseau et Aligner, réglées dans Propriétés ou par le pilote.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct OptionsModifier {
    /// Nombre d'exemplaires, original compris (au moins 2).
    pub nombre: usize,
    /// Le dernier point désigne la dernière copie (longueur ou angle total), sinon la
    /// deuxième (pas).
    pub total: bool,
    /// Référence de l'outil Aligner : un point et la direction unitaire de la ligne.
    pub reference: Option<((f64, f64), (f64, f64))>,
    /// Tracé en cours de l'outil « Tracer le chemin ».
    pub chemin: super::chemin_trace::TraceChemin,
}

impl Default for OptionsModifier {
    fn default() -> Self { Self { nombre: 3, total: false, reference: None, chemin: Default::default() } }
}

impl OptionsModifier {
    /// Options du pilote après la clé de l'outil : un nombre, `pas` ou `total`.
    pub(crate) fn appliquer(&mut self, options: &str) -> Result<(), String> {
        for mot in options.split_whitespace() {
            match mot {
                "pas" => self.total = false,
                "total" => self.total = true,
                n => match n.parse::<usize>() {
                    Ok(n) if (2..=500).contains(&n) => self.nombre = n,
                    _ => return Err(format!("Option de réseau inconnue « {n} » : nombre de 2 à 500, pas ou total.")),
                },
            }
        }
        Ok(())
    }

    /// Transformations de l'original vers chaque copie (`nombre - 1` étapes).
    pub(super) fn etapes(&self, t: Transformation) -> Vec<Transformation> {
        let n = self.nombre.max(2);
        let pas = if self.total { 1.0 / (n - 1) as f64 } else { 1.0 };
        (1..n).map(|k| {
            let s = k as f64 * pas;
            match t {
                Transformation::Translation { dh, dv } => Transformation::Translation { dh: dh * s, dv: dv * s },
                Transformation::Rotation { centre, angle } => Transformation::Rotation { centre, angle: angle * s },
                autre => autre,
            }
        }).collect()
    }
}

/// Réglages du réseau dans Propriétés, tant que l'outil est actif.
pub(super) fn options_reseau(ui: &mut egui::Ui, ed: &mut EditeurFamille) {
    if !matches!(ed.outil, OutilFamille::Reseau | OutilFamille::ReseauPolaire) { return; }
    egui::CollapsingHeader::new("Réseau").default_open(true).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label("Nombre (original compris)");
            ui.add(egui::DragValue::new(&mut ed.modifier.nombre).range(2..=500));
        });
        let avant = ed.modifier.total;
        ui.radio_value(&mut ed.modifier.total, false, "Le dernier clic donne le pas (deuxième exemplaire)");
        ui.radio_value(&mut ed.modifier.total, true, "Le dernier clic donne le dernier exemplaire");
        if avant != ed.modifier.total { ed.message = Some(ed.invite_transformation().into()); }
    });
}

impl EditeurFamille {
    /// Duplique les éléments de la sélection sans les déplacer et rend les copies
    /// (plans, contraintes et cotes refusés, rien n'est alors créé).
    pub(super) fn dupliquer(&mut self, selection: &[SelectionFamille]) -> Result<Vec<SelectionFamille>, String> {
        if selection.iter().any(|s| !matches!(s, SelectionFamille::Element2D(_) | SelectionFamille::Extrusion(_) | SelectionFamille::Volume(_))) {
            return Err("Copie impossible : retirer les plans, contraintes et cotes de la sélection.".into());
        }
        let mut copies = Vec::with_capacity(selection.len());
        for s in selection {
            match *s {
                SelectionFamille::Element2D(k) => {
                    let copie = self.famille.elements_2d.get(k).ok_or("Élément introuvable.")?.clone();
                    self.famille.elements_2d.push(copie);
                    copies.push(SelectionFamille::Element2D(self.famille.elements_2d.len() - 1));
                }
                SelectionFamille::Extrusion(k) => {
                    let mut copie = self.famille.extrusions.get(k).ok_or("Forme introuvable.")?.clone();
                    copie.nom = nom_libre(|n| self.famille.extrusions.iter().any(|e| e.nom == n), &format!("{} copie", copie.nom));
                    self.famille.extrusions.push(copie);
                    copies.push(SelectionFamille::Extrusion(self.famille.extrusions.len() - 1));
                }
                SelectionFamille::Volume(k) => {
                    let mut copie = self.famille.volumes.get(k).ok_or("Volume introuvable.")?.clone();
                    copie.nom = nom_libre(|n| self.famille.volumes.iter().any(|v| v.nom == n), &format!("{} copie", copie.nom));
                    self.famille.volumes.push(copie);
                    copies.push(SelectionFamille::Volume(self.famille.volumes.len() - 1));
                }
                _ => unreachable!("sélection vérifiée"),
            }
        }
        Ok(copies)
    }

    /// Crée les copies du réseau, toutes ou aucune, sélectionne l'original et ses copies,
    /// et rend le nombre de copies.
    pub(super) fn reseau_selection(&mut self, selection: &[SelectionFamille], t: Transformation) -> Result<usize, String> {
        let sauvegarde = self.famille.clone();
        let mut tout = selection.to_vec();
        let resultat = (|| {
            for etape in self.modifier.etapes(t) {
                let copies = self.dupliquer(selection)?;
                self.transformer_selection(&copies, etape)?;
                tout.extend(copies);
            }
            Ok::<_, String>(())
        })();
        if let Err(e) = resultat {
            self.famille = sauvegarde;
            return Err(e);
        }
        let n = tout.len() - selection.len();
        self.selectionner(tout);
        Ok(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn etapes_du_reseau_lineaire_et_polaire() {
        let mut o = OptionsModifier { nombre: 4, ..Default::default() };
        let pas = Transformation::Translation { dh: 100.0, dv: -50.0 };
        assert_eq!(o.etapes(pas), vec![
            Transformation::Translation { dh: 100.0, dv: -50.0 },
            Transformation::Translation { dh: 200.0, dv: -100.0 },
            Transformation::Translation { dh: 300.0, dv: -150.0 },
        ]);
        // longueur totale : le dernier exemplaire est au point cliqué
        o.total = true;
        assert_eq!(o.etapes(Transformation::Translation { dh: 300.0, dv: 0.0 }).last(), Some(&Transformation::Translation { dh: 300.0, dv: 0.0 }));
        assert_eq!(o.etapes(Transformation::Translation { dh: 300.0, dv: 0.0 })[0], Transformation::Translation { dh: 100.0, dv: 0.0 });
        // polaire : 6 exemplaires répartis sur 300° (dernier au clic), centre conservé
        o.nombre = 6;
        let tour = Transformation::Rotation { centre: (5.0, 5.0), angle: 300_f64.to_radians() };
        let angles: Vec<f64> = o.etapes(tour).into_iter().map(|t| match t { Transformation::Rotation { centre, angle } => { assert_eq!(centre, (5.0, 5.0)); angle.to_degrees() } _ => panic!() }).collect();
        for (a, b) in angles.iter().zip([60.0, 120.0, 180.0, 240.0, 300.0]) { assert!((a - b).abs() < 1e-9); }
    }

    #[test]
    fn options_du_pilote() {
        let mut o = OptionsModifier::default();
        o.appliquer("5 total").unwrap();
        assert_eq!((o.nombre, o.total), (5, true));
        o.appliquer("pas").unwrap();
        assert!(!o.total);
        assert!(o.appliquer("1").is_err());
        assert!(o.appliquer("dix").is_err());
        o.appliquer("").unwrap();
        assert_eq!(o.nombre, 5);
    }
}
