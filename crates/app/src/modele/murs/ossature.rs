//! Couches à ossature : montants, lisses et chevêtres d'une couche de mur, solives
//! d'une couche de dalle. Le matériau de la couche remplit les travées ; l'ossature
//! découpe la couche en bandes (membrure ou remplissage) : poché des coupes et
//! surfaces du solide en 3D.
use crate::modele::volumes::solides::{hote, repere};
use crate::modele::{Axe, Couche, Mur, Projet};
use glam::DVec2;
use serde::{Deserialize, Serialize};

/// Matériau des membrures quand l'ossature n'en nomme pas.
pub const MATERIAU_OSSATURE: &str = "Bois d'ossature";

/// Ossature d'une couche : membrures régulières dans l'épaisseur de la couche.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Ossature {
    /// Largeur des membrures (mm), le long du mur ; leur profondeur est l'épaisseur de
    /// la couche.
    pub largeur: f64,
    /// Entraxe des membrures (mm).
    pub entraxe: f64,
    /// Matériau des membrures ; None : [`MATERIAU_OSSATURE`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub materiau: Option<String>,
    /// Murs : lisses basse et haute, linteaux et appuis des ouvertures.
    #[serde(default = "vrai")]
    pub lisses: bool,
    /// Dalles : direction des solives en plan (degrés depuis l'axe X).
    #[serde(default)]
    pub angle: f64,
    /// Murs : membrures horizontales (lisses à l'entraxe sur la hauteur) au lieu de
    /// montants.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub horizontale: bool,
    /// Plume du contour du remplissage en coupe ; None : [`PLUME_REMPLISSAGE`]. Les
    /// membrures gardent la plume de la couche.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plume_remplissage: Option<u8>,
}

/// Plume du remplissage d'une couche à ossature : plus fine que celle d'un isolant
/// seul (3), pour que l'ossature se lise en premier.
pub const PLUME_REMPLISSAGE: u8 = 2;

fn vrai() -> bool {
    true
}

impl Default for Ossature {
    fn default() -> Self {
        Self { largeur: 45.0, entraxe: 600.0, materiau: None, lisses: true, angle: 0.0, horizontale: false, plume_remplissage: None }
    }
}

impl Ossature {
    pub fn materiau(&self) -> &str {
        self.materiau.as_deref().unwrap_or(MATERIAU_OSSATURE)
    }

    pub fn plume_remplissage(&self) -> u8 {
        self.plume_remplissage.unwrap_or(PLUME_REMPLISSAGE).clamp(1, 12)
    }

    pub fn valide(&self) -> bool {
        self.largeur.is_finite() && self.entraxe.is_finite() && self.largeur > 0.0 && self.entraxe > self.largeur
    }

    /// Membrures régulières sur une longueur `l` : une à chaque bout, les autres à
    /// l'entraxe depuis le début (intervalles début-fin).
    pub fn travees(&self, l: f64) -> Vec<(f64, f64)> {
        let w = self.largeur;
        if !self.valide() || l <= 2.0 * w {
            return Vec::new();
        }
        let mut r = vec![(0.0, w)];
        let mut s = self.entraxe;
        while s + w / 2.0 < l - w {
            r.push((s - w / 2.0, s + w / 2.0));
            s += self.entraxe;
        }
        r.push((l - w, l));
        r
    }
}

/// Région d'une couche à ossature : demi-espaces `n · x + w >= 0` (mm, repère du
/// projet), membrure ou remplissage.
#[derive(Debug, Clone)]
pub struct Bande {
    pub plans: Vec<[f64; 4]>,
    pub membrure: bool,
}

/// Ossature d'un mur monocouche (couche d'assemblage comprise).
pub fn ossature_mur<'a>(p: &'a Projet, m: &Mur) -> Option<&'a Ossature> {
    match p.type_mur(&m.type_mur)?.couches.as_slice() {
        [Couche { ossature: Some(o), .. }] if o.valide() => Some(o),
        _ => None,
    }
}

