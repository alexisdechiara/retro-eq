//! Les raccourcis clavier globaux : ils marchent même quand un jeu a le clavier.
//!
//! Deux familles (ADR-0008) :
//! - F6 à F10 ne sont pris **que pendant un test**. Le reste du temps ils sont rendus au
//!   système, pour ne pas les retirer aux jeux.
//! - Ctrl+Alt+Entrée, Ctrl+Alt+0 à 4 sont pris tant que l'appli tourne.
//!
//! Le système prévient par des événements « enfoncée » et « relâchée ». La partie qui en
//! fait des actions ([`Appuis`]) est du Rust pur, testé sans clavier : c'est elle qui
//! distingue l'appui court (une réponse) de l'appui maintenu (réécouter).

use global_hotkey::hotkey::{Code, HotKey, Modifiers};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
use retro_core::{Cote, Reponse};
use std::collections::HashMap;

/// En dessous de cette durée, F6 ou F8 est une réponse ; au-delà, c'est une réécoute.
pub const SEUIL_MAINTIEN_MS: u64 = 350;

/// Une touche (ou combinaison) que l'appli écoute.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Touche {
    A,
    Pareil,
    B,
    PlusTard,
    Annuler,
    Demarrer,
    /// Ctrl+Alt+1 à 4.
    Profil(u8),
    Egaliseur,
}

impl Touche {
    /// Le nom affiché dans les messages de conflit.
    pub fn nom(self) -> String {
        match self {
            Touche::A => "F6".into(),
            Touche::Pareil => "F7".into(),
            Touche::B => "F8".into(),
            Touche::PlusTard => "F9".into(),
            Touche::Annuler => "F10".into(),
            Touche::Demarrer => "Ctrl+Alt+Entrée".into(),
            Touche::Profil(n) => format!("Ctrl+Alt+{n}"),
            Touche::Egaliseur => "Ctrl+Alt+0".into(),
        }
    }

    /// La ligne du tableau des raccourcis (page Réglages) où cette touche figure.
    pub fn ligne(self) -> usize {
        match self {
            Touche::A => 0,
            Touche::Pareil => 1,
            Touche::B => 2,
            Touche::PlusTard => 3,
            Touche::Annuler => 4,
            Touche::Demarrer => 5,
            Touche::Profil(_) => 6,
            Touche::Egaliseur => 7,
        }
    }

    fn cote(self) -> Option<Cote> {
        match self {
            Touche::A => Some(Cote::A),
            Touche::B => Some(Cote::B),
            _ => None,
        }
    }

    fn combinaison(self) -> HotKey {
        let ctrl_alt = Some(Modifiers::CONTROL | Modifiers::ALT);
        match self {
            Touche::A => HotKey::new(None, Code::F6),
            Touche::Pareil => HotKey::new(None, Code::F7),
            Touche::B => HotKey::new(None, Code::F8),
            Touche::PlusTard => HotKey::new(None, Code::F9),
            Touche::Annuler => HotKey::new(None, Code::F10),
            Touche::Demarrer => HotKey::new(ctrl_alt, Code::Enter),
            Touche::Egaliseur => HotKey::new(ctrl_alt, Code::Digit0),
            Touche::Profil(1) => HotKey::new(ctrl_alt, Code::Digit1),
            Touche::Profil(2) => HotKey::new(ctrl_alt, Code::Digit2),
            Touche::Profil(3) => HotKey::new(ctrl_alt, Code::Digit3),
            Touche::Profil(_) => HotKey::new(ctrl_alt, Code::Digit4),
        }
    }
}

const TOUCHES_DE_TEST: [Touche; 5] = [
    Touche::A,
    Touche::Pareil,
    Touche::B,
    Touche::PlusTard,
    Touche::Annuler,
];

const TOUCHES_PERMANENTES: [Touche; 6] = [
    Touche::Demarrer,
    Touche::Egaliseur,
    Touche::Profil(1),
    Touche::Profil(2),
    Touche::Profil(3),
    Touche::Profil(4),
];

