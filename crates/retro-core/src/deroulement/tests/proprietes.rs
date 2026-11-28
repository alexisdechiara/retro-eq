//! Tests au hasard : de longues suites d'événements tirées avec une graine. Le [`Banc`]
//! vérifie les invariants après chaque événement ; ici on s'occupe de produire des suites
//! variées et de vérifier ce qui se voit sur la durée.

use super::*;
use crate::hasard::{Hasard, HasardGraine};
use crate::plan::{duel_verification, plan};
use crate::zone::Zone;

fn reponse_au_hasard(h: &mut HasardGraine) -> Reponse {
    [Reponse::A, Reponse::Pareil, Reponse::B][h.entier(3) as usize]
}

fn cote_au_hasard(h: &mut HasardGraine) -> Cote {
    if h.pile_ou_face() {
        Cote::A
    } else {
        Cote::B
    }
}

/// Un événement tiré au hasard, tous types confondus. `reponses` règle la part de
/// réponses (sur 100) : peu de réponses donne des tests longs.
pub(super) fn evenement_au_hasard(h: &mut HasardGraine, reponses: u32, abandon: bool) -> Evenement {
    if h.entier(100) < reponses {
        return Evenement::Reponse(reponse_au_hasard(h));
    }
    if abandon && h.entier(400) == 0 {
        return Evenement::Abandonner;
    }
    match h.entier(100) {
        0..=54 => Evenement::Tic,
        55..=58 => Evenement::PlusTard,
        59..=62 => Evenement::Annuler,
        63..=66 => Evenement::Pause,
        67..=74 => Evenement::Reprendre,
        75..=76 => Evenement::PasserDuel,
        77..=79 => Evenement::RecommencerDuel,
        80..=88 => Evenement::Reecouter {
            cote: cote_au_hasard(h),
            appuye: h.pile_ou_face(),
        },
        89..=93 => Evenement::AppelEnCours(h.pile_ou_face()),
        94..=98 => Evenement::PremierPlan(
            ["Jeu.exe", "Discord.exe", "Navigateur.exe", ""][h.entier(4) as usize].into(),
        ),
        _ => Evenement::Demarrer,
    }
}

/// Le temps avance d'une durée au hasard : le plus souvent moins d'un intervalle, parfois
/// rien du tout, parfois une longue absence (PC endormi).
pub(super) fn duree_au_hasard(h: &mut HasardGraine) -> u64 {
    match h.entier(20) {
        0 => 0,
        1 => 2 * INTERVALLE + u64::from(h.entier(100_000)),
        2 => 3_600_000,
        _ => u64::from(h.entier(INTERVALLE as u32 + 2_000)),
    }
}

pub(super) fn mode_au_hasard(h: &mut HasardGraine) -> Mode {
    match h.entier(4) {
        0 => Mode::DuelsEnJeu { precis: true },
        1 => Mode::UneSeuleZone {
            zone: Zone::TOUTES[h.entier(7) as usize],
            duels: 1 + h.entier(12),
        },
        2 => Mode::ProfilContreProfil {
            autre: Courbe::default(),
            duels: h.entier(9),
        },
        _ => Mode::DuelsEnJeu { precis: false },
    }
}

/// Un banc sur un vrai plan, tiré comme le ferait l'appli.
pub(super) fn banc_au_hasard(h: &mut HasardGraine) -> Banc {
    let mode = mode_au_hasard(h);
    let p = plan(&mode, &version(), &[], h);
    let mut parametres = parametres(mode, p);
    parametres.verification = duel_verification(h);
    parametres.reglages = parametres
        .reglages
        .avec_pause_conseillee_apres_ms(u64::from(h.entier(600_000)));
    Banc::avec(parametres)
}

/// Mène le test à son terme en répondant à chaque duel : rend le nombre de réponses.
pub(super) fn finir_en_repondant(b: &mut Banc, h: &mut HasardGraine) -> u32 {
    b.envoyer(Evenement::Demarrer);
    b.envoyer(Evenement::AppelEnCours(false));
    b.envoyer(Evenement::Reprendre);
    let mut reponses = 0;
    // Au plus une réponse par duel, plus le duel de vérification.
    let limite = b.vue().total + 1;
    while b.etat() != EtatTest::Termine {
        assert!(reponses < limite, "le test ne se termine pas");
        if h.pile_ou_face() {
            b.tic_apres(INTERVALLE);
        }
        let avant = b.vue().historique.len();
        b.repondre(reponse_au_hasard(h));
        assert_eq!(b.vue().historique.len(), avant + 1);
        reponses += 1;
    }
    reponses
}

