//! Plans de référence d'orientation libre. Un plan de référence est soit un plan d'axe
//! (`PlanRef` : X, Y ou Z et une position), soit un plan tourné (`PlanTourne`) : il part
//! de son plan hôte et pivote, d'un angle cotable, autour de la droite d'appui, intersection
//! de l'hôte et d'un plan d'appui. Hôte et appui peuvent être eux-mêmes tournés : toutes
//! les orientations sont atteignables et le plan suit les paramètres (cote angulaire
//! `Contrainte::Angle` libellée par une formule).
//!
//! Une forme à profil peut être posée sur un plan de référence (`SupportForme::Plan`) :
//! son esquisse est exprimée dans le repère du plan et le suit ; ou sur un repère figé
//! (`SupportForme::Repere`, face d'un solide au moment du choix).
use super::{evaluer, AxePlan, Contrainte, FamilleModelee, Resolution};
use glam::{DMat3, DQuat, DVec3};
use serde::{Deserialize, Serialize};

/// Plan tourné de `angle` degrés depuis `hote` autour de la droite `hote` ∩ `appui`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PlanTourne {
    pub nom: String,
    pub hote: String,
    pub appui: String,
    /// Angle dessiné (degrés) : celui d'un plan qu'aucune cote angulaire ne pilote.
    pub angle: f64,
}

/// Repère orthonormé d'un plan : origine, axes `h` et `v` dans le plan ; normale h × v.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Repere {
    pub origine: DVec3,
    pub h: DVec3,
    pub v: DVec3,
}

impl Repere {
    /// Repère d'un plan d'axe à sa position : axes de la vue d'esquisse parallèle.
    pub fn axe(axe: AxePlan, position: f64) -> Repere {
        match axe {
            AxePlan::X => Repere { origine: DVec3::X * position, h: DVec3::Y, v: DVec3::Z },
            AxePlan::Y => Repere { origine: DVec3::Y * position, h: DVec3::X, v: DVec3::Z },
            AxePlan::Z => Repere { origine: DVec3::Z * position, h: DVec3::X, v: DVec3::Y },
        }
    }
    pub fn normale(&self) -> DVec3 {
        self.h.cross(self.v)
    }
    /// Point de la famille depuis (h, v, profondeur suivant la normale).
    pub fn point(&self, h: f64, v: f64, d: f64) -> DVec3 {
        self.origine + self.h * h + self.v * v + self.normale() * d
    }
    /// Coordonnées locales (h, v, profondeur) d'un point de la famille.
    pub fn local(&self, p: DVec3) -> DVec3 {
        let q = p - self.origine;
        DVec3::new(q.dot(self.h), q.dot(self.v), q.dot(self.normale()))
    }
    pub fn valide(&self) -> bool {
        self.origine.is_finite() && self.h.is_finite() && self.v.is_finite()
            && (self.h.length_squared() - 1.0).abs() < 1e-6 && (self.v.length_squared() - 1.0).abs() < 1e-6 && self.h.dot(self.v).abs() < 1e-6
    }
    /// Plans parallèles (normales colinéaires).
    pub fn parallele(&self, autre: &Repere) -> bool {
        self.normale().cross(autre.normale()).length() < 1e-9
    }
}

/// Support d'une forme à profil esquissée dans un repère propre.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(tag = "genre", rename_all = "snake_case", deny_unknown_fields)]
pub enum SupportForme {
    /// Plan de référence (d'axe ou tourné) : la forme suit ses contraintes et son angle.
    Plan { plan: String },
    /// Repère figé, pris sur la face d'un solide au moment du choix.
    Repere { origine: [f64; 3], horizontal: [f64; 3], vertical: [f64; 3] },
}

impl SupportForme {
    /// Support figé sur un repère.
    pub fn fige(r: &Repere) -> SupportForme {
        SupportForme::Repere { origine: r.origine.to_array(), horizontal: r.h.to_array(), vertical: r.v.to_array() }
    }
}

/// Droite d'intersection de deux plans (point le plus proche de l'origine du premier,
/// direction unitaire) ; None s'ils sont parallèles.
pub fn intersection(a: &Repere, b: &Repere) -> Option<(DVec3, DVec3)> {
    let (na, nb) = (a.normale(), b.normale());
    let d = na.cross(nb);
    if d.length() < 1e-9 {
        return None;
    }
    let d = d.normalize();
    let m = DMat3::from_cols(na, nb, d).transpose();
    let p = m.inverse() * DVec3::new(na.dot(a.origine), nb.dot(b.origine), d.dot(a.origine));
    p.is_finite().then_some((p, d))
}

