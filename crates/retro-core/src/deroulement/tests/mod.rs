//! Tests du déroulement. Le [`Banc`] joue le rôle de l'appli : il envoie les événements,
//! fait avancer une fausse horloge, garde les effets, et vérifie après **chaque**
//! événement les règles qui doivent toujours tenir (voir `verifier_invariants`).

mod proprietes;
mod reprise;
mod scenarios;

use super::*;
use crate::profil::Origine;
use crate::zone::{Sens, Zone};
use retro_dsp::{Filtre, TypeFiltre};

pub(super) const INTERVALLE: u64 = 10_000;
pub(super) const T0: u64 = 1_000_000;
pub(super) const ORDRE: [Cote; 6] = [Cote::A, Cote::B, Cote::B, Cote::A, Cote::B, Cote::A];

/// Une référence qui dépasse 0 dB : sa copie « mise au volume » a donc un autre préampli,
/// ce qui permet de vérifier que c'est bien la courbe d'origine qui est remise.
pub(super) fn reference() -> Courbe {
    Courbe {
        preampli_db: -2.0,
        filtres: vec![Filtre {
            type_filtre: TypeFiltre::Cloche,
            frequence_hz: 1000.0,
            gain_db: 5.0,
            q: 1.0,
        }],
    }
}

pub(super) fn version() -> Version {
    Version {
        numero: 3,
        creee_le: "2025-01-01T00:00:00Z".into(),
        courbe: reference(),
        origine: Origine::Manuel,
        pas_par_zone: [4.0, 2.0, 4.0, 4.0, 4.0, 4.0, 1.0],
    }
}

pub(super) fn spectre() -> Spectre {
    Spectre::rose(32)
}

pub(super) fn duel(sujet: Sujet, reference_est_a: bool) -> DuelPrevu {
    DuelPrevu {
        sujet,
        reference_est_a,
        ordre_passages: ORDRE,
    }
}

pub(super) fn zone(zone: Zone, sens: Sens, reference_est_a: bool) -> DuelPrevu {
    duel(Sujet::Zone { zone, sens }, reference_est_a)
}

/// Deux duels « plus de graves », un dans chaque orientation : de quoi retenir un
/// changement (il faut deux victoires) en répondant B puis A.
pub(super) fn plan_gagnant() -> Vec<DuelPrevu> {
    vec![
        zone(Zone::Graves, Sens::Plus, true),
        zone(Zone::Graves, Sens::Plus, false),
    ]
}

/// Le nouveau tirage annoncé dans ces effets, s'il y en a un.
pub(super) fn tirage_de(effets: &[Effet]) -> Option<(u32, DuelPrevu)> {
    effets.iter().find_map(|e| match e {
        Effet::Journaliser(Entree::DuelRetire { duel, prevu, .. }) => Some((*duel, *prevu)),
        _ => None,
    })
}

pub(super) fn controle() -> DuelPrevu {
    duel(Sujet::Controle, true)
}

/// Le duel de vérification tiré d'avance : la référence y est B.
pub(super) fn verification() -> DuelPrevu {
    duel(Sujet::Verification, false)
}

pub(super) fn parametres(mode: Mode, plan: Vec<DuelPrevu>) -> ParametresTest {
    ParametresTest {
        test_id: "t1".into(),
        profil_id: "casque".into(),
        reference: version(),
        mode,
        plan,
        reglages: ReglagesTest::nouveau(INTERVALLE as u32, true),
        variantes: ReglagesVariantes::default(),
        spectre: spectre(),
        verification: verification(),
    }
}

pub(super) fn contexte() -> ContexteReprise {
    ContexteReprise {
        reference: version(),
        variantes: ReglagesVariantes::default(),
        spectre: spectre(),
        verification: verification(),
    }
}

/// Les deux côtés attendus d'un duel sur une zone : `(référence, variante)` au même volume.
pub(super) fn cotes_zone(z: Zone, sens: Sens, spectre: &Spectre) -> (Courbe, Courbe) {
    let v = version();
    let var = variante(&v.courbe, z, sens, v.pas_par_zone[z.index()]);
    egaliser_volumes(&v.courbe, &var, spectre)
}

