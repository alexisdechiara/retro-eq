//! Les types partagés par tout le workspace : un filtre, et une courbe faite de filtres.

use serde::{Deserialize, Serialize};

/// La forme d'un filtre d'égaliseur.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TypeFiltre {
    /// Cloche centrée sur une fréquence (« PK » chez Equalizer APO).
    Cloche,
    /// Plateau qui monte ou baisse tout ce qui est sous la fréquence (« LSC »).
    PlateauBas,
    /// Plateau qui monte ou baisse tout ce qui est au-dessus de la fréquence (« HSC »).
    PlateauHaut,
}

/// Un filtre d'égaliseur paramétrique.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Filtre {
    pub type_filtre: TypeFiltre,
    pub frequence_hz: f64,
    pub gain_db: f64,
    /// Finesse du filtre : plus Q est grand, plus le filtre est étroit.
    pub q: f64,
}

/// Un réglage d'égaliseur complet : un préampli et une liste de filtres.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Courbe {
    pub preampli_db: f64,
    pub filtres: Vec<Filtre>,
}