/// Repère du plan tourné de `angle` degrés depuis `hote` autour de `hote` ∩ `appui` :
/// h suit la droite d'appui, l'origine est sur elle.
pub fn tourner(hote: &Repere, appui: &Repere, angle: f64) -> Option<Repere> {
    let (p, d) = intersection(hote, appui)?;
    let n = DQuat::from_axis_angle(d, angle.to_radians()) * hote.normale();
    Some(Repere { origine: p, h: d, v: n.cross(d).normalize() })
}

impl FamilleModelee {
    pub fn plan_tourne(&self, nom: &str) -> Option<&PlanTourne> {
        self.plans_tournes.iter().find(|p| p.nom == nom)
    }

    /// Plan de référence existant, d'axe ou tourné.
    pub fn existe_plan(&self, nom: &str) -> bool {
        self.plan(nom).is_some() || self.plan_tourne(nom).is_some()
    }

    /// Noms de tous les plans de référence, plans d'axe d'abord.
    pub fn noms_plans(&self) -> Vec<&str> {
        self.plans.iter().map(|p| p.nom.as_str()).chain(self.plans_tournes.iter().map(|p| p.nom.as_str())).collect()
    }

    /// Repère résolu d'un plan de référence quelconque.
    pub fn repere_plan(&self, nom: &str, r: &Resolution) -> Option<Repere> {
        if let Some(p) = self.plan(nom) {
            return Some(Repere::axe(p.axe, r.positions.get(nom).copied().unwrap_or(p.position)));
        }
        r.reperes.get(nom).copied()
    }

    /// Repère d'un support de forme (plan suivi ou repère figé).
    pub fn repere_support(&self, s: &SupportForme, r: &Resolution) -> Option<Repere> {
        match s {
            SupportForme::Plan { plan } => self.repere_plan(plan, r),
            SupportForme::Repere { origine, horizontal, vertical } => {
                let rep = Repere { origine: DVec3::from_array(*origine), h: DVec3::from_array(*horizontal), v: DVec3::from_array(*vertical) };
                rep.valide().then_some(rep)
            }
        }
    }

    /// Angle courant (degrés) d'un plan tourné : la cote angulaire de plus haute priorité
    /// qui le pilote, sinon l'angle dessiné. Les cotes écartées vont dans `conflits`.
    fn angle_resolu(&self, p: &PlanTourne, r: &mut Resolution, erreurs: &mut Vec<String>) -> f64 {
        let mut cotes: Vec<(usize, &Contrainte)> = self.contraintes.iter().enumerate()
            .filter(|(_, c)| matches!(c, Contrainte::Angle { b, .. } if *b == p.nom)).collect();
        cotes.sort_by_key(|(_, c)| std::cmp::Reverse(c.priorite()));
        let mut angle = None;
        for (i, c) in cotes {
            let Contrainte::Angle { valeur, .. } = c else { continue };
            if angle.is_some() {
                r.conflits.push(i);
                continue;
            }
            match evaluer(valeur, &r.valeurs) {
                Ok(v) => angle = Some(v),
                Err(e) => erreurs.push(format!("Contrainte {} : {e}", i + 1)),
            }
        }
        angle.unwrap_or(p.angle)
    }

    /// Résout les plans tournés dans l'ordre de leurs dépendances (après les plans d'axe).
    pub(super) fn resoudre_orientations(&self, r: &mut Resolution, erreurs: &mut Vec<String>) {
        let mut restants: Vec<&PlanTourne> = self.plans_tournes.iter().collect();
        loop {
            let avant = restants.len();
            let mut suivants = Vec::new();
            for p in restants {
                let (Some(hote), Some(appui)) = (self.repere_plan(&p.hote, r), self.repere_plan(&p.appui, r)) else {
                    suivants.push(p);
                    continue;
                };
                let angle = self.angle_resolu(p, r, erreurs);
                match tourner(&hote, &appui, angle) {
                    Some(rep) => { r.reperes.insert(p.nom.clone(), rep); }
                    None => erreurs.push(format!("{} : hôte « {} » et appui « {} » parallèles, pas de droite d'appui.", p.nom, p.hote, p.appui)),
                }
            }
            restants = suivants;
            if restants.is_empty() || restants.len() == avant {
                break;
            }
        }
        for p in restants {
            erreurs.push(format!("{} : hôte ou appui absent, ou plans tournés l'un autour de l'autre.", p.nom));
        }
    }

