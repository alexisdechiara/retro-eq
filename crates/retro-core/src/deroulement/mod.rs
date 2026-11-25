//! Le déroulement d'un test : la machine à états qui joue les duels.
//!
//! **Des événements en entrée, des effets en sortie.** Le déroulement ne parle jamais au
//! moteur audio, au disque ni à l'horloge : l'appli lui dit ce qui se passe
//! ([`Evenement`], avec l'heure en ms) et il répond par la liste de ce qu'il faut faire
//! ([`Effet`] : appliquer une courbe, écrire une ligne du journal, afficher la question…).
//! C'est le même principe qu'un *reducer* côté web : comme rien n'est exécuté ici, toute la
//! logique se teste sans casque, sans Windows et sans attendre 20 secondes par passage, et
//! la même suite d'événements donne toujours la même suite d'effets.
//!
//! Deux règles se tiennent l'une l'autre (ADR-0012) :
//!
//! - **À l'aveugle** : pendant un duel (écoute et attente de la réponse), on n'entend que
//!   les deux côtés mis au même volume, jamais la référence telle quelle. Sinon le son
//!   qui change, ou pas, au moment de la question trahirait qui est qui.
//! - **Réversible** : hors d'un duel (pause, fin, abandon), le dernier
//!   [`Effet::Appliquer`] émis porte la référence donnée au constructeur, avec son propre
//!   préampli. Un côté ne reste donc actif sans rien faire que le temps du délai de
//!   réponse (2 minutes par défaut), après quoi le test se met en pause tout seul.
//!
//! Remettre la référence pendant un duel peut trahir un côté : tout duel interrompu par
//! une pause est donc rejoué avec un **nouveau tirage** (qui est A, ordre des passages).

mod reprise;
mod vue;

#[cfg(test)]
mod tests;

pub use reprise::{ContexteReprise, ErreurReprise};
pub use vue::{DuelFini, EtatTest, VueTest};

use crate::journal::{Entree, RaisonPause};
use crate::hasard::HasardGraine;
use crate::plan::{retirer_duel, Cote, DuelPrevu, Mode, Reponse, Sujet};
use crate::profil::Version;
use crate::reglages::ReglagesTest;
use crate::resultat::{calculer_resultat, ResultatTest};
use crate::zone::{variante, ReglagesVariantes};
use retro_dsp::{egaliser_volumes, Courbe, Spectre};

/// Ce que l'appli signale au déroulement. Un événement qui n'a pas de sens dans l'état
/// courant (une réponse pendant une pause, « reprendre » en pleine écoute…) est ignoré :
/// aucun effet, jamais d'erreur. L'appli peut donc transmettre les touches sans filtrer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Evenement {
    /// Lance le test (seulement depuis l'état `Pret`).
    Demarrer,
    /// Le temps passe : à envoyer régulièrement (plusieurs fois par seconde suffit).
    Tic,
    /// La réponse au duel en cours. Acceptée pendant l'écoute comme pendant l'attente.
    Reponse(Reponse),
    /// « Plus tard » : pause, et le duel en cours sera rejoué depuis son premier passage.
    PlusTard,
    /// Annule la dernière réponse donnée et rejoue ce duel depuis son premier passage.
    ///
    /// Deux limites voulues, pour garder un plan linéaire : la toute dernière réponse d'un
    /// test ne peut pas être annulée quand aucun duel de vérification ne suit (le test
    /// est déjà terminé) ; et les duels *passés* après la réponse annulée redeviennent à
    /// jouer, puisqu'on revient en arrière jusqu'à elle.
    Annuler,
    /// Pause demandée par l'utilisateur. Le duel en cours sera rejoué, avec un nouveau tirage.
    Pause,
    /// Fin de la pause, quelle qu'en soit la raison. Ignoré tant qu'un appel est en cours.
    Reprendre,
    /// Le duel en cours est noté « passé », sans réponse.
    PasserDuel,
    /// Le duel en cours recommence depuis son premier passage, avec un nouveau tirage.
    RecommencerDuel,
    /// Touche de réécoute enfoncée (`appuye: true`) ou relâchée, pendant l'attente de la
    /// réponse. L'appui fait entendre ce côté, qui **reste** en place au relâchement (on
    /// ne repasse pas par la référence). Chaque appui relance le délai de réponse. Ce
    /// n'est jamais une réponse : l'appli distingue l'appui court (elle envoie `Reponse`)
    /// de l'appui long (elle envoie `Reecouter`).
    Reecouter { cote: Cote, appuye: bool },
    /// Un appel vocal commence (`true`) ou se termine (`false`). Seuls les changements
    /// comptent : répéter la même valeur ne fait rien.
    AppelEnCours(bool),
    /// L'appli au premier plan a changé. Une chaîne vide veut dire « inconnue ».
    PremierPlan(String),
    /// Abandonne le test : la référence est remise, aucun résultat n'est calculé.
    Abandonner,
}