pub(super) fn journalisees(effets: &[Effet]) -> Vec<Entree> {
    effets
        .iter()
        .filter_map(|e| match e {
            Effet::Journaliser(entree) => Some(entree.clone()),
            _ => None,
        })
        .collect()
}

pub(super) fn appliquees(effets: &[Effet]) -> Vec<(Option<Cote>, Courbe)> {
    effets
        .iter()
        .filter_map(|e| match e {
            Effet::Appliquer { cote, courbe } => Some((*cote, courbe.clone())),
            _ => None,
        })
        .collect()
}

pub(super) fn resultat_de(effets: &[Effet]) -> Option<ResultatTest> {
    effets.iter().find_map(|e| match e {
        Effet::Termine(r) => Some(r.clone()),
        _ => None,
    })
}

pub(super) struct Banc {
    pub d: Deroulement,
    /// La fausse horloge.
    pub t: u64,
    /// Tous les effets depuis le début.
    pub tous: Vec<Effet>,
    /// Tout ce qui a été journalisé, dans l'ordre : le contenu du fichier `.jsonl`.
    pub journal: Vec<Entree>,
    /// La dernière courbe donnée au moteur.
    pub appliquee: Option<(Option<Cote>, Courbe)>,
    pub reference: Courbe,
}

impl Banc {
    pub fn nouveau(plan: Vec<DuelPrevu>) -> Banc {
        Banc::avec(parametres(Mode::DuelsEnJeu { precis: false }, plan))
    }

    pub fn avec(p: ParametresTest) -> Banc {
        let reference = p.reference.courbe.clone();
        Banc::autour(Deroulement::nouveau(p), reference, T0)
    }

    pub fn autour(d: Deroulement, reference: Courbe, t: u64) -> Banc {
        Banc {
            d,
            t,
            tous: Vec::new(),
            journal: Vec::new(),
            appliquee: None,
            reference,
        }
    }

    /// Enregistre des effets comme l'appli les exécuterait.
    pub fn executer(&mut self, effets: &[Effet]) {
        for e in effets {
            match e {
                Effet::Appliquer { cote, courbe } => self.appliquee = Some((*cote, courbe.clone())),
                Effet::Journaliser(entree) => self.journal.push(entree.clone()),
                _ => {}
            }
        }
        self.tous.extend_from_slice(effets);
    }

    pub fn envoyer(&mut self, evt: Evenement) -> Vec<Effet> {
        let avant = self.etat();
        let effets = self.d.traiter(evt.clone(), self.t);
        self.executer(&effets);
        self.verifier_invariants(&evt, avant, &effets);
        effets
    }

    /// Ce qui doit être vrai après n'importe quel événement, dans n'importe quel état.
    fn verifier_invariants(&self, evt: &Evenement, avant: EtatTest, effets: &[Effet]) {
        let vue = self.vue();
        let contexte = format!("après {evt:?} depuis {avant:?} → {:?}", vue.etat);

        // 1. À l'aveugle pendant un duel, référence remise en dehors (ADR-0012).
        let en_duel = matches!(
            vue.etat,
            EtatTest::EnEcoute { .. } | EtatTest::AttenteReponse
        );
        if en_duel {
            let cote = vue
                .cote_actif
                .unwrap_or_else(|| panic!("aucun côté {contexte}"));
            let (applique, courbe) = self.appliquee.as_ref().unwrap();
            assert_eq!(*applique, Some(cote), "mauvais côté {contexte}");
            let duel = self.prevu(vue.duel as usize - 1);
            let (cote_ref, cote_var) = self.d.courbes_duel.as_ref().unwrap();
            let attendue = if cote == duel.cote_reference() {
                cote_ref
            } else {
                cote_var
            };
            assert_eq!(courbe, attendue, "pas la courbe de ce côté {contexte}");
            assert_ne!(
                courbe, &self.reference,
                "référence nue en plein duel {contexte}"
            );
            assert!(
                !effets
                    .iter()
                    .any(|e| matches!(e, Effet::Appliquer { cote: None, .. })),
                "la référence a été appliquée pendant un duel {contexte} : {effets:?}"
            );
        } else {
            assert_eq!(vue.cote_actif, None, "{contexte}");
            if let Some((cote, courbe)) = &self.appliquee {
                assert_eq!(*cote, None, "un côté reste appliqué {contexte}");
                assert_eq!(courbe, &self.reference, "pas la référence {contexte}");
            }
        }

        // 2. On ne sort jamais du plan.
        assert!(vue.duel <= vue.total, "{contexte}");
        if let EtatTest::EnEcoute { duel, passage } = vue.etat {
            assert!(duel < vue.total, "{contexte}");
            assert!(passage < 6, "{contexte}");
            assert_eq!(duel + 1, vue.duel, "{contexte}");
        }
        assert!(vue.passages_a <= 3 && vue.passages_b <= 3, "{contexte}");
        assert!((0.0..=1.0).contains(&vue.progression), "{contexte}");
        assert!(vue.historique.len() as u32 <= vue.total, "{contexte}");

        // 3. Une réponse acceptée est journalisée avant tout autre effet ; refusée, elle
        //    ne produit rien.
        if let Evenement::Reponse(r) = evt {
            let acceptee = matches!(avant, EtatTest::EnEcoute { .. } | EtatTest::AttenteReponse);
            if acceptee {
                assert!(
                    matches!(effets.first(), Some(Effet::Journaliser(Entree::ReponseDonnee { reponse, .. })) if reponse == r),
                    "réponse pas journalisée en premier {contexte} : {effets:?}"
                );
            } else {
                assert!(
                    effets.is_empty(),
                    "réponse hors duel {contexte} : {effets:?}"
                );
            }
        }

        // 4. Un test clos ne fait plus rien.
        if matches!(avant, EtatTest::Termine | EtatTest::Abandonne) {
            assert!(effets.is_empty(), "{contexte} : {effets:?}");
            assert_eq!(vue.etat, avant);
        }
    }

