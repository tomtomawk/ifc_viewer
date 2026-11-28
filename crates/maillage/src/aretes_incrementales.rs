//! Arêtes jointes par matériau, gardées d'une construction de la scène à la suivante :
//! seules les droites qui portent un côté de triangle d'un élément modifié (ajouté,
//! retiré ou changé de géométrie) sont recalculées. Les côtés d'une droite sont rangés
//! dans un ordre fixe (élément, coordonnées) avant le calcul : le résultat ne dépend pas
//! de l'ordre des modifications successives.
use formats::ifc::geom::ProductGeom;
use glam::DVec3;
use noyau::aretes::{balayer, cotes, AreteDouce, Aretes, CleDroite, Cote};
use noyau::Id;
use rayon::prelude::*;
use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, OnceLock};

#[derive(Default)]
struct Droite {
    cotes: Vec<Cote<Id>>,
    vives: Vec<([DVec3; 2], Id)>,
    douces: Vec<(AreteDouce, Id)>,
    /// À recalculer (un de ses côtés a changé).
    sale: bool,
}

type Table<K, V> = HashMap<K, V, std::hash::BuildHasherDefault<Hachage>>;

/// Hachage rapide des clés entières (droites quantifiées, identifiants).
#[derive(Default)]
struct Hachage(u64);

impl std::hash::Hasher for Hachage {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, octets: &[u8]) {
        for &o in octets {
            self.write_u64(o as u64);
        }
    }
    fn write_u64(&mut self, x: u64) {
        self.0 = (self.0.rotate_left(5) ^ x).wrapping_mul(0x517C_C1B7_2722_0A95);
    }
    fn write_i64(&mut self, x: i64) {
        self.write_u64(x as u64);
    }
    fn write_u32(&mut self, x: u32) {
        self.write_u64(x as u64);
    }
    fn write_usize(&mut self, x: usize) {
        self.write_u64(x as u64);
    }
}

/// Groupe de matériau : ses éléments (empreinte de leurs triangles, droites touchées),
/// ses droites, et ses arêtes par élément (reprises si aucune droite n'a changé).
#[derive(Default)]
struct Groupe {
    objets: Table<Id, (u64, Vec<CleDroite>)>,
    droites: Table<CleDroite, Droite>,
    sortie: Vec<(Id, Aretes)>,
}

#[derive(Default)]
struct Etat {
    /// Noms des groupes (matériau, ou partie sans matériau d'un élément) et leur rang.
    rangs: HashMap<String, u32>,
    groupes: Table<u32, Groupe>,
}

fn etat() -> &'static Mutex<Etat> {
    static ETAT: OnceLock<Mutex<Etat>> = OnceLock::new();
    ETAT.get_or_init(Default::default)
}

/// Empreinte des coordonnées exactes des triangles, dans l'ordre.
fn empreinte(tris: &[[DVec3; 3]]) -> u64 {
    let mut h = 0x9E37_79B9_7F4A_7C15u64 ^ tris.len() as u64;
    for v in tris.iter().flatten() {
        for x in v.to_array() {
            h = (h.rotate_left(5) ^ x.to_bits()).wrapping_mul(0x517C_C1B7_2722_0A95);
        }
    }
    h
}

/// Ordre fixe des côtés d'une droite : élément, puis extrémités, normale et sens.
fn ordre(a: &Cote<Id>, b: &Cote<Id>) -> std::cmp::Ordering {
    let cle = |c: &Cote<Id>| [c.a.x, c.a.y, c.a.z, c.b.x, c.b.y, c.b.z, c.n.x, c.n.y, c.n.z];
    a.proprietaire.cmp(&b.proprietaire).then_with(|| {
        let (x, y) = (cle(a), cle(b));
        (0..9).map(|i| x[i].total_cmp(&y[i])).find(|o| o.is_ne()).unwrap_or(std::cmp::Ordering::Equal)
    }).then(a.sens.cmp(&b.sens))
}

/// Côtés d'un élément rangés par droite, dans l'ordre de ses triangles.
fn cotes_objet(id: Id, tris: &[[DVec3; 3]]) -> Table<CleDroite, Vec<Cote<Id>>> {
    let mut t: Table<CleDroite, Vec<Cote<Id>>> = Table::default();
    for tri in tris {
        cotes(tri, id, |cle, c| t.entry(cle).or_default().push(c));
    }
    t
}