/// Ce que l'appli doit faire, dans l'ordre de la liste rendue par [`Deroulement::traiter`].
#[derive(Debug, Clone, PartialEq)]
pub enum Effet {
    /// Donner cette courbe au moteur. `cote` vaut `Some` pendant un passage ou une
    /// réécoute, et `None` quand c'est la référence qui est remise.
    Appliquer { cote: Option<Cote>, courbe: Courbe },
    /// Ajouter cette ligne au journal du test, sur le disque.
    Journaliser(Entree),
    /// Les 6 passages sont finis : afficher la question « A, Pareil ou B ? ». Le dernier
    /// côté entendu reste en place.
    DemanderReponse,
    /// Retirer la question de l'écran.
    MasquerQuestion,
    /// Bip court qui confirme qu'une réponse a été prise (réglage « bip »).
    Bip,
    /// 40 minutes d'écoute cumulée : proposer une pause. Émis une seule fois par test.
    ConseillerPause,
    /// Le test est fini, voici son résultat.
    Termine(ResultatTest),
}

/// Tout ce qu'il faut pour créer un [`Deroulement`].
///
/// Le déroulement fabrique lui-même la courbe de chaque côté d'un duel : il lui faut donc
/// la version de référence (sa courbe et ses pas par zone), le mode (pour la courbe de
/// l'autre profil) et un spectre (pour mettre A et B au même volume).
#[derive(Debug, Clone, PartialEq)]
pub struct ParametresTest {
    pub test_id: String,
    pub profil_id: String,
    /// La version de référence. `reference.courbe` est **la** référence du test, celle qui
    /// est remise à chaque sortie de l'écoute ; `reference.pas_par_zone` donne la taille
    /// des variantes.
    pub reference: Version,
    pub mode: Mode,
    /// Les duels à jouer, tels que rendus par [`crate::plan::plan`].
    pub plan: Vec<DuelPrevu>,
    pub reglages: ReglagesTest,
    pub variantes: ReglagesVariantes,
    /// Le spectre de ce qui joue (à défaut : `Spectre::rose`). Voir [`Deroulement::maj_spectre`].
    pub spectre: Spectre,
    /// Le duel de vérification, tiré d'avance avec [`crate::plan::duel_verification`]. Il
    /// n'est joué que si le test retient un changement. Le tirer d'avance évite au
    /// déroulement de porter une source de hasard : il reste entièrement déterministe.
    pub verification: DuelPrevu,
}

/// Une pause plus longue que cela repose l'oreille : le compteur d'écoute qui mène au
/// conseil de pause repart de zéro (« 40 minutes d'écoute sans pause »).
pub const PAUSE_QUI_REPOSE_MS: u64 = 5 * 60 * 1000;

/// L'état interne. `EtatTest` en est la version publique, sans les détails.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Pret,
    Ecoute { passage: usize, debut_ms: u64 },
    /// La question est posée. `applique` est le côté en place ; `depuis_ms` le début du
    /// délai de réponse (relancé par chaque réécoute).
    Attente { applique: Cote, depuis_ms: u64 },
    /// À la reprise, le duel courant repart toujours de son premier passage.
    Pause { raison: RaisonPause, depuis_ms: u64 },
    Termine,
    Abandonne,
}