/// ADR-0012, lu dans le journal : un duel qui a commencé à jouer puis a été interrompu
/// par une pause (la référence a été entendue) ou recommencé ne redémarre jamais sans un
/// nouveau tirage écrit juste avant.
pub(super) fn verifier_les_nouveaux_tirages(journal: &[Entree]) {
    let mut entame = false;
    let mut a_retirer = false;
    for (i, entree) in journal.iter().enumerate() {
        match entree {
            Entree::PassageDemarre { .. } => entame = true,
            Entree::Pause { .. } => a_retirer |= entame,
            Entree::DuelRecommence { .. } => a_retirer = true,
            Entree::ReponseDonnee { .. }
            | Entree::DuelPasse { .. }
            | Entree::ReponseAnnulee { .. }
            | Entree::DuelRetire { .. } => {
                entame = false;
                a_retirer = false;
            }
            Entree::DuelDemarre { duel, .. } => {
                assert!(
                    !a_retirer,
                    "duel {duel} rejoué sans nouveau tirage (entrée {i})"
                );
                if i > 0 {
                    if let Entree::DuelRetire { duel: retire, .. } = &journal[i - 1] {
                        assert_eq!(retire, duel);
                    }
                }
            }
            _ => {}
        }
    }
}

#[test]
fn sans_pause_la_reference_n_est_jamais_entendue_avant_la_fin_du_test() {
    // La fuite relevée par la relecture : à l'intérieur d'un duel, tout `Appliquer` doit
    // porter un côté. On envoie tout ce qui ne met pas en pause, sur des tests entiers.
    let mut reecoutes = 0;
    for graine in 300..360 {
        let mut h = HasardGraine::new(graine);
        let mut b = banc_au_hasard(&mut h);
        b.demarrer();
        let mut pas = 0;
        while b.etat() != EtatTest::Termine {
            pas += 1;
            assert!(pas < 20_000);
            // Jamais plus que le délai de réponse d'un coup : pas de pause automatique.
            b.avancer(u64::from(h.entier(INTERVALLE as u32 + 2_000)));
            let bientot_en_pause = b
                .vue()
                .ms_avant_pause_sans_reponse
                .is_some_and(|reste| reste < 2 * INTERVALLE);
            let evt = match h.entier(40) {
                _ if bientot_en_pause => Evenement::Reponse(reponse_au_hasard(&mut h)),
                0..=24 => Evenement::Tic,
                25..=27 => Evenement::Reponse(reponse_au_hasard(&mut h)),
                28 => Evenement::Annuler,
                29 => Evenement::PasserDuel,
                30..=31 => Evenement::RecommencerDuel,
                32..=37 => Evenement::Reecouter {
                    cote: cote_au_hasard(&mut h),
                    appuye: h.pile_ou_face(),
                },
                38 => Evenement::PremierPlan("Jeu.exe".into()),
                _ => Evenement::Reprendre,
            };
            let effets = b.envoyer(evt.clone());
            if matches!(evt, Evenement::Reecouter { .. }) && !effets.is_empty() {
                reecoutes += 1;
            }
            if b.etat() != EtatTest::Termine {
                assert!(
                    matches!(
                        b.etat(),
                        EtatTest::EnEcoute { .. } | EtatTest::AttenteReponse
                    ),
                    "{:?}",
                    b.etat()
                );
                assert!(
                    appliquees(&effets).iter().all(|(cote, _)| cote.is_some()),
                    "référence entendue en plein test après {evt:?}"
                );
            }
        }
        // Du premier passage à la dernière réponse : uniquement des côtés, puis la référence.
        let toutes = appliquees(&b.tous);
        let (derniere, avant) = toutes.split_last().unwrap();
        assert_eq!(*derniere, (None, reference()));
        assert!(avant
            .iter()
            .all(|(cote, courbe)| cote.is_some() && *courbe != reference()));
    }
    assert!(reecoutes > 5, "{reecoutes}");
}