impl Groupe {
    /// Met le groupe à jour d'après les triangles de ses éléments ; vrai si une droite a
    /// été recalculée.
    fn actualiser(&mut self, entrees: &Table<Id, Vec<[DVec3; 3]>>) -> bool {
        let empreintes: Table<Id, u64> = entrees.iter().map(|(id, t)| (*id, empreinte(t))).collect();
        let changes: Vec<Id> = entrees.keys().filter(|id| self.objets.get(*id).map(|o| o.0) != empreintes.get(*id).copied()).copied().collect();
        let retires: Vec<Id> = self.objets.keys().filter(|id| !entrees.contains_key(*id)).copied().collect();
        if changes.is_empty() && retires.is_empty() {
            return false;
        }
        if changes.len() * 2 > entrees.len() {
            // plus de la moitié des éléments changés : groupe refait d'un bloc
            self.objets.clear();
            self.droites.clear();
            let mut ids: Vec<&Id> = entrees.keys().collect();
            ids.sort_unstable();
            for id in ids {
                let mut l = Vec::new();
                for t in &entrees[id] {
                    cotes(t, *id, |cle, c| {
                        self.droites.entry(cle).or_default().cotes.push(c);
                        l.push(cle);
                    });
                }
                l.sort_unstable();
                l.dedup();
                self.objets.insert(*id, (empreintes[id], l));
            }
            self.droites.values_mut().for_each(|d| d.sale = true);
        } else {
            for id in retires {
                for cle in self.objets.remove(&id).map(|o| o.1).unwrap_or_default() {
                    if let Some(d) = self.droites.get_mut(&cle) {
                        d.cotes.retain(|c| c.proprietaire != id);
                        d.sale = true;
                    }
                }
            }
            // éléments changés : comparés droite par droite à ce qu'ils avaient (une
            // découpe voisine ne change que quelques droites)
            for id in changes {
                let nouveaux = cotes_objet(id, &entrees[&id]);
                let anciennes = self.objets.remove(&id).map(|o| o.1).unwrap_or_default();
                for cle in anciennes.iter().filter(|c| !nouveaux.contains_key(*c)) {
                    if let Some(d) = self.droites.get_mut(cle) {
                        d.cotes.retain(|c| c.proprietaire != id);
                        d.sale = true;
                    }
                }
                let mut lignes = Vec::with_capacity(nouveaux.len());
                for (cle, mut cotes) in nouveaux {
                    lignes.push(cle);
                    // même ordre que les côtés rangés de la droite (élément d'abord : contigus)
                    cotes.sort_unstable_by(ordre);
                    let d = self.droites.entry(cle).or_default();
                    let mut presents = d.cotes.iter().filter(|c| c.proprietaire == id);
                    let identiques = cotes.iter().all(|c| presents.next() == Some(c)) && presents.next().is_none();
                    if !identiques {
                        d.cotes.retain(|c| c.proprietaire != id);
                        d.cotes.extend(cotes);
                        d.sale = true;
                    }
                }
                self.objets.insert(id, (empreintes[&id], lignes));
            }
        }
        let mut refaites = false;
        for d in self.droites.values_mut().filter(|d| d.sale) {
            d.cotes.sort_unstable_by(ordre);
            (d.vives, d.douces) = balayer(&d.cotes, true);
            d.sale = false;
            refaites = true;
        }
        self.droites.retain(|_, d| !d.cotes.is_empty());
        refaites
    }

    /// Arêtes de chaque élément du groupe.
    fn repartir(&mut self) {
        let mut par_objet: Table<Id, Aretes> = Table::default();
        for d in self.droites.values() {
            for (s, id) in &d.vives {
                par_objet.entry(*id).or_default().vives.push(*s);
            }
            for (s, id) in &d.douces {
                par_objet.entry(*id).or_default().douces.push(*s);
            }
        }
        self.sortie = par_objet.into_iter().collect();
    }
}

