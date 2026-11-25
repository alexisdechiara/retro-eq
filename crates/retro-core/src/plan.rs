//! Le plan d'un test : la liste des duels prévus avant que le test ne commence.
//!
//! Le plan décide *quoi* comparer et dans quel ordre ; il ne fabrique aucune courbe (le
//! déroulement les calcule au moment de jouer, avec les pas de la version de référence).

use crate::hasard::Hasard;
use crate::profil::Version;
use crate::zone::{Sens, Zone};
use retro_dsp::Courbe;
use serde::{Deserialize, Serialize};

/// Un côté d'un duel : on écoute tour à tour A et B, sans savoir lequel est la référence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Cote {
    A,
    B,
}

impl Cote {
    /// L'autre côté.
    pub fn autre(self) -> Cote {
        match self {
            Cote::A => Cote::B,
            Cote::B => Cote::A,
        }
    }
}

/// La réponse donnée à la fin d'un duel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Reponse {
    A,
    Pareil,
    B,
}

/// Ce que la réponse à un duel dit, une fois traduite : qui de la variante ou de la
/// référence a été préférée.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Preference {
    /// La variante (ou l'autre profil, ou la nouvelle version) a été préférée.
    Variante,
    /// La référence a été préférée.
    Reference,
    /// « Pareil » : personne ne gagne.
    Aucune,
}

/// Ce qu'un duel oppose à la référence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Sujet {
    /// Une variante : la référence plus un filtre sur `zone`, dans le `sens` donné.
    Zone { zone: Zone, sens: Sens },
    /// Duel de contrôle : les deux côtés sont la référence. Il mesure le hasard.
    Controle,
    /// Le mode « profil contre profil » : l'autre profil contre la référence.
    AutreProfil,
    /// Duel final : la référence contre la nouvelle version, avant de l'activer.
    Verification,
}

/// Un duel prévu : son sujet, qui est A, et l'ordre des 6 passages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DuelPrevu {
    pub sujet: Sujet,
    /// Vrai si la référence est le côté A (tiré à pile ou face, pour que A ne trahisse rien).
    pub reference_est_a: bool,
    /// Côté joué à chacun des 6 passages : 3 A et 3 B, jamais 3 fois de suite le même.
    pub ordre_passages: [Cote; 6],
}

impl DuelPrevu {
    /// Le côté (A ou B) qui joue la référence.
    pub fn cote_reference(&self) -> Cote {
        if self.reference_est_a {
            Cote::A
        } else {
            Cote::B
        }
    }

    /// Le côté qui joue la variante.
    pub fn cote_variante(&self) -> Cote {
        self.cote_reference().autre()
    }

    /// Vrai pour un duel de contrôle.
    pub fn est_controle(&self) -> bool {
        self.sujet == Sujet::Controle
    }

    /// Traduit une réponse « A / Pareil / B » en « variante / référence / aucune » grâce à
    /// `reference_est_a`. C'est la seule façon correcte de lire une réponse.
    pub fn preference(&self, reponse: Reponse) -> Preference {
        match (reponse, self.reference_est_a) {
            (Reponse::Pareil, _) => Preference::Aucune,
            (Reponse::A, true) | (Reponse::B, false) => Preference::Reference,
            (Reponse::A, false) | (Reponse::B, true) => Preference::Variante,
        }
    }
}

/// Une façon de mener un test (ADR-0011).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Mode {
    /// Sur ce qui joue sur le PC : 12 duels (rapide) ou 30 (précis).
    DuelsEnJeu { precis: bool },
    /// Toute l'attention sur une seule zone.
    UneSeuleZone { zone: Zone, duels: u32 },
    /// Un autre profil contre la référence.
    ProfilContreProfil { autre: Courbe, duels: u32 },
}

impl Mode {
    /// Nombre de duels du plan, contrôles compris. Il appartient au mode, pas au nombre de zones.
    pub fn nombre_duels(&self) -> u32 {
        match self {
            Mode::DuelsEnJeu { precis: false } => 12,
            Mode::DuelsEnJeu { precis: true } => 30,
            Mode::UneSeuleZone { duels, .. } | Mode::ProfilContreProfil { duels, .. } => *duels,
        }
    }
}

