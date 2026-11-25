//! Domaine de Retro EQ : zones, profils et versions, plan de test, confiance, résultat,
//! journal et stockage, import et export.
//!
//! Rien ici ne dépend d'une API système autre que les fichiers, et le crate ne lit jamais
//! l'horloge : l'appelant passe les horodatages (sauf [`hasard::HasardReel`], qui s'en sert
//! pour sa graine).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

pub mod autoeq_base;
pub mod confiance;
pub mod depot;
pub mod deroulement;
pub mod egaliseur;
pub mod formats;
pub mod hasard;
pub mod journal;
pub mod plan;
pub mod presets;
pub mod profil;
pub mod reglages;
pub mod resultat;
pub mod zone;

pub use confiance::{risque_hasard, Confiance};
pub use depot::{Depot, ErreurDepot, EtatMoteurSauve, FichierIgnore, ProfilsListes};
pub use deroulement::{
    ContexteReprise, Deroulement, DuelFini, Effet, ErreurReprise, EtatTest, Evenement,
    ParametresTest, VueTest,
};
pub use egaliseur::{courbe_10_bandes, FadersEtSupplementaires, BANDES_HZ, Q_BANDES};
pub use hasard::{Hasard, HasardGraine, HasardReel};
pub use journal::{Entree, Journal, LectureJournal, RaisonPause};
pub use plan::{
    duel_verification, plan, plan_avec_historique, retirer_duel, Cote, Couple, DuelPrevu,
    HistoriquePlan, Mode, Preference, Reponse, Sujet,
};
pub use presets::{presets, Preset};
pub use profil::{ErreurProfil, Origine, Profil, Version};
pub use reglages::{Langue, Navigation, ReglagesAppli, ReglagesTest};
pub use resultat::{
    calculer_resultat, BilanControles, BilanDuels, ComptesSens, PreferenceAppli, ResultatTest,
    ResultatZone,
};
pub use zone::{filtre_de_zone, variante, ReglagesVariantes, Sens, Zone};
