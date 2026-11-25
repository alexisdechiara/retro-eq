//! L'icône dans la barre des tâches (zone de notification) : ouvrir la fenêtre, couper
//! l'égaliseur, quitter. Elle n'existe que sous Windows ; ailleurs ce module est vide et
//! fermer la fenêtre quitte l'appli.

/// Ce que l'utilisateur a demandé depuis l'icône.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
// Hors Windows il n'y a pas d'icône : personne ne fabrique ces demandes.
#[cfg_attr(not(windows), allow(dead_code))]
pub enum EvenementBarre {
    Ouvrir,
    BasculerEgaliseur,
    Quitter,
}

#[cfg(windows)]
mod systeme {
    use super::EvenementBarre;
    use crate::textes;
    use retro_core::Langue;
    use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem};
    use tray_icon::{Icon, MouseButton, TrayIcon, TrayIconBuilder, TrayIconEvent};

    const COTE: u32 = 32;

    /// L'icône, dessinée en mémoire aux couleurs du logo : un carré sombre arrondi et trois
    /// barres d'égaliseur (ambre, sarcelle, orange). Aucun fichier image à livrer.
    fn pixels() -> Vec<u8> {
        let fond = [0x1B, 0x1E, 0x22, 0xFF];
        let barres = [
            (6..11, 14, [0xF3, 0xA7, 0x12, 0xFF]),
            (13..18, 7, [0x2A, 0xB7, 0xA9, 0xFF]),
            (20..25, 18, [0xFF, 0x8A, 0x1F, 0xFF]),
        ];
        let mut rgba = Vec::with_capacity((COTE * COTE * 4) as usize);
        for y in 0..COTE {
            for x in 0..COTE {
                let bord = |v: u32| !(3..COTE - 3).contains(&v);
                let coin = bord(x) && bord(y);
                let couleur = if coin {
                    [0, 0, 0, 0]
                } else {
                    barres
                        .iter()
                        .find(|(colonnes, haut, _)| colonnes.contains(&x) && y >= *haut && y < 26)
                        .map_or(fond, |(_, _, c)| *c)
                };
                rgba.extend_from_slice(&couleur);
            }
        }
        rgba
    }

    /// L'icône et son menu. Garder cette valeur en vie garde l'icône affichée.
    pub struct Barre {
        _icone: TrayIcon,
        ouvrir: MenuItem,
        quitter: MenuItem,
        egaliseur: CheckMenuItem,
    }

    impl Barre {
        /// Crée l'icône. `None` si Windows refuse (pas de barre des tâches, par exemple) :
        /// l'appli marche quand même, fermer la fenêtre la quitte alors.
        pub fn nouvelle(egaliseur_actif: bool, langue: Langue) -> Option<Barre> {
            let (texte_ouvrir, texte_egaliseur, texte_quitter) = textes::menu_barre(langue);
            let ouvrir = MenuItem::new(texte_ouvrir, true, None);
            let egaliseur = CheckMenuItem::new(texte_egaliseur, true, egaliseur_actif, None);
            let quitter = MenuItem::new(texte_quitter, true, None);
            let menu = Menu::new();
            menu.append_items(&[
                &ouvrir,
                &egaliseur,
                &PredefinedMenuItem::separator(),
                &quitter,
            ])
            .ok()?;
            let icone = TrayIconBuilder::new()
                .with_tooltip("Retro EQ")
                .with_icon(Icon::from_rgba(pixels(), COTE, COTE).ok()?)
                .with_menu(Box::new(menu))
                .build()
                .ok()?;
            Some(Barre {
                _icone: icone,
                ouvrir,
                quitter,
                egaliseur,
            })
        }

        /// Remet les libellés du menu dans la langue de l'appli.
        pub fn regler_langue(&self, langue: Langue) {
            let (ouvrir, egaliseur, quitter) = textes::menu_barre(langue);
            self.ouvrir.set_text(ouvrir);
            self.egaliseur.set_text(egaliseur);
            self.quitter.set_text(quitter);
        }

        /// Met la coche « Égaliseur actif » d'accord avec l'appli.
        pub fn regler_egaliseur(&self, actif: bool) {
            if self.egaliseur.is_checked() != actif {
                self.egaliseur.set_checked(actif);
            }
        }

        /// Les demandes arrivées depuis la dernière fois (lues depuis le minuteur de l'interface).
        pub fn evenements(&self) -> Vec<EvenementBarre> {
            let mut evenements = Vec::new();
            while let Ok(evenement) = MenuEvent::receiver().try_recv() {
                if evenement.id == *self.ouvrir.id() {
                    evenements.push(EvenementBarre::Ouvrir);
                } else if evenement.id == *self.quitter.id() {
                    evenements.push(EvenementBarre::Quitter);
                } else if evenement.id == *self.egaliseur.id() {
                    evenements.push(EvenementBarre::BasculerEgaliseur);
                }
            }
            while let Ok(evenement) = TrayIconEvent::receiver().try_recv() {
                // Un clic gauche sur l'icône rouvre la fenêtre.
                if let TrayIconEvent::Click {
                    button: MouseButton::Left,
                    ..
                } = evenement
                {
                    evenements.push(EvenementBarre::Ouvrir);
                }
            }
            evenements
        }
    }
}

#[cfg(not(windows))]
mod systeme {
    use super::EvenementBarre;
    use retro_core::Langue;

    /// Hors Windows, il n'y a pas d'icône : `nouvelle` rend toujours `None`.
    pub struct Barre;

    impl Barre {
        pub fn nouvelle(_egaliseur_actif: bool, _langue: Langue) -> Option<Barre> {
            None
        }

        pub fn regler_egaliseur(&self, _actif: bool) {}

        pub fn regler_langue(&self, _langue: Langue) {}

        pub fn evenements(&self) -> Vec<EvenementBarre> {
            Vec::new()
        }
    }
}

pub use systeme::Barre;