/// Nombre de duels de contrôle : un sur six, arrondi au plus proche (la moitié vers le haut).
/// Cela donne au moins 1 dès 3 duels ; on plafonne à la moitié des duels pour pouvoir
/// toujours les séparer.
fn nombre_controles(total: usize) -> usize {
    ((total + 3) / 6).min(total / 2)
}

/// Un couple (zone, sens) : ce qu'un duel sur une zone compare à la référence.
pub type Couple = (Zone, Sens);

/// Ce que les tests précédents d'un profil apprennent au plan suivant.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct HistoriquePlan {
    /// Les couples déjà testés, du plus ancien au plus récent (un couple peut y figurer
    /// plusieurs fois : la dernière fois compte).
    pub couples: Vec<Couple>,
    /// Les indices du dernier test (une seule victoire, voir
    /// [`crate::resultat::ResultatTest::indices`]) : ils passent en tête du plan.
    pub indices: Vec<Couple>,
}

impl HistoriquePlan {
    /// L'historique quand on ne connaît que les zones testées (du plus ancien test au
    /// plus récent) : chaque zone compte pour ses deux sens, et il n'y a pas d'indice.
    pub fn depuis_zones(zones: &[Zone]) -> HistoriquePlan {
        HistoriquePlan {
            couples: zones
                .iter()
                .flat_map(|&z| [(z, Sens::Plus), (z, Sens::Moins)])
                .collect(),
            indices: Vec::new(),
        }
    }

    /// Ajoute un test terminé : les couples de son plan, et les indices de son résultat
    /// (`resultat.indices`), qui remplacent ceux du test d'avant.
    pub fn ajouter_test(&mut self, plan: &[DuelPrevu], indices: &[Couple]) {
        self.couples.extend(plan.iter().filter_map(|d| match d.sujet {
            Sujet::Zone { zone, sens } => Some((zone, sens)),
            _ => None,
        }));
        self.indices = indices.to_vec();
    }
}

/// Les 14 couples dans l'ordre où le plan les prend : d'abord les indices du dernier test,
/// puis les couples jamais testés (dans l'ordre du spectre), puis les autres, du testé le
/// moins récemment au plus récemment.
fn couples_par_priorite(historique: &HistoriquePlan) -> Vec<Couple> {
    let tous = Zone::TOUTES
        .into_iter()
        .flat_map(|z| [(z, Sens::Plus), (z, Sens::Moins)]);
    let derniere_fois = |c: Couple| historique.couples.iter().rposition(|&x| x == c);
    let mut ordre: Vec<Couple> = Vec::new();
    for &c in &historique.indices {
        if !ordre.contains(&c) {
            ordre.push(c);
        }
    }
    let mut reste: Vec<(Option<usize>, Couple)> = tous
        .filter(|c| !ordre.contains(c))
        .map(|c| (derniere_fois(c), c))
        .collect();
    // Tri stable : `None` (jamais testé) passe avant tout `Some`, et l'ordre du spectre
    // départage les jamais testés.
    reste.sort_by_key(|(rang, _)| *rang);
    ordre.extend(reste.into_iter().map(|(_, c)| c));
    ordre
}

/// Les sujets des duels « réels » (hors contrôle), dans l'ordre de distribution.
fn sujets_reels(
    mode: &Mode,
    nombre: usize,
    historique: &HistoriquePlan,
    hasard: &mut impl Hasard,
) -> Vec<Sujet> {
    match mode {
        Mode::DuelsEnJeu { .. } => {
            // Les duels vont par paires : chaque couple retenu est joué deux fois (une
            // seule victoire ne prouve rien, voir ADR-0005). Avec un budget impair, le
            // premier couple est joué une troisième fois.
            let couples = couples_par_priorite(historique);
            let retenus = (nombre / 2).clamp(1, couples.len());
            (0..nombre)
                .filter_map(|i| couples.get(i % retenus))
                .map(|&(zone, sens)| Sujet::Zone { zone, sens })
                .collect()
        }
        Mode::UneSeuleZone { zone, .. } => {
            let depart = if hasard.pile_ou_face() {
                Sens::Plus
            } else {
                Sens::Moins
            };
            (0..nombre)
                .map(|i| Sujet::Zone {
                    zone: *zone,
                    sens: if i % 2 == 0 { depart } else { depart.oppose() },
                })
                .collect()
        }
        Mode::ProfilContreProfil { .. } => vec![Sujet::AutreProfil; nombre],
    }
}