/// La machine à états d'un test. Voir la documentation du module.
#[derive(Debug, Clone, PartialEq)]
pub struct Deroulement {
    test_id: String,
    profil_id: String,
    /// La version de référence ; `version.courbe` est la référence du test.
    version: Version,
    mode: Mode,
    /// Le plan tel qu'il est réellement joué : les duels retirés y remplacent ceux
    /// d'origine, et le duel de vérification vient en dernier quand `verification_ajoutee`.
    plan: Vec<DuelPrevu>,
    /// Vrai quand le duel courant a été interrompu : il sera retiré avant d'être rejoué.
    a_retirer: bool,
    /// Graine des nouveaux tirages, dérivée de l'identifiant du test.
    graine: u64,
    /// Nombre de tirages déjà faits : le n-ième tirage est toujours le même pour un test
    /// donné, ce qui garde le déroulement déterministe et reconstructible.
    tirages: u64,
    verification_prevue: DuelPrevu,
    verification_ajoutee: bool,
    reglages: ReglagesTest,
    variantes: ReglagesVariantes,
    spectre: Spectre,
    /// L'issue de chaque duel clos : `Some(réponse)` ou `None` s'il a été passé. Sa
    /// longueur est l'indice du duel courant.
    issues: Vec<Option<Reponse>>,
    /// L'appli attribuée à chaque duel clos (même longueur que `issues`).
    applis: Vec<Option<String>>,
    phase: Phase,
    /// Les deux côtés du duel courant, mis au même volume : `(référence, variante)`.
    /// Calculés au premier passage puis gardés jusqu'à la fin du duel.
    courbes_duel: Option<(Courbe, Courbe)>,
    /// L'appli au premier plan au début de chaque passage du duel courant.
    applis_duel: Vec<Option<String>>,
    premier_plan: Option<String>,
    appel: bool,
    /// Écoute cumulée des passages déjà fermés (ms). Les pauses n'y entrent pas.
    ecoute_ms: u64,
    pause_conseillee: bool,
}

impl Deroulement {
    /// Un test prêt à démarrer. Rien n'est émis avant [`Evenement::Demarrer`].
    pub fn nouveau(parametres: ParametresTest) -> Deroulement {
        let ParametresTest {
            test_id,
            profil_id,
            reference,
            mode,
            plan,
            reglages,
            variantes,
            spectre,
            verification,
        } = parametres;
        let graine = graine_de(&test_id);
        Deroulement {
            test_id,
            profil_id,
            version: reference,
            mode,
            plan,
            a_retirer: false,
            graine,
            tirages: 0,
            verification_prevue: verification,
            verification_ajoutee: false,
            reglages,
            variantes,
            spectre,
            issues: Vec::new(),
            applis: Vec::new(),
            phase: Phase::Pret,
            courbes_duel: None,
            applis_duel: Vec::new(),
            premier_plan: None,
            appel: false,
            ecoute_ms: 0,
            pause_conseillee: false,
        }
    }

    /// Remplace le spectre qui sert à mettre A et B au même volume.
    ///
    /// Il sert à partir du prochain début de duel : les deux courbes d'un duel sont
    /// calculées ensemble et gardées jusqu'à sa fin, sinon A et B ne seraient plus
    /// comparables d'un passage à l'autre.
    pub fn maj_spectre(&mut self, spectre: Spectre) {
        self.spectre = spectre;
    }

    /// Traite un événement arrivé à `maintenant_ms` et rend les effets à exécuter, dans
    /// l'ordre. Une liste vide veut dire que l'événement a été ignoré (ou qu'il n'y avait
    /// rien à faire, comme un `Tic` en cours de passage).
    pub fn traiter(&mut self, evt: Evenement, maintenant_ms: u64) -> Vec<Effet> {
        let t = maintenant_ms;
        let mut effets = Vec::new();
        match evt {
            Evenement::Demarrer => self.demarrer(t, &mut effets),
            Evenement::Tic => self.tic(t, &mut effets),
            Evenement::Reponse(reponse) => self.repondre(reponse, t, &mut effets),
            Evenement::PlusTard => {
                if self.en_duel() {
                    self.mettre_en_pause(RaisonPause::PlusTard, t, &mut effets);
                }
            }
            Evenement::Annuler => self.annuler(t, &mut effets),
            Evenement::Pause => self.pause_manuelle(t, &mut effets),
            Evenement::Reprendre => {
                // Pendant un appel, on ne relance pas l'écoute par-dessus la conversation.
                if matches!(self.phase, Phase::Pause { .. }) && !self.appel {
                    self.reprendre(t, &mut effets);
                }
            }
            Evenement::PasserDuel => self.passer_duel(t, &mut effets),
            Evenement::RecommencerDuel => self.recommencer_duel(t, &mut effets),
            Evenement::Reecouter { cote, appuye } => self.reecouter(cote, appuye, t, &mut effets),
            Evenement::AppelEnCours(en_cours) => self.appel_en_cours(en_cours, t, &mut effets),
            Evenement::PremierPlan(nom) => {
                self.premier_plan = Some(nom).filter(|n| !n.is_empty());
            }
            Evenement::Abandonner => self.abandonner(t, &mut effets),
        }
        self.conseiller_pause(t, &mut effets);
        effets
    }

