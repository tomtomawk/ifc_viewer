//! Types de garde-corps et disposition commune à l'élévation et au modèle.
use super::escalier::{GardeCorps, Remplissage, Tri};
use crate::modele::Projet;
use crate::modele::volumes::solides::prisme;
use glam::{DVec2, DVec3};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Matériaux des types prédéfinis (créés à la volée s'ils manquent au projet).
pub const MATERIAU_ACIER: &str = "Acier laqué";
pub const MATERIAU_VERRE: &str = "Verre feuilleté";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Lisse {
    pub altitude: f64,
    pub profil: Option<String>,
    /// Matériau propre à l'élément ; None : matériau global du type.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub materiau: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BarreauMotif {
    pub pas: f64,
    pub profil: Option<String>,
    /// Matériau propre à l'élément ; None : matériau global du type.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub materiau: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BarreauExtremite {
    pub retrait: f64,
    pub profil: Option<String>,
    /// Matériau propre à l'élément ; None : matériau global du type.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub materiau: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TypeGardeCorps {
    pub id: String,
    pub nom: String,
    pub hauteur: f64,
    pub remplissage: Remplissage,
    pub main_courante: Option<String>,
    pub lisses: Vec<Lisse>,
    pub bas: f64,
    pub jeu_haut: f64,
    pub depart: Option<BarreauExtremite>,
    pub arrivee: Option<BarreauExtremite>,
    pub motif: Vec<BarreauMotif>,
    /// Matériau global : montants, lisses et barreaux sans matériau propre.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub materiau: Option<String>,
    /// Matériau de la main courante ; None : matériau global.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub materiau_main_courante: Option<String>,
    /// Matériau du panneau ou du vitrage (remplissage plein ou vitré).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub materiau_remplissage: Option<String>,
}

/// Solide d'un garde-corps : matériau du projet (None : couleur par défaut), vitrage,
/// triangles.
pub type SolideGardeCorps = (Option<String>, bool, Vec<Tri>);

pub struct Section {
    contour: Vec<[f64; 2]>,
    pub largeur: f64,
    pub hauteur: f64,
}

pub struct Barreau<'a> {
    pub x: f64,
    pub profil: Option<&'a str>,
    pub materiau: Option<&'a str>,
    pub extremite: bool,
}

impl Section {
    pub fn lire(p: &Projet, id: Option<&str>, carre: f64) -> Result<Self, String> {
        let mut contour = if let Some(id) = id {
            p.profils
                .iter()
                .find(|profil| profil.id == id)
                .ok_or("Profil de garde-corps absent.")?
                .points()?
        } else {
            let d = carre / 2.0;
            vec![[-d, -d], [d, -d], [d, d], [-d, d]]
        };
        let (min, max) = contour.iter().map(|&q| DVec2::from_array(q)).fold(
            (DVec2::splat(f64::INFINITY), DVec2::splat(f64::NEG_INFINITY)),
            |(min, max), q| (min.min(q), max.max(q)),
        );
        let centre = (min + max) / 2.0;
        for q in &mut contour {
            *q = (DVec2::from_array(*q) - centre).to_array();
        }
        Ok(Self {
            contour,
            largeur: max.x - min.x,
            hauteur: max.y - min.y,
        })
    }

    fn extruder(
        &self,
        a: DVec3,
        b: DVec3,
        u: DVec3,
        v: DVec3,
    ) -> impl Iterator<Item = Tri> + use<> {
        let delta = b - a;
        prisme(&self.contour, 0.0, 1.0)
            .into_iter()
            .map(move |tri| tri.map(|q| a + u * q.x + v * q.y + delta * q.z))
    }
}

impl TypeGardeCorps {
    pub fn standard(hauteur: f64, remplissage: Remplissage) -> Self {
        let bord = Some(BarreauExtremite {
            retrait: 0.0,
            profil: None,
            materiau: None,
        });
        Self {
            id: String::new(),
            nom: "Garde-corps".into(),
            hauteur,
            remplissage,
            main_courante: None,
            lisses: vec![Lisse {
                altitude: 120.0,
                profil: None,
                materiau: None,
            }],
            bas: 140.0,
            jeu_haut: 50.0,
            depart: bord.clone(),
            arrivee: bord,
            motif: vec![BarreauMotif {
                pas: 110.0,
                profil: None,
                materiau: None,
            }],
            materiau: None,
            materiau_main_courante: None,
            materiau_remplissage: None,
        }
    }

