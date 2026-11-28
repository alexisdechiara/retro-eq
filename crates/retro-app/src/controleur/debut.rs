//! Ce qui se règle au début d'un test, à côté du déroulement lui-même :
//!
//! - la **réduction de bruit** du casque (ticket 24). L'appli ne peut pas la lire : l'écran
//!   « Nouveau test » la demande, préremplie avec ce que le profil a retenu, et le contrôleur
//!   l'écrit dans le profil au démarrage du test ;
//! - les **touches mortes** (ticket 30) : F6 à F10 déjà prises par une autre appli au
//!   moment où le test les réserve. Le pont les signale, le contrôleur le dit.
//!
//! Ce fichier ne fait qu'ajouter des méthodes à `Controleur` (un deuxième bloc `impl`, ce
//! que Rust permet dans n'importe quel module enfant de celui qui définit le type).

use super::{Controleur, Issue};
use crate::textes;

impl Controleur {
    // ----- Réduction de bruit (ticket 24) -----

    /// Le choix « réduction de bruit activée ou coupée » de l'écran « Nouveau test ». Il
    /// n'est écrit dans le profil qu'au démarrage du test : choisir puis renoncer ne
    /// change rien.
    ///
    /// Le choix est noté avec l'identifiant du profil actif : s'il change de profil avant
    /// de démarrer, l'écran reprend la valeur du nouveau profil au lieu de lui imposer un
    /// choix fait pour un autre.
    pub fn choisir_reduction_bruit(&mut self, activee: bool) {
        if let Some(id) = self.actif.clone() {
            self.reduction_bruit_choisie = Some((id, activee));
        }
    }

    /// Ce que l'écran « Nouveau test » montre : le choix en cours pour le profil actif, sinon
    /// ce que le profil a retenu, sinon rien (`None` : aucune option n'est cochée).
    pub(super) fn reduction_bruit_affichee(&self) -> Option<bool> {
        let profil = self.profil_actif().ok()?;
        match &self.reduction_bruit_choisie {
            Some((id, activee)) if id == profil.id() => Some(*activee),
            _ => profil.reduction_bruit(),
        }
    }

    /// Au démarrage d'un test : le profil retient la réduction de bruit choisie (une seule
    /// écriture, et seulement si elle change). Ce n'est pas un réglage du son, donc aucune
    /// version n'est créée.
    pub(super) fn retenir_reduction_bruit(&mut self, profil_id: &str) -> Issue<()> {
        let Some((id, activee)) = self.reduction_bruit_choisie.clone() else {
            return Ok(());
        };
        if id != profil_id {
            return Ok(());
        }
        let profil = self.profil_mut(profil_id)?;
        if profil.reduction_bruit() != Some(activee) {
            profil.definir_reduction_bruit(Some(activee));
            let profil = profil.clone();
            self.depot.sauver_profil(&profil)?;
        }
        Ok(())
    }

    // ----- Touches mortes (ticket 30) -----

    /// Le pont dit quelles touches de réponse (F6 à F10) une autre appli garde pour elle
    /// pendant le test. Appelé souvent (à chaque image) : seul un changement, ou un test qui
    /// commence, déclenche une annonce. Une liste vide efface l'alerte.
    pub fn signaler_touches_mortes(&mut self, touches: Vec<String>) {
        let test_id = self.test.as_ref().map(|t| t.test_id.clone());
        let deja_dit = self.touches_mortes == touches && self.touches_mortes_annoncees == test_id;
        self.touches_mortes = touches;
        if deja_dit || !self.test_en_cours() || self.touches_mortes.is_empty() {
            return;
        }
        self.touches_mortes_annoncees = test_id;
        self.annonce = Some(super::Annonce {
            texte: textes::annonce_touches_mortes(&self.touches_mortes, self.reglages.langue),
            erreur: true,
        });
    }

    /// Les touches mortes, à montrer pendant un test (« F8, F10 »), ou une chaîne vide.
    pub(super) fn touches_mortes_affichees(&self) -> String {
        if self.test_en_cours() {
            self.touches_mortes.join(", ")
        } else {
            String::new()
        }
    }
}
