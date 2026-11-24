//! Traitement du signal de Retro EQ : filtres, courbes de réponse, volume perçu, analyse.
//! Rien ici ne dépend de Windows.

mod courbe;

pub use courbe::{Courbe, Filtre, TypeFiltre};