/// Mélange les duels réels. En « duels en jeu », on évite que les deux duels d'un même
/// couple se suivent : on retire jusqu'à trouver un ordre sans voisins identiques, et à
/// défaut on garde l'ordre de distribution (c0, c1, …, c0, c1, …), qui n'en a jamais.
fn melanger_reels(mode: &Mode, reels: Vec<Sujet>, hasard: &mut impl Hasard) -> Vec<Sujet> {
    let mut melange = reels.clone();
    if !matches!(mode, Mode::DuelsEnJeu { .. }) {
        hasard.melanger(&mut melange);
        return melange;
    }
    for _ in 0..200 {
        hasard.melanger(&mut melange);
        if !melange.windows(2).any(|w| w[0] == w[1]) {
            return melange;
        }
    }
    reels
}

/// Insère les contrôles parmi les duels réels : jamais en premier, jamais deux à la suite.
///
/// Astuce : choisir `c` positions non adjacentes parmi `1..n` revient à choisir `c` valeurs
/// distinctes parmi `1..=n-c` puis à décaler la i-ème de `i`.
fn inserer_controles(reels: Vec<Sujet>, controles: usize, hasard: &mut impl Hasard) -> Vec<Sujet> {
    let total = reels.len() + controles;
    let mut choix: Vec<usize> = (1..=reels.len()).collect();
    hasard.melanger(&mut choix);
    choix.truncate(controles);
    choix.sort_unstable();
    let positions: Vec<usize> = choix.iter().enumerate().map(|(i, p)| p + i).collect();

    let mut reels = reels.into_iter();
    (0..total)
        .filter_map(|i| {
            if positions.contains(&i) {
                Some(Sujet::Controle)
            } else {
                reels.next()
            }
        })
        .collect()
}

/// Les ordres de passages possibles : 3 A et 3 B, jamais trois fois de suite le même côté.
fn ordres_possibles() -> Vec<[Cote; 6]> {
    (0u32..64)
        .filter(|m| m.count_ones() == 3)
        .map(|m| {
            let mut ordre = [Cote::A; 6];
            for (i, c) in ordre.iter_mut().enumerate() {
                if m >> i & 1 == 1 {
                    *c = Cote::B;
                }
            }
            ordre
        })
        .filter(|o| !o.windows(3).any(|w| w[0] == w[1] && w[1] == w[2]))
        .collect()
}

fn duel(sujet: Sujet, ordres: &[[Cote; 6]], hasard: &mut impl Hasard) -> DuelPrevu {
    let reference_est_a = hasard.pile_ou_face();
    let ordre_passages = ordres[hasard.entier(ordres.len() as u32) as usize];
    DuelPrevu {
        sujet,
        reference_est_a,
        ordre_passages,
    }
}

/// Le plan d'un test, quand on ne connaît des tests précédents que les zones testées
/// (du plus ancien au plus récent). Voir [`plan_avec_historique`], à préférer : elle sait
/// aussi remettre en tête les indices du dernier test.
pub fn plan(
    mode: &Mode,
    version: &Version,
    historique_zones: &[Zone],
    hasard: &mut impl Hasard,
) -> Vec<DuelPrevu> {
    let historique = HistoriquePlan::depuis_zones(historique_zones);
    plan_avec_historique(mode, version, &historique, hasard)
}