    /// Contrôle des plans tournés et des cotes angulaires (noms, références).
    pub(super) fn valider_orientations(&self) -> Result<(), String> {
        for p in &self.plans_tournes {
            if p.hote == p.nom || p.appui == p.nom || p.hote == p.appui || !self.existe_plan(&p.hote) || !self.existe_plan(&p.appui) || !p.angle.is_finite() {
                return Err(format!("{} : hôte et appui doivent être deux autres plans de référence existants.", p.nom));
            }
        }
        for (i, c) in self.contraintes.iter().enumerate() {
            match c {
                Contrainte::Angle { a, b, .. } => {
                    if self.plan_tourne(b).is_none_or(|p| &p.hote != a) {
                        return Err(format!("Contrainte {} : une cote angulaire relie un plan tourné « {b} » à son hôte « {a} ».", i + 1));
                    }
                }
                _ => {
                    let axes: Vec<Option<AxePlan>> = c.plans().iter().map(|n| self.plan(n).map(|p| p.axe)).collect();
                    if axes.iter().any(Option::is_none) || axes.windows(2).any(|w| w[0] != w[1]) {
                        return Err(format!("Contrainte {} : cotes et égalités relient des plans d'axe parallèles existants.", i + 1));
                    }
                }
            }
        }
        Ok(())
    }

    /// Plan tourné passant par trois points (repère de la famille, mm) : sa droite
    /// d'appui est l'intersection de deux plans de référence existants contenue dans le
    /// plan des trois points ; parmi les solutions, l'angle le plus faible.
    pub fn plan_par_trois_points(&self, r: &Resolution, nom: &str, [a, b, c]: [DVec3; 3]) -> Result<PlanTourne, String> {
        let n = (b - a).cross(c - a);
        if !n.is_finite() || n.length() < 1e-6 {
            return Err("Trois points distincts et non alignés attendus.".into());
        }
        let n = n.normalize();
        let noms = self.noms_plans();
        let mut meilleur: Option<PlanTourne> = None;
        for hote in &noms {
            let Some(rh) = self.repere_plan(hote, r) else { continue };
            for appui in noms.iter().filter(|x| *x != hote) {
                let Some(ra) = self.repere_plan(appui, r) else { continue };
                let Some((p, d)) = intersection(&rh, &ra) else { continue };
                if d.dot(n).abs() > 1e-6 || n.dot(p - a).abs() > 0.5 {
                    continue;
                }
                let nh = rh.normale();
                // un plan n'a pas de sens : angle ramené dans ]-90°, 90°]
                let angle = nh.cross(n).dot(d).atan2(nh.dot(n)).to_degrees();
                let angle = if angle > 90.0 { angle - 180.0 } else if angle <= -90.0 { angle + 180.0 } else { angle };
                let angle = (angle * 1e6).round() / 1e6;
                if angle.abs() < 1e-6 {
                    return Err(format!("Ce plan est déjà le plan de référence « {hote} »."));
                }
                if meilleur.as_ref().is_none_or(|m| angle.abs() < m.angle.abs()) {
                    meilleur = Some(PlanTourne { nom: nom.into(), hote: (*hote).into(), appui: (*appui).into(), angle });
                }
            }
        }
        meilleur.ok_or_else(|| "Aucune droite d'appui dans ce plan : il doit contenir l'intersection de deux plans de référence (en ajouter un d'abord).".into())
    }
}

#[cfg(test)]
mod tests {
    use super::super::*;
    use super::*;

    const DIM: (f64, f64, f64) = (1200.0, 1350.0, 300.0);

    /// Fenêtre avec « Incliné » tourné depuis « Bas » autour de Bas ∩ Gauche, piloté par
    /// le paramètre « Pente », et « Biais » tourné autour de Incliné ∩ Haut.
    fn famille() -> FamilleModelee {
        let mut f = FamilleModelee::gabarit_fenetre();
        f.parametres.push(Parametre::nouveau("Pente", "30"));
        f.plans_tournes.push(PlanTourne { nom: "Incliné".into(), hote: "Bas".into(), appui: "Gauche".into(), angle: 10.0 });
        f.plans_tournes.push(PlanTourne { nom: "Biais".into(), hote: "Incliné".into(), appui: "Origine Y".into(), angle: 45.0 });
        f.plans.push(PlanRef { nom: "Origine Y".into(), axe: AxePlan::Y, position: 0.0, fixe: true, hote: None });
        f.contraintes.push(Contrainte::Angle { a: "Bas".into(), b: "Incliné".into(), valeur: "Pente".into(), priorite: 5, ligne: None });
        f
    }

    fn resolution(f: &FamilleModelee) -> Resolution {
        let (l, h, e) = DIM;
        f.resoudre(l, h, e)
    }

