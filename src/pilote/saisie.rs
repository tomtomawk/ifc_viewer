//! Entrées egui produites par le pilote, validées avant leur mise en file.
use eframe::egui::{self, Event, Modifiers, PointerButton};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum Saisie {
    Clic {
        x: f32,
        y: f32,
        #[serde(default)]
        bouton: Bouton,
        #[serde(default)]
        maj: bool,
        #[serde(default)]
        ctrl: bool,
    },
    Pointer {
        x: f32,
        y: f32,
    },
    Bouton {
        x: f32,
        y: f32,
        bouton: Bouton,
        appuye: bool,
        #[serde(default)]
        maj: bool,
        #[serde(default)]
        ctrl: bool,
    },
    Defiler {
        x: f32,
        y: f32,
        dx: f32,
        dy: f32,
    },
    Texte {
        texte: String,
    },
    Touche {
        touche: String,
        #[serde(default)]
        maj: bool,
        #[serde(default)]
        ctrl: bool,
        #[serde(default)]
        alt: bool,
    },
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Bouton {
    #[default]
    Gauche,
    Droit,
    Milieu,
}
impl Bouton {
    fn egui(self) -> PointerButton {
        match self {
            Self::Gauche => PointerButton::Primary,
            Self::Droit => PointerButton::Secondary,
            Self::Milieu => PointerButton::Middle,
        }
    }
}
fn position(x: f32, y: f32) -> Result<egui::Pos2, String> {
    if !x.is_finite() || !y.is_finite() {
        return Err("Coordonnées non finies".into());
    }
    Ok(egui::pos2(x, y))
}
fn modifiers(maj: bool, ctrl: bool, alt: bool) -> Modifiers {
    Modifiers {
        shift: maj,
        ctrl,
        command: ctrl,
        alt,
        ..Default::default()
    }
}

pub fn evenements(saisies: Vec<Saisie>) -> Result<Vec<Event>, String> {
    let mut out = Vec::new();
    for s in saisies {
        match s {
            Saisie::Pointer { x, y } => out.push(Event::PointerMoved(position(x, y)?)),
            Saisie::Clic {
                x,
                y,
                bouton,
                maj,
                ctrl,
            } => {
                let pos = position(x, y)?;
                let button = bouton.egui();
                out.push(Event::PointerMoved(pos));
                for pressed in [true, false] {
                    out.push(Event::PointerButton {
                        pos,
                        button,
                        pressed,
                        modifiers: modifiers(maj, ctrl, false),
                    });
                }
            }
            Saisie::Bouton {
                x,
                y,
                bouton,
                appuye,
                maj,
                ctrl,
            } => {
                let pos = position(x, y)?;
                out.push(Event::PointerMoved(pos));
                out.push(Event::PointerButton {
                    pos,
                    button: bouton.egui(),
                    pressed: appuye,
                    modifiers: modifiers(maj, ctrl, false),
                });
            }
            Saisie::Defiler { x, y, dx, dy } => {
                if !dx.is_finite() || !dy.is_finite() {
                    return Err("Défilement non fini".into());
                }
                out.push(Event::PointerMoved(position(x, y)?));
                out.push(Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: egui::vec2(dx, dy),
                    phase: egui::TouchPhase::Move,
                    source: egui::MouseWheelSource::Wheel,
                    modifiers: Modifiers::NONE,
                });
            }
            Saisie::Texte { texte } => out.push(Event::Text(texte)),
            Saisie::Touche {
                touche,
                maj,
                ctrl,
                alt,
            } => {
                let key = egui::Key::from_name(&touche).ok_or_else(|| {
                    format!(
                        "Touche inconnue : {touche} (Enter, Escape, Delete, Tab, ArrowLeft, A…)"
                    )
                })?;
                for pressed in [true, false] {
                    out.push(Event::Key {
                        key,
                        physical_key: None,
                        pressed,
                        repeat: false,
                        modifiers: modifiers(maj, ctrl, alt),
                    });
                }
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn clic_et_clavier_ordonnances_et_valides() {
        let s = serde_json::from_str(
            r#"[{"action":"clic","x":10,"y":20,"maj":true},{"action":"touche","touche":"Enter"}]"#,
        )
        .unwrap();
        let e = evenements(s).unwrap();
        assert_eq!(e.len(), 5);
        assert!(matches!(
            e[1],
            Event::PointerButton {
                pressed: true,
                modifiers: Modifiers { shift: true, .. },
                ..
            }
        ));
        assert!(matches!(e[2], Event::PointerButton { pressed: false, .. }));
        assert!(
            evenements(vec![Saisie::Touche {
                touche: "inconnue".into(),
                maj: false,
                ctrl: false,
                alt: false
            }])
            .is_err()
        );
    }
}