    pub fn avancer(&mut self, ms: u64) {
        self.t += ms;
    }

    /// Fait passer `ms` puis envoie un `Tic`.
    pub fn tic_apres(&mut self, ms: u64) -> Vec<Effet> {
        self.avancer(ms);
        self.envoyer(Evenement::Tic)
    }

    pub fn demarrer(&mut self) -> Vec<Effet> {
        self.envoyer(Evenement::Demarrer)
    }

    /// Laisse le passage en cours aller à son terme.
    pub fn ecouter_un_passage(&mut self) -> Vec<Effet> {
        self.tic_apres(INTERVALLE)
    }

    /// Écoute tout le duel en cours jusqu'à la question. Rend les effets du dernier tic.
    pub fn ecouter_un_duel(&mut self) -> Vec<Effet> {
        let mut derniers = Vec::new();
        for _ in 0..12 {
            if !matches!(self.etat(), EtatTest::EnEcoute { .. }) {
                break;
            }
            derniers = self.ecouter_un_passage();
        }
        assert_eq!(self.etat(), EtatTest::AttenteReponse);
        derniers
    }

    pub fn repondre(&mut self, r: Reponse) -> Vec<Effet> {
        self.envoyer(Evenement::Reponse(r))
    }

    /// Le duel d'indice `i` tel qu'il est joué (après un éventuel nouveau tirage).
    pub fn prevu(&self, i: usize) -> DuelPrevu {
        self.d.plan[i]
    }

    /// Répond au duel en cours par sa signification, quelle que soit l'orientation tirée.
    pub fn repondre_selon(&mut self, preference: crate::plan::Preference) -> Vec<Effet> {
        use crate::plan::Preference as P;
        let duel = self.prevu(self.d.issues.len());
        let reponse = match (preference, duel.reference_est_a) {
            (P::Aucune, _) => Reponse::Pareil,
            (P::Variante, true) | (P::Reference, false) => Reponse::B,
            (P::Variante, false) | (P::Reference, true) => Reponse::A,
        };
        self.repondre(reponse)
    }

    pub fn etat(&self) -> EtatTest {
        self.d.vue(self.t).etat
    }

    pub fn vue(&self) -> VueTest {
        self.d.vue(self.t)
    }

    pub fn resultat(&self) -> Option<ResultatTest> {
        resultat_de(&self.tous)
    }

    pub fn reference_remise(&self) -> bool {
        self.appliquee == Some((None, self.reference.clone()))
    }

    pub fn compte(&self, effet: &Effet) -> usize {
        self.tous.iter().filter(|e| *e == effet).count()
    }
}