    // ----- Petits accès -----

    fn delai_reponse_ms(&self) -> u64 {
        u64::from(self.reglages.delai_reponse_ms())
    }

    fn intervalle_ms(&self) -> u64 {
        u64::from(self.reglages.intervalle_ms())
    }

    /// Indice du duel courant dans le plan (celui qui se joue ou va se jouer).
    fn indice_courant(&self) -> usize {
        self.issues.len()
    }

    fn duel_courant(&self) -> Option<DuelPrevu> {
        self.plan.get(self.indice_courant()).copied()
    }

    /// Vrai pendant l'écoute ou l'attente de la réponse : un duel est « sur la table ».
    fn en_duel(&self) -> bool {
        matches!(self.phase, Phase::Ecoute { .. } | Phase::Attente { .. })
    }

    fn en_attente(&self) -> bool {
        matches!(self.phase, Phase::Attente { .. })
    }

    /// Nombre de duels du plan d'origine, sans le duel de vérification.
    fn duels_du_plan(&self) -> usize {
        self.plan
            .len()
            .saturating_sub(usize::from(self.verification_ajoutee))
    }

    // ----- Courbes -----

    /// L'effet qui remet la référence : la courbe de l'utilisateur, telle quelle.
    fn remettre_reference(&self, effets: &mut Vec<Effet>) {
        effets.push(Effet::Appliquer {
            cote: None,
            courbe: self.version.courbe.clone(),
        });
    }

    /// Les deux côtés d'un duel, mis au même volume perçu : `(référence, variante)`.
    fn calculer_courbes(&self, duel: &DuelPrevu) -> (Courbe, Courbe) {
        let reference = &self.version.courbe;
        let autre = match duel.sujet {
            Sujet::Zone { zone, sens } => {
                let pas = self
                    .version
                    .pas_par_zone
                    .get(zone.index())
                    .copied()
                    .unwrap_or(self.variantes.pas_initial_db);
                variante(reference, zone, sens, pas)
            }
            Sujet::Controle => {
                // Les deux côtés sont la même courbe, à l'octet près : on ne laisse pas un
                // arrondi du calcul de volume créer une différence qui n'existe pas.
                let (r, _) = egaliser_volumes(reference, reference, &self.spectre);
                return (r.clone(), r);
            }
            Sujet::AutreProfil => match &self.mode {
                Mode::ProfilContreProfil { autre, .. } => autre.clone(),
                // Plan incohérent avec le mode : le duel se comporte comme un contrôle.
                _ => reference.clone(),
            },
            Sujet::Verification => self.resultat().nouvelle_courbe,
        };
        egaliser_volumes(reference, &autre, &self.spectre)
    }

    /// La courbe à faire entendre pour `cote` dans ce duel.
    fn courbe_du_cote(&mut self, duel: &DuelPrevu, cote: Cote) -> Courbe {
        let courbes = match self.courbes_duel.take() {
            Some(c) => c,
            None => self.calculer_courbes(duel),
        };
        let courbe = if cote == duel.cote_reference() {
            courbes.0.clone()
        } else {
            courbes.1.clone()
        };
        self.courbes_duel = Some(courbes);
        courbe
    }

    // ----- Résultat -----

