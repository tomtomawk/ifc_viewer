//! Documentation native : références, annotations par vue, feuilles et nomenclatures.
use super::*;
use crate::trace2d::{Dessin, Forme, Primitive, Role, Style};
use glam::{DVec2, DVec3};
use std::collections::BTreeMap;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Quadrillage {
    pub id: String,
    pub nom: String,
    pub debut: [f64; 2],
    pub fin: [f64; 2],
}
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "ancrage", rename_all = "snake_case")]
pub enum PointCote {
    Fixe { point: [f64; 2] },
    Mur { id: String, fin: bool },
}
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "forme", rename_all = "snake_case")]
pub enum AnnotationForme {
    Texte {
        point: [f64; 2],
        texte: String,
    },
    Ligne {
        debut: [f64; 2],
        fin: [f64; 2],
    },
    Zone {
        contour: Vec<[f64; 2]>,
    },
    Cote {
        debut: PointCote,
        fin: PointCote,
        decalage: f64,
    },
    Etiquette {
        element: String,
        point: [f64; 2],
    },
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Annotation {
    pub id: String,
    pub vue: String,
    pub hauteur: f64,
    #[serde(flatten)]
    pub forme: AnnotationForme,
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct VuePlacee {
    pub vue: String,
    pub position: [f64; 2],
    pub echelle: f64,
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Feuille {
    pub id: String,
    pub numero: String,
    pub nom: String,
    pub largeur: f64,
    pub hauteur: f64,
    pub cartouche: String,
    pub vues: Vec<VuePlacee>,
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Nomenclature {
    pub id: String,
    pub nom: String,
    pub categorie: String,
    pub regrouper: bool,
}

pub fn repere(p: &Projet, v: &Vue) -> (DVec3, DVec3, DVec3) {
    match &v.genre {
        GenreVue::Plan { niveau, coupe } => (
            DVec3::new(
                0.0,
                0.0,
                p.niveau(niveau).map_or(0.0, |n| n.altitude) + coupe,
            ),
            DVec3::X,
            DVec3::Y,
        ),
        GenreVue::Coupe { debut, fin, .. } => {
            let a = DVec2::from_array(*debut);
            let u = (DVec2::from_array(*fin) - a).normalize_or_zero();
            (a.extend(0.0), u.extend(0.0), DVec3::Z)
        }
    }
}
fn texte(s: &str, pos: DVec3, u: DVec3, v: DVec3, hauteur: f64, couleur: [u8; 4]) -> Primitive {
    let t = crate::texte::ligne(s, "Arial", hauteur, false);
    Primitive {
        forme: Forme::Triangles(
            t.triangles
                .into_iter()
                .map(|t| t.map(|p| pos + u * p[0] + v * p[1]))
                .collect(),
        ),
        style: Style {
            couleur,
            epaisseur: 0.0,
        },
        role: Role::Annotation,
        element: None,
    }
}
fn ligne(a: DVec3, b: DVec3) -> Primitive {
    Primitive {
        forme: Forme::Segments(vec![[a, b]]),
        style: Style {
            couleur: [30, 30, 30, 255],
            epaisseur: 0.18,
        },
        role: Role::Annotation,
        element: None,
    }
}
fn point_cote(p: &Projet, v: &Vue, point: &PointCote) -> Option<DVec2> {
    match point {
        PointCote::Fixe { point } => Some(DVec2::from_array(*point)),
        PointCote::Mur { id, fin } => {
            let (a, b, z) = p.axe_element(id)?;
            let pt = DVec2::from_array(if *fin { b } else { a }).extend(z);
            let (o, u, h) = repere(p, v);
            let d = pt - o;
            Some(DVec2::new(d.dot(u), d.dot(h)))
        }
    }
}
pub fn annotations(p: &Projet, v: &Vue) -> Vec<Primitive> {
    let (o, u, h) = repere(p, v);
    let mut out = Vec::new();
    let pt = |p: DVec2| (o + u * p.x + h * p.y) * 0.001;
    for a in p.annotations.iter().filter(|a| a.vue == v.id) {
        let taille = a.hauteur * v.echelle * 0.001;
        match &a.forme {
            AnnotationForme::Texte { point, texte: s } => out.push(texte(
                s,
                pt(DVec2::from_array(*point)),
                u,
                h,
                taille,
                [30, 30, 30, 255],
            )),
            AnnotationForme::Ligne { debut, fin } => out.push(ligne(
                pt(DVec2::from_array(*debut)),
                pt(DVec2::from_array(*fin)),
            )),
            AnnotationForme::Zone { contour } => out.push(Primitive {
                forme: Forme::Surface(vec![
                    contour.iter().map(|p| pt(DVec2::from_array(*p))).collect(),
                ]),
                style: Style {
                    couleur: [215, 215, 210, 255],
                    epaisseur: 0.0,
                },
                role: Role::Annotation,
                element: None,
            }),
            AnnotationForme::Cote {
                debut,
                fin,
                decalage,
            } => match (point_cote(p, v, debut), point_cote(p, v, fin)) {
                (Some(a), Some(b)) => {
                    let n = (b - a).normalize_or_zero().perp() * decalage;
                    out.extend([
                        ligne(pt(a), pt(a + n)),
                        ligne(pt(b), pt(b + n)),
                        ligne(pt(a + n), pt(b + n)),
                    ]);
                    out.push(texte(
                        &format!("{:.0}", (b - a).length()),
                        pt((a + b) / 2.0 + n),
                        u,
                        h,
                        taille,
                        [30, 30, 30, 255],
                    ));
                }
                _ => out.push(texte(
                    "Cote : référence perdue",
                    o * 0.001,
                    u,
                    h,
                    taille,
                    [190, 35, 35, 255],
                )),
            },
            AnnotationForme::Etiquette { element, point } => {
                let nom = p
                    .murs
                    .iter()
                    .find(|m| &m.id == element)
                    .map(|m| m.nom.as_str())
                    .or_else(|| {
                        p.dalles
                            .iter()
                            .find(|m| &m.id == element)
                            .map(|m| m.nom.as_str())
                    })
                    .or_else(|| {
                        p.importes
                            .iter()
                            .find(|m| &m.id == element)
                            .map(|m| m.nom.as_str())
                    });
                out.push(texte(
                    nom.unwrap_or("Référence perdue"),
                    pt(DVec2::from_array(*point)),
                    u,
                    h,
                    taille,
                    if nom.is_some() {
                        [30, 30, 30, 255]
                    } else {
                        [190, 35, 35, 255]
                    },
                ));
            }
        }
    }
    if matches!(v.genre, GenreVue::Plan { .. }) {
        for g in &p.quadrillages {
            let (a, b) = (pt(DVec2::from_array(g.debut)), pt(DVec2::from_array(g.fin)));
            out.push(ligne(a, b));
            out.push(texte(
                &g.nom,
                b,
                u,
                h,
                2.5 * v.echelle * 0.001,
                [70, 70, 70, 255],
            ));
        }
    }
    out
}

pub fn dessin_vue(p: &Projet, id: &str) -> Result<Dessin, String> {
    let v = p.vues.iter().find(|v| v.id == id).ok_or("Vue absente.")?;
    let s = crate::scene::depuis_natif(p, std::path::Path::new("projet.modele.json"))?;
    let (o, u, h) = repere(p, v);
    let oeil = u.cross(h);
    let normale = -oeil;
    let plan = normale.extend(-normale.dot(o * 0.001 - s.origin)).as_vec4();
    let elevation = matches!(
        v.genre,
        GenreVue::Coupe {
            elevation: true,
            ..
        }
    );
    let mut visible = vec![true; s.objects.len()];
    if let GenreVue::Coupe {
        debut,
        fin,
        profondeur,
        ..
    } = &v.genre
    {
        let a = DVec2::from_array(*debut);
        let d = DVec2::from_array(*fin) - a;
        let u = d.normalize();
        let n = u.perp();
        for (i, obj) in s.objects.iter().enumerate() {
            let corners = (0..8).map(|k| {
                DVec3::new(
                    if k & 1 == 0 { obj.min.x } else { obj.max.x } as f64,
                    if k & 2 == 0 { obj.min.y } else { obj.max.y } as f64,
                    if k & 4 == 0 { obj.min.z } else { obj.max.z } as f64,
                )
            });
            let points: Vec<_> = corners
                .map(|q| (q + s.origin).truncate() * 1000.0 - a)
                .collect();
            visible[i] = points.iter().any(|q| q.dot(u) >= 0.0)
                && points.iter().any(|q| q.dot(u) <= d.length())
                && points.iter().any(|q| q.dot(n) >= 0.0)
                && points.iter().any(|q| q.dot(n) <= *profondeur);
        }
    }
    let mut primitives = if elevation {
        Vec::new()
    } else {
        crate::dessin::coupe(&s, &visible, plan, v.echelle, crate::dessin::Poche::Hachure)
    };
    primitives.extend(crate::dessin::vu(
        &s,
        &visible,
        u,
        h,
        oeil,
        Some(plan),
        4096,
    ));
    // Revenir au repère absolu, comme les annotations.
    let mut primitives: Vec<_> = primitives
        .into_iter()
        .map(|pr| pr.translater(s.origin))
        .collect();
    primitives.extend(annotations(p, v));
    Ok(Dessin {
        droite: u,
        haut: h,
        echelle: v.echelle,
        primitives,
    })
}

pub fn feuille(p: &Projet, f: &Feuille) -> Result<Dessin, String> {
    let pt = |x, y| DVec3::new(x / 1000.0, y / 1000.0, 0.0);
    let mut primitives = vec![Primitive {
        forme: Forme::Polyligne {
            points: vec![
                pt(0.0, 0.0),
                pt(f.largeur, 0.0),
                pt(f.largeur, f.hauteur),
                pt(0.0, f.hauteur),
            ],
            fermee: true,
        },
        style: Style {
            couleur: [20, 20, 20, 255],
            epaisseur: 0.18,
        },
        role: Role::Annotation,
        element: None,
    }];
    primitives.push(texte(
        &format!("{} — {}", f.numero, f.nom),
        pt(10.0, 10.0),
        DVec3::X,
        DVec3::Y,
        0.003,
        [20, 20, 20, 255],
    ));
    primitives.push(texte(
        &f.cartouche,
        pt(10.0, 5.0),
        DVec3::X,
        DVec3::Y,
        0.002,
        [20, 20, 20, 255],
    ));
    for v in &f.vues {
        let mut projet = p.clone();
        if let Some(vue) = projet.vues.iter_mut().find(|x| x.id == v.vue) {
            vue.echelle = v.echelle;
        }
        let dessin = dessin_vue(&projet, &v.vue)?;
        let vue = p.vues.iter().find(|x| x.id == v.vue).unwrap();
        let (origine, _, _) = repere(p, vue);
        let proj = |q: DVec3| {
            let q = q - origine * 0.001;
            pt(
                v.position[0] + q.dot(dessin.droite) * 1000.0 / v.echelle,
                v.position[1] + q.dot(dessin.haut) * 1000.0 / v.echelle,
            )
        };
        let mut bas_titre = v.position[1];
        for mut pr in dessin.primitives {
            pr.forme = match pr.forme {
                Forme::Polyligne { points, fermee } => Forme::Polyligne {
                    points: points.into_iter().map(proj).collect(),
                    fermee,
                },
                Forme::Segments(s) => Forme::Segments(s.into_iter().map(|s| s.map(proj)).collect()),
                Forme::Triangles(t) => {
                    Forme::Triangles(t.into_iter().map(|t| t.map(proj)).collect())
                }
                Forme::Surface(c) => Forme::Surface(
                    c.into_iter()
                        .map(|c| c.into_iter().map(proj).collect())
                        .collect(),
                ),
            };
            let points: Box<dyn Iterator<Item = &DVec3> + '_> = match &pr.forme {
                Forme::Polyligne { points, .. } => Box::new(points.iter()),
                Forme::Segments(s) => Box::new(s.iter().flatten()),
                Forme::Triangles(t) => Box::new(t.iter().flatten()),
                Forme::Surface(c) => Box::new(c.iter().flatten()),
            };
            for p in points {
                bas_titre = bas_titre.min(p.y * 1000.0);
            }
            primitives.push(pr);
        }
        primitives.push(texte(
            &format!("{} · 1:{}", vue.nom, v.echelle),
            pt(v.position[0], bas_titre - 7.0),
            DVec3::X,
            DVec3::Y,
            0.0025,
            [20, 20, 20, 255],
        ));
    }
    Ok(Dessin {
        droite: DVec3::X,
        haut: DVec3::Y,
        echelle: 1.0,
        primitives,
    })
}

/// Quantités nettes : murs percés, dalles avec trémies. Pas de double compte des assemblages.
pub fn tableau(p: &Projet, n: &Nomenclature) -> Result<Vec<Vec<String>>, String> {
    let mut groupes: BTreeMap<(String, String), (usize, f64)> = BTreeMap::new();
    let jonctions = jonctions::coupes(p);
    let mut ajouter = |cat: &str, nom: String, v: f64| {
        if !n.categorie.is_empty() && !cat.to_lowercase().contains(&n.categorie.to_lowercase()) {
            return;
        }
        let e = groupes.entry((cat.into(), nom)).or_default();
        e.0 += 1;
        e.1 += v;
    };
    for m in &p.murs {
        ajouter(
            "Murs",
            if n.regrouper {
                p.type_mur(&m.type_mur).unwrap().nom.clone()
            } else {
                format!("{} ({})", m.nom, m.id)
            },
            geometrie::volume(&solides::mur(p, m, &jonctions)?) / 1e9,
        );
    }
    for d in &p.dalles {
        ajouter(
            d.sorte.nom(),
            if n.regrouper {
                p.type_dalle(&d.type_dalle).unwrap().nom.clone()
            } else {
                format!("{} ({})", d.nom, d.id)
            },
            p.volume_dalle(d).unwrap_or(0.0) / 1e9,
        );
    }
    for o in &p.ouvertures {
        ajouter(
            match o.genre {
                GenreOuverture::Porte => "Portes",
                GenreOuverture::Fenetre => "Fenêtres",
                GenreOuverture::Baie => "Baies",
            },
            o.nom.clone(),
            0.0,
        );
    }
    for e in &p.importes {
        ajouter(
            &e.categorie,
            if n.regrouper {
                e.nom.clone()
            } else {
                format!("{} ({})", e.nom, e.id)
            },
            geometrie::volume(&solides::maillage(&e.maillage)?) / 1e9,
        );
    }
    let mut lignes = vec![vec![
        "Catégorie".into(),
        "Type / élément".into(),
        "Nombre".into(),
        "Volume net (m³)".into(),
    ]];
    for ((cat, nom), (nb, v)) in groupes {
        lignes.push(vec![cat, nom, nb.to_string(), format!("{v:.6}")]);
    }
    Ok(lignes)
}
pub fn csv(lignes: &[Vec<String>]) -> String {
    lignes
        .iter()
        .map(|l| {
            l.iter()
                .map(|s| format!("\"{}\"", s.replace('"', "\"\"")))
                .collect::<Vec<_>>()
                .join(";")
        })
        .collect::<Vec<_>>()
        .join("\r\n")
        + "\r\n"
}

/// Feuille au format papier exact, sans la marge automatique des exports de vues.
pub fn svg_feuille(p: &Projet, f: &Feuille) -> Result<String, String> {
    let mut svg = feuille(p, f)?.svg(&format!("{} — {}", f.numero, f.nom));
    let a = svg.find("<svg ").ok_or("SVG absent")?;
    let b = a + svg[a..].find('>').ok_or("SVG invalide")? + 1;
    svg.replace_range(a..b,&format!("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}mm\" height=\"{}mm\" viewBox=\"0 -{} {} {}\">",f.largeur,f.hauteur,f.hauteur,f.largeur,f.hauteur));
    Ok(svg)
}
pub fn pdf(svg: &str) -> Result<Vec<u8>, String> {
    let tree = svg2pdf::usvg::Tree::from_str(svg, &svg2pdf::usvg::Options::default())
        .map_err(|e| format!("SVG : {e}"))?;
    // usvg interprète les mm avec 96 px/pouce ; conserver la taille physique du papier.
    svg2pdf::to_pdf(
        &tree,
        svg2pdf::ConversionOptions::default(),
        svg2pdf::PageOptions { dpi: 96.0 },
    )
    .map_err(|e| format!("PDF : {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn documents_compatibles_v0_references_et_pdf() {
        let (mut p, _) =
            depuis_ifc::convertir(std::path::Path::new("samples/test_ifc4_murs.ifc"), &|_| {})
                .unwrap();
        let vue = p
            .ajouter_vue_plan(&p.niveaux[0].id.clone(), 1200.0)
            .unwrap();
        p.annotations.push(Annotation {
            id: "annotation-test".into(),
            vue: vue.clone(),
            hauteur: 2.5,
            forme: AnnotationForme::Cote {
                debut: PointCote::Mur {
                    id: "supprime".into(),
                    fin: false,
                },
                fin: PointCote::Fixe {
                    point: [1000.0, 0.0],
                },
                decalage: 500.0,
            },
        });
        let f = Feuille {
            id: "f".into(),
            numero: "A01".into(),
            nom: "Plan".into(),
            largeur: 420.0,
            hauteur: 297.0,
            cartouche: "Agence & associés".into(),
            vues: vec![VuePlacee {
                vue,
                position: [30.0, 80.0],
                echelle: 100.0,
            }],
        };
        p.feuilles.push(f.clone());
        p.verifier().unwrap();
        let relu = Projet::lire_json(&p.json().unwrap()).unwrap();
        assert_eq!(relu.feuilles.len(), 1);
        let svg = svg_feuille(&p, &f).unwrap();
        assert!(svg.contains("width=\"420mm\""));
        assert!(svg.contains("height=\"297mm\""));
        let pdf = pdf(&svg).unwrap();
        assert!(pdf.starts_with(b"%PDF-"));
        p.supprimer_vue(&f.vues[0].vue).unwrap();
        assert!(p.annotations.is_empty());
        assert!(p.feuilles[0].vues.is_empty());
    }
    #[test]
    fn nomenclature_csv_et_quantites_sans_double_compte() {
        let (p, _) =
            depuis_ifc::convertir(std::path::Path::new("samples/test_ifc4_murs.ifc"), &|_| {})
                .unwrap();
        let n = Nomenclature {
            id: "n".into(),
            nom: "Murs".into(),
            categorie: "Murs".into(),
            regrouper: true,
        };
        let t = tableau(&p, &n).unwrap();
        let count: usize = t
            .iter()
            .skip(1)
            .map(|r| r[2].parse::<usize>().unwrap())
            .sum();
        assert_eq!(count, p.murs.len());
        assert_eq!(
            csv(&[vec!["A;B".into(), "Il dit \"oui\"".into()]]),
            "\"A;B\";\"Il dit \"\"oui\"\"\"\r\n"
        );
    }
}
