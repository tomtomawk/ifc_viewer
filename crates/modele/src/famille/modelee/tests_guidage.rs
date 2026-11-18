use super::*;
use super::super::tests_maillage::{verifier_fermeture, volume};
use glam::DVec3;

fn profil() -> Vec<[f64;2]> { vec![[-100.0,-50.0],[100.0,-50.0],[100.0,50.0],[-100.0,50.0]] }
fn carre(c: f64) -> Vec<[f64;2]> { vec![[-c,-c],[c,-c],[c,c],[-c,c]] }
fn relu(g: &FormeGuidee) -> FormeGuidee { serde_json::from_str(&serde_json::to_string(g).unwrap()).unwrap() }

#[test]
fn guidages_droits_volumes_et_orientation() {
    for inverse in [false,true] {
        let mut p=profil();
        if inverse { p.reverse(); }
        let fin: Vec<_>=p.iter().map(|q| [q[0]/2.0,q[1]/2.0]).collect();
        let chemin=vec![[0.0;3],[0.0,0.0,1000.0]];
        for (g,attendu) in [
            (FormeGuidee::extrusion(chemin.clone()),20_000_000.0),
            (FormeGuidee::raccordement(fin.clone(),1000.0),35_000_000.0/3.0),
            (FormeGuidee::raccordement_chemin(fin,chemin),35_000_000.0/3.0),
        ] {
            g.valider(&p).unwrap();
            let tris=g.triangles(&p);
            verifier_fermeture(&tris);
            assert!((volume(&tris)-attendu).abs()/attendu<1e-8);
            assert_eq!(g,relu(&g));
            assert!(!serde_json::to_string(&g).unwrap().contains("arcs"), "champs facultatifs omis");
        }
    }
}

#[test]
fn guidages_chemin_spatial_et_sections_intermediaires() {
    let p=profil();
    let chemin=vec![[0.0;3],[0.0,0.0,1000.0],[500.0,0.0,2000.0],[500.0,500.0,3000.0]];
    for g in [FormeGuidee::extrusion(chemin.clone()), FormeGuidee::raccordement_chemin(p.iter().map(|q| [q[0]*0.5,q[1]*0.8]).collect(),chemin)] {
        let tris=g.triangles(&p);
        verifier_fermeture(&tris);
        assert!(volume(&tris)>0.0);
    }
}

#[test]
fn onglet_exact_sur_chemin_coude() {
    // coude plan à 90° : onglet exact, volume = aire × longueur de l'axe (profil centré)
    let g=FormeGuidee::extrusion(vec![[0.0;3],[0.0,0.0,1000.0],[1000.0,0.0,1000.0]]);
    let tris=g.triangles(&carre(50.0));
    verifier_fermeture(&tris);
    assert!((volume(&tris)-10_000.0*2000.0).abs()<1e-3);
}

#[test]
fn tore_partiel_par_chemin_en_arc() {
    let r=1000.0;
    let s=std::f64::consts::FRAC_1_SQRT_2;
    let g=FormeGuidee::ExtrusionChemin { chemin: vec![[0.0;3],[r-r*s,r*s,0.0],[r,r,0.0]], arcs: vec![1], ferme: false };
    let texte=serde_json::to_string(&g).unwrap();
    assert!(texte.contains("\"arcs\":[1]"));
    assert_eq!(g,relu(&g));
    let tris=g.triangles(&carre(50.0));
    verifier_fermeture(&tris);
    // 90° en cordes de 5° au plus : 18 cordes ; onglets exacts
    let cordes=18.0*2.0*r*(2.5f64.to_radians()).sin();
    assert!((volume(&tris)-10_000.0*cordes).abs()/(10_000.0*cordes)<1e-9);
    let pappus=10_000.0*r*std::f64::consts::FRAC_PI_2;
    assert!((volume(&tris)-pappus).abs()/pappus<1e-3);
    // points entre les rayons 950 et 1050 autour du centre (r, 0), aux extrémités perpendiculaires aux cordes près
    let centre=DVec3::new(r,0.0,0.0);
    for t in &tris { for p in t { let d=(*p-centre).truncate().length(); assert!(d>945.0 && d<1055.0, "{d} {p}"); } }
}

