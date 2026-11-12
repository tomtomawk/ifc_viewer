//! Hôte élargi d'une ouverture et retours aux insertions.
//!
//! Des murs droits parallèles et jointifs (face contre face) qui couvrent la baie
//! forment, pour la découpe, un seul mur composé : l'ouverture les perce tous, et sa
//! famille est résolue sur l'épaisseur totale. Une couche marquée « retour » tourne
//! dans les tableaux et le linteau, dans l'épaisseur du mur, jusqu'au plan
//! « Retour extérieur » ou « Retour intérieur » de la famille selon son côté (de part
//! et d'autre de la structure). Sans ces plans, pas de retour.
use crate::modele::famille::modelee::FamilleModelee;
use crate::modele::volumes::solides::{prisme, repere, Tri};
use crate::modele::*;
use glam::DVec2;

/// Mur d'un hôte élargi : identifiant et faces (mm, depuis l'axe de l'hôte, extérieur
/// positif).
#[derive(Clone, Debug)]
pub struct Membre {
    pub id: String,
    pub y0: f64,
    pub y1: f64,
}

/// Hôte élargi : axe au milieu de l'épaisseur totale, épaisseur, niveau de l'hôte et murs.
#[derive(Clone, Debug)]
pub struct HoteElargi {
    pub axe: Axe,
    pub epaisseur: f64,
    pub niveau: String,
    pub membres: Vec<Membre>,
}

fn droite(m: &Mur) -> Option<(DVec2, DVec2)> {
    match m.axe {
        Axe::Droit { debut, fin } => Some((DVec2::from_array(debut), DVec2::from_array(fin))),
        Axe::Arc { .. } => None,
    }
}

/// Hôte élargi de l'ouverture `o` (hôte seul pour un mur courbe).
pub fn hote_elargi(p: &Projet, o: &Ouverture) -> Result<HoteElargi, String> {
    let (axe, e, niveau) = crate::modele::volumes::solides::hote(p, &o.mur)?;
    let couches: Vec<&Mur> = match p.assemblage(&o.mur) {
        Some(_) => p.couches_assemblage(&o.mur),
        None => p.murs.iter().filter(|m| m.id == o.mur).collect(),
    };
    let Axe::Droit { debut, fin } = axe else {
        return Ok(HoteElargi { axe, epaisseur: e, niveau, membres: couches.iter().map(|m| Membre { id: m.id.clone(), y0: -e / 2.0, y1: e / 2.0 }).collect() });
    };
    let (a, b) = (DVec2::from_array(debut), DVec2::from_array(fin));
    let u = (b - a).try_normalize().ok_or("Axe de l'hôte nul.")?;
    let n = u.perp();
    let z = p.niveau(&niveau).ok_or("Niveau de l'ouverture introuvable.")?.altitude + o.allege;
    let (x0, x1) = (o.abscisse - o.largeur / 2.0, o.abscisse + o.largeur / 2.0);
    // faces d'un mur droit parallèle, dans le repère de l'axe de l'hôte
    let faces = |m: &Mur| -> Option<(f64, f64)> {
        let (c, d) = droite(m)?;
        let v = (d - c).try_normalize()?;
        if u.perp_dot(v).abs() > 1e-3 {
            return None;
        }
        let ep = p.type_mur(&m.type_mur)?.epaisseur();
        let y = (c - a).dot(n);
        Some((y - ep / 2.0, y + ep / 2.0))
    };
    // Une couche non parallèle (désaxée, courbe) est ignorée seule ; les autres restent empilées.
    let mut membres: Vec<Membre> = couches.iter().filter_map(|m| faces(m).map(|(y0, y1)| Membre { id: m.id.clone(), y0, y1 })).collect();
    if membres.is_empty() {
        return Ok(HoteElargi { axe, epaisseur: e, niveau, membres: couches.iter().map(|m| Membre { id: m.id.clone(), y0: -e / 2.0, y1: e / 2.0 }).collect() });
    }
    let Some(hote) = p.murs.iter().find(|m| m.id == o.mur).or(couches.first().copied()) else {
        return Ok(HoteElargi { axe, epaisseur: e, niveau, membres });
    };
    // voisins jointifs, de proche en proche
    loop {
        let (lo, hi) = membres.iter().fold((f64::MAX, f64::MIN), |(lo, hi), m| (lo.min(m.y0), hi.max(m.y1)));
        let mut ajout = false;
        for m in &p.murs {
            if membres.iter().any(|x| x.id == m.id) {
                continue;
            }
            if m.phase != hote.phase || m.variante != hote.variante || m.sous_projet != hote.sous_projet {
                continue;
            }
            let Some((y0, y1)) = faces(m) else { continue };
            if (y0 - hi).abs() > 1.0 && (y1 - lo).abs() > 1.0 {
                continue;
            }
            let (c, d) = droite(m).unwrap();
            let (s0, s1) = ((c - a).dot(u), (d - a).dot(u));
            if s0.max(s1).min(x1) - s0.min(s1).max(x0) < 1.0 {
                continue;
            }
            if p.bas_haut(m).is_none_or(|(bas, haut)| haut.min(z + o.hauteur) - bas.max(z) < 1.0) {
                continue;
            }
            membres.push(Membre { id: m.id.clone(), y0, y1 });
            ajout = true;
        }
        if !ajout {
            break;
        }
    }
    let (lo, hi) = membres.iter().fold((f64::MAX, f64::MIN), |(lo, hi), m| (lo.min(m.y0), hi.max(m.y1)));
    let milieu = (lo + hi) / 2.0;
    for m in &mut membres {
        m.y0 -= milieu;
        m.y1 -= milieu;
    }
    Ok(HoteElargi { axe: assemblage::decaler_axe(&axe, milieu), epaisseur: hi - lo, niveau, membres })
}