/// Le plan d'un test.
///
/// Règles (ADR-0005) : un duel sur six est un contrôle, jamais le premier ni deux à la
/// suite ; en « duels en jeu », les autres vont par paires, chaque couple (zone, sens)
/// retenu étant joué deux fois (5 couples en rapide, 12 en précis plus un troisième duel
/// pour le premier), sans que les deux duels d'un couple se suivent ; les couples sont pris
/// dans l'ordre de [`HistoriquePlan`] : indices du dernier test, puis les moins récemment
/// testés ; `reference_est_a` et l'ordre des passages sont tirés au hasard.
///
/// `_version` est réservé : le plan n'en dépend pas encore (les pas servent au déroulement).
pub fn plan_avec_historique(
    mode: &Mode,
    _version: &Version,
    historique: &HistoriquePlan,
    hasard: &mut impl Hasard,
) -> Vec<DuelPrevu> {
    let total = mode.nombre_duels() as usize;
    let controles = nombre_controles(total);
    let reels = sujets_reels(mode, total - controles, historique, hasard);
    let reels = melanger_reels(mode, reels, hasard);
    let sujets = inserer_controles(reels, controles, hasard);
    let ordres = ordres_possibles();
    sujets
        .into_iter()
        .map(|sujet| duel(sujet, &ordres, hasard))
        .collect()
}

/// Tire à nouveau l'orientation et l'ordre des passages d'un duel, sans changer son sujet.
/// Sert quand un duel interrompu est rejoué (ADR-0012).
pub fn retirer_duel(prevu: &DuelPrevu, hasard: &mut impl Hasard) -> DuelPrevu {
    duel(prevu.sujet, &ordres_possibles(), hasard)
}