/// Ce qu'une touche demande à l'appli.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Repondre(Reponse),
    Reecouter { cote: Cote, appuye: bool },
    PlusTard,
    Annuler,
    DemarrerOuReprendre,
    ActiverProfil(usize),
    BasculerEgaliseur,
}

#[derive(Debug, Clone, Copy)]
struct Appui {
    depuis_ms: u64,
    /// La touche a déjà produit son action (réponse immédiate ou touche simple).
    consomme: bool,
    /// Une réécoute est en cours : le relâchement devra remettre la référence.
    reecoute: bool,
}

/// Transforme « enfoncée / relâchée » en actions.
///
/// Pour F6 et F8 : si la question est posée (`reecoute_possible`), on attend de savoir si
/// l'appui est court (réponse, au relâchement) ou maintenu (réécoute, dès 350 ms). Sinon,
/// pendant les passages, la réponse part dès l'appui : aucune raison d'attendre.
#[derive(Debug, Default)]
pub struct Appuis {
    enfoncees: HashMap<Touche, Appui>,
}

impl Appuis {
    pub fn appui(&mut self, touche: Touche, t: u64, reecoute_possible: bool) -> Option<Action> {
        // Le clavier répète une touche maintenue : seul le premier « enfoncée » compte.
        if self.enfoncees.contains_key(&touche) {
            return None;
        }
        let attend = touche.cote().is_some() && reecoute_possible;
        self.enfoncees.insert(
            touche,
            Appui {
                depuis_ms: t,
                consomme: !attend,
                reecoute: false,
            },
        );
        if attend {
            return None;
        }
        Some(match touche {
            Touche::A => Action::Repondre(Reponse::A),
            Touche::B => Action::Repondre(Reponse::B),
            Touche::Pareil => Action::Repondre(Reponse::Pareil),
            Touche::PlusTard => Action::PlusTard,
            Touche::Annuler => Action::Annuler,
            Touche::Demarrer => Action::DemarrerOuReprendre,
            Touche::Profil(n) => Action::ActiverProfil(usize::from(n)),
            Touche::Egaliseur => Action::BasculerEgaliseur,
        })
    }

    pub fn relache(&mut self, touche: Touche) -> Option<Action> {
        let appui = self.enfoncees.remove(&touche)?;
        let cote = touche.cote()?;
        if appui.reecoute {
            Some(Action::Reecouter {
                cote,
                appuye: false,
            })
        } else if !appui.consomme {
            Some(Action::Repondre(match cote {
                Cote::A => Reponse::A,
                Cote::B => Reponse::B,
            }))
        } else {
            None
        }
    }

    /// À appeler régulièrement : fait démarrer la réécoute des touches maintenues assez longtemps.
    pub fn tic(&mut self, t: u64) -> Vec<Action> {
        let mut actions = Vec::new();
        for (touche, appui) in &mut self.enfoncees {
            let assez = t.saturating_sub(appui.depuis_ms) >= SEUIL_MAINTIEN_MS;
            if let (Some(cote), false, true) = (touche.cote(), appui.consomme, assez) {
                appui.consomme = true;
                appui.reecoute = true;
                actions.push(Action::Reecouter { cote, appuye: true });
            }
        }
        actions
    }

    /// Oublie tout (les touches de test viennent d'être rendues au système). Rend les
    /// réécoutes à arrêter : une variante ne doit pas rester active.
    pub fn vider(&mut self) -> Vec<Action> {
        self.enfoncees
            .drain()
            .filter(|(_, appui)| appui.reecoute)
            .filter_map(|(touche, _)| touche.cote())
            .map(|cote| Action::Reecouter {
                cote,
                appuye: false,
            })
            .collect()
    }
}

/// Les raccourcis enregistrés auprès du système.
///
/// Sous Windows, le gestionnaire doit vivre sur le fil qui fait tourner la boucle de
/// messages : c'est le fil de l'interface, et c'est là que cette structure est créée et
/// gardée. Les événements arrivent dans un canal global, lu depuis le minuteur de l'interface.
pub struct Raccourcis {
    gestionnaire: Option<GlobalHotKeyManager>,
    /// Identifiant système d'une combinaison → notre touche.
    touches: HashMap<u32, Touche>,
    test_pris: bool,
    conflits: Vec<Touche>,
    appuis: Appuis,
    /// Raccourcis volontairement non enregistrés (capture d'écran) : ce n'est pas une panne.
    inertes: bool,
}

