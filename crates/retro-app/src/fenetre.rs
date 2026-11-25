//! La fenêtre n'a pas de bordure Windows : c'est à nous de la déplacer, la réduire
//! et l'agrandir quand l'interface le demande. Elle n'a que deux tailles : 1280 × 800, ou agrandie.

use crate::{Appli, FenetrePrincipale};
use i_slint_backend_winit::winit::window::WindowButtons;
use i_slint_backend_winit::WinitWindowAccessor;
use slint::ComponentHandle;
use std::time::Duration;

/// La taille de la fenêtre quand elle n'est pas agrandie, en pixels logiques (ceux de Slint).
const LARGEUR: f32 = 1280.0;
const HAUTEUR: f32 = 800.0;

/// Branche les boutons et la barre de titre sur la vraie fenêtre.
///
/// Rend un minuteur à garder en vie tant que la fenêtre existe : il veille à ce qu'elle
/// reste de taille fixe (voir [`figer_la_taille`]).
pub fn brancher(fenetre: &FenetrePrincipale) -> slint::Timer {
    let appli = fenetre.global::<Appli>();

    let faible = fenetre.as_weak();
    appli.on_deplacer_fenetre(move || {
        if let Some(fenetre) = faible.upgrade() {
            // Windows prend la main sur le déplacement (glisser la barre de titre).
            fenetre.window().with_winit_window(|w| {
                let _ = w.drag_window();
            });
        }
    });

    let faible = fenetre.as_weak();
    appli.on_reduire(move || {
        if let Some(fenetre) = faible.upgrade() {
            fenetre.window().set_minimized(true);
        }
    });

    let faible = fenetre.as_weak();
    appli.on_agrandir(move || {
        let Some(fenetre) = faible.upgrade() else {
            return;
        };
        if fenetre.window().is_maximized() {
            // Revenir à 1280 × 800, puis refermer la limite de taille.
            fenetre.window().set_maximized(false);
            fenetre.global::<Appli>().set_agrandie(false);
        } else {
            // La fenêtre est limitée à 1280 × 800 (voir app.slint) : on lève d'abord la
            // limite, et on agrandit un instant plus tard, une fois que Slint l'a transmise à
            // Windows. Agrandie tout de suite, elle resterait bloquée à 1280 × 800.
            fenetre.global::<Appli>().set_agrandie(true);
            let faible = fenetre.as_weak();
            slint::Timer::single_shot(Duration::from_millis(50), move || {
                if let Some(fenetre) = faible.upgrade() {
                    fenetre.window().set_maximized(true);
                }
            });
        }
    });

    let veille = slint::Timer::default();
    let faible = fenetre.as_weak();
    veille.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(200),
        move || {
            if let Some(fenetre) = faible.upgrade() {
                figer_la_taille(&fenetre);
            }
        },
    );
    veille
}

/// Retire à Windows le droit de redimensionner la fenêtre : plus de bords étirables, plus
/// d'ancrage Snap (glisser contre un bord, Win+flèches), plus de bouton « agrandir » système.
/// Notre bouton agrandir passe par `set_maximized`, qui marche quand même.
///
/// C'est un filet de sécurité. La vraie protection est dans app.slint : taille mini = taille
/// maxi, donc Slint juge lui-même la fenêtre non redimensionnable. Mais Slint refait ce
/// réglage à chaque changement de contraintes (par exemple pendant l'agrandissement, quand
/// la limite est levée) : la veille repasse derrière. Constaté sur le PC le 25/11/2026 : la
/// fenêtre avait les styles Windows `WS_THICKFRAME` et `WS_MAXIMIZEBOX`.
fn figer_la_taille(fenetre: &FenetrePrincipale) {
    let fenetre_slint = fenetre.window();
    // Hors agrandissement, toujours 1280 × 800. Constaté sur le PC : créée sur l'écran à
    // 150 % puis posée par Windows sur un écran à 125 %, la fenêtre gardait ses pixels et
    // faisait 1536 × 960.
    if !fenetre_slint.is_maximized()
        && !fenetre_slint.is_minimized()
        && !fenetre.global::<Appli>().get_agrandie()
    {
        let taille = fenetre_slint
            .size()
            .to_logical(fenetre_slint.scale_factor());
        if (taille.width - LARGEUR).abs() > 1.0 || (taille.height - HAUTEUR).abs() > 1.0 {
            fenetre_slint.set_size(slint::LogicalSize::new(LARGEUR, HAUTEUR));
        }
    }
    fenetre_slint.with_winit_window(|w| {
        if w.is_resizable() {
            w.set_resizable(false);
        }
        let boutons = w.enabled_buttons();
        if boutons.contains(WindowButtons::MAXIMIZE) {
            w.set_enabled_buttons(boutons - WindowButtons::MAXIMIZE);
        }
    });
}