/// Arêtes des éléments calculées par matériau (voir `noyau::aretes::aretes_jointes`), en
/// reprenant les groupes et les droites que la modification ne touche pas.
pub(crate) fn aretes_jointes(geoms: &[ProductGeom], materiaux: &[Vec<Option<String>>], exclus: &HashSet<Id>) -> HashMap<Id, Aretes> {
    let mut guard = etat().lock().unwrap_or_else(|e| e.into_inner());
    let e = &mut *guard;
    // triangles de chaque élément, par groupe
    let mut entrees: Table<u32, Table<Id, Vec<[DVec3; 3]>>> = Table::default();
    for (g, noms) in geoms.iter().zip(materiaux).filter(|(g, _)| !exclus.contains(&g.id)) {
        // parties du style Réaliste seulement (géométrie de rendu) : sans arête
        for (i, p) in g.parts.iter().enumerate().filter(|(i, _)| !formats::ifc::assemblage::rendu_seul(g, *i)) {
            let nom = noms.get(i).cloned().flatten().unwrap_or_else(|| format!("\u{0}{}:{i}", g.id));
            let n = e.rangs.len() as u32;
            let rang = *e.rangs.entry(nom).or_insert(n);
            entrees.entry(rang).or_default().entry(g.id).or_default().extend(p.tris.iter().copied());
        }
    }
    // groupes disparus : oubliés ; les autres mis à jour en parallèle
    e.groupes.retain(|g, _| entrees.contains_key(g));
    for g in entrees.keys() {
        e.groupes.entry(*g).or_default();
    }
    e.groupes.par_iter_mut().for_each(|(g, groupe)| {
        if groupe.actualiser(&entrees[g]) || groupe.sortie.is_empty() {
            groupe.repartir();
        }
    });
    // arêtes rendues à leur élément
    let mut par_objet: HashMap<Id, Aretes> = geoms.iter().map(|g| (g.id, Aretes::default())).collect();
    for groupe in e.groupes.values() {
        for (id, a) in &groupe.sortie {
            if let Some(x) = par_objet.get_mut(id) {
                x.vives.extend_from_slice(&a.vives);
                x.douces.extend_from_slice(&a.douces);
            }
        }
    }
    par_objet.par_iter_mut().for_each(|(_, a)| a.ordonner());
    par_objet
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Après des modifications successives, le résultat est celui d'un calcul complet
    /// sur l'état final (mémoire vidée).
    #[test]
    fn incremental_egal_au_calcul_complet() {
        let p = modele::Projet::lire_json(include_str!("../../../samples/maison_native.bem")).unwrap();
        let scene = |p: &modele::Projet| crate::depuis_natif(p, std::path::Path::new("essai")).unwrap();
        let aretes = |s: &scene::Scene| {
            let mut v: Vec<(u64, Vec<[i64; 3]>)> = s.objects.iter().map(|o| {
                let r = o.aretes;
                let mut x: Vec<[i64; 3]> = s.vertices[r.start as usize..(r.start + r.count) as usize].iter().map(|v| v.pos.map(|c| (c as f64 * 1e4).round() as i64)).collect();
                x.sort();
                (o.id, x)
            }).collect();
            v.sort();
            v
        };
        let _ = scene(&p);
        let mut q = p.clone();
        for (k, m) in q.murs.iter_mut().enumerate().take(3) {
            if let modele::Axe::Droit { debut, fin } = &mut m.axe {
                debut[1] += 100.0 * (k + 1) as f64;
                fin[1] += 100.0 * (k + 1) as f64;
            }
        }
        let _ = scene(&q);
        let incrementale = aretes(&scene(&p));
        *etat().lock().unwrap() = Etat::default();
        let complete = aretes(&scene(&p));
        assert_eq!(incrementale, complete);
    }
}
#[cfg(test)]
mod mesure {
    /// Mesure et contrôle sur les grands échantillons (ignoré par défaut) :
    /// `cargo test -p maillage --release --lib mesure -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn grands_echantillons() {
        for f in ["ilot_demo/ilot_demo.bem", "immeuble_essai_murs/immeuble_essai_murs.bem", "maison_exemple.bem"] {
            let p = modele::Projet::lire_json(&std::fs::read_to_string(format!("{}/../../samples/{f}", env!("CARGO_MANIFEST_DIR"))).unwrap()).unwrap();
            let scene = |p: &modele::Projet| crate::depuis_natif(p, std::path::Path::new("essai")).unwrap();
            let aretes = |s: &scene::Scene| { let mut v: Vec<(u64, Vec<[i64; 3]>)> = s.objects.iter().map(|o| { let r = o.aretes; let mut x: Vec<[i64; 3]> = s.vertices[r.start as usize..(r.start + r.count) as usize].iter().map(|v| v.pos.map(|c| (c as f64 * 1e4).round() as i64)).collect(); x.sort(); (o.id, x) }).collect(); v.sort(); v };
            let mut q = p.clone();
            let _ = scene(&q);
            for k in 0..3 {
                let m = q.murs[k * 7 % q.murs.len()].clone();
                for x in q.murs.iter_mut().filter(|x| x.id == m.id || (m.assemblage.is_some() && x.assemblage == m.assemblage)) {
                    if let modele::Axe::Droit { debut, fin } = &mut x.axe { debut[1] += 50.0; fin[1] += 50.0; }
                }
                let t = web_time::Instant::now();
                let _ = scene(&q);
                eprintln!("{f} déplacement {k} : scène {:.0} ms", t.elapsed().as_secs_f64() * 1e3);
            }
            let inc = aretes(&scene(&q));
            *super::etat().lock().unwrap() = super::Etat::default();
            let complet = aretes(&scene(&q));
            eprintln!("{f} : {} objets, identiques {}", inc.len(), inc == complet);
            assert!(inc == complet);
        }
    }
}
