//! Traitement du signal de Retro EQ : filtres, courbes de réponse, volume perçu, analyse.
//! Rien ici ne dépend de Windows.

pub mod analyse;
pub mod biquad;
mod courbe;
mod spectre;
mod volume;

pub use courbe::{frequences_log, Courbe, Filtre, TypeFiltre};
pub use spectre::Spectre;
pub use volume::{
    compensation_db, egaliser_volumes, ponderation_k_db, volume_percu_db, VolumeIntegre,
    PORTE_SILENCE_DB,
};