    /// Types prédéfinis, proposés dans le sélecteur et copiés dans le projet au choix.
    pub fn predefinis() -> Vec<TypeGardeCorps> {
        let base = |nom: &str, remplissage| TypeGardeCorps { nom: nom.into(), ..Self::standard(1000.0, remplissage) };
        let acier = Some(MATERIAU_ACIER.to_string());
        let verre = Some(MATERIAU_VERRE.to_string());
        let lisse = |altitude: f64| Lisse { altitude, profil: None, materiau: None };
        vec![
            TypeGardeCorps { materiau: acier.clone(), ..base("Barreaudage vertical", Remplissage::Barreaudage) },
            TypeGardeCorps {
                materiau: acier.clone(),
                lisses: [150.0, 300.0, 450.0, 600.0, 750.0].map(lisse).to_vec(),
                motif: Vec::new(),
                bas: 0.0,
                ..base("Lisses horizontales", Remplissage::Barreaudage)
            },
            TypeGardeCorps {
                materiau: acier.clone(),
                materiau_remplissage: verre.clone(),
                lisses: Vec::new(),
                bas: 100.0,
                jeu_haut: 100.0,
                ..base("Vitré sur montants", Remplissage::Vitre)
            },
            TypeGardeCorps {
                materiau: acier.clone(),
                materiau_remplissage: verre,
                lisses: Vec::new(),
                depart: None,
                arrivee: None,
                bas: 0.0,
                jeu_haut: 50.0,
                ..base("Tout verre, main courante", Remplissage::Vitre)
            },
            TypeGardeCorps { materiau: acier, lisses: Vec::new(), ..base("Panneau plein", Remplissage::Plein) },
        ]
    }

    /// Matériau effectif d'un élément : le sien, sinon le matériau global.
    pub fn materiau_de<'a>(&'a self, propre: &'a Option<String>) -> Option<&'a str> {
        propre.as_deref().or(self.materiau.as_deref())
    }

    pub fn valider(&self, p: &Projet) -> Result<(), String> {
        let valide = |v: f64, min: f64, max: f64| v.is_finite() && (min..=max).contains(&v);
        if self.nom.trim().is_empty() {
            return Err("Donnez un nom au type de garde-corps.".into());
        }
        if !valide(self.hauteur, 201.0, 100_000.0)
            || !valide(self.bas, 0.0, self.hauteur)
            || !valide(self.jeu_haut, 0.0, self.hauteur)
            || self.bas + self.jeu_haut >= self.hauteur
        {
            return Err("Vérifiez la hauteur et les limites du remplissage.".into());
        }
        if self
            .lisses
            .iter()
            .any(|l| !valide(l.altitude, 0.0, self.hauteur))
        {
            return Err(
                "Les axes des lisses doivent être compris dans la hauteur du garde-corps.".into(),
            );
        }
        if self
            .depart
            .iter()
            .chain(&self.arrivee)
            .any(|b| !valide(b.retrait, 0.0, 100_000.0))
            || self.motif.iter().any(|b| !valide(b.pas, 1.0, 100_000.0))
        {
            return Err("Vérifiez les retraits et les entraxes du motif (1 mm minimum).".into());
        }
        Section::lire(p, self.main_courante.as_deref(), 50.0)?;
        for lisse in &self.lisses {
            Section::lire(p, lisse.profil.as_deref(), 40.0)?;
        }
        for bord in self.depart.iter().chain(&self.arrivee) {
            Section::lire(p, bord.profil.as_deref(), 50.0)?;
        }
        for barreau in &self.motif {
            Section::lire(p, barreau.profil.as_deref(), 20.0)?;
        }
        Ok(())
    }

