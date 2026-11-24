//! Reprise d'un test après un arrêt brutal : l'état est reconstruit depuis le journal.

use super::{Deroulement, Effet, ParametresTest, Phase, Retour};
use crate::journal::{Entree, RaisonPause};
use crate::plan::{DuelPrevu, Sujet};
use crate::profil::Version;
use crate::zone::ReglagesVariantes;
use retro_dsp::Spectre;
use thiserror::Error;

/// Ce que le journal ne contient pas et qu'il faut redonner pour reprendre un test.
#[derive(Debug, Clone, PartialEq)]
pub struct ContexteReprise {
    /// La version de référence du test (celle dont le numéro est dans le journal). Seuls
    /// ses pas par zone servent : la courbe de référence est relue dans le journal.
    pub reference: Version,
    pub variantes: ReglagesVariantes,
    pub spectre: Spectre,
    /// Un duel de vérification tiré d'avance, pour le cas où le journal n'en contient pas
    /// encore. S'il en contient un, c'est celui du journal qui est gardé.
    pub verification: DuelPrevu,
}

/// Pourquoi un journal ne permet pas de reprendre un test.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ErreurReprise {
    #[error("le journal est vide")]
    JournalVide,
    #[error("le journal ne commence pas par le démarrage du test")]
    SansDemarrage,
    #[error("le test est déjà clos (terminé ou abandonné) : rien à reprendre")]
    TestClos,
    #[error("la version fournie (n° {fournie}) n'est pas la référence du test (n° {attendue})")]
    MauvaiseVersion { attendue: u32, fournie: u32 },
    #[error("journal incohérent à l'entrée {entree} : {raison}")]
    Incoherent { entree: usize, raison: &'static str },
}

impl Deroulement {
    /// Reconstruit un test interrompu à partir de son journal.
    ///
    /// Les réponses sont gardées, les annulations respectées, les duels passés restent
    /// passés, un duel de vérification déjà ajouté est retrouvé. Le duel interrompu sera
    /// rejoué depuis son premier passage (ce qu'on en avait entendu avant l'arrêt ne
    /// compte plus). Le temps d'écoute cumulé repart de zéro.
    ///
    /// Le test revient **en pause manuelle**. Les effets rendus sont à exécuter tout de
    /// suite : ils remettent la référence dans le moteur (une variante a pu rester active
    /// au moment de l'arrêt) et notent la pause dans le journal. `Evenement::Reprendre`
    /// relancera ensuite le duel : `Reprise`, `DuelDemarre`, `PassageDemarre`, puis la
    /// courbe du premier côté.
    pub fn reprendre_depuis(
        journal: &[Entree],
        contexte: ContexteReprise,
        maintenant_ms: u64,
    ) -> Result<(Deroulement, Vec<Effet>), ErreurReprise> {
        let (premiere, suite) = journal.split_first().ok_or(ErreurReprise::JournalVide)?;
        let Entree::TestDemarre {
            test_id,
            profil_id,
            version_reference,
            courbe_reference,
            mode,
            plan,
            reglages,
            ..
        } = premiere
        else {
            return Err(ErreurReprise::SansDemarrage);
        };
        if contexte.reference.numero != *version_reference {
            return Err(ErreurReprise::MauvaiseVersion {
                attendue: *version_reference,
                fournie: contexte.reference.numero,
            });
        }
        let mut d = Deroulement::nouveau(ParametresTest {
            test_id: test_id.clone(),
            profil_id: profil_id.clone(),
            // La courbe du journal fait foi : c'est celle que le test a fait entendre.
            reference: Version {
                courbe: courbe_reference.clone(),
                ..contexte.reference
            },
            mode: mode.clone(),
            plan: plan.clone(),
            reglages: reglages.clone(),
            variantes: contexte.variantes,
            spectre: contexte.spectre,
            verification: contexte.verification,
        });

        for (i, entree) in suite.iter().enumerate() {
            let incoherent = |raison| ErreurReprise::Incoherent {
                entree: i + 1,
                raison,
            };
            let est_courant = |duel: &u32| *duel as usize == d.indice_courant();
            match entree {
                Entree::TestDemarre { .. } => return Err(incoherent("second démarrage")),
                Entree::TestTermine { .. } | Entree::TestAbandonne { .. } => {
                    return Err(ErreurReprise::TestClos)
                }
                Entree::DuelDemarre { .. } | Entree::DuelRecommence { .. } => {
                    d.applis_duel.clear();
                }
                Entree::PassageDemarre {
                    duel,
                    passage,
                    appli_premier_plan,
                    ..
                } => {
                    if est_courant(duel) {
                        d.applis_duel.truncate(*passage as usize);
                        d.applis_duel.push(appli_premier_plan.clone());
                    }
                }
                Entree::ReponseDonnee { duel, reponse, .. } => {
                    if !est_courant(duel) || d.duel_courant().is_none() {
                        return Err(incoherent(
                            "réponse à un duel qui n'est pas le duel courant",
                        ));
                    }
                    d.applis.push(d.appli_majoritaire());
                    d.issues.push(Some(*reponse));
                    d.applis_duel.clear();
                }
                Entree::DuelPasse { duel, .. } => {
                    if !est_courant(duel) || d.duel_courant().is_none() {
                        return Err(incoherent("duel passé qui n'est pas le duel courant"));
                    }
                    d.applis.push(d.appli_majoritaire());
                    d.issues.push(None);
                    d.applis_duel.clear();
                }
                Entree::ReponseAnnulee { duel, .. } => {
                    let duel = *duel as usize;
                    if !matches!(d.issues.get(duel), Some(Some(_))) {
                        return Err(incoherent("annulation d'un duel sans réponse"));
                    }
                    d.revenir_au_duel(duel);
                }
                Entree::DuelVerificationAjoute { duel, .. } => {
                    if d.verification_ajoutee || d.duel_courant().is_some() {
                        return Err(incoherent(
                            "duel de vérification ajouté avant la fin du plan",
                        ));
                    }
                    d.plan.push(DuelPrevu {
                        sujet: Sujet::Verification,
                        ..*duel
                    });
                    d.verification_ajoutee = true;
                }
                Entree::Pause { .. } | Entree::Reprise { .. } => {}
            }
        }

        d.applis_duel.clear();
        d.phase = Phase::Pause {
            raison: RaisonPause::Manuelle,
            retour: Retour::DebutDuel,
        };
        let mut effets = Vec::new();
        d.remettre_reference(&mut effets);
        effets.push(Effet::Journaliser(Entree::Pause {
            raison: RaisonPause::Manuelle,
            horodatage_ms: maintenant_ms,
        }));
        Ok((d, effets))
    }
}