#[test]
fn chemins_fermes_polygone_et_tore_complet() {
    let g=FormeGuidee::ExtrusionChemin { chemin: vec![[0.0;3],[2000.0,0.0,0.0],[2000.0,2000.0,0.0],[0.0,2000.0,0.0]], arcs: vec![], ferme: true };
    assert!(serde_json::to_string(&g).unwrap().contains("\"ferme\":true"));
    assert_eq!(g,relu(&g));
    let tris=g.triangles(&carre(50.0));
    verifier_fermeture(&tris);
    assert!((volume(&tris)-10_000.0*8000.0).abs()<1e-3);
    // cercle : deux arcs, le second se referme sur le premier point
    let r=1000.0;
    let tore=FormeGuidee::ExtrusionChemin { chemin: vec![[r,0.0,0.0],[0.0,r,0.0],[-r,0.0,0.0],[0.0,-r,0.0]], arcs: vec![1,3], ferme: true };
    let tris=tore.triangles(&carre(50.0));
    verifier_fermeture(&tris);
    let cordes=72.0*2.0*r*(2.5f64.to_radians()).sin();
    assert!((volume(&tris)-10_000.0*cordes).abs()/(10_000.0*cordes)<1e-9);
    // fermé non plan (vrillage réparti) : reste fermé et de volume positif
    let gauche=FormeGuidee::ExtrusionChemin { chemin: vec![[0.0;3],[2000.0,0.0,500.0],[2000.0,2000.0,0.0],[0.0,2000.0,800.0]], arcs: vec![], ferme: true };
    let tris=gauche.triangles(&carre(50.0));
    verifier_fermeture(&tris);
    assert!(volume(&tris)>0.0);
    // profil trop grand pour les tronçons : onglets recouverts, refus
    let petit=FormeGuidee::ExtrusionChemin { chemin: vec![[0.0;3],[200.0,0.0,0.0],[200.0,200.0,0.0],[0.0,200.0,0.0]], arcs: vec![], ferme: true };
    assert!(petit.valider(&carre(150.0)).is_err());
}

/// Aire de la coupe horizontale z = h d'un maillage fermé.
fn aire_coupe(tris: &[[DVec3;3]], h: f64) -> f64 {
    let mut aire=0.0;
    for t in tris {
        let mut pts=Vec::new();
        for i in 0..3 {
            let (a,b)=(t[i],t[(i+1)%3]);
            if (a.z-h)*(b.z-h)<0.0 { let u=(h-a.z)/(b.z-a.z); pts.push(a+(b-a)*u); }
        }
        if let [a,b]=pts.as_slice() {
            // sens : la normale extérieure tournée vers la gauche du segment
            let n=(t[1]-t[0]).cross(t[2]-t[0]);
            let (a,b)=if (*b-*a).cross(DVec3::Z).dot(n)>0.0 {(*b,*a)} else {(*a,*b)};
            aire+=(a.x*b.y-b.x*a.y)/2.0;
        }
    }
    aire.abs()
}

#[test]
fn raccordement_carre_octogone() {
    let carre=carre(100.0);
    let octogone: Vec<[f64;2]>=(0..8).map(|k| { let a=(22.5+45.0*k as f64).to_radians(); [(80.0*a.cos()).round(),(80.0*a.sin()).round()] }).collect();
    let g=FormeGuidee::raccordement(octogone.clone(),1000.0);
    g.valider(&carre).unwrap();
    assert_eq!(g,relu(&g));
    let tris=g.triangles(&carre);
    verifier_fermeture(&tris);
    let (a0,a1)=(40_000.0, super::super::raccord::aire(&octogone)/2.0);
    let m=aire_coupe(&tris,500.0);
    let attendu=1000.0/6.0*(a0+4.0*m+a1);
    assert!((volume(&tris)-attendu).abs()/attendu<1e-9, "{} {}", volume(&tris), attendu);
    assert!(volume(&tris)>1000.0*a1 && volume(&tris)<1000.0*a0);
    // le premier sommet d'arrivée est le plus proche du premier sommet de départ
    let (d,a)=super::super::raccord::correspondance(&carre,&octogone);
    assert_eq!(d.len(),a.len());
    assert!(d.len()>=8);
    assert_eq!(a[0],octogone[4], "égalité de distance : le premier trouvé");
    // sur un chemin coudé aussi
    let gc=FormeGuidee::raccordement_chemin(octogone,vec![[0.0;3],[0.0,0.0,1000.0],[800.0,0.0,1500.0]]);
    let tris=gc.triangles(&carre);
    verifier_fermeture(&tris);
}