    pub fn barreaux(&self, longueur: f64) -> Result<Vec<Barreau<'_>>, String> {
        if !longueur.is_finite() {
            return Err("Longueur de garde-corps non valide.".into());
        }
        let debut = self.depart.as_ref().map_or(0.0, |b| b.retrait);
        let fin = longueur - self.arrivee.as_ref().map_or(0.0, |b| b.retrait);
        let mut barreaux = Vec::new();
        if debut >= fin {
            return Ok(barreaux);
        }
        if let Some(b) = &self.depart {
            barreaux.push(Barreau {
                x: debut,
                profil: b.profil.as_deref(),
                materiau: b.materiau.as_deref(),
                extremite: true,
            });
        }
        if self.remplissage == Remplissage::Barreaudage {
            let mut x = debut;
            for b in self.motif.iter().cycle() {
                if !b.pas.is_finite() || b.pas < 1.0 {
                    return Err("Entraxe du motif non valide.".into());
                }
                x += b.pas;
                if x >= fin - 1e-6 {
                    break;
                }
                if !x.is_finite() || barreaux.len() >= 10_000 {
                    return Err("Trop de répétitions dans un segment du garde-corps.".into());
                }
                barreaux.push(Barreau {
                    x,
                    profil: b.profil.as_deref(),
                    materiau: b.materiau.as_deref(),
                    extremite: false,
                });
            }
        }
        if let Some(b) = &self.arrivee {
            barreaux.push(Barreau {
                x: fin,
                profil: b.profil.as_deref(),
                materiau: b.materiau.as_deref(),
                extremite: true,
            });
        }
        Ok(barreaux)
    }

    pub fn solides(&self, p: &Projet, g: &GardeCorps) -> Result<Vec<SolideGardeCorps>, String> {
        self.valider(p)?;
        // triangles regroupés par (matériau, vitrage), dans l'ordre d'apparition
        let mut groupes: Vec<SolideGardeCorps> = Vec::new();
        let mut ajouter = |materiau: Option<&str>, vitre: bool, tris: &mut dyn Iterator<Item = Tri>| {
            let k = match groupes.iter().position(|(m, v, _)| m.as_deref() == materiau && *v == vitre) {
                Some(k) => k,
                None => {
                    groupes.push((materiau.map(str::to_owned), vitre, Vec::new()));
                    groupes.len() - 1
                }
            };
            groupes[k].2.extend(tris);
        };
        let main = Section::lire(p, self.main_courante.as_deref(), 50.0)?;
        let mut rails = vec![(self.hauteur - main.hauteur / 2.0, main, self.materiau_de(&self.materiau_main_courante))];
        for lisse in &self.lisses {
            rails.push((
                lisse.altitude,
                Section::lire(p, lisse.profil.as_deref(), 40.0)?,
                self.materiau_de(&lisse.materiau),
            ));
        }
        let haut = self.hauteur - self.jeu_haut;
        let mut precedent_termine_au_sommet = false;
        let mut sections_verticales = HashMap::new();
        for segment in g.trace.windows(2) {
            let a = DVec2::from_array(segment[0]);
            let b = DVec2::from_array(segment[1]);
            let longueur = a.distance(b);
            if !longueur.is_finite() {
                return Err("Longueur de garde-corps non valide.".into());
            }
            if longueur < 1e-6 {
                continue;
            }
            let axe = (b - a) / longueur;
            let n = axe.perp().extend(0.0);
            let position = |q: DVec2| Ok::<_, String>(q.extend(g.base(p, q)?));
            // Lisses continues en plan, échantillonnées uniquement le long d'un escalier.
            let divisions = if g.escalier.is_some() {
                (longueur / 250.0).ceil() as usize
            } else {
                1
            };
            if divisions > 10_000 {
                return Err("Segment de garde-corps trop long.".into());
            }
            let mut origine = position(a)?;
            for i in 1..=divisions {
                let destination = position(a.lerp(b, i as f64 / divisions as f64))?;
                for (z, section, materiau) in &rails {
                    ajouter(*materiau, false, &mut section.extruder(
                        origine + DVec3::Z * *z,
                        destination + DVec3::Z * *z,
                        n,
                        DVec3::Z,
                    ));
                }
                if self.remplissage != Remplissage::Barreaudage {
                    let section = Section {
                        contour: vec![
                            [-10.0, self.bas],
                            [10.0, self.bas],
                            [10.0, haut],
                            [-10.0, haut],
                        ],
                        largeur: 20.0,
                        hauteur: haut - self.bas,
                    };
                    let vitre = self.remplissage == Remplissage::Vitre;
                    ajouter(self.materiau_remplissage.as_deref(), vitre, &mut section.extruder(origine, destination, n, DVec3::Z));
                }
                origine = destination;
            }
            let barreaux = self.barreaux(longueur)?;
            for barreau in &barreaux {
                if barreau.x == 0.0 && precedent_termine_au_sommet {
                    continue;
                }
                let q = position(a + axe * barreau.x)?;
                let (bas, carre) = if barreau.extremite {
                    (0.0, 50)
                } else {
                    (self.bas, 20)
                };
                let cle = (barreau.profil, carre);
                let section = match sections_verticales.entry(cle) {
                    std::collections::hash_map::Entry::Occupied(e) => e.into_mut(),
                    std::collections::hash_map::Entry::Vacant(e) => {
                        e.insert(Section::lire(p, barreau.profil, f64::from(carre))?)
                    }
                };
                ajouter(barreau.materiau.or(self.materiau.as_deref()), false, &mut section.extruder(
                    q + DVec3::Z * bas,
                    q + DVec3::Z * haut,
                    axe.extend(0.0),
                    n,
                ));
            }
            precedent_termine_au_sommet = barreaux.last().is_some_and(|b| b.x == longueur);
        }
        groupes.retain(|(_, _, t)| !t.is_empty());
        if groupes.is_empty() {
            return Err("Le tracé du garde-corps doit avoir une longueur positive.".into());
        }
        Ok(groupes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn motif_enchaine_les_entraxes_entre_les_montants() {
        let mut t = TypeGardeCorps::standard(1000.0, Remplissage::Barreaudage);
        t.depart.as_mut().unwrap().retrait = 40.0;
        t.arrivee.as_mut().unwrap().retrait = 70.0;
        t.motif.push(BarreauMotif {
            pas: 160.0,
            profil: Some("rond".into()),
            materiau: None,
        });
        let obtenu = t
            .barreaux(700.0)
            .unwrap()
            .into_iter()
            .map(|b| (b.x, b.profil, b.extremite))
            .collect::<Vec<_>>();
        assert_eq!(
            obtenu,
            [
                (40.0, None, true),
                (150.0, None, false),
                (310.0, Some("rond"), false),
                (420.0, None, false),
                (580.0, Some("rond"), false),
                (630.0, None, true),
            ]
        );
        assert!(t.barreaux(100.0).unwrap().is_empty());
    }

    #[test]
    fn le_type_recharge_extrude_le_profil_choisi() {
        use crate::modele::famille::profil::{PointProfil, Profil};
        let mut p = Projet::nouveau("Garde-corps");
        p.niveaux.push(crate::modele::Niveau {
            id: "niv".into(),
            nom: "Sol".into(),
            altitude: 0.0,
            origine: None,
        });
        p.profils.push(Profil {
            id: "section".into(),
            nom: "Triangle".into(),
            parametres: vec![],
            contour: [(0, 0), (60, 0), (0, 30)]
                .map(|(x, y)| PointProfil {
                    x: x.to_string(),
                    y: y.to_string(),
                })
                .to_vec(),
        });
        let mut t = TypeGardeCorps::standard(1000.0, Remplissage::Barreaudage);
        t.id = "type".into();
        t.lisses.clear();
        t.main_courante = Some("section".into());
        t.depart = None;
        t.arrivee = None;
        t.motif.clear();
        p.types_garde_corps.push(t);
        p.garde_corps.push(GardeCorps {
            id: "g".into(),
            nom: "Occurrence".into(),
            niveau: "niv".into(),
            decalage: 0.0,
            trace: vec![[0.0, 0.0], [1500.0, 0.0]],
            hauteur: 1000.0,
            remplissage: Remplissage::Barreaudage,
            type_garde_corps: Some("type".into()),
            escalier: None,
        });
        let p = Projet::lire_json(&p.json().unwrap()).unwrap();
        let solides = p.garde_corps[0].solides(&p).unwrap();
        let triangles = &solides[0].2;
        assert_eq!(triangles.len(), 8);
        assert!(triangles.iter().flatten().any(|q| q.z == 1000.0));
        assert!(triangles.iter().flatten().any(|q| q.z == 970.0));
        let volume = crate::modele::volumes::geometrie::volume(triangles);
        assert!((volume - 1500.0 * 60.0 * 30.0 / 2.0).abs() < 1e-5);
    }

    #[test]
    fn predefinis_valides_et_materiaux_par_element() {
        let mut p = Projet::nouveau("Garde-corps");
        p.niveaux.push(crate::modele::Niveau { id: "niv".into(), nom: "Sol".into(), altitude: 0.0, origine: None });
        for (k, t) in TypeGardeCorps::predefinis().into_iter().enumerate() {
            t.valider(&p).unwrap();
            let mut t = TypeGardeCorps { id: format!("t{k}"), ..t };
            if let Some(l) = t.lisses.first_mut() {
                l.materiau = Some("Bois".into());
            }
            p.types_garde_corps.push(t.clone());
            let g = GardeCorps { id: format!("g{k}"), nom: "G".into(), niveau: "niv".into(), decalage: 0.0, trace: vec![[0.0, 0.0], [2000.0, 0.0]], hauteur: t.hauteur, remplissage: t.remplissage, type_garde_corps: Some(t.id.clone()), escalier: None };
            let solides = t.solides(&p, &g).unwrap();
            assert!(solides.iter().any(|(m, _, _)| m.as_deref() == Some(MATERIAU_ACIER)), "{}", t.nom);
            if !t.lisses.is_empty() {
                assert!(solides.iter().any(|(m, _, _)| m.as_deref() == Some("Bois")));
            }
            if t.remplissage == Remplissage::Vitre {
                assert!(solides.iter().any(|(m, v, _)| *v && m.as_deref() == Some(MATERIAU_VERRE)));
            }
        }
        // type enregistré avant les matériaux : relu sans erreur
        let ancien = r#"{"id":"x","nom":"A","hauteur":1000.0,"remplissage":"barreaudage","main_courante":null,"lisses":[{"altitude":120.0,"profil":null}],"bas":140.0,"jeu_haut":50.0,"depart":null,"arrivee":null,"motif":[]}"#;
        let t: TypeGardeCorps = serde_json::from_str(ancien).unwrap();
        assert!(t.materiau.is_none() && t.lisses[0].materiau.is_none());
    }
}
