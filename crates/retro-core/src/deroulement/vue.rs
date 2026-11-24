//! Ce que l'interface montre d'un test en cours : une photo, calculée à la demande.

use super::{Deroulement, Phase, Retour};
use crate::journal::RaisonPause;
use crate::plan::{Cote, Preference, Reponse, Sujet};

/// L'état d'un test, vu de l'extérieur.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EtatTest {
    /// Créé, pas encore démarré.
    Pret,
    /// Un passage joue. `duel` (indice dans le plan) et `passage` (0 à 5) partent de 0,
    /// comme dans le journal.
    EnEcoute {
        duel: u32,
        passage: u32,
    },
    /// Les 6 passages sont finis, la référence est remise, on attend A, Pareil ou B.
    AttenteReponse,
    EnPause {
        raison: RaisonPause,
    },
    /// Fini, résultat rendu.
    Termine,
    /// Abandonné, sans résultat.
    Abandonne,
}

/// Un duel clos, tel que l'interface peut le montrer dans l'historique.
///
/// C'est le seul endroit où l'on apprend qui était la référence : uniquement pour les
/// duels déjà clos.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DuelFini {
    /// Numéro du duel, à partir de 1.
    pub numero: u32,
    pub sujet: Sujet,
    /// La réponse donnée ; `None` si le duel a été passé.
    pub reponse: Option<Reponse>,
    /// Le côté qui jouait la référence.
    pub reference: Cote,
    /// La réponse traduite (variante, référence ou aucune) ; `None` si le duel a été passé.
    pub preference: Option<Preference>,
}

/// Tout ce que l'interface affiche pendant un test.
///
/// Le test reste à l'aveugle : cette structure ne contient ni le sujet du duel en cours ni
/// qui y joue la référence. Il n'y a donc aucun moyen de les lire depuis l'interface, même
/// par erreur. Seule exception voulue : `est_verification`.
#[derive(Debug, Clone, PartialEq)]
pub struct VueTest {
    pub etat: EtatTest,
    /// Numéro du duel en cours, à partir de 1 (0 seulement si le plan est vide).
    pub duel: u32,
    /// Nombre total de duels. Il **augmente de un** quand un duel de vérification est
    /// ajouté à la fin du plan (et revient en arrière si une annulation le retire).
    pub total: u32,
    /// Le côté qu'on entend : pendant un passage, ou pendant une réécoute.
    pub cote_actif: Option<Cote>,
    /// Passages de A entièrement écoutés dans le duel en cours (0 à 3).
    pub passages_a: u8,
    /// Passages de B entièrement écoutés dans le duel en cours (0 à 3).
    pub passages_b: u8,
    /// Avancement du passage en cours, de 0.0 à 1.0 (0.0 hors écoute).
    pub progression: f64,
    /// Temps restant avant la prochaine bascule ; `None` hors écoute.
    pub ms_avant_bascule: Option<u64>,
    /// Le duel en cours est le duel de vérification (référence contre nouvelle version).
    pub est_verification: bool,
    /// Une pause a déjà été conseillée pendant ce test.
    pub pause_conseillee: bool,
    /// Les duels clos, du premier au dernier.
    pub historique: Vec<DuelFini>,
}

impl Deroulement {
    /// La photo du test à `maintenant_ms`. Ne change rien à l'état : on peut l'appeler à
    /// chaque image.
    pub fn vue(&self, maintenant_ms: u64) -> VueTest {
        let courant = self.indice_courant();
        let total = self.plan.len();
        let duel = self.duel_courant();
        let en_jeu = !matches!(self.phase, Phase::Pret | Phase::Termine | Phase::Abandonne);

        let etat = match self.phase {
            Phase::Pret => EtatTest::Pret,
            Phase::Ecoute { passage, .. } => EtatTest::EnEcoute {
                duel: courant as u32,
                passage: passage as u32,
            },
            Phase::Attente { .. } => EtatTest::AttenteReponse,
            Phase::Pause { raison, .. } => EtatTest::EnPause { raison },
            Phase::Termine => EtatTest::Termine,
            Phase::Abandonne => EtatTest::Abandonne,
        };

        // Nombre de passages entièrement joués dans le duel en cours.
        let passages_finis = match self.phase {
            Phase::Ecoute { passage, .. } => passage,
            Phase::Attente { .. } => usize::MAX,
            Phase::Pause { retour, .. } => match retour {
                Retour::DebutDuel => 0,
                Retour::Passage(passage) => passage,
                Retour::Attente => usize::MAX,
            },
            _ => 0,
        };
        let finis_de = |cote: Cote| {
            duel.map_or(0, |d| {
                d.ordre_passages
                    .iter()
                    .take(passages_finis)
                    .filter(|&&c| c == cote)
                    .count() as u8
            })
        };

        let (cote_actif, progression, ms_avant_bascule) = match self.phase {
            Phase::Ecoute { passage, debut_ms } => {
                let intervalle = self.intervalle_ms();
                let ecoule = maintenant_ms.saturating_sub(debut_ms).min(intervalle);
                let progression = if intervalle == 0 {
                    1.0
                } else {
                    ecoule as f64 / intervalle as f64
                };
                (
                    duel.and_then(|d| d.ordre_passages.get(passage).copied()),
                    progression,
                    Some(intervalle - ecoule),
                )
            }
            Phase::Attente { reecoute } => (reecoute, 0.0, None),
            _ => (None, 0.0, None),
        };

        let historique = self
            .plan
            .iter()
            .zip(&self.issues)
            .enumerate()
            .map(|(i, (d, issue))| DuelFini {
                numero: i as u32 + 1,
                sujet: d.sujet,
                reponse: *issue,
                reference: d.cote_reference(),
                preference: issue.map(|r| d.preference(r)),
            })
            .collect();

        VueTest {
            etat,
            duel: (courant + 1).min(total) as u32,
            total: total as u32,
            cote_actif,
            passages_a: finis_de(Cote::A),
            passages_b: finis_de(Cote::B),
            progression,
            ms_avant_bascule,
            est_verification: en_jeu && duel.is_some_and(|d| d.sujet == Sujet::Verification),
            pause_conseillee: self.pause_conseillee,
            historique,
        }
    }
}