    /// Le résultat d'après les réponses connues (et la vérification si elle a été répondue).
    fn resultat(&self) -> ResultatTest {
        let n = self.duels_du_plan();
        let plan = self.plan.get(..n).unwrap_or(&self.plan);
        let verification = if self.verification_ajoutee {
            match (self.plan.last(), self.issues.get(n).copied().flatten()) {
                (Some(duel), Some(reponse)) => Some((duel, reponse)),
                _ => None,
            }
        } else {
            None
        };
        calculer_resultat(
            &self.version.courbe,
            &self.version,
            &self.variantes,
            plan,
            &self.issues,
            verification,
            &self.applis,
        )
    }

    /// L'appli vue le plus souvent pendant les passages du duel courant (à égalité : la
    /// première vue). Les passages sans appli connue ne comptent pas.
    fn appli_majoritaire(&self) -> Option<String> {
        let mut comptes: Vec<(&String, u32)> = Vec::new();
        for appli in self.applis_duel.iter().flatten() {
            match comptes.iter_mut().find(|(nom, _)| *nom == appli) {
                Some((_, n)) => *n += 1,
                None => comptes.push((appli, 1)),
            }
        }
        let max = comptes.iter().map(|(_, n)| *n).max()?;
        comptes
            .into_iter()
            .find(|(_, n)| *n == max)
            .map(|(nom, _)| nom.clone())
    }

    // ----- Temps d'écoute -----

    /// Ferme le passage en cours, s'il y en a un : son temps d'écoute (au plus un
    /// intervalle) rejoint le cumul.
    fn fermer_passage(&mut self, t: u64) {
        self.ecoute_ms = self.ecoute_totale_ms(t);
    }

    fn ecoute_totale_ms(&self, t: u64) -> u64 {
        match self.phase {
            Phase::Ecoute { debut_ms, .. } => self
                .ecoute_ms
                .saturating_add(t.saturating_sub(debut_ms).min(self.intervalle_ms())),
            _ => self.ecoute_ms,
        }
    }

    fn conseiller_pause(&mut self, t: u64, effets: &mut Vec<Effet>) {
        if self.pause_conseillee || !self.en_duel() {
            return;
        }
        if self.ecoute_totale_ms(t) >= self.reglages.pause_conseillee_apres_ms() {
            self.pause_conseillee = true;
            effets.push(Effet::ConseillerPause);
        }
    }

    // ----- Avancer dans le plan -----

    /// Lance le duel courant depuis son premier passage, ou finit le test s'il n'en reste pas.
    fn demarrer_duel(&mut self, t: u64, effets: &mut Vec<Effet>) {
        let Some(duel) = self.duel_courant() else {
            return self.fin_de_plan(t, effets);
        };
        if std::mem::take(&mut self.a_retirer) {
            // Le duel a été interrompu et la référence entendue : ce qu'on a pu deviner
            // ne doit plus servir. Même sujet, nouveau tirage, noté au journal.
            let prevu = self.tirer(&duel);
            if let Some(place) = self.plan.get_mut(self.issues.len()) {
                *place = prevu;
            }
            effets.push(Effet::Journaliser(Entree::DuelRetire {
                duel: self.indice_courant() as u32,
                prevu,
                horodatage_ms: t,
            }));
        }
        effets.push(Effet::Journaliser(Entree::DuelDemarre {
            duel: self.indice_courant() as u32,
            horodatage_ms: t,
        }));
        self.demarrer_passage(0, t, effets);
    }

    /// Lance (ou relance) un passage du duel courant : ligne de journal, puis courbe du côté.
    fn demarrer_passage(&mut self, passage: usize, t: u64, effets: &mut Vec<Effet>) {
        let Some(duel) = self.duel_courant() else {
            return self.fin_de_plan(t, effets);
        };
        let Some(&cote) = duel.ordre_passages.get(passage) else {
            return self.attendre_reponse(t, effets);
        };
        if passage == 0 {
            // Nouveau départ du duel : courbes recalculées avec le spectre du moment.
            self.courbes_duel = None;
        }
        self.applis_duel.truncate(passage);
        self.applis_duel.push(self.premier_plan.clone());
        effets.push(Effet::Journaliser(Entree::PassageDemarre {
            duel: self.indice_courant() as u32,
            passage: passage as u32,
            cote,
            appli_premier_plan: self.premier_plan.clone(),
            horodatage_ms: t,
        }));
        let courbe = self.courbe_du_cote(&duel, cote);
        effets.push(Effet::Appliquer {
            cote: Some(cote),
            courbe,
        });
        self.phase = Phase::Ecoute {
            passage,
            debut_ms: t,
        };
    }