#[test]
fn guidages_refusent_entrees_invalides() {
    for chemin in [vec![],vec![[0.0;3]],vec![[0.0;3],[0.0;3]],vec![[0.0;3],[0.0,0.0,100.0],[0.0,0.0,50.0]],vec![[0.0;3],[f64::NAN,0.0,10.0]]] {
        let g=FormeGuidee::extrusion(chemin);
        assert!(g.valider(&profil()).is_err()); assert!(g.triangles(&profil()).is_empty());
    }
    let mut fin=profil(); fin.reverse();
    assert!(FormeGuidee::raccordement(fin,100.0).valider(&profil()).is_err());
    assert!(FormeGuidee::raccordement(profil(),0.0).valider(&profil()).is_err());
    let droit=vec![[0.0;3],[0.0,0.0,500.0],[0.0,0.0,1000.0]];
    let arc=|arcs: Vec<usize>, chemin: Vec<[f64;3]>, ferme| FormeGuidee::ExtrusionChemin { chemin, arcs, ferme };
    assert!(arc(vec![1],droit.clone(),false).valider(&profil()).is_err(), "arc par trois points alignés");
    assert!(arc(vec![0],droit.clone(),false).valider(&profil()).is_err(), "arc sans voisin");
    assert!(arc(vec![2],droit.clone(),false).valider(&profil()).is_err());
    assert!(arc(vec![],droit[..2].to_vec(),true).valider(&profil()).is_err(), "fermé à deux points");
    let quatre=vec![[0.0;3],[1000.0,500.0,0.0],[2000.0,0.0,0.0],[3000.0,500.0,0.0]];
    assert!(arc(vec![1,2],quatre,false).valider(&profil()).is_err(), "arcs voisins");
    assert!(arc(vec![],vec![[0.0;3],[1000.0,0.0,0.0],[0.0,0.0,0.0]],false).valider(&profil()).is_err());
    // lecture : champs mal typés refusés
    assert!(serde_json::from_str::<FormeGuidee>(r#"{"genre":"extrusion_chemin","chemin":[[0,0,0],[0,0,1]],"arcs":[-1]}"#).is_err());
    assert!(serde_json::from_str::<FormeGuidee>(r#"{"genre":"extrusion_chemin","chemin":[[0,0,0],[0,0,1]],"ferme":"oui"}"#).is_err());
    assert!(serde_json::from_str::<FormeGuidee>(r#"{"genre":"raccordement","profil_fin":[],"hauteur":1,"ancrage_fin":[{"h":1}]}"#).is_err());
}

#[test]
fn guidage_famille_rechargee_et_repere_oblique() {
    use super::super::{FamilleModelee,FormeProfil,PlanConstruction,VueEsquisse,Face,PointEsquisse,Materiau};
    let mut famille=FamilleModelee::gabarit_fenetre();
    famille.volumes.clear(); famille.extrusions.clear();
    let plan=PlanConstruction::deux_points("Oblique".into(),VueEsquisse::Plan,[0.0,0.0],[100.0,100.0]).unwrap();
    let face=|v| Face {plan:String::new(),decalage:v};
    famille.extrusions.push(FormeProfil { visibilite: Default::default(),
        operation: super::super::OperationForme::Guidage { guidage: FormeGuidee::raccordement_chemin(profil().iter().map(|p| [p[0]*0.5,p[1]*0.5]).collect(),vec![[0.0;3],[0.0,0.0,1000.0]]) },construction:Some(plan.id.clone()),nom:"Guidage test".into(),vide:true,materiau:Materiau::Menuiserie,vue:VueEsquisse::Plan,
        profil:profil().iter().map(|p| PointEsquisse {h:face(p[0]),v:face(p[1])}).collect(),
        profondeur:[face(0.0),face(1000.0)],visible:None,materiau_parametre:None,
    });
    famille.plans_construction.push(plan);
    famille.valider(1200.0,1350.0,300.0).unwrap();
    let relue:FamilleModelee=serde_json::from_str(&serde_json::to_string(&famille).unwrap()).unwrap();
    assert_eq!(famille,relue);
    let solides=relue.solides(&relue.resoudre(1200.0,1350.0,300.0));
    assert!(solides[0].vide);
    verifier_fermeture(&solides[0].tris);
    assert!((volume(&solides[0].tris)-35_000_000.0/3.0).abs()<0.001);
}

#[test]
fn second_profil_pilote_par_les_plans() {
    use super::super::{FamilleModelee,FormeProfil,VueEsquisse,Face,PointEsquisse,Materiau,OperationForme};
    let mut famille=FamilleModelee::gabarit_fenetre();
    famille.volumes.clear(); famille.extrusions.clear();
    let (ah,av,ad)=VueEsquisse::Elevation.axes();
    let plan_d=famille.plans.iter().find(|p| p.axe==ad).unwrap().nom.clone();
    let plan_h=famille.plans.iter().find(|p| p.axe==ah).unwrap().nom.clone();
    let plan_v=famille.plans.iter().find(|p| p.axe==av).unwrap().nom.clone();
    let pt=|h: f64, v: f64| PointEsquisse { h: Face { plan: plan_h.clone(), decalage: h }, v: Face { plan: plan_v.clone(), decalage: v } };
    let profil: Vec<_>=carre(100.0).iter().map(|q| pt(q[0],q[1])).collect();
    let ancrage: Vec<_>=(0..6).map(|k| { let a=(60.0*k as f64).to_radians(); pt((60.0*a.cos()).round(),(60.0*a.sin()).round()) }).collect();
    let guidage=FormeGuidee::Raccordement { profil_fin: vec![], hauteur: 500.0, ancrage_fin: Some(ancrage) };
    famille.extrusions.push(FormeProfil { visibilite: Default::default(), operation: OperationForme::Guidage { guidage }, construction: None, nom: "Raccord ancré".into(), vide: false, materiau: Materiau::Menuiserie,
        vue: VueEsquisse::Elevation, profil, profondeur: [Face { plan: plan_d.clone(), decalage: 0.0 }, Face { plan: plan_d.clone(), decalage: 100.0 }], visible: None, materiau_parametre: None });
    famille.valider(1200.0,1350.0,300.0).unwrap();
    let r=famille.resoudre(1200.0,1350.0,300.0);
    let OperationForme::Guidage { guidage }=&famille.extrusions[0].operation else { unreachable!() };
    let resolu=guidage.resolue(|p| famille.point_esquisse(&r,p));
    assert_eq!(resolu.profil_fin().unwrap().len(),6);
    let base=r.positions[&plan_h];
    assert!((resolu.profil_fin().unwrap()[0][0]-(base+60.0)).abs()<1e-9);
    let texte=serde_json::to_string(&famille).unwrap();
    assert!(texte.contains("ancrage_fin"));
    let relue:FamilleModelee=serde_json::from_str(&texte).unwrap();
    assert_eq!(relue,famille);
    // plan absent dans l'ancrage : famille refusée
    let mut fausse=famille.clone();
    if let OperationForme::Guidage { guidage } = &mut fausse.extrusions[0].operation {
        guidage.ancrage_fin_mut().unwrap().as_mut().unwrap()[0].h.plan="Inconnu".into();
    }
    assert!(fausse.valider(1200.0,1350.0,300.0).is_err());
}
