//! Moteurs audio de Retro EQ.
//!
//! Le reste de l'appli ne parle jamais à Equalizer APO directement : elle
//! passe par le trait [`Moteur`]. Ainsi on pourra un jour le remplacer par un
//! moteur maison sans toucher à l'interface (voir l'ADR 0002).

#![cfg_attr(test, allow(clippy::unwrap_used))]

mod equalizer_apo;
#[cfg(windows)]
mod registre;
mod simule;
mod texte;

use std::io;
use std::path::PathBuf;

use retro_dsp::Courbe;
use thiserror::Error;

pub use equalizer_apo::EqualizerApo;
pub use simule::{Appel, MoteurSimule};
pub use texte::texte_config;

/// Ce qui peut mal tourner quand on pilote un moteur.
#[derive(Debug, Error)]
pub enum ErreurMoteur {
    /// Le dossier de configuration n'existe pas : Equalizer APO n'est
    /// probablement pas installé (ou pas là où on le cherche).
    #[error("le dossier de configuration d'Equalizer APO est introuvable : {0}")]
    DossierIntrouvable(PathBuf),
    /// Une opération sur un fichier a échoué (droits d'écriture, disque plein...).
    #[error("{action} : {source}")]
    Io {
        /// Ce qu'on essayait de faire, en français, pour le journal.
        action: String,
        #[source]
        source: io::Error,
    },
    /// La courbe contient un nombre inutilisable (NaN, infini, fréquence <= 0...).
    #[error("courbe invalide : {0}")]
    CourbeInvalide(String),
    /// Le registre Windows n'a pas pu être lu.
    #[error("lecture du registre impossible : {0}")]
    Registre(String),
    /// Échec demandé par un test sur le moteur simulé.
    #[error("échec simulé : {0}")]
    Simule(String),
}

/// Où en est le moteur sur cette machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EtatMoteur {
    /// Le moteur n'est pas installé.
    Absent,
    /// Installé, mais Retro EQ n'est pas branché dessus.
    Inactif,
    /// Installé et branché sur Retro EQ.
    Actif,
}

/// Un périphérique de lecture vu par Windows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SortieAudio {
    /// Identifiant technique stable.
    pub id: String,
    /// Nom à afficher.
    pub nom: String,
    /// Le moteur est-il déjà installé sur cette sortie ?
    pub moteur_installe: bool,
}

/// Ce qui applique réellement une courbe au son.
///
/// `Send` car l'appli le range dans une `Box<dyn Moteur>` partagée entre fils.
pub trait Moteur: Send {
    /// Applique la courbe au son.
    fn appliquer(&mut self, courbe: &Courbe) -> Result<(), ErreurMoteur>;
    /// Égaliseur à plat, moteur toujours en place.
    fn couper(&mut self) -> Result<(), ErreurMoteur>;
    /// Absent, Inactif ou Actif.
    fn etat(&self) -> EtatMoteur;
    /// Les sorties audio connues, avec ou sans moteur.
    fn sorties(&self) -> Result<Vec<SortieAudio>, ErreurMoteur>;
    /// Rend la configuration d'origine.
    fn retirer(&mut self) -> Result<(), ErreurMoteur>;
    /// Après `retirer` : l'ancienne copie de la configuration que le moteur a
    /// gardée sur le disque parce qu'elle ne correspondait plus à la
    /// configuration de l'utilisateur, pour que l'appli lui dise où elle est.
    /// `None` quand il n'y en a pas (cas général, et moteurs sans fichiers).
    fn copie_de_securite(&self) -> Option<PathBuf> {
        None
    }
}