    /// Les passages sont finis : la question est posée, et le dernier côté entendu reste
    /// en place. Remettre la référence ici dirait si ce dernier côté était la variante.
    fn attendre_reponse(&mut self, t: u64, effets: &mut Vec<Effet>) {
        let dernier = self
            .duel_courant()
            .and_then(|d| d.ordre_passages.last().copied())
            .unwrap_or(Cote::A);
        self.phase = Phase::Attente {
            applique: dernier,
            depuis_ms: t,
        };
        effets.push(Effet::DemanderReponse);
    }

    /// Un nouveau tirage pour ce duel : même sujet, orientation et ordre des passages
    /// retirés. Le n-ième tirage d'un test ne dépend que de sa graine et de n.
    fn tirer(&mut self, duel: &DuelPrevu) -> DuelPrevu {
        let graine = self
            .graine
            .wrapping_add(self.tirages.wrapping_mul(0x9E37_79B9_7F4A_7C15));
        self.tirages = self.tirages.wrapping_add(1);
        retirer_duel(duel, &mut HasardGraine::new(graine))
    }

    /// Clôt le duel courant avec son issue et passe à la suite.
    fn clore_duel(&mut self, issue: Option<Reponse>, t: u64, effets: &mut Vec<Effet>) {
        self.applis.push(self.appli_majoritaire());
        self.issues.push(issue);
        self.applis_duel.clear();
        self.courbes_duel = None;
        self.a_retirer = false;
        self.demarrer_duel(t, effets);
    }

    /// Plus aucun duel à jouer : on ajoute le duel de vérification s'il le faut, sinon on
    /// rend le résultat.
    fn fin_de_plan(&mut self, t: u64, effets: &mut Vec<Effet>) {
        let mode_verifie = matches!(
            self.mode,
            Mode::DuelsEnJeu { .. } | Mode::UneSeuleZone { .. }
        );
        if mode_verifie && !self.verification_ajoutee && self.resultat().a_des_changements() {
            let duel = DuelPrevu {
                sujet: Sujet::Verification,
                ..self.verification_prevue
            };
            self.plan.push(duel);
            self.verification_ajoutee = true;
            effets.push(Effet::Journaliser(Entree::DuelVerificationAjoute {
                duel,
                horodatage_ms: t,
            }));
            return self.demarrer_duel(t, effets);
        }
        let resultat = self.resultat();
        self.phase = Phase::Termine;
        self.remettre_reference(effets);
        effets.push(Effet::Journaliser(Entree::TestTermine {
            resultat: resultat.clone(),
            horodatage_ms: t,
        }));
        effets.push(Effet::Termine(resultat));
    }

    /// Oublie tout ce qui a été répondu à partir du duel `duel` : il redevient le duel
    /// courant. Si un duel de vérification avait été ajouté, il est retiré (il dépendait
    /// des réponses qu'on vient de défaire).
    fn revenir_au_duel(&mut self, duel: usize) {
        self.issues.truncate(duel);
        self.applis.truncate(duel);
        self.applis_duel.clear();
        self.courbes_duel = None;
        self.a_retirer = false;
        if self.verification_ajoutee && duel < self.duels_du_plan() {
            self.plan.pop();
            self.verification_ajoutee = false;
        }
    }

    // ----- Un gestionnaire par événement -----

    fn demarrer(&mut self, t: u64, effets: &mut Vec<Effet>) {
        if self.phase != Phase::Pret {
            return;
        }
        effets.push(Effet::Journaliser(Entree::TestDemarre {
            test_id: self.test_id.clone(),
            profil_id: self.profil_id.clone(),
            version_reference: self.version.numero,
            courbe_reference: self.version.courbe.clone(),
            mode: self.mode.clone(),
            plan: self.plan.clone(),
            reglages: self.reglages.clone(),
            variantes: self.variantes,
            horodatage_ms: t,
        }));
        if self.appel && !self.plan.is_empty() {
            // Un appel est déjà en cours : le test attend sa fin avant le premier passage.
            self.mettre_en_pause(RaisonPause::Appel, t, effets);
        } else {
            self.demarrer_duel(t, effets);
        }
    }

