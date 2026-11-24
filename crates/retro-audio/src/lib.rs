//! Capture audio et observation du système pour Retro EQ.
//!
//! Chaque capacité est un trait (`Capture`, `PremierPlan`, ...). Sous Windows, `windows_impl`
//! les réalise avec les API du système ; ailleurs (et dans les tests), `simule` fournit des
//! versions factices. L'appli passe par `systeme` et n'a donc aucun `cfg` à écrire.

use std::sync::mpsc::Receiver;

pub mod mesure;
pub mod simule;
pub mod systeme;
#[cfg(windows)]
pub mod windows_impl;

pub use mesure::Mesure;

/// Ce qui peut mal tourner côté audio.
#[derive(Debug, thiserror::Error)]
pub enum ErreurAudio {
    /// Aucune sortie audio (ou aucun accès) pour le moment.
    #[error("périphérique audio indisponible : {0}")]
    Indisponible(String),
    /// Le format du flux n'est pas géré (ni flottant 32 bits ni entier 16/24/32 bits).
    #[error("format audio non géré : {0}")]
    FormatInconnu(String),
    /// Erreur renvoyée par le système.
    #[error("erreur du système audio : {0}")]
    Systeme(String),
}

/// Un bloc de son mono (moyenne des canaux), de l'ordre de 20 à 50 ms.
#[derive(Debug, Clone, PartialEq)]
pub struct Trame {
    pub echantillons: Vec<f32>,
    pub frequence_echantillonnage: u32,
}

/// Une sortie audio du système (casque, enceintes, dongle USB...).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InfoSortie {
    /// Identifiant stable du système : change si le casque passe sur un autre port USB.
    pub id: String,
    /// Nom lisible, celui du panneau de son de Windows.
    pub nom: String,
    pub par_defaut: bool,
}

/// Écoute de ce que joue la sortie par défaut. `Send` pour pouvoir vivre dans l'état de l'appli
/// pendant que le son est lu par un fil d'arrière-plan.
pub trait Capture: Send {
    /// Lance la capture et rend le canal des trames. Le fil d'arrière-plan s'arrête quand le
    /// récepteur est lâché, quand `arreter` est appelé, ou sur erreur (le canal se ferme alors :
    /// à l'appli de relancer).
    fn demarrer(&mut self) -> Result<Receiver<Trame>, ErreurAudio>;
    /// Arrête la capture et attend la fin du fil. Sans effet si rien ne tourne.
    fn arreter(&mut self);
}

/// Quelle appli est au premier plan.
pub trait PremierPlan: Send {
    /// Nom de l'exécutable sans chemin ni extension (ex. `VALORANT-Win64-Shipping`).
    fn appli(&self) -> Option<String>;
}

/// Y a-t-il un appel vocal en cours ?
pub trait DetecteurAppel: Send {
    fn appel_en_cours(&self) -> bool;
}

/// Un jeu occupe-t-il l'écran en plein écran exclusif (où aucune fenêtre ne s'affiche) ?
pub trait PleinEcran: Send {
    fn exclusif(&self) -> bool;
}

/// Les sorties audio présentes, pour repérer un nouveau casque.
pub trait SortiesAudio: Send {
    fn sortie_par_defaut(&self) -> Option<InfoSortie>;
    fn sorties(&self) -> Vec<InfoSortie>;
}

/// Petit son de confirmation, utile quand aucune fenêtre ne peut s'afficher.
pub trait Bip: Send {
    /// Ne bloque jamais l'appelant.
    fn bip(&self);
}