/// Le duel de vérification, ajouté en fin de test : référence contre nouvelle version.
pub fn duel_verification(hasard: &mut impl Hasard) -> DuelPrevu {
    duel(Sujet::Verification, &ordres_possibles(), hasard)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hasard::HasardGraine;
    use crate::profil::Origine;

    fn version() -> Version {
        Version {
            numero: 1,
            creee_le: "2025-01-01T00:00:00Z".into(),
            courbe: Courbe::default(),
            origine: Origine::Manuel,
            pas_par_zone: [4.0; 7],
        }
    }

    fn modes_a_tester() -> Vec<Mode> {
        let mut v = vec![
            Mode::DuelsEnJeu { precis: false },
            Mode::DuelsEnJeu { precis: true },
        ];
        for duels in [1, 2, 3, 5, 6, 7, 9, 12, 15, 18, 20, 30] {
            v.push(Mode::UneSeuleZone {
                zone: Zone::Presence,
                duels,
            });
            v.push(Mode::ProfilContreProfil {
                autre: Courbe::default(),
                duels,
            });
        }
        v
    }

    fn plan_de(mode: &Mode, graine: u64, histo: &[Zone]) -> Vec<DuelPrevu> {
        plan(mode, &version(), histo, &mut HasardGraine::new(graine))
    }

    #[test]
    fn nombre_de_duels_par_mode() {
        assert_eq!(Mode::DuelsEnJeu { precis: false }.nombre_duels(), 12);
        assert_eq!(Mode::DuelsEnJeu { precis: true }.nombre_duels(), 30);
        let m = Mode::UneSeuleZone {
            zone: Zone::Air,
            duels: 8,
        };
        assert_eq!(m.nombre_duels(), 8);
    }

    #[test]
    fn deux_controles_sur_12_et_cinq_sur_30() {
        let compte = |p: &[DuelPrevu]| p.iter().filter(|d| d.est_controle()).count();
        assert_eq!(
            compte(&plan_de(&Mode::DuelsEnJeu { precis: false }, 1, &[])),
            2
        );
        assert_eq!(
            compte(&plan_de(&Mode::DuelsEnJeu { precis: true }, 1, &[])),
            5
        );
    }

    #[test]
    fn proprietes_du_plan_sur_beaucoup_de_graines() {
        for mode in modes_a_tester() {
            let n = mode.nombre_duels() as usize;
            let attendu = (n as f64 / 6.0).round() as usize;
            for graine in 0..200 {
                let p = plan_de(&mode, graine, &[]);
                assert_eq!(p.len(), n, "{mode:?}");
                let controles: Vec<usize> = (0..p.len()).filter(|&i| p[i].est_controle()).collect();
                assert_eq!(controles.len(), attendu, "{mode:?} graine {graine}");
                if n >= 6 {
                    assert!(!controles.is_empty());
                }
                assert!(!controles.contains(&0), "contrôle en premier");
                assert!(
                    controles.windows(2).all(|w| w[1] - w[0] >= 2),
                    "contrôles adjacents {controles:?}"
                );
            }
        }
    }

    #[test]
    fn chaque_ordre_de_passages_a_3_a_3_b_et_pas_trois_de_suite() {
        for mode in modes_a_tester() {
            for graine in 0..100 {
                for d in plan_de(&mode, graine, &[]) {
                    let a = d.ordre_passages.iter().filter(|&&c| c == Cote::A).count();
                    assert_eq!(a, 3);
                    assert!(!d
                        .ordre_passages
                        .windows(3)
                        .any(|w| w[0] == w[1] && w[1] == w[2]));
                }
            }
        }
    }

    #[test]
    fn il_y_a_14_ordres_possibles() {
        // C(6,3) = 20 arrangements, moins 6 qui contiennent AAA ou BBB.
        assert_eq!(ordres_possibles().len(), 14);
    }

    #[test]
    fn tous_les_ordres_et_les_deux_orientations_sortent() {
        let mut ordres = std::collections::HashSet::new();
        let mut a = 0;
        let mut total = 0;
        for graine in 0..300 {
            for d in plan_de(&Mode::DuelsEnJeu { precis: true }, graine, &[]) {
                ordres.insert(d.ordre_passages);
                total += 1;
                if d.reference_est_a {
                    a += 1;
                }
            }
        }
        assert_eq!(ordres.len(), 14);
        let part = a as f64 / total as f64;
        assert!((0.45..0.55).contains(&part), "{part}");
    }

    fn couples_de(p: &[DuelPrevu]) -> Vec<(Zone, Sens)> {
        p.iter()
            .filter_map(|d| match d.sujet {
                Sujet::Zone { zone, sens } => Some((zone, sens)),
                _ => None,
            })
            .collect()
    }

    fn comptes_de(p: &[DuelPrevu]) -> std::collections::HashMap<(Zone, Sens), u32> {
        let mut comptes = std::collections::HashMap::new();
        for c in couples_de(p) {
            *comptes.entry(c).or_insert(0u32) += 1;
        }
        comptes
    }

    #[test]
    fn avec_12_duels_les_10_reels_jouent_5_couples_deux_fois_chacun() {
        for graine in 0..200 {
            let p = plan_de(&Mode::DuelsEnJeu { precis: false }, graine, &[]);
            let comptes = comptes_de(&p);
            assert_eq!(comptes.len(), 5, "5 couples");
            assert!(comptes.values().all(|&n| n == 2), "{comptes:?}");
            // Sans historique : les 5 premiers couples dans l'ordre du spectre.
            for c in [
                (Zone::SubGraves, Sens::Plus),
                (Zone::SubGraves, Sens::Moins),
                (Zone::Graves, Sens::Plus),
                (Zone::Graves, Sens::Moins),
                (Zone::BasMediums, Sens::Plus),
            ] {
                assert!(comptes.contains_key(&c), "{c:?}");
            }
        }
    }

    #[test]
    fn les_deux_duels_d_un_couple_ne_se_suivent_jamais() {
        for mode in [
            Mode::DuelsEnJeu { precis: false },
            Mode::DuelsEnJeu { precis: true },
        ] {
            for graine in 0..300 {
                let p = plan_de(&mode, graine, &[]);
                assert!(
                    !p.windows(2)
                        .any(|w| w[0].sujet == w[1].sujet && !w[0].est_controle()),
                    "{mode:?} graine {graine}"
                );
            }
        }
    }

    #[test]
    fn les_indices_du_test_precedent_passent_en_tete_puis_les_couples_les_plus_anciens() {
        let historique = HistoriquePlan {
            // Du plus ancien au plus récent.
            couples: vec![
                (Zone::Graves, Sens::Plus),
                (Zone::SubGraves, Sens::Plus),
                (Zone::SubGraves, Sens::Moins),
                (Zone::Air, Sens::Moins),
                (Zone::Graves, Sens::Plus),
            ],
            indices: vec![(Zone::Air, Sens::Moins), (Zone::Presence, Sens::Plus)],
        };
        let ordre = couples_par_priorite(&historique);
        assert_eq!(ordre.len(), 14);
        assert_eq!(ordre[0], (Zone::Air, Sens::Moins));
        assert_eq!(ordre[1], (Zone::Presence, Sens::Plus));
        // Puis les jamais testés, dans l'ordre du spectre.
        assert_eq!(ordre[2], (Zone::Graves, Sens::Moins));
        assert_eq!(ordre[3], (Zone::BasMediums, Sens::Plus));
        // Et en dernier les testés, du plus ancien au plus récent (dernière fois).
        assert_eq!(
            ordre[11..],
            [
                (Zone::SubGraves, Sens::Plus),
                (Zone::SubGraves, Sens::Moins),
                (Zone::Graves, Sens::Plus)
            ]
        );
        for graine in 0..50 {
            let p = plan_avec_historique(
                &Mode::DuelsEnJeu { precis: false },
                &version(),
                &historique,
                &mut HasardGraine::new(graine),
            );
            let comptes = comptes_de(&p);
            assert_eq!(comptes.get(&(Zone::Air, Sens::Moins)), Some(&2));
            assert_eq!(comptes.get(&(Zone::Presence, Sens::Plus)), Some(&2));
            assert_eq!(comptes.len(), 5);
        }
    }

    #[test]
    fn l_historique_se_construit_test_apres_test() {
        let mut h = HistoriquePlan::default();
        let p1 = plan_avec_historique(
            &Mode::DuelsEnJeu { precis: false },
            &version(),
            &h,
            &mut HasardGraine::new(1),
        );
        h.ajouter_test(&p1, &[(Zone::Graves, Sens::Plus)]);
        assert_eq!(h.couples.len(), 10);
        assert_eq!(h.indices, vec![(Zone::Graves, Sens::Plus)]);
        let p2 = plan_avec_historique(
            &Mode::DuelsEnJeu { precis: false },
            &version(),
            &h,
            &mut HasardGraine::new(2),
        );
        let c2 = comptes_de(&p2);
        // L'indice est rejoué, puis viennent 4 couples jamais testés.
        assert!(c2.contains_key(&(Zone::Graves, Sens::Plus)));
        let c1 = comptes_de(&p1);
        assert_eq!(c2.keys().filter(|c| c1.contains_key(c)).count(), 1);
        // Les indices sont ceux du dernier test seulement.
        h.ajouter_test(&p2, &[]);
        assert!(h.indices.is_empty());
        assert_eq!(h.couples.len(), 20);
        let j = serde_json::to_string(&h).unwrap();
        assert_eq!(serde_json::from_str::<HistoriquePlan>(&j).unwrap(), h);
    }

    #[test]
    fn l_ancienne_signature_par_zones_marche_toujours() {
        let h = HistoriquePlan::depuis_zones(&[Zone::Graves]);
        assert_eq!(
            h.couples,
            vec![(Zone::Graves, Sens::Plus), (Zone::Graves, Sens::Moins)]
        );
        assert!(h.indices.is_empty());
        let m = Mode::DuelsEnJeu { precis: false };
        assert_eq!(
            plan_de(&m, 5, &[Zone::Graves]),
            plan_avec_historique(&m, &version(), &h, &mut HasardGraine::new(5))
        );
    }

    #[test]
    fn les_zones_de_l_historique_passent_en_dernier() {
        let histo = [Zone::Graves, Zone::Mediums];
        for graine in 0..100 {
            let p = plan_de(&Mode::DuelsEnJeu { precis: false }, graine, &histo);
            for d in &p {
                if let Sujet::Zone { zone, .. } = d.sujet {
                    assert!(!histo.contains(&zone), "{zone:?} testée trop tôt");
                }
            }
        }
    }

    #[test]
    fn avec_30_duels_12_couples_sont_joues_deux_fois_et_le_premier_trois_fois() {
        for graine in 0..50 {
            let p = plan_de(&Mode::DuelsEnJeu { precis: true }, graine, &[]);
            let comptes = comptes_de(&p);
            assert_eq!(comptes.len(), 12);
            assert_eq!(comptes.values().sum::<u32>(), 25);
            assert_eq!(comptes[&(Zone::SubGraves, Sens::Plus)], 3);
            assert_eq!(comptes.values().filter(|&&n| n == 2).count(), 11);
        }
    }

    #[test]
    fn une_seule_zone_alterne_plus_et_moins_sur_cette_zone() {
        for duels in [6, 7, 12, 18] {
            for graine in 0..50 {
                let mode = Mode::UneSeuleZone {
                    zone: Zone::Aigus,
                    duels,
                };
                let p = plan_de(&mode, graine, &[Zone::Aigus]);
                let mut plus = 0i32;
                let mut moins = 0i32;
                for d in &p {
                    match d.sujet {
                        Sujet::Zone { zone, sens } => {
                            assert_eq!(zone, Zone::Aigus);
                            if sens == Sens::Plus {
                                plus += 1
                            } else {
                                moins += 1
                            }
                        }
                        Sujet::Controle => {}
                        autre => panic!("sujet inattendu {autre:?}"),
                    }
                }
                assert!((plus - moins).abs() <= 1, "{plus} {moins}");
                assert!(plus > 0 && moins > 0);
            }
        }
    }

    #[test]
    fn profil_contre_profil_n_a_que_l_autre_profil_et_des_controles() {
        let mode = Mode::ProfilContreProfil {
            autre: Courbe::default(),
            duels: 12,
        };
        for graine in 0..50 {
            let p = plan_de(&mode, graine, &[]);
            assert_eq!(p.iter().filter(|d| d.est_controle()).count(), 2);
            assert!(p
                .iter()
                .filter(|d| !d.est_controle())
                .all(|d| d.sujet == Sujet::AutreProfil));
        }
    }

    #[test]
    fn le_plan_est_deterministe_pour_une_graine_et_varie_selon_elle() {
        let m = Mode::DuelsEnJeu { precis: false };
        assert_eq!(plan_de(&m, 11, &[]), plan_de(&m, 11, &[]));
        assert_ne!(plan_de(&m, 11, &[]), plan_de(&m, 12, &[]));
    }

    #[test]
    fn un_plan_vide_ou_minuscule_ne_plante_pas() {
        for duels in 0..3 {
            let mode = Mode::UneSeuleZone {
                zone: Zone::Air,
                duels,
            };
            let p = plan_de(&mode, 1, &[]);
            assert_eq!(p.len(), duels as usize);
            assert!(p.iter().all(|d| !d.est_controle()));
        }
    }

    #[test]
    fn la_reponse_se_traduit_selon_l_orientation() {
        let ordre = [Cote::A, Cote::B, Cote::A, Cote::B, Cote::A, Cote::B];
        let ref_a = DuelPrevu {
            sujet: Sujet::Controle,
            reference_est_a: true,
            ordre_passages: ordre,
        };
        let ref_b = DuelPrevu {
            reference_est_a: false,
            ..ref_a
        };
        assert_eq!(ref_a.preference(Reponse::A), Preference::Reference);
        assert_eq!(ref_a.preference(Reponse::B), Preference::Variante);
        assert_eq!(ref_b.preference(Reponse::A), Preference::Variante);
        assert_eq!(ref_b.preference(Reponse::B), Preference::Reference);
        assert_eq!(ref_a.preference(Reponse::Pareil), Preference::Aucune);
        assert_eq!(ref_b.preference(Reponse::Pareil), Preference::Aucune);
        assert_eq!(ref_a.cote_reference(), Cote::A);
        assert_eq!(ref_a.cote_variante(), Cote::B);
        assert_eq!(ref_b.cote_reference(), Cote::B);
    }

    #[test]
    fn le_duel_de_verification_est_bien_forme() {
        let d = duel_verification(&mut HasardGraine::new(2));
        assert_eq!(d.sujet, Sujet::Verification);
        assert_eq!(
            d.ordre_passages.iter().filter(|&&c| c == Cote::A).count(),
            3
        );
    }

    #[test]
    fn le_plan_passe_par_json() {
        let p = plan_de(&Mode::DuelsEnJeu { precis: false }, 3, &[]);
        let j = serde_json::to_string(&p).unwrap();
        assert_eq!(serde_json::from_str::<Vec<DuelPrevu>>(&j).unwrap(), p);
        let m = Mode::ProfilContreProfil {
            autre: Courbe::default(),
            duels: 6,
        };
        let j = serde_json::to_string(&m).unwrap();
        assert_eq!(serde_json::from_str::<Mode>(&j).unwrap(), m);
    }
}