impl Raccourcis {
    /// Enregistre les raccourcis permanents et vérifie que F6 à F10 sont libres. Un
    /// raccourci déjà pris par une autre appli est noté comme conflit ; rien ne plante.
    pub fn nouveau() -> Raccourcis {
        let mut r = Raccourcis {
            gestionnaire: GlobalHotKeyManager::new().ok(),
            touches: HashMap::new(),
            test_pris: false,
            conflits: Vec::new(),
            appuis: Appuis::default(),
            inertes: false,
        };
        for touche in TOUCHES_PERMANENTES {
            r.enregistrer(touche);
        }
        r.sonder_touches_de_test();
        r
    }

    /// Des raccourcis inertes : pour les captures d'écran et les essais sans clavier.
    pub fn inertes() -> Raccourcis {
        Raccourcis {
            gestionnaire: None,
            touches: HashMap::new(),
            test_pris: false,
            conflits: Vec::new(),
            appuis: Appuis::default(),
            inertes: true,
        }
    }

    fn enregistrer(&mut self, touche: Touche) -> bool {
        let Some(gestionnaire) = &self.gestionnaire else {
            return false;
        };
        let combinaison = touche.combinaison();
        match gestionnaire.register(combinaison) {
            Ok(()) => {
                self.touches.insert(combinaison.id(), touche);
                self.conflits.retain(|c| *c != touche);
                true
            }
            Err(_) => {
                if !self.conflits.contains(&touche) {
                    self.conflits.push(touche);
                }
                false
            }
        }
    }

    fn liberer(&mut self, touche: Touche) {
        let combinaison = touche.combinaison();
        if self.touches.remove(&combinaison.id()).is_some() {
            if let Some(gestionnaire) = &self.gestionnaire {
                let _ = gestionnaire.unregister(combinaison);
            }
        }
    }

    /// Prend puis rend aussitôt F6 à F10, pour savoir s'ils sont libres sans les garder.
    /// Sans effet pendant un test (ils sont déjà pris, par nous).
    pub fn sonder_touches_de_test(&mut self) {
        if self.test_pris {
            return;
        }
        for touche in TOUCHES_DE_TEST {
            if self.enregistrer(touche) {
                self.liberer(touche);
            }
        }
    }

    /// Prend F6 à F10 quand un test attend des réponses, les rend sinon. Rend les actions
    /// à exécuter au passage (arrêt d'une réécoute restée en cours).
    pub fn regler_touches_de_test(&mut self, actives: bool) -> Vec<Action> {
        if actives == self.test_pris {
            return Vec::new();
        }
        self.test_pris = actives;
        if actives {
            for touche in TOUCHES_DE_TEST {
                self.enregistrer(touche);
            }
            Vec::new()
        } else {
            for touche in TOUCHES_DE_TEST {
                self.liberer(touche);
            }
            self.appuis.vider()
        }
    }

    /// Lit les événements arrivés depuis la dernière fois et rend les actions à exécuter.
    pub fn actions(&mut self, t: u64, reecoute_possible: bool) -> Vec<Action> {
        let mut actions = Vec::new();
        if self.gestionnaire.is_none() {
            return actions;
        }
        while let Ok(evenement) = GlobalHotKeyEvent::receiver().try_recv() {
            let Some(&touche) = self.touches.get(&evenement.id) else {
                continue;
            };
            let action = match evenement.state {
                HotKeyState::Pressed => self.appuis.appui(touche, t, reecoute_possible),
                HotKeyState::Released => self.appuis.relache(touche),
            };
            actions.extend(action);
        }
        actions.extend(self.appuis.tic(t));
        actions
    }

    /// Le système a-t-il accepté de nous laisser enregistrer des raccourcis ?
    pub fn disponibles(&self) -> bool {
        self.gestionnaire.is_some() || self.inertes
    }