    #[test]
    fn plan_tourne_enchaine_suit_le_parametre() {
        let mut f = famille();
        let (l, h, e) = DIM;
        f.valider(l, h, e).unwrap();
        let r = resolution(&f);
        assert!(r.erreurs.is_empty(), "{:?}", r.erreurs);
        let incline = f.repere_plan("Incliné", &r).unwrap();
        // contient la droite Bas ∩ Gauche (x = -600, z = 0), incliné de 30° sur l'horizontale
        assert!(incline.local(DVec3::new(-600.0, 500.0, 0.0)).z.abs() < 1e-9);
        assert!((incline.normale().angle_between(DVec3::Z).to_degrees() - 30.0).abs() < 1e-9);
        // Biais : à 45° d'Incliné, contient la droite Incliné ∩ Origine Y
        let biais_suit = |f: &FamilleModelee, r: &Resolution, incline: &Repere| {
            let biais = f.repere_plan("Biais", r).unwrap();
            let (p, d) = intersection(incline, &Repere::axe(AxePlan::Y, 0.0)).unwrap();
            assert!((biais.normale().angle_between(incline.normale()).to_degrees() - 45.0).abs() < 1e-9);
            assert!(biais.normale().dot(d).abs() < 1e-9 && biais.local(p).z.abs() < 1e-9 && biais.local(p + d * 700.0).z.abs() < 1e-9);
        };
        biais_suit(&f, &r, &incline);
        // le paramètre change : les deux plans suivent
        f.parametres.iter_mut().find(|p| p.nom == "Pente").unwrap().formule = "45".into();
        let r = resolution(&f);
        let incline = f.repere_plan("Incliné", &r).unwrap();
        assert!((incline.normale().angle_between(DVec3::Z).to_degrees() - 45.0).abs() < 1e-9);
        biais_suit(&f, &r, &incline);
        // la largeur change : la droite d'appui suit le plan Gauche
        let r = f.resoudre(1600.0, h, e);
        assert!(f.repere_plan("Incliné", &r).unwrap().local(DVec3::new(-800.0, 0.0, 0.0)).z.abs() < 1e-9);
    }

    #[test]
    fn cote_angulaire_entre_plans_non_paralleles_seulement() {
        let (l, h, e) = DIM;
        let mut f = famille();
        // sans cote angulaire : angle dessiné
        f.contraintes.pop();
        let r = resolution(&f);
        assert!((f.repere_plan("Incliné", &r).unwrap().normale().angle_between(DVec3::Z).to_degrees() - 10.0).abs() < 1e-9);
        // cote angulaire hors de l'hôte refusée ; cote linéaire vers un plan tourné refusée
        let mut g = famille();
        g.contraintes.push(Contrainte::Angle { a: "Gauche".into(), b: "Incliné".into(), valeur: "20".into(), priorite: 1, ligne: None });
        assert!(g.valider(l, h, e).is_err());
        let mut g = famille();
        g.contraintes.push(Contrainte::Distance { a: "Bas".into(), b: "Incliné".into(), valeur: "20".into(), priorite: 1, ligne: None });
        assert!(g.valider(l, h, e).is_err());
        // deux cotes angulaires : la moins prioritaire est en conflit
        let mut g = famille();
        g.contraintes.push(Contrainte::Angle { a: "Bas".into(), b: "Incliné".into(), valeur: "60".into(), priorite: 1, ligne: None });
        let r = resolution(&g);
        assert_eq!(r.conflits, vec![g.contraintes.len() - 1]);
        // hôte et appui parallèles : pas de droite d'appui
        let mut g = famille();
        g.plans_tournes[0].appui = "Haut".into();
        let r = resolution(&g);
        assert!(r.erreurs.iter().any(|e| e.contains("parallèles")), "{:?}", r.erreurs);
        // cycle
        let mut g = famille();
        g.plans_tournes[0].hote = "Biais".into();
        assert!(!resolution(&g).erreurs.is_empty());
    }

