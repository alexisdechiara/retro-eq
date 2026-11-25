//! L'icône dans la barre des tâches (zone de notification) : ouvrir la fenêtre, couper
//! l'égaliseur, quitter. Elle n'existe que sous Windows ; ailleurs ce module est vide et
//! fermer la fenêtre quitte l'appli.

/// Ce que l'utilisateur a demandé depuis l'icône.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvenementBarre {
    Ouvrir,
    BasculerEgaliseur,
    Quitter,
}

#[cfg(windows)]
mod systeme {
    use super::EvenementBarre;
    use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem};
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
                let coin = (x < 3 || x >= COTE - 3) && (y < 3 || y >= COTE - 3);
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
        ouvrir: MenuId,
        quitter: MenuId,
        egaliseur: CheckMenuItem,
    }

    impl Barre {
        /// Crée l'icône. `None` si Windows refuse (pas de barre des tâches, par exemple) :
        /// l'appli marche quand même, fermer la fenêtre la quitte alors.
        pub fn nouvelle(egaliseur_actif: bool) -> Option<Barre> {
            let ouvrir = MenuItem::new("Ouvrir Retro EQ", true, None);
            let egaliseur = CheckMenuItem::new("Égaliseur actif", true, egaliseur_actif, None);
            let quitter = MenuItem::new("Quitter", true, None);
            let menu = Menu::new();
            menu.append_items(&[&ouvrir, &egaliseur, &PredefinedMenuItem::separator(), &quitter])
                .ok()?;
            let icone = TrayIconBuilder::new()
                .with_tooltip("Retro EQ")
                .with_icon(Icon::from_rgba(pixels(), COTE, COTE).ok()?)
                .with_menu(Box::new(menu))
                .build()
                .ok()?;
            Some(Barre {
                _icone: icone,
                ouvrir: ouvrir.id().clone(),
                quitter: quitter.id().clone(),
                egaliseur,
            })
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
                if evenement.id == self.ouvrir {
                    evenements.push(EvenementBarre::Ouvrir);
                } else if evenement.id == self.quitter {
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

    /// Hors Windows, il n'y a pas d'icône : `nouvelle` rend toujours `None`.
    pub struct Barre;

    impl Barre {
        pub fn nouvelle(_egaliseur_actif: bool) -> Option<Barre> {
            None
        }

        pub fn regler_egaliseur(&self, _actif: bool) {}

        pub fn evenements(&self) -> Vec<EvenementBarre> {
            Vec::new()
        }
    }
}

pub use systeme::Barre;
