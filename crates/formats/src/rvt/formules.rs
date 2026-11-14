//! Formules Revit : arbres d'expression des paramètres de familles
//! (`NamedParam.m_oExpression`) et des champs calculés des nomenclatures
//! (`ScheduleField.m_pExpression`) (LISEZMOI-RVT.md du dossier Decodage RVT, § 7
//! « Formules »).
//!
//! Classes dérivées de `Expression` : `BinaryOperatorExpression` (`m_binaryOperator`,
//! `m_pLeftSubexpression`, `m_pRightSubexpression`), `FunctionExpression`
//! (`m_function`, `m_subexpressions`), `ParenExpression` (`m_pSubexpression`),
//! `UnaryOperatorExpression`, `NumberConstantExpression` (`m_value` en unités
//! internes, `m_specTypeId`), `StringConstantExpression`, `ParameterExpression`
//! (`m_paramId`), `ScheduleFieldExpression` (`m_fieldId`).
//!
//! Codes établis par l'essai E18 (une formule par fonction, valeurs de FamilyManager)
//! et contrôlés sur les valeurs que Revit enregistre dans les types (572 formules de
//! Kerreut et Saint-Malo) : opérateurs 1 +, 2 −, 3 ×, 4 ÷, 5 ^, 6 =, 7 >, 8 < ;
//! opérateur unaire 1 − ; fonctions 1 sin, 2 cos, 3 tan, 4 asin, 5 acos, 6 atan, 7 exp,
//! 8 log (décimal), 9 sqrt, 10 if, 11 or, 12 and, 13 not, 14 abs, 16 pi, 18 round,
//! 19 roundup, 20 rounddown. Un autre code n'est pas évalué : la formule n'a pas de
//! valeur.

use super::champs::{Obj, Record, Val};
use super::parametres::{Brut, Stockage};

#[derive(Clone, Debug, PartialEq)]
pub enum Expr {
    Nombre(f64),
    Texte(String),
    /// Paramètre (id de l'élément paramètre, ou id négatif d'un paramètre intégré).
    Param(i64),
    /// Autre champ de la nomenclature (`ScheduleFieldId.m_id`).
    Champ(i64),
    Binaire(i64, Box<Expr>, Box<Expr>),
    Unaire(i64, Box<Expr>),
    Fonction(i64, Vec<Expr>),
    Parenthese(Box<Expr>),
    /// Classe non lue.
    Autre(String),
}

/// Valeur d'une expression : nombre (unités internes ; un Oui/Non vaut 1 ou 0) ou texte.
#[derive(Clone, Debug, PartialEq)]
pub enum Valeur {
    Nombre(f64),
    Texte(String),
}

impl Valeur {
    pub fn nombre(&self) -> Option<f64> {
        match self {
            Valeur::Nombre(x) => Some(*x),
            Valeur::Texte(_) => None,
        }
    }
}

/// Valeur d'un paramètre pour une formule (un ElementId vaut son numéro).
pub fn de_brut(b: &Brut, st: Stockage) -> Valeur {
    match b.selon(st) {
        Brut::Reel(x) => Valeur::Nombre(x),
        Brut::Entier(i) | Brut::Id(i) => Valeur::Nombre(i as f64),
        Brut::Texte(s) => Valeur::Texte(s),
        Brut::Nomme { reel, .. } => Valeur::Nombre(reel),
    }
}

/// Objet désigné par un pointeur possédé, ou incorporé.
fn corps<'r, 's>(r: &'r Record<'s>, v: &'r Val<'s>) -> Option<&'r Obj<'s>> {
    r.deref(v).map(|b| &b.obj).or(v.obj())
}