/// Membrures d'un mur à ossature, en rectangles (abscisse début, abscisse fin, z bas,
/// z haut) le long de son axe : lisses, montants, et autour de chaque ouverture deux
/// montants, un linteau et un appui.
pub fn membrures_mur(p: &Projet, m: &Mur) -> Vec<[f64; 4]> {
    let Some(o) = ossature_mur(p, m) else { return Vec::new() };
    let Some((zb, zh)) = p.bas_haut(m) else { return Vec::new() };
    let (l, w) = (m.axe.longueur(), o.largeur);
    if l <= 2.0 * w || zh - zb <= 2.0 * w {
        return Vec::new();
    }
    let mut r = Vec::new();
    let (za, zc) = if o.horizontale {
        // lisses à l'entraxe sur la hauteur, une en pied et une en tête
        r.extend(o.travees(zh - zb).into_iter().map(|(a, b)| [0.0, l, zb + a, zb + b]));
        (zb, zh)
    } else {
        let bornes = if o.lisses {
            r.push([0.0, l, zb, zb + w]);
            r.push([0.0, l, zh - w, zh]);
            (zb + w, zh - w)
        } else {
            (zb, zh)
        };
        r.extend(o.travees(l).into_iter().map(|(a, b)| [a, b, bornes.0, bornes.1]));
        bornes
    };
    for ouv in p.ouvertures.iter().filter(|x| x.mur == m.id || Some(&x.mur) == m.assemblage.as_ref()) {
        let Ok((axe, _, niveau)) = hote(p, &ouv.mur) else { continue };
        let Some(zo) = p.niveau(&niveau).map(|n| n.altitude + ouv.allege) else { continue };
        let centre = m.axe.abscisse(repere(&axe, ouv.abscisse).0.to_array());
        let (a, b) = (centre - ouv.largeur / 2.0, centre + ouv.largeur / 2.0);
        r.push([a - w, a, za, zc]);
        r.push([b, b + w, za, zc]);
        if o.lisses {
            r.push([a - w, b + w, zo + ouv.hauteur, zo + ouv.hauteur + w]);
            r.push([a - w, b + w, zo - w, zo]);
        }
    }
    r.into_iter()
        .map(|[a, b, c, d]| [a.max(0.0), b.min(l), c.max(zb), d.min(zh)])
        .filter(|[a, b, c, d]| b - a > 1e-6 && d - c > 1e-6)
        .collect()
}

/// Longueur cumulée des membrures d'un mur (mm) : la grande dimension de chacune.
pub fn lineaire_mur(p: &Projet, m: &Mur) -> f64 {
    membrures_mur(p, m).iter().map(|[a, b, c, d]| (b - a).max(d - c)).sum()
}

/// Plans d'une portion d'axe entre deux abscisses et deux altitudes (None : sans borne).
fn plans_portion(axe: &Axe, s: (Option<f64>, Option<f64>), z: (Option<f64>, Option<f64>)) -> Vec<[f64; 4]> {
    let mut plans = Vec::new();
    let borne = |x: f64, sens: f64| {
        let (c, t) = repere(axe, x);
        [t.x * sens, t.y * sens, 0.0, -t.dot(c) * sens]
    };
    if let Some(a) = s.0 {
        plans.push(borne(a, 1.0));
    }
    if let Some(b) = s.1 {
        plans.push(borne(b, -1.0));
    }
    // arc : côté du centre où se trouve la portion (les deux plans seuls couperaient
    // aussi le côté opposé du cercle)
    if let (Axe::Arc { centre, .. }, Some(x)) = (axe, s.0.zip(s.1).map(|(a, b)| (a + b) / 2.0).or(s.0).or(s.1)) {
        let c = DVec2::from_array(*centre);
        let radial = (repere(axe, x).0 - c).normalize_or_zero();
        plans.push([radial.x, radial.y, 0.0, -radial.dot(c)]);
    }
    if let Some(a) = z.0 {
        plans.push([0.0, 0.0, 1.0, -a]);
    }
    if let Some(b) = z.1 {
        plans.push([0.0, 0.0, -1.0, b]);
    }
    if plans.is_empty() {
        plans.push([0.0, 0.0, 0.0, 1.0]);
    }
    plans
}

