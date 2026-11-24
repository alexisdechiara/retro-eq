//! La fenêtre n'a pas de bordure Windows : c'est à nous de la déplacer, la réduire
//! et l'agrandir quand l'interface le demande. Elle n'a que deux tailles : 1280 × 800, ou agrandie.

use crate::{Appli, FenetrePrincipale};
use i_slint_backend_winit::WinitWindowAccessor;
use slint::ComponentHandle;

/// Branche les boutons et la barre de titre sur la vraie fenêtre.
pub fn brancher(fenetre: &FenetrePrincipale) {
    let appli = fenetre.global::<Appli>();

    let faible = fenetre.as_weak();
    appli.on_deplacer_fenetre(move || {
        if let Some(fenetre) = faible.upgrade() {
            // Windows prend la main sur le déplacement : c'est ce qui donne l'ancrage aux bords de l'écran.
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
        if let Some(fenetre) = faible.upgrade() {
            let agrandie = !fenetre.window().is_maximized();
            fenetre.window().set_maximized(agrandie);
            fenetre.global::<Appli>().set_agrandie(agrandie);
        }
    });
}