/// Arbre d'une expression.
pub fn lire(r: &Record, o: &Obj) -> Option<Expr> {
    let sous = |k: &str| o.get(k).and_then(|v| corps(r, v)).and_then(|x| lire(r, x)).map(Box::new);
    let int = |k: &str| o.get(k).and_then(Val::int).unwrap_or(-1);
    Some(match o.class_name() {
        "BinaryOperatorExpression" => Expr::Binaire(int("m_binaryOperator"), sous("m_pLeftSubexpression")?, sous("m_pRightSubexpression")?),
        "UnaryOperatorExpression" => Expr::Unaire(int("m_unaryOperator"), sous("m_pSubexpression")?),
        "ParenExpression" => Expr::Parenthese(sous("m_pSubexpression")?),
        "FunctionExpression" => Expr::Fonction(
            int("m_function"),
            o.get("m_subexpressions").map(Val::list).unwrap_or_default().iter().map(|v| corps(r, v).and_then(|x| lire(r, x))).collect::<Option<Vec<_>>>()?,
        ),
        "NumberConstantExpression" => Expr::Nombre(o.get("m_value").and_then(Val::real)?),
        "StringConstantExpression" => Expr::Texte(o.get("m_value").and_then(Val::str).unwrap_or_default().to_string()),
        "ParameterExpression" => Expr::Param(o.get("m_paramId").and_then(Val::element_id)?),
        "ScheduleFieldExpression" => Expr::Champ(o.get("m_fieldId").and_then(Val::obj).and_then(|f| f.get("m_id")).and_then(Val::int)?),
        c => Expr::Autre(c.to_string()),
    })
}

/// Valeur d'une expression ; `variable` donne celle d'un paramètre ou d'un champ. None
/// si une variable manque ou si un code n'est pas établi.
pub fn evaluer(e: &Expr, variable: &dyn Fn(&Expr) -> Option<Valeur>) -> Option<Valeur> {
    let n = |e: &Expr| evaluer(e, variable)?.nombre();
    let oui = |b: bool| Valeur::Nombre(if b { 1.0 } else { 0.0 });
    Some(match e {
        Expr::Nombre(x) => Valeur::Nombre(*x),
        Expr::Texte(s) => Valeur::Texte(s.clone()),
        Expr::Param(_) | Expr::Champ(_) => variable(e)?,
        Expr::Parenthese(x) => evaluer(x, variable)?,
        Expr::Binaire(op, a, b) => {
            let (a, b) = (n(a)?, n(b)?);
            match op {
                1 => Valeur::Nombre(a + b),
                2 => Valeur::Nombre(a - b),
                3 => Valeur::Nombre(a * b),
                4 if b != 0.0 => Valeur::Nombre(a / b),
                5 => Valeur::Nombre(a.powf(b)),
                6 => oui((a - b).abs() <= 1e-9 * a.abs().max(b.abs()).max(1.0)),
                7 => oui(a > b),
                8 => oui(a < b),
                _ => return None,
            }
        }
        Expr::Fonction(f, args) => match (f, args.as_slice()) {
            // angles en radians (unités internes)
            (1, [a]) => Valeur::Nombre(n(a)?.sin()),
            (2, [a]) => Valeur::Nombre(n(a)?.cos()),
            (3, [a]) => Valeur::Nombre(n(a)?.tan()),
            (4, [a]) => Valeur::Nombre(n(a)?.asin()),
            (5, [a]) => Valeur::Nombre(n(a)?.acos()),
            (6, [a]) => Valeur::Nombre(n(a)?.atan()),
            (7, [a]) => Valeur::Nombre(n(a)?.exp()),
            (8, [a]) => Valeur::Nombre(n(a)?.log10()),
            (9, [a]) => Valeur::Nombre(n(a)?.sqrt()),
            // if : seule la branche retenue est évaluée
            (10, [c, a, b]) => evaluer(if n(c)? != 0.0 { a } else { b }, variable)?,
            (11, l) => oui(l.iter().map(|x| n(x).map(|v| v != 0.0)).collect::<Option<Vec<_>>>()?.into_iter().any(|v| v)),
            (12, l) => oui(l.iter().map(|x| n(x).map(|v| v != 0.0)).collect::<Option<Vec<_>>>()?.into_iter().all(|v| v)),
            (13, [a]) => oui(n(a)? == 0.0),
            (14, [a]) => Valeur::Nombre(n(a)?.abs()),
            (16, []) => Valeur::Nombre(std::f64::consts::PI),
            // round : demi loin de zéro (2,5 → 3 ; −2,7 → −3) ; roundup (−2,7 → −2),
            // rounddown (−2,7 → −3) ; tolérance pour un quotient tombant juste
            // (10,000…01 → 10)
            (18, [a]) => {
                let x = n(a)?;
                Valeur::Nombre((x + 1e-9 * x.signum()).round())
            }
            (19, [a]) => Valeur::Nombre((n(a)? - 1e-9).ceil()),
            (20, [a]) => Valeur::Nombre((n(a)? + 1e-9).floor()),
            _ => return None,
        },
        Expr::Unaire(1, a) => Valeur::Nombre(-n(a)?),
        Expr::Unaire(..) | Expr::Autre(_) => return None,
    })
}