/// Bandes de coupe d'un mur à ossature : la couche découpée en cases le long de l'axe
/// et en hauteur, sans recouvrement. Les cases de rive restent ouvertes vers
/// l'extérieur (onglets des jonctions, hauts attachés). Vide sans ossature.
pub fn bandes_mur(p: &Projet, m: &Mur) -> Vec<Bande> {
    let membrures = membrures_mur(p, m);
    let Some((zb, zh)) = p.bas_haut(m).filter(|_| !membrures.is_empty()) else { return Vec::new() };
    let l = m.axe.longueur();
    let bornes = |fixes: [f64; 2], k: usize| {
        let mut v: Vec<f64> = fixes.into_iter().chain(membrures.iter().flat_map(|r| [r[k], r[k + 1]])).collect();
        v.sort_by(|a, b| a.total_cmp(b));
        v.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
        v
    };
    let (ss, zs) = (bornes([0.0, l], 0), bornes([zb, zh], 2));
    let mut bandes = Vec::new();
    for i in 0..ss.len() - 1 {
        let sm = (ss[i] + ss[i + 1]) / 2.0;
        let s = ((i > 0).then_some(ss[i]), (i + 2 < ss.len()).then_some(ss[i + 1]));
        let pleine = |j: usize| {
            let zm = (zs[j] + zs[j + 1]) / 2.0;
            membrures.iter().any(|r| r[0] < sm && sm < r[1] && r[2] < zm && zm < r[3])
        };
        let mut j = 0;
        while j + 1 < zs.len() {
            let membrure = pleine(j);
            let mut k = j + 1;
            while k + 1 < zs.len() && pleine(k) == membrure {
                k += 1;
            }
            let z = ((j > 0).then_some(zs[j]), (k + 1 < zs.len()).then_some(zs[k]));
            bandes.push(Bande { plans: plans_portion(&m.axe, s, z), membrure });
            j = k;
        }
    }
    bandes
}

/// Répartit les triangles d'un solide (mm) entre remplissage et membrures : chaque
/// triangle est découpé par les bandes, qui se partagent l'espace sans recouvrement.
/// Les surfaces du solide montrent ainsi l'ossature en 3D sans solide supplémentaire.
pub fn repartir(tris: &[[glam::DVec3; 3]], bandes: &[Bande]) -> (Vec<[glam::DVec3; 3]>, Vec<[glam::DVec3; 3]>) {
    use glam::DVec3;
    let (mut remplissage, mut membrures) = (Vec::new(), Vec::new());
    let mut polygone: Vec<DVec3> = Vec::new();
    let mut suivant: Vec<DVec3> = Vec::new();
    for b in bandes {
        let sortie = if b.membrure { &mut membrures } else { &mut remplissage };
        for t in tris {
            polygone.clear();
            polygone.extend_from_slice(t);
            for q in &b.plans {
                let (n, d) = (DVec3::new(q[0], q[1], q[2]), q[3]);
                suivant.clear();
                for k in 0..polygone.len() {
                    let (a, c) = (polygone[k], polygone[(k + 1) % polygone.len()]);
                    let (sa, sc) = (n.dot(a) + d, n.dot(c) + d);
                    if sa >= 0.0 {
                        suivant.push(a);
                    }
                    if (sa >= 0.0) != (sc >= 0.0) {
                        suivant.push(a + (c - a) * (sa / (sa - sc)));
                    }
                }
                std::mem::swap(&mut polygone, &mut suivant);
                if polygone.len() < 3 {
                    break;
                }
            }
            for k in 1..polygone.len().saturating_sub(1) {
                let t = [polygone[0], polygone[k], polygone[k + 1]];
                if (t[1] - t[0]).cross(t[2] - t[0]).length_squared() > 1e-6 {
                    sortie.push(t);
                }
            }
        }
    }
    (remplissage, membrures)
}