/// Le mur `m` est-il percé par l'ouverture `o` ?
pub fn perce(p: &Projet, o: &Ouverture, m: &Mur) -> bool {
    if o.mur == m.id || m.assemblage.as_ref() == Some(&o.mur) {
        return true;
    }
    proche(p, o, m) && hote_elargi(p, o).is_ok_and(|h| h.membres.iter().any(|x| x.id == m.id))
}

/// Tri rapide de `perce` : mur droit parallèle à l'hôte de l'ouverture et proche de lui.
fn proche(p: &Projet, o: &Ouverture, m: &Mur) -> bool {
    let (Ok((Axe::Droit { debut, fin }, e, _)), Some((c, d))) = (crate::modele::volumes::solides::hote(p, &o.mur), droite(m)) else { return false };
    let (a, b) = (DVec2::from_array(debut), DVec2::from_array(fin));
    let (Some(u), Some(v)) = ((b - a).try_normalize(), (d - c).try_normalize()) else { return false };
    u.perp_dot(v).abs() <= 1e-3 && (c - a).dot(u.perp()).abs() <= e / 2.0 + 5000.0
}

/// Hôtes élargis de toutes les ouvertures et, par mur, ses ouvertures (rangs dans
/// `p.ouvertures`, croissants) : mêmes réponses que `perce` et `hote_elargi` appelés
/// mur par mur, calculées une seule fois pour tout le projet.
pub struct Percements {
    pub hotes: Vec<Result<HoteElargi, String>>,
    pub par_mur: std::collections::HashMap<String, Vec<usize>>,
}

impl Percements {
    pub fn calculer(p: &Projet) -> Self {
        use rayon::prelude::*;
        use std::collections::HashMap;
        let hotes: Vec<Result<HoteElargi, String>> = p.ouvertures.par_iter().map(|o| hote_elargi(p, o)).collect();
        let par_id: HashMap<&str, &Mur> = p.murs.iter().map(|m| (m.id.as_str(), m)).collect();
        let mut couches: HashMap<&str, Vec<&Mur>> = HashMap::new();
        for m in &p.murs {
            if let Some(a) = &m.assemblage { couches.entry(a.as_str()).or_default().push(m); }
        }
        let mut par_mur: HashMap<String, Vec<usize>> = HashMap::new();
        for (k, o) in p.ouvertures.iter().enumerate() {
            let mut perces: Vec<&str> = Vec::new();
            if let Some(m) = par_id.get(o.mur.as_str()) { perces.push(m.id.as_str()); }
            perces.extend(couches.get(o.mur.as_str()).into_iter().flatten().map(|m| m.id.as_str()));
            if let Ok(h) = &hotes[k] {
                for x in &h.membres {
                    if !perces.contains(&x.id.as_str()) && par_id.get(x.id.as_str()).is_some_and(|m| proche(p, o, m)) { perces.push(x.id.as_str()); }
                }
            }
            for id in perces {
                let liste = par_mur.entry(id.to_string()).or_default();
                if liste.last() != Some(&k) { liste.push(k); }
            }
        }
        Self { hotes, par_mur }
    }
}