    /// Fait avancer les passages.
    ///
    /// - Moins d'un intervalle depuis le début du passage : rien.
    /// - Entre un et deux intervalles : le passage est fini, le suivant commence
    ///   *maintenant* (chaque passage dure donc au moins un intervalle entier).
    /// - Plus de deux intervalles : le PC dormait ou l'appli était figée. On ne sait pas
    ///   ce qui a été entendu, donc on ne compte rien : le passage en cours recommence.
    ///
    /// Un `Tic` ne fait jamais avancer de plus d'un passage.
    ///
    /// Pendant la question, le tic surveille le délai de réponse : dépassé, le test se
    /// met en pause tout seul et la référence est remise (ADR-0012).
    fn tic(&mut self, t: u64, effets: &mut Vec<Effet>) {
        if let Phase::Attente { depuis_ms, .. } = self.phase {
            if t.saturating_sub(depuis_ms) >= self.delai_reponse_ms() {
                self.mettre_en_pause(RaisonPause::SansReponse, t, effets);
            }
            return;
        }
        let Phase::Ecoute { passage, debut_ms } = self.phase else {
            return;
        };
        let ecoule = t.saturating_sub(debut_ms);
        let intervalle = self.intervalle_ms();
        if ecoule < intervalle {
            return;
        }
        if ecoule > intervalle.saturating_mul(2) {
            return self.demarrer_passage(passage, t, effets);
        }
        self.fermer_passage(t);
        self.demarrer_passage(passage + 1, t, effets);
    }

    fn repondre(&mut self, reponse: Reponse, t: u64, effets: &mut Vec<Effet>) {
        if !self.en_duel() || self.duel_courant().is_none() {
            return;
        }
        // La réponse d'abord : elle doit être sur le disque avant toute autre conséquence.
        effets.push(Effet::Journaliser(Entree::ReponseDonnee {
            duel: self.indice_courant() as u32,
            reponse,
            horodatage_ms: t,
        }));
        if self.reglages.bip() {
            effets.push(Effet::Bip);
        }
        if self.en_attente() {
            effets.push(Effet::MasquerQuestion);
        }
        self.fermer_passage(t);
        self.clore_duel(Some(reponse), t, effets);
    }

    /// Met le test en pause : la référence est remise *avant* toute écriture. Si un duel
    /// était en cours, il sera retiré à la reprise.
    fn mettre_en_pause(&mut self, raison: RaisonPause, t: u64, effets: &mut Vec<Effet>) {
        let en_attente = self.en_attente();
        if self.en_duel() {
            self.a_retirer = true;
        }
        self.fermer_passage(t);
        self.remettre_reference(effets);
        if en_attente {
            effets.push(Effet::MasquerQuestion);
        }
        effets.push(Effet::Journaliser(Entree::Pause {
            raison,
            horodatage_ms: t,
        }));
        self.phase = Phase::Pause {
            raison,
            depuis_ms: t,
        };
    }

    fn pause_manuelle(&mut self, t: u64, effets: &mut Vec<Effet>) {
        if self.en_duel() {
            self.mettre_en_pause(RaisonPause::Manuelle, t, effets);
        } else if let Phase::Pause {
            raison: RaisonPause::Appel,
            depuis_ms,
        } = self.phase
        {
            // Pause demandée pendant une pause d'appel : elle devient manuelle, et la fin
            // de l'appel ne relancera pas le test tout seul.
            self.phase = Phase::Pause {
                raison: RaisonPause::Manuelle,
                depuis_ms,
            };
            effets.push(Effet::Journaliser(Entree::Pause {
                raison: RaisonPause::Manuelle,
                horodatage_ms: t,
            }));
        }
    }

    /// Fin de pause : le duel courant repart de son premier passage (retiré s'il avait
    /// été interrompu). Après une vraie pause, le compteur d'écoute repart de zéro.
    fn reprendre(&mut self, t: u64, effets: &mut Vec<Effet>) {
        if let Phase::Pause { depuis_ms, .. } = self.phase {
            if t.saturating_sub(depuis_ms) > PAUSE_QUI_REPOSE_MS {
                self.ecoute_ms = 0;
            }
        }
        effets.push(Effet::Journaliser(Entree::Reprise { horodatage_ms: t }));
        self.demarrer_duel(t, effets);
    }