#[test]
fn des_milliers_d_evenements_au_hasard_ne_cassent_aucun_invariant() {
    let mut etats_vus = std::collections::HashSet::new();
    let mut termines = 0;
    for graine in 0..60 {
        let mut h = HasardGraine::new(graine);
        let mut b = banc_au_hasard(&mut h);
        // Peu de réponses pour les graines paires (tests longs), beaucoup pour les autres.
        let reponses = if graine % 2 == 0 { 1 } else { 12 };
        for _ in 0..3_000 {
            b.avancer(duree_au_hasard(&mut h));
            b.envoyer(evenement_au_hasard(&mut h, reponses, false));
            etats_vus.insert(std::mem::discriminant(&b.etat()));
        }
        verifier_les_nouveaux_tirages(&b.journal);
        // Les horodatages du journal ne reculent jamais.
        assert!(b
            .journal
            .windows(2)
            .all(|w| w[0].horodatage_ms() <= w[1].horodatage_ms()));
        assert!(b.compte(&Effet::ConseillerPause) <= 1);
        // Tant que des réponses arrivent, le test finit toujours.
        if b.etat() == EtatTest::Termine {
            termines += 1;
        }
        finir_en_repondant(&mut b, &mut h);
        assert_eq!(b.etat(), EtatTest::Termine);
        assert!(b.reference_remise() || b.vue().total == 0);
        // Exactement un résultat, et il est aussi dans le journal, en dernière ligne.
        let resultat = b.resultat().unwrap();
        assert_eq!(
            b.tous
                .iter()
                .filter(|e| matches!(e, Effet::Termine(_)))
                .count(),
            1
        );
        assert!(
            matches!(b.journal.last(), Some(Entree::TestTermine { resultat: r, .. }) if *r == resultat)
        );
        assert!(matches!(
            b.journal.first(),
            Some(Entree::TestDemarre { .. })
        ));
    }
    // Le tirage a bien exploré tous les états, et des tests se sont finis tout seuls.
    assert_eq!(
        etats_vus.len(),
        5,
        "Pret, EnEcoute, AttenteReponse, EnPause, Termine"
    );
    assert!(termines > 10, "{termines}");
}

#[test]
fn avec_des_abandons_au_hasard_la_reference_est_toujours_remise() {
    let mut abandonnes = 0;
    for graine in 100..160 {
        let mut h = HasardGraine::new(graine);
        let mut b = banc_au_hasard(&mut h);
        for _ in 0..1_500 {
            b.avancer(duree_au_hasard(&mut h));
            b.envoyer(evenement_au_hasard(&mut h, 2, true));
        }
        if b.etat() == EtatTest::Abandonne {
            abandonnes += 1;
            assert!(b.reference_remise());
            assert!(b.resultat().is_none());
            // Le journal est clos par l'abandon, sauf abandon avant le départ (journal vide).
            assert!(matches!(
                b.journal.last(),
                None | Some(Entree::TestAbandonne { .. })
            ));
        }
    }
    assert!(abandonnes > 20, "{abandonnes}");
}

#[test]
fn le_meme_scenario_donne_toujours_les_memes_effets() {
    let jouer = |graine| {
        let mut h = HasardGraine::new(graine);
        let mut b = banc_au_hasard(&mut h);
        for _ in 0..800 {
            b.avancer(duree_au_hasard(&mut h));
            b.envoyer(evenement_au_hasard(&mut h, 5, true));
        }
        (b.tous, b.d)
    };
    assert_eq!(jouer(7), jouer(7));
    assert_ne!(jouer(7).0, jouer(8).0);
}

#[test]
fn le_cote_entendu_est_toujours_celui_du_plan() {
    // Chaque `Appliquer` d'un passage porte le côté annoncé par le `PassageDemarre` qui
    // le précède, et ce côté est celui du plan.
    for graine in 200..220 {
        let mut h = HasardGraine::new(graine);
        let mut b = banc_au_hasard(&mut h);
        for _ in 0..1_000 {
            b.avancer(duree_au_hasard(&mut h));
            let effets = b.envoyer(evenement_au_hasard(&mut h, 4, false));
            for paire in effets.windows(2) {
                if let Effet::Journaliser(Entree::PassageDemarre {
                    duel,
                    passage,
                    cote,
                    ..
                }) = &paire[0]
                {
                    assert!(
                        matches!(&paire[1], Effet::Appliquer { cote: Some(c), .. } if c == cote)
                    );
                    let prevu = b.d.plan[*duel as usize].ordre_passages[*passage as usize];
                    assert_eq!(*cote, prevu);
                }
            }
        }
    }
}