/// Bandes d'une couche de dalle à solives : colonnes perpendiculaires à la direction
/// des solives sur l'emprise des contours (plans à ajouter à ceux de la couche).
pub fn bandes_dalle(o: &Ossature, contours: &[Vec<[f64; 2]>]) -> Vec<Bande> {
    let n = DVec2::from_angle(o.angle.to_radians()).perp();
    let cotes: Vec<f64> = contours.iter().flatten().map(|q| n.dot(DVec2::from_array(*q))).collect();
    let (min, max) = cotes.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), x| (a.min(*x), b.max(*x)));
    let travees = o.travees(max - min);
    let mut bandes = Vec::new();
    let plans = |a: Option<f64>, b: Option<f64>| {
        let mut v = Vec::new();
        if let Some(a) = a {
            v.push([n.x, n.y, 0.0, -(min + a)]);
        }
        if let Some(b) = b {
            v.push([-n.x, -n.y, 0.0, min + b]);
        }
        v
    };
    for (k, (a, b)) in travees.iter().enumerate() {
        bandes.push(Bande { plans: plans((k > 0).then_some(*a), (k + 1 < travees.len()).then_some(*b)), membrure: true });
        if let Some((suivant, _)) = travees.get(k + 1) {
            bandes.push(Bande { plans: plans(Some(*b), Some(*suivant)), membrure: false });
        }
    }
    bandes
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modele::edition::ParametresMur;
    use crate::modele::{FonctionCouche, GenreOuverture, Niveau, Ouverture, TypeCompose};

    fn projet() -> (Projet, String) {
        let mut p = Projet::nouveau("Ossature");
        p.niveaux.push(Niveau { id: "n".into(), nom: "RDC".into(), altitude: 0.0, origine: None });
        let couche = Couche { materiau: Some("Laine".into()), epaisseur: 145.0, fonction: FonctionCouche::Structure, ossature: Some(Ossature::default()), ..Default::default() };
        p.types_murs.push(TypeCompose { id: "mob".into(), nom: "MOB 145".into(), couches: vec![couche], couches_detaillees: true, categorie: None, origine: None });
        let id = p.ajouter_mur([0.0, 0.0], [3000.0, 0.0], &ParametresMur { type_mur: Some("mob".into()), niveau: Some("n".into()), ..Default::default() }).unwrap();
        (p, id)
    }

    fn dedans(b: &Bande, x: [f64; 3]) -> bool {
        b.plans.iter().all(|q| q[0] * x[0] + q[1] * x[1] + q[2] * x[2] + q[3] >= 0.0)
    }

    #[test]
    fn montants_a_l_entraxe_et_aux_bouts() {
        let o = Ossature::default();
        let t = o.travees(3000.0);
        assert_eq!(t.first(), Some(&(0.0, 45.0)));
        assert_eq!(t.last(), Some(&(2955.0, 3000.0)));
        assert_eq!(t.len(), 6);
        assert_eq!(t[1], (577.5, 622.5));
        assert!(Ossature { entraxe: 30.0, ..o }.travees(3000.0).is_empty());
    }

    #[test]
    fn bandes_du_mur_sans_recouvrement() {
        let (p, id) = projet();
        let m = p.murs.iter().find(|m| m.id == id).unwrap();
        let (zb, zh) = p.bas_haut(m).unwrap();
        let bandes = bandes_mur(&p, m);
        assert!(bandes.iter().any(|b| b.membrure) && bandes.iter().any(|b| !b.membrure));
        // un point : une seule bande ; montant à 600, travée à 300, lisse en pied
        let zm = (zb + zh) / 2.0;
        for (x, z, membrure) in [(600.0, zm, true), (300.0, zm, false), (300.0, zb + 10.0, true), (-50.0, zm, true), (3050.0, zh + 100.0, true)] {
            let touchees: Vec<_> = bandes.iter().filter(|b| dedans(b, [x, 0.0, z])).collect();
            assert_eq!(touchees.len(), 1, "x = {x}, z = {z}");
            assert_eq!(touchees[0].membrure, membrure, "x = {x}, z = {z}");
        }
        // 6 montants et 2 lisses
        let attendu = 6.0 * (zh - zb - 90.0) + 2.0 * 3000.0;
        assert!((lineaire_mur(&p, m) - attendu).abs() < 1e-6);
    }

    #[test]
    fn chevetre_autour_d_une_ouverture() {
        let (mut p, id) = projet();
        p.ouvertures.push(Ouverture { id: "f".into(), nom: "Fenêtre".into(), genre: GenreOuverture::Libre, mur: id.clone(), type_ouverture: None, abscisse: 1500.0, largeur: 800.0, hauteur: 1000.0, allege: 900.0, representation: None, valeurs: Default::default(), materiaux: Default::default(), retournee: false, inversee: false, origine: None });
        let m = p.murs.iter().find(|m| m.id == id).unwrap();
        let r = membrures_mur(&p, m);
        // montants de part et d'autre, linteau et appui
        assert!(r.iter().any(|x| x[0] == 1055.0 && x[1] == 1100.0));
        assert!(r.iter().any(|x| x[0] == 1900.0 && x[1] == 1945.0));
        assert!(r.iter().any(|x| x[2] == 1900.0 && x[3] == 1945.0 && x[0] == 1055.0));
        assert!(r.iter().any(|x| x[2] == 855.0 && x[3] == 900.0));
    }

    #[test]
    fn ossature_horizontale_et_surfaces_reparties() {
        let (mut p, id) = projet();
        p.types_murs[0].couches[0].ossature = Some(Ossature { horizontale: true, ..Default::default() });
        let m = p.murs.iter().find(|m| m.id == id).unwrap();
        let (zb, zh) = p.bas_haut(m).unwrap();
        let r = membrures_mur(&p, m);
        // lisses sur toute la longueur, à l'entraxe sur la hauteur
        assert!(r.iter().all(|x| x[0] == 0.0 && x[1] == 3000.0 && (x[3] - x[2] - 45.0).abs() < 1e-9));
        assert_eq!(r.len(), Ossature::default().travees(zh - zb).len());
        // surfaces du solide : aire conservée, part des membrures sur une grande face
        let tris = crate::modele::volumes::solides::mur(&p, m, &Default::default()).unwrap();
        let aire = |t: &[[glam::DVec3; 3]]| t.iter().map(|t| (t[1] - t[0]).cross(t[2] - t[0]).length() / 2.0).sum::<f64>();
        let (remplissage, membrures) = repartir(&tris, &bandes_mur(&p, m));
        assert!((aire(&remplissage) + aire(&membrures) - aire(&tris)).abs() < 1.0);
        let face = |t: &[[glam::DVec3; 3]]| aire(&t.iter().filter(|t| t.iter().all(|q| (q.y - 72.5).abs() < 1e-6)).cloned().collect::<Vec<_>>());
        assert!((face(&membrures) - r.len() as f64 * 45.0 * 3000.0).abs() < 1.0, "{}", face(&membrures));
    }

    #[test]
    fn solives_de_dalle_et_lecture_sans_ossature() {
        let o = Ossature { largeur: 60.0, entraxe: 400.0, angle: 90.0, ..Default::default() };
        let bandes = bandes_dalle(&o, &[vec![[0.0, 0.0], [2000.0, 0.0], [2000.0, 3000.0], [0.0, 3000.0]]]);
        // solives le long de Y : réparties sur X
        assert_eq!(bandes.iter().filter(|b| b.membrure).count(), 6);
        assert!(bandes.iter().filter(|b| b.membrure).any(|b| dedans(b, [400.0, 1500.0, 0.0])));
        assert!(bandes.iter().filter(|b| !b.membrure).any(|b| dedans(b, [200.0, 1500.0, 0.0])));
        let c: Couche = serde_json::from_str(r#"{"materiau":null,"epaisseur":100.0}"#).unwrap();
        assert_eq!(c.ossature, None);
        assert!(!serde_json::to_string(&c).unwrap().contains("ossature"));
    }
}