fn sans_accent(s: &str) -> String {
    s.to_lowercase().replace(['é', 'è', 'ê', 'ë'], "e").split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Position (repère de la famille, mm) du plan nommé « Retour extérieur » ou
/// « Retour intérieur ».
pub fn plan_retour(f: &FamilleModelee, r: &crate::modele::famille::modelee::Resolution, exterieur: bool) -> Option<f64> {
    let nom = if exterieur { "retour exterieur" } else { "retour interieur" };
    let plan = f.plans.iter().find(|pl| pl.axe == crate::modele::famille::modelee::AxePlan::Y && sans_accent(&pl.nom) == nom)?;
    r.positions.get(&plan.nom).copied()
}

/// Retours d'une ouverture pour le mur `id` : volumes à retirer (les couches qui se
/// retournent prennent la place) et volume ajouté (le retour de la couche elle-même).
pub struct Retours {
    pub retirer: Vec<Vec<Tri>>,
    pub ajouter: Vec<Vec<Tri>>,
}

/// Calcule les retours avec l'hôte élargi déjà calculé, ou le calcule si absent.
pub fn retours_avec(p: &Projet, o: &Ouverture, id: &str, hote: Option<&Result<HoteElargi, String>>) -> Result<Retours, String> {
    let vide = Retours { retirer: Vec::new(), ajouter: Vec::new() };
    let Some(f) = o.type_ouverture.as_ref().and_then(|t| p.types_ouvertures.iter().find(|x| &x.id == t)).and_then(|t| t.modele.as_ref()).filter(|_| o.representation.is_none()) else {
        return Ok(vide);
    };
    let h = match hote { Some(h) => h.clone()?, None => hote_elargi(p, o)? };
    let Some(moi) = h.membres.iter().find(|m| m.id == id).cloned() else { return Ok(vide) };
    let surcharges = p.surcharges(o);
    let r = f.resoudre_occurrence(o.largeur, o.hauteur, h.epaisseur, &surcharges, &o.materiaux);
    // côté famille → côté mur : une ouverture retournée échange extérieur et intérieur
    let signe = if o.retournee { -1.0 } else { 1.0 };
    let limites = [(1.0, plan_retour(f, &r, true).map(|y| y * signe)), (-1.0, plan_retour(f, &r, false).map(|y| y * signe))];
    let limites: Vec<(f64, f64)> = limites
        .into_iter()
        .filter_map(|(cote, l)| l.map(|l| (cote * signe, l)))
        .collect();
    if limites.is_empty() {
        return Ok(vide);
    }
    // couche d'un mur : celle du type de son assemblage (au même rang), sinon celle de
    // son type monocouche
    let couche = |m: &Membre| -> Option<Couche> {
        let mur = p.murs.iter().find(|x| x.id == m.id)?;
        let du_type = mur.assemblage.as_ref().and_then(|a| p.assemblage(a)).and_then(|a| {
            let k = a.couches.iter().position(|c| c == &mur.id)?;
            let t = p.type_mur(&a.type_mur)?;
            (t.couches.len() == a.couches.len()).then(|| t.couches[k].clone())
        });
        du_type.or_else(|| {
            let t = p.type_mur(&mur.type_mur)?;
            (t.couches.len() == 1).then(|| t.couches[0].clone())
        })
    };
    // structure : sépare les couches qui se retournent depuis l'extérieur et l'intérieur
    let structure: Vec<&Membre> = h.membres.iter().filter(|m| couche(m).is_some_and(|c| c.fonction == FonctionCouche::Structure)).collect();
    let (s_lo, s_hi) = if structure.is_empty() {
        (0.0, 0.0)
    } else {
        structure.iter().fold((f64::MAX, f64::MIN), |(a, b), m| (a.min(m.y0), b.max(m.y1)))
    };
    let z = p.niveau(&h.niveau).ok_or("Niveau introuvable.")?.altitude + o.allege;
    let (c, u) = repere(&h.axe, o.abscisse);
    let n = u.perp();
    let boite = |x0: f64, x1: f64, y0: f64, y1: f64, z0: f64, z1: f64| -> Vec<Tri> {
        let contour = [(x0, y0), (x1, y0), (x1, y1), (x0, y1)].map(|(x, y)| (c + u * x + n * y).to_array());
        prisme(&contour, z0, z1)
    };
    let (l, haut) = (o.largeur / 2.0, o.hauteur);
    let mut out = vide;
    for (cote, limite) in limites {
        // couches de ce côté qui se retournent, de la face vers l'intérieur du mur
        let mut couches: Vec<(&Membre, f64)> = h
            .membres
            .iter()
            .filter(|m| couche(m).is_some_and(|c| c.retour && c.fonction != FonctionCouche::Structure))
            .filter(|m| if cote > 0.0 { m.y0 >= s_hi - 1e-6 } else { m.y1 <= s_lo + 1e-6 })
            .map(|m| (m, m.y1 - m.y0))
            .collect();
        couches.sort_by(|a, b| (cote * b.0.y0).total_cmp(&(cote * a.0.y0)));
        let mut decalage = 0.0;
        for (m, t) in couches {
            // face intérieure de la couche (côté structure) et limite du retour
            let face = if cote > 0.0 { m.y0 } else { m.y1 };
            if cote * (face - limite) <= 1.0 {
                decalage += t;
                continue;
            }
            let (ya, yb) = if cote > 0.0 { (limite, face) } else { (face, limite) };
            let e = decalage + t;
            // place prise sur les autres murs entre la couche et la limite
            if moi.id != m.id && moi.y1 > ya + 1e-6 && moi.y0 < yb - 1e-6 {
                out.retirer.push(boite(-l - e, l + e, ya, yb, z, z + haut + e));
            }
            if moi.id == m.id {
                // recouvrement de 0,5 mm dans la couche pour une union franche
                let (ya, yb) = if cote > 0.0 { (ya, yb + 0.5) } else { (ya - 0.5, yb) };
                let gauche = boite(-l - e, -l - decalage, ya, yb, z, z + haut + e);
                let droite = boite(l + decalage, l + e, ya, yb, z, z + haut + e);
                let linteau = boite(-l - e, l + e, ya, yb, z + haut + decalage, z + haut + e);
                out.ajouter.extend([gauche, droite, linteau]);
            }
            decalage += t;
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modele::famille::modelee::{AxePlan, FamilleModelee, PlanRef};

    fn volume(t: &[Tri]) -> f64 {
        t.iter().map(|[a, b, c]| a.dot(b.cross(*c)) / 6.0).sum()
    }

    /// Béton 200 (structure), isolant 100 et enduit 20 à l'extérieur, plaque 13 à
    /// l'intérieur : quatre murs distincts, jointifs. Fenêtre dans le béton.
    fn projet(retour: bool) -> Projet {
        let mut p = Projet::nouveau("Retours");
        p.niveaux.push(Niveau { id: "n".into(), nom: "RDC".into(), altitude: 0.0, origine: None });
        for (id, e, f, y) in [("b", 200.0, FonctionCouche::Structure, 0.0), ("i", 100.0, FonctionCouche::Isolation, 150.0), ("e", 20.0, FonctionCouche::Finition, 210.0), ("p", 13.0, FonctionCouche::Finition, -106.5)] {
            p.types_murs.push(TypeCompose { id: id.into(), nom: id.into(), couches: vec![Couche { epaisseur: e, fonction: f, retour: retour && f != FonctionCouche::Structure, ..Default::default() }], categorie: None, couches_detaillees: true, origine: None });
            p.murs.push(Mur { id: id.into(), nom: id.into(), type_mur: id.into(), niveau: "n".into(), decalage_bas: 0.0, haut: Haut::Libre { hauteur: 3000.0 }, axe: Axe::Droit { debut: [0.0, y], fin: [6000.0, y] }, assemblage: None, phase: None, sous_projet: None, variante: None, origine: None });
        }
        let mut f = FamilleModelee::gabarit_fenetre();
        f.plans.push(PlanRef { nom: "Retour extérieur".into(), axe: AxePlan::Y, position: 20.0, fixe: true, hote: None });
        f.plans.push(PlanRef { nom: "Retour intérieur".into(), axe: AxePlan::Y, position: -120.0, fixe: true, hote: None });
        p.types_ouvertures.push(TypeOuverture { id: "f".into(), nom: "OF".into(), genre: GenreOuverture::Fenetre, largeur: 1200.0, hauteur: 1350.0, famille: None, modele: Some(f), origine: None, famille_nom: None });
        p.ouvertures.push(Ouverture { id: "o".into(), nom: "F".into(), genre: GenreOuverture::Fenetre, mur: "b".into(), type_ouverture: Some("f".into()), abscisse: 3000.0, largeur: 1200.0, hauteur: 1350.0, allege: 900.0, representation: None, valeurs: Default::default(), materiaux: Default::default(), retournee: false, inversee: false, origine: None });
        p.verifier().unwrap();
        p
    }

    fn volumes(p: &Projet) -> BTreeMap<String, f64> {
        let coupes = crate::modele::murs::jonctions::coupes(p);
        p.murs.iter().map(|m| (m.id.clone(), volume(&crate::modele::volumes::solides::mur(p, m, &coupes).unwrap()))).collect()
    }

    #[test]
    fn murs_jointifs_forment_un_seul_hote() {
        let p = projet(false);
        let h = hote_elargi(&p, &p.ouvertures[0]).unwrap();
        assert_eq!(h.membres.len(), 4);
        assert!((h.epaisseur - 333.0).abs() < 1e-6);
        // chaque mur est percé de la baie sur toute son épaisseur
        let v = volumes(&p);
        let baie = 1200.0 * 1350.0;
        for (id, e) in [("b", 200.0), ("i", 100.0), ("e", 20.0), ("p", 13.0)] {
            let plein = 6000.0 * 3000.0 * e;
            assert!((v[id] - (plein - baie * e)).abs() < 1.0, "{id} : {} au lieu de {}", v[id], plein - baie * e);
        }
    }

    #[test]
    fn couches_retournees_dans_tableaux_et_linteau() {
        let (sans, avec) = (volumes(&projet(false)), volumes(&projet(true)));
        // faces du groupe depuis son milieu (53,5) : béton −153,5…46,5, isolant
        // 46,5…146,5, enduit 146,5…166,5, plaque −166,5…−153,5 ; limites 20 et −120
        let anneau = |e: f64| (1200.0 + 2.0 * e) * (1350.0 + e) - 1200.0 * 1350.0;
        // enduit : retour de 20 sur 126,5 de profondeur (de sa face à la limite)
        assert!((avec["e"] - sans["e"] - anneau(20.0) * 126.5).abs() < 2.0, "{} {}", avec["e"], sans["e"]);
        // isolant : place de l'enduit retirée sur son épaisseur, puis son retour de
        // 100 (décalé de 20) sur 26,5
        let isolant = -(anneau(20.0) * 100.0) + (anneau(120.0) - anneau(20.0)) * 26.5;
        assert!((avec["i"] - sans["i"] - isolant).abs() < 2.0, "{} {}", avec["i"] - sans["i"], isolant);
        // béton : place des retours retirée (120 sur 26,5 côté extérieur, 13 sur 33,5)
        let beton = -(anneau(120.0) * 26.5 + anneau(13.0) * 33.5);
        assert!((avec["b"] - sans["b"] - beton).abs() < 2.0, "{} {}", avec["b"] - sans["b"], beton);
        // plaque : retour de 13 sur 33,5
        assert!((avec["p"] - sans["p"] - anneau(13.0) * 33.5).abs() < 2.0);
    }

    #[test]
    fn gabarit_charge_l_of_et_retourne_le_mur_double() {
        let g = crate::config::gabarit::Gabarit::defaut();
        let livrees = crate::config::gabarit::familles_livrees();
        assert_eq!(g.types_ouvertures.len(), livrees.len());
        for (a, b) in g.types_ouvertures.iter().zip(&livrees) {
            assert_eq!((&a.nom, a.largeur, a.hauteur, &a.famille), (&b.nom, b.largeur, b.hauteur, &b.famille), "gabarit à régénérer : cargo test --bin bimedit -- --ignored ecrire_familles_du_gabarit");
        }
        let mut p = Projet::depuis_gabarit("OF", &g);
        let t = p.types_ouvertures.iter().find(|t| t.nom == "Fenêtre OF 2 vantaux").unwrap().clone();
        let tm = p.types_murs.iter().find(|t| t.nom.starts_with("Mur BA 200 + doublage")).unwrap().id.clone();
        let mur = p.ajouter_mur([0.0, 0.0], [5000.0, 0.0], &crate::modele::edition::ParametresMur { type_mur: Some(tm), ..Default::default() }).unwrap();
        let sans = volumes(&p);
        p.ouvertures.push(Ouverture { id: "o".into(), nom: "F".into(), genre: GenreOuverture::Fenetre, mur: mur.clone(), type_ouverture: Some(t.id.clone()), abscisse: 2500.0, largeur: t.largeur, hauteur: t.hauteur, allege: 900.0, representation: None, valeurs: Default::default(), materiaux: Default::default(), retournee: false, inversee: false, origine: None });
        p.verifier().unwrap();
        let avec = volumes(&p);
        let couches = p.assemblage(&mur).unwrap().couches.clone();
        let baie = t.largeur * t.hauteur;
        // béton : percé de la baie et creusé par les retours ; enduit : retourné jusqu'au dormant
        let (enduit, beton) = (&couches[0], &couches[1]);
        assert!(sans[beton] - avec[beton] > baie * 200.0 + 1.0, "béton sans place pour les retours");
        assert!(avec[enduit] > sans[enduit] - baie * 15.0 + 1.0, "enduit non retourné");
    }
}