    #[test]
    fn trois_points_vers_appui_et_angle() {
        let f = FamilleModelee::gabarit_fenetre();
        let r = resolution(&f);
        // plan contenant la droite Bas ∩ Gauche, incliné de 30°
        let (s, c) = 30f64.to_radians().sin_cos();
        let pts = [DVec3::new(-600.0, 0.0, 0.0), DVec3::new(-600.0, 400.0, 0.0), DVec3::new(-600.0 + 500.0 * c, 0.0, 500.0 * s)];
        let p = f.plan_par_trois_points(&r, "Tourné", pts).unwrap();
        assert_eq!((p.hote.as_str(), p.appui.as_str()), ("Bas", "Gauche"));
        let mut g = f.clone();
        g.plans_tournes.push(p);
        let r = resolution(&g);
        let rep = g.repere_plan("Tourné", &r).unwrap();
        for q in pts {
            assert!(rep.local(q).z.abs() < 1e-6);
        }
        // points alignés ou plan sans droite d'appui : refus
        assert!(f.plan_par_trois_points(&r, "X", [DVec3::ZERO, DVec3::X, DVec3::X * 2.0]).is_err());
        let quelconque = [DVec3::new(1.0, 7.0, 3.0), DVec3::new(130.0, -50.0, 77.0), DVec3::new(-40.0, 90.0, 250.0)];
        assert!(f.plan_par_trois_points(&r, "X", quelconque).is_err());
    }

    #[test]
    fn forme_posee_sur_plan_tourne_suit_le_parametre() {
        let mut f = famille();
        let face = |x| Face { plan: String::new(), decalage: x };
        f.extrusions.push(FormeProfil {
            operation: OperationForme::Extrusion, support: Some(SupportForme::Plan { plan: "Incliné".into() }), nom: "Pupitre".into(), vide: false,
            materiau: Materiau::Menuiserie, vue: VueEsquisse::Plan,
            profil: [[0.0, 0.0], [300.0, 0.0], [300.0, 200.0], [0.0, 200.0]].into_iter().map(|q| PointEsquisse { h: face(q[0]), v: face(q[1]) }).collect(),
            profondeur: [face(0.0), face(50.0)], visible: None, visibilite: Default::default(), materiau_parametre: None,
        });
        let (l, h, e) = DIM;
        f.valider(l, h, e).unwrap();
        let normale = |f: &FamilleModelee| {
            let r = resolution(f);
            let s = f.solides(&r).pop().unwrap();
            let vol: f64 = s.tris.iter().map(|t| t[0].dot(t[1].cross(t[2])) / 6.0).sum();
            assert!((vol.abs() - 300.0 * 200.0 * 50.0).abs() < 1e-3, "{vol}");
            f.repere_plan("Incliné", &r).map(|rep| s.tris.iter().flatten().all(|p| { let z = rep.local(*p).z; z > -1e-6 && z < 50.0 + 1e-6 })).unwrap()
        };
        assert!(normale(&f));
        let avant = f.solides(&resolution(&f)).pop().unwrap().tris;
        f.parametres.iter_mut().find(|p| p.nom == "Pente").unwrap().formule = "60".into();
        assert!(normale(&f));
        assert_ne!(f.solides(&resolution(&f)).pop().unwrap().tris, avant);
        // aller-retour JSON et repère figé
        let relue: FamilleModelee = serde_json::from_str(&serde_json::to_string(&f).unwrap()).unwrap();
        assert_eq!(relue, f);
        let rep = f.repere_plan("Incliné", &resolution(&f)).unwrap();
        f.extrusions.last_mut().unwrap().support = Some(SupportForme::fige(&rep));
        f.parametres.iter_mut().find(|p| p.nom == "Pente").unwrap().formule = "15".into();
        f.valider(l, h, e).unwrap();
        let s = f.solides(&resolution(&f)).pop().unwrap();
        assert!(s.tris.iter().flatten().all(|p| rep.local(*p).z > -1e-6));
    }

    #[test]
    fn champs_inconnus_refuses() {
        let f = famille();
        let mut v = serde_json::to_value(&f).unwrap();
        v["inconnu"] = serde_json::json!([]);
        assert!(serde_json::from_value::<FamilleModelee>(v).is_err());
        let mut v = serde_json::to_value(&f).unwrap();
        v["plans_tournes"][0]["inconnu"] = serde_json::json!(1);
        assert!(serde_json::from_value::<FamilleModelee>(v).is_err());
        let mut v = serde_json::to_value(&f).unwrap();
        v["plans"][0]["inconnu"] = serde_json::json!(1);
        assert!(serde_json::from_value::<FamilleModelee>(v).is_err());
        assert!(serde_json::from_str::<SupportForme>(r#"{"genre":"plan","plan":"A","x":1}"#).is_err());
        let fichier = FichierFamille { format: FORMAT_FAMILLE.into(), nom: "F".into(), largeur: 1200.0, hauteur: 1350.0, profondeur: 300.0, famille: f };
        let mut v = serde_json::to_value(&fichier).unwrap();
        v["famille"]["extrusions"][0]["inconnu"] = serde_json::json!(true);
        assert!(FichierFamille::lire(&v.to_string()).is_err());
    }
}
