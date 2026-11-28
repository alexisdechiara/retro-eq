//! Le duel d'essai du premier lancement (ticket 25) : un seul duel, référence contre
//! référence avec 6 dB de graves en plus (`retro_core::essai`), pour vérifier de bout en
//! bout que le son change sur le casque.
//!
//! Il passe par **la même machine à états qu'un vrai test** (`Deroulement`) et par les
//! mêmes chemins du contrôleur (`tic`, `repondre`, `mettre_en_pause`, `arreter_test`,
//! `avant_de_quitter`…) : la référence est donc remise à chaque pause, à la fin, à
//! l'abandon et en quittant, exactement comme pour un vrai duel. Trois différences :
//!
//! - aucun journal n'est écrit. Après un arrêt brutal, `etat-moteur.json` (qui ne contient
//!   jamais une variante) remet la référence au démarrage, et il n'y a rien à reprendre :
//!   l'essai dure une minute, on le refait ;
//! - à la fin, aucune version n'est ajoutée et le dernier résultat ne bouge pas : seul un
//!   [`BilanEssai`] est gardé, le temps de la session ;
//! - la question posée est « laquelle a plus de graves ? » plutôt que « laquelle
//!   préfères-tu ? » : c'est l'interface qui la formule, d'après `VueDuels::essai`.

use super::{Controleur, ErreurControleur, TestEnCours, VueEssai};
use crate::textes;
use retro_core::{
    duel_d_essai, duel_verification, BilanEssai, Deroulement, EtatTest, Evenement, ParametresTest,
    ReglagesTest, ReglagesVariantes,
};

/// La durée d'un passage de l'essai : la plus courte permise (10 s), pour que tout l'essai
/// tienne en une minute. Il vérifie que le son change, pas une préférence fine.
const INTERVALLE_ESSAI_MS: u32 = 10_000;

impl Controleur {
    /// Le duel d'essai est-il lancé et pas fini ?
    pub fn essai_en_cours(&self) -> bool {
        self.test_en_cours() && self.test.as_ref().is_some_and(|t| t.essai)
    }

    /// Lance le duel d'essai sur la version active du profil actif. Rend vrai si l'essai
    /// tourne après l'appel. Un essai déjà lancé et en pause est repris, pas recommencé.
    /// Refusé (et dit) pendant un vrai test, sans accord pour écrire dans le moteur, ou avec
    /// un brouillon d'égaliseur en cours.
    pub fn demarrer_essai(&mut self, t: u64) -> bool {
        if self.essai_en_cours() {
            if matches!(self.etat_test(), Some(EtatTest::EnPause { .. })) {
                self.envoyer(Evenement::Reprendre, t);
            }
            return true;
        }
        self.tenter(|c| {
            c.exiger_pas_de_test()?;
            if c.options.moteur_absent {
                return Err(ErreurControleur::MoteurAbsent);
            }
            if !c.reglages.moteur_autorise {
                return Err(ErreurControleur::MoteurNonAutorise);
            }
            if c.brouillon.is_some() {
                return Err(ErreurControleur::BrouillonEnCours);
            }
            let profil = c.profil_actif()?;
            let (profil_id, reference) = (profil.id().to_string(), profil.version_active().clone());
            let mut hasard = c.hasard();
            let essai = duel_d_essai(&reference.courbe, &mut hasard);
            // Le mode « profil contre profil » n'ajoute jamais de duel de vérification ;
            // `ParametresTest` en demande un quand même, qui ne sera pas joué.
            let verification = duel_verification(&mut hasard);
            let test_id = format!("essai-{t}");
            let deroulement = Deroulement::nouveau(ParametresTest {
                test_id: test_id.clone(),
                profil_id: profil_id.clone(),
                reference: reference.clone(),
                mode: essai.mode.clone(),
                plan: essai.plan,
                reglages: ReglagesTest::nouveau(INTERVALLE_ESSAI_MS, c.reglages.bip),
                variantes: ReglagesVariantes::default(),
                spectre: c.spectre.clone(),
                verification,
            });
            // Comme un vrai test, l'essai se fait égaliseur en marche.
            if c.reglages.egaliseur_coupe {
                c.reglages.egaliseur_coupe = false;
                c.sauver_reglages();
            }
            c.bilan_essai = None;
            c.test = Some(TestEnCours {
                deroulement,
                journal: None,
                essai: true,
                test_id,
                profil_id,
                reference,
                mode: essai.mode,
                autre: None,
                autre_version: None,
            });
            Ok(None)
        });
        if self.etat_test() == Some(EtatTest::Pret) {
            self.envoyer(Evenement::Demarrer, t);
        }
        self.essai_en_cours()
    }

    /// Appelé par `conclure` quand le test fini était l'essai : on garde le bilan, et rien
    /// d'autre. La référence a déjà été remise par la machine à états (effet `Appliquer`
    /// juste avant `Termine`).
    pub(super) fn conclure_essai(&mut self) {
        let dernier = self
            .test
            .as_ref()
            .and_then(|t| t.deroulement.vue(0).historique.last().copied());
        self.bilan_essai = dernier.as_ref().map(BilanEssai::depuis);
        // Le test fini n'a plus rien à montrer : l'écran Duels redevient vide, et le
        // prochain vrai test partira de zéro.
        self.test = None;
    }

    /// L'écran du duel d'essai.
    pub fn vue_essai(&self) -> VueEssai {
        let l = self.reglages.langue;
        VueEssai {
            en_cours: self.essai_en_cours(),
            issue: self.bilan_essai.map(|b| b.issue),
            detail: self
                .bilan_essai
                .map(|b| textes::detail_essai(&b, l))
                .unwrap_or_default(),
            diagnostic: self.bilan_essai.is_some_and(|b| !b.son_change()),
        }
    }
}