    /// Les noms des raccourcis déjà pris par une autre appli.
    pub fn conflits(&self) -> Vec<String> {
        self.conflits.iter().map(|t| t.nom()).collect()
    }

    /// Pour chaque ligne du tableau des raccourcis : y a-t-il un conflit ?
    pub fn conflits_par_ligne(&self) -> [bool; 8] {
        let mut lignes = [false; 8];
        for touche in &self.conflits {
            lignes[touche.ligne()] = true;
        }
        lignes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn un_appui_court_apres_la_question_est_une_reponse_au_relachement() {
        let mut a = Appuis::default();
        assert_eq!(a.appui(Touche::A, 1_000, true), None);
        assert!(a.tic(1_200).is_empty());
        assert_eq!(a.relache(Touche::A), Some(Action::Repondre(Reponse::A)));
        assert!(a.tic(5_000).is_empty());
    }

    #[test]
    fn un_appui_maintenu_fait_reecouter_jusqu_au_relachement() {
        let mut a = Appuis::default();
        assert_eq!(a.appui(Touche::B, 1_000, true), None);
        assert!(a.tic(1_349).is_empty());
        assert_eq!(
            a.tic(1_350),
            vec![Action::Reecouter {
                cote: Cote::B,
                appuye: true
            }]
        );
        // Une seule fois, même si on continue de tenir.
        assert!(a.tic(3_000).is_empty());
        // Ce n'est jamais une réponse.
        assert_eq!(
            a.relache(Touche::B),
            Some(Action::Reecouter {
                cote: Cote::B,
                appuye: false
            })
        );
    }

    #[test]
    fn pendant_les_passages_la_reponse_part_des_l_appui() {
        let mut a = Appuis::default();
        assert_eq!(
            a.appui(Touche::A, 1_000, false),
            Some(Action::Repondre(Reponse::A))
        );
        // Tenir la touche ne déclenche rien de plus, la relâcher non plus.
        assert!(a.tic(5_000).is_empty());
        assert_eq!(a.relache(Touche::A), None);
    }

    #[test]
    fn la_repetition_du_clavier_ne_repond_pas_deux_fois() {
        let mut a = Appuis::default();
        assert_eq!(
            a.appui(Touche::Pareil, 0, true),
            Some(Action::Repondre(Reponse::Pareil))
        );
        assert_eq!(a.appui(Touche::Pareil, 30, true), None);
        assert_eq!(a.appui(Touche::Pareil, 60, true), None);
        assert_eq!(a.relache(Touche::Pareil), None);
        assert_eq!(
            a.appui(Touche::Pareil, 500, true),
            Some(Action::Repondre(Reponse::Pareil))
        );
    }

    #[test]
    fn les_autres_touches_agissent_a_l_appui() {
        let mut a = Appuis::default();
        assert_eq!(a.appui(Touche::PlusTard, 0, false), Some(Action::PlusTard));
        assert_eq!(a.appui(Touche::Annuler, 0, false), Some(Action::Annuler));
        assert_eq!(
            a.appui(Touche::Demarrer, 0, false),
            Some(Action::DemarrerOuReprendre)
        );
        assert_eq!(
            a.appui(Touche::Profil(3), 0, false),
            Some(Action::ActiverProfil(3))
        );
        assert_eq!(
            a.appui(Touche::Egaliseur, 0, false),
            Some(Action::BasculerEgaliseur)
        );
    }

    #[test]
    fn rendre_les_touches_arrete_une_reecoute_en_cours() {
        let mut a = Appuis::default();
        a.appui(Touche::A, 0, true);
        a.appui(Touche::B, 300, true);
        a.tic(400);
        assert_eq!(
            a.vider(),
            vec![Action::Reecouter {
                cote: Cote::A,
                appuye: false
            }]
        );
        assert_eq!(a.relache(Touche::A), None);
    }

    #[test]
    fn chaque_touche_a_une_combinaison_distincte() {
        let mut ids: Vec<u32> = TOUCHES_DE_TEST
            .iter()
            .chain(&TOUCHES_PERMANENTES)
            .map(|t| t.combinaison().id())
            .collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), 11);
    }
}