    fn appel_en_cours(&mut self, en_cours: bool, t: u64, effets: &mut Vec<Effet>) {
        if en_cours == self.appel {
            return;
        }
        self.appel = en_cours;
        if en_cours {
            if self.en_duel() {
                self.mettre_en_pause(RaisonPause::Appel, t, effets);
            }
        } else if matches!(
            self.phase,
            Phase::Pause {
                raison: RaisonPause::Appel,
                ..
            }
        ) {
            self.reprendre(t, effets);
        }
    }

    fn annuler(&mut self, t: u64, effets: &mut Vec<Effet>) {
        if !self.en_duel() {
            return;
        }
        let Some(duel) = self.issues.iter().rposition(Option::is_some) else {
            return;
        };
        let en_attente = self.en_attente();
        self.fermer_passage(t);
        self.revenir_au_duel(duel);
        effets.push(Effet::Journaliser(Entree::ReponseAnnulee {
            duel: duel as u32,
            horodatage_ms: t,
        }));
        if en_attente {
            effets.push(Effet::MasquerQuestion);
        }
        self.demarrer_duel(t, effets);
    }

    fn passer_duel(&mut self, t: u64, effets: &mut Vec<Effet>) {
        if !self.en_duel() || self.duel_courant().is_none() {
            return;
        }
        effets.push(Effet::Journaliser(Entree::DuelPasse {
            duel: self.indice_courant() as u32,
            horodatage_ms: t,
        }));
        if self.en_attente() {
            effets.push(Effet::MasquerQuestion);
        }
        self.fermer_passage(t);
        self.clore_duel(None, t, effets);
    }

    fn recommencer_duel(&mut self, t: u64, effets: &mut Vec<Effet>) {
        if !self.en_duel() {
            return;
        }
        effets.push(Effet::Journaliser(Entree::DuelRecommence {
            duel: self.indice_courant() as u32,
            horodatage_ms: t,
        }));
        if self.en_attente() {
            effets.push(Effet::MasquerQuestion);
        }
        self.fermer_passage(t);
        // L'utilisateur a pu entendre une bascule entière : on retire aussi.
        self.a_retirer = true;
        self.demarrer_duel(t, effets);
    }

    fn reecouter(&mut self, cote: Cote, appuye: bool, t: u64, effets: &mut Vec<Effet>) {
        let Phase::Attente { applique, .. } = self.phase else {
            return;
        };
        let Some(duel) = self.duel_courant() else {
            return;
        };
        if !appuye {
            return; // au relâchement, le côté reste : on ne repasse pas par la référence
        }
        // Chaque appui relance le délai de réponse.
        self.phase = Phase::Attente {
            applique: cote,
            depuis_ms: t,
        };
        if applique != cote {
            let courbe = self.courbe_du_cote(&duel, cote);
            effets.push(Effet::Appliquer {
                cote: Some(cote),
                courbe,
            });
        }
    }

    fn abandonner(&mut self, t: u64, effets: &mut Vec<Effet>) {
        match self.phase {
            Phase::Termine | Phase::Abandonne => return,
            Phase::Pret => {
                // Rien n'a été écrit : pas de journal à clore.
                self.remettre_reference(effets);
            }
            _ => {
                let en_attente = self.en_attente();
                self.fermer_passage(t);
                self.remettre_reference(effets);
                if en_attente {
                    effets.push(Effet::MasquerQuestion);
                }
                effets.push(Effet::Journaliser(Entree::TestAbandonne {
                    horodatage_ms: t,
                }));
            }
        }
        self.phase = Phase::Abandonne;
    }
}

/// Une graine tirée de l'identifiant du test (hachage FNV-1a) : deux tests différents
/// n'ont pas les mêmes tirages, et un test reconstruit depuis son journal retrouve les siens.
fn graine_de(test_id: &str) -> u64 {
    test_id.bytes().fold(0xCBF2_9CE4_8422_2325, |h, octet| {
        (h ^ u64::from(octet)).wrapping_mul(0x0000_0100_0000_01B3)
    })
}
