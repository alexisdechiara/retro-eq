//! Le résultat d'un test : ce qu'on retient des réponses, et avec quelle confiance.
//!
//! Tout est ici une fonction pure : mêmes réponses, même résultat. Les règles viennent de
//! l'ADR-0005 (variantes par zone) et de la spec (« Confiance et résultat »).

use crate::confiance::Confiance;
use crate::plan::{DuelPrevu, Preference, Reponse, Sujet};
use crate::profil::Version;
use crate::zone::{filtre_de_zone, ReglagesVariantes, Sens, Zone};
use retro_dsp::{Courbe, Filtre};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Ce que le test a appris d'une zone.
///
/// `duels`, `pour_variante`, `pour_reference` et `pareil` comptent les duels répondus de la
/// zone, **dans les deux sens** (ce qu'on affiche). La confiance, elle, n'utilise que le sens
/// retenu.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResultatZone {
    pub zone: Zone,
    /// Changement retenu (dB, signé) ; 0 si la zone ne change pas.
    pub gain_db: f64,
    pub duels: u32,
    pub pour_variante: u32,
    pub pour_reference: u32,
    pub pareil: u32,
    /// Confiance du changement ; `Indice` quand il n'y a pas de changement.
    pub confiance: Confiance,
    /// La variante a gagné dans les deux sens : on ne change rien et on le signale.
    pub contradictoire: bool,
}

impl ResultatZone {
    /// Vrai si le test change cette zone.
    pub fn est_changee(&self) -> bool {
        self.gain_db != 0.0
    }
}

/// Ce que les duels de contrôle (deux fois la référence) disent du hasard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BilanControles {
    /// Contrôles prévus au plan.
    pub total: u32,
    /// Contrôles qui ont reçu une réponse (les autres ont été passés).
    pub repondus: u32,
    /// Réponses « A » ou « B » : l'auditeur a cru entendre une différence qui n'existe pas.
    pub a_ou_b: u32,
    /// Réponses « Pareil » : la bonne réponse.
    pub pareil: u32,
    /// Au moins la moitié des contrôles répondus ont reçu A ou B : toutes les confiances
    /// baissent d'un cran.
    pub penalise: bool,
}

/// Bilan d'un ensemble de duels contre la référence (mode profil contre profil).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BilanDuels {
    pub duels: u32,
    pub pour_variante: u32,
    pub pour_reference: u32,
    pub pareil: u32,
    /// Qui l'emporte (`Aucune` à égalité).
    pub preference: Preference,
    /// Confiance dans la préférence ; `Indice` à égalité.
    pub confiance: Confiance,
}

/// Ce qu'on préfère dans une appli donnée.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PreferenceAppli {
    pub appli: String,
    pub duels: u32,
    pub pour_variante: u32,
    pub pour_reference: u32,
    pub pareil: u32,
    pub preference: Preference,
}

/// Le résultat complet d'un test.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResultatTest {
    /// Une entrée par zone testée (au moins un duel répondu), dans l'ordre du spectre.
    pub zones: Vec<ResultatZone>,
    pub controles: BilanControles,
    /// La référence plus les filtres retenus, avec le préampli automatique.
    pub nouvelle_courbe: Courbe,
    /// Pas de chaque zone pour la prochaine version (ordre de `Zone::TOUTES`).
    pub nouveaux_pas: [f64; 7],
    /// La réponse brute au duel de vérification, s'il a eu lieu.
    pub verification: Option<Reponse>,
    /// Cette même réponse, traduite : la nouvelle version a-t-elle été préférée ?
    pub verification_preference: Option<Preference>,
    pub confiance: Confiance,
    pub par_appli: Vec<PreferenceAppli>,
    /// Présent pour le mode profil contre profil.
    pub autre_profil: Option<BilanDuels>,
}

impl ResultatTest {
    /// Vrai si au moins une zone change : c'est la condition pour jouer un duel de vérification.
    pub fn a_des_changements(&self) -> bool {
        self.zones.iter().any(ResultatZone::est_changee)
    }
}

/// Compte les réponses d'un ensemble de duels.
#[derive(Debug, Default, Clone, Copy)]
struct Comptes {
    variante: u32,
    reference: u32,
    pareil: u32,
}

impl Comptes {
    fn ajouter(&mut self, p: Preference) {
        match p {
            Preference::Variante => self.variante += 1,
            Preference::Reference => self.reference += 1,
            Preference::Aucune => self.pareil += 1,
        }
    }

    fn total(&self) -> u32 {
        self.variante + self.reference + self.pareil
    }

    fn preference(&self) -> Preference {
        match self.variante.cmp(&self.reference) {
            std::cmp::Ordering::Greater => Preference::Variante,
            std::cmp::Ordering::Less => Preference::Reference,
            std::cmp::Ordering::Equal => Preference::Aucune,
        }
    }
}

/// Deux filtres sont « identiques » s'ils ont le même type, la même fréquence et le même Q
/// (le gain ne compte pas : c'est ce qu'on additionne).
fn meme_filtre(a: &Filtre, b: &Filtre) -> bool {
    a.type_filtre == b.type_filtre
        && (a.frequence_hz - b.frequence_hz).abs() < 1e-6
        && (a.q - b.q).abs() < 1e-6
}

/// Ajoute `nouveau` aux filtres : fusionné (gains additionnés) avec un filtre identique s'il
/// existe, et retiré si la somme s'annule.
fn ajouter_fusionne(filtres: &mut Vec<Filtre>, nouveau: Filtre) {
    match filtres.iter().position(|f| meme_filtre(f, &nouveau)) {
        Some(i) => {
            filtres[i].gain_db += nouveau.gain_db;
            if filtres[i].gain_db.abs() < 1e-9 {
                filtres.remove(i);
            }
        }
        None => filtres.push(nouveau),
    }
}

/// Calcule le résultat d'un test.
///
/// - `reference` : la courbe de référence du test ; `version` : la version dont elle vient
///   (ses `pas_par_zone` ont servi à fabriquer les variantes).
/// - `plan` : les duels réellement joués (sans le duel de vérification) ; `reponses` : une
///   réponse par duel, dans le même ordre, `None` si le duel a été passé.
/// - `verification` : le duel de vérification et sa réponse, s'il a eu lieu. La réponse
///   « A » ou « B » n'a de sens qu'avec `reference_est_a` de *ce* duel, d'où le couple.
/// - `applis` : le nom de l'appli au premier plan pendant chaque duel, même ordre que `plan`.
///
/// Les réponses sont toujours traduites par [`DuelPrevu::preference`] : « A » peut vouloir
/// dire « la variante » ou « la référence » selon `reference_est_a`.
pub fn calculer_resultat(
    reference: &Courbe,
    version: &Version,
    reglages: &ReglagesVariantes,
    plan: &[DuelPrevu],
    reponses: &[Option<Reponse>],
    verification: Option<(&DuelPrevu, Reponse)>,
    applis: &[Option<String>],
) -> ResultatTest {
    // 1. Dépouillement : une passe sur le plan.
    let mut par_zone = [[Comptes::default(); 2]; 7]; // [zone][0 = Plus, 1 = Moins]
    let mut autre_profil: Option<Comptes> = None;
    let mut par_appli: BTreeMap<String, Comptes> = BTreeMap::new();
    let mut controles = BilanControles {
        total: 0,
        repondus: 0,
        a_ou_b: 0,
        pareil: 0,
        penalise: false,
    };

    for (i, duel) in plan.iter().enumerate() {
        let reponse = reponses.get(i).copied().flatten();
        if duel.sujet == Sujet::Controle {
            controles.total += 1;
        }
        let Some(reponse) = reponse else { continue };
        let preference = duel.preference(reponse);
        match duel.sujet {
            Sujet::Controle => {
                controles.repondus += 1;
                if reponse == Reponse::Pareil {
                    controles.pareil += 1;
                } else {
                    controles.a_ou_b += 1;
                }
                continue;
            }
            Sujet::Zone { zone, sens } => {
                let s = if sens == Sens::Plus { 0 } else { 1 };
                par_zone[zone.index()][s].ajouter(preference);
            }
            Sujet::AutreProfil => autre_profil
                .get_or_insert_with(Comptes::default)
                .ajouter(preference),
            Sujet::Verification => continue,
        }
        if let Some(Some(appli)) = applis.get(i) {
            par_appli
                .entry(appli.clone())
                .or_default()
                .ajouter(preference);
        }
    }
    controles.penalise = controles.repondus > 0 && controles.a_ou_b * 2 >= controles.repondus;

    let baisse = |c: Confiance| {
        if controles.penalise {
            c.baisser_d_un_cran()
        } else {
            c
        }
    };

    // 2. Une conclusion par zone testée.
    let mut zones = Vec::new();
    let mut nouveaux_pas = version.pas_par_zone;
    let mut filtres = reference.filtres.clone();
    let mut confiances_brutes_changees = Vec::new();
    for zone in Zone::TOUTES {
        let [plus, moins] = par_zone[zone.index()];
        let repondus = plus.total() + moins.total();
        if repondus == 0 {
            continue; // zone non testée : son pas ne bouge pas
        }
        let plus_gagne = plus.variante > plus.reference;
        let moins_gagne = moins.variante > moins.reference;
        let contradictoire = plus_gagne && moins_gagne;
        let retenu = match (plus_gagne, moins_gagne) {
            (true, false) => Some((Sens::Plus, plus)),
            (false, true) => Some((Sens::Moins, moins)),
            _ => None,
        };
        let pas = version.pas_par_zone[zone.index()];
        let (gain_db, confiance) = match retenu {
            Some((sens, c)) => {
                let gain = sens.signe() * pas;
                ajouter_fusionne(&mut filtres, filtre_de_zone(zone, gain));
                let brute = Confiance::depuis_victoires(c.variante, c.variante + c.reference);
                confiances_brutes_changees.push(brute);
                (gain, baisse(brute))
            }
            None => {
                nouveaux_pas[zone.index()] = reglages.pas_reduit(pas);
                (0.0, Confiance::Indice)
            }
        };
        zones.push(ResultatZone {
            zone,
            gain_db,
            duels: repondus,
            pour_variante: plus.variante + moins.variante,
            pour_reference: plus.reference + moins.reference,
            pareil: plus.pareil + moins.pareil,
            confiance,
            contradictoire,
        });
    }

    let nouvelle_courbe = Courbe {
        preampli_db: reference.preampli_db,
        filtres,
    }
    .avec_preampli_auto();

    // 3. Profil contre profil.
    let autre_profil = autre_profil.map(|c| {
        let preference = c.preference();
        let gagnant = c.variante.max(c.reference);
        let brute = if preference == Preference::Aucune {
            Confiance::Indice
        } else {
            Confiance::depuis_victoires(gagnant, c.variante + c.reference)
        };
        BilanDuels {
            duels: c.total(),
            pour_variante: c.variante,
            pour_reference: c.reference,
            pareil: c.pareil,
            preference,
            confiance: baisse(brute),
        }
    });

    // 4. Vérification et confiance du test.
    let verification_preference = verification.map(|(duel, rep)| duel.preference(rep));
    let confiance = if let Some(p) = verification_preference {
        // La vérification est un seul duel : « préférée 1 fois sur 1 » ne dépasse jamais Indice.
        let victoires = u32::from(p == Preference::Variante);
        let total = u32::from(p != Preference::Aucune);
        baisse(Confiance::depuis_victoires(victoires, total))
    } else if let Some(plus_basse) = confiances_brutes_changees.iter().copied().min() {
        baisse(plus_basse)
    } else {
        // Aucune zone changée : en profil contre profil, c'est le bilan de l'autre profil
        // (déjà baissé si besoin) ; sinon rien n'est établi.
        autre_profil
            .as_ref()
            .map_or(Confiance::Indice, |b| b.confiance)
    };

    let par_appli = par_appli
        .into_iter()
        .map(|(appli, c)| PreferenceAppli {
            appli,
            duels: c.total(),
            pour_variante: c.variante,
            pour_reference: c.reference,
            pareil: c.pareil,
            preference: c.preference(),
        })
        .collect();

    ResultatTest {
        zones,
        controles,
        nouvelle_courbe,
        nouveaux_pas,
        verification: verification.map(|(_, r)| r),
        verification_preference,
        confiance,
        par_appli,
        autre_profil,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::Cote;
    use retro_dsp::{frequences_log, TypeFiltre};

    const ORDRE: [Cote; 6] = [Cote::A, Cote::B, Cote::A, Cote::B, Cote::A, Cote::B];

    fn duel(sujet: Sujet, reference_est_a: bool) -> DuelPrevu {
        DuelPrevu {
            sujet,
            reference_est_a,
            ordre_passages: ORDRE,
        }
    }

    fn duel_zone(zone: Zone, sens: Sens, reference_est_a: bool) -> DuelPrevu {
        duel(Sujet::Zone { zone, sens }, reference_est_a)
    }

    fn version(pas: [f64; 7]) -> Version {
        Version {
            numero: 1,
            creee_le: "2025-01-01T00:00:00Z".into(),
            courbe: Courbe::default(),
            origine: crate::profil::Origine::Manuel,
            pas_par_zone: pas,
        }
    }

    /// Réponse « la variante gagne » pour ce duel, quelle que soit l'orientation.
    fn pour_variante(d: &DuelPrevu) -> Option<Reponse> {
        Some(if d.reference_est_a {
            Reponse::B
        } else {
            Reponse::A
        })
    }

    fn pour_reference(d: &DuelPrevu) -> Option<Reponse> {
        Some(if d.reference_est_a {
            Reponse::A
        } else {
            Reponse::B
        })
    }

    fn calcul(
        reference: &Courbe,
        pas: [f64; 7],
        plan: &[DuelPrevu],
        reponses: &[Option<Reponse>],
    ) -> ResultatTest {
        calculer_resultat(
            reference,
            &version(pas),
            &ReglagesVariantes::default(),
            plan,
            reponses,
            None,
            &vec![None; plan.len()],
        )
    }

    fn zone_de(r: &ResultatTest, zone: Zone) -> &ResultatZone {
        r.zones.iter().find(|z| z.zone == zone).unwrap()
    }

    #[test]
    fn la_reponse_est_traduite_dans_les_deux_orientations() {
        // Référence = A : répondre B veut dire « la variante ».
        let p = vec![duel_zone(Zone::Graves, Sens::Plus, true)];
        let r = calcul(&Courbe::default(), [4.0; 7], &p, &[Some(Reponse::B)]);
        assert_eq!(zone_de(&r, Zone::Graves).pour_variante, 1);
        assert_eq!(zone_de(&r, Zone::Graves).gain_db, 4.0);
        let r = calcul(&Courbe::default(), [4.0; 7], &p, &[Some(Reponse::A)]);
        assert_eq!(zone_de(&r, Zone::Graves).pour_reference, 1);
        assert_eq!(zone_de(&r, Zone::Graves).gain_db, 0.0);

        // Référence = B : répondre A veut dire « la variante ».
        let p = vec![duel_zone(Zone::Graves, Sens::Plus, false)];
        let r = calcul(&Courbe::default(), [4.0; 7], &p, &[Some(Reponse::A)]);
        assert_eq!(zone_de(&r, Zone::Graves).pour_variante, 1);
        assert_eq!(zone_de(&r, Zone::Graves).gain_db, 4.0);
        let r = calcul(&Courbe::default(), [4.0; 7], &p, &[Some(Reponse::B)]);
        assert_eq!(zone_de(&r, Zone::Graves).pour_reference, 1);
        assert_eq!(zone_de(&r, Zone::Graves).gain_db, 0.0);
    }

    #[test]
    fn une_zone_gagnante_en_moins_baisse_le_gain() {
        let p = vec![duel_zone(Zone::Aigus, Sens::Moins, false)];
        let r = calcul(&Courbe::default(), [4.0; 7], &p, &[pour_variante(&p[0])]);
        assert_eq!(zone_de(&r, Zone::Aigus).gain_db, -4.0);
        let f = r.nouvelle_courbe.filtres.last().unwrap();
        assert_eq!(f.gain_db, -4.0);
        assert_eq!(f.type_filtre, TypeFiltre::Cloche);
    }

    #[test]
    fn le_gain_retenu_est_le_pas_de_la_zone_dans_la_version() {
        let mut pas = [4.0; 7];
        pas[Zone::Presence.index()] = 2.0;
        let p = vec![duel_zone(Zone::Presence, Sens::Plus, true)];
        let r = calcul(&Courbe::default(), pas, &p, &[pour_variante(&p[0])]);
        assert_eq!(zone_de(&r, Zone::Presence).gain_db, 2.0);
    }

    #[test]
    fn la_variante_doit_gagner_plus_de_duels_que_la_reference() {
        let mut p = Vec::new();
        for i in 0..4 {
            p.push(duel_zone(Zone::Mediums, Sens::Plus, i % 2 == 0));
        }
        // 2 pour la variante, 2 pour la référence : égalité, pas de changement.
        let rep = [
            pour_variante(&p[0]),
            pour_variante(&p[1]),
            pour_reference(&p[2]),
            pour_reference(&p[3]),
        ];
        let r = calcul(&Courbe::default(), [4.0; 7], &p, &rep);
        assert_eq!(zone_de(&r, Zone::Mediums).gain_db, 0.0);
        // 3 contre 1 : changement.
        let rep = [
            pour_variante(&p[0]),
            pour_variante(&p[1]),
            pour_variante(&p[2]),
            pour_reference(&p[3]),
        ];
        let r = calcul(&Courbe::default(), [4.0; 7], &p, &rep);
        assert_eq!(zone_de(&r, Zone::Mediums).gain_db, 4.0);
    }

    #[test]
    fn pareil_ne_compte_pour_personne_et_pas_dans_le_total_binomial() {
        let p: Vec<_> = (0..4)
            .map(|i| duel_zone(Zone::Graves, Sens::Plus, i % 2 == 0))
            .collect();
        let rep = [
            pour_variante(&p[0]),
            Some(Reponse::Pareil),
            Some(Reponse::Pareil),
            Some(Reponse::Pareil),
        ];
        let r = calcul(&Courbe::default(), [4.0; 7], &p, &rep);
        let z = zone_de(&r, Zone::Graves);
        assert_eq!(
            (z.duels, z.pour_variante, z.pour_reference, z.pareil),
            (4, 1, 0, 3)
        );
        assert_eq!(z.gain_db, 4.0);
        // 1 victoire sur 1 duel hors Pareil : risque 0,5, Indice.
        assert_eq!(z.confiance, Confiance::Indice);
    }

    #[test]
    fn confiance_d_une_zone_selon_les_victoires_dans_le_sens_retenu() {
        for (victoires, attendu) in [
            (1, Confiance::Indice),
            (2, Confiance::Probable),
            (4, Confiance::Probable),
            (5, Confiance::Sur),
        ] {
            let p: Vec<_> = (0..victoires)
                .map(|i| duel_zone(Zone::Graves, Sens::Plus, i % 2 == 0))
                .collect();
            let rep: Vec<_> = p.iter().map(pour_variante).collect();
            let r = calcul(&Courbe::default(), [4.0; 7], &p, &rep);
            assert_eq!(zone_de(&r, Zone::Graves).confiance, attendu, "{victoires}");
        }
    }

    #[test]
    fn la_confiance_ignore_l_autre_sens() {
        // Plus : 2 pour la variante. Moins : 3 pour la référence (sans effet sur le risque).
        let mut p = vec![
            duel_zone(Zone::Graves, Sens::Plus, true),
            duel_zone(Zone::Graves, Sens::Plus, false),
        ];
        for _ in 0..3 {
            p.push(duel_zone(Zone::Graves, Sens::Moins, true));
        }
        let rep: Vec<_> = p
            .iter()
            .enumerate()
            .map(|(i, d)| {
                if i < 2 {
                    pour_variante(d)
                } else {
                    pour_reference(d)
                }
            })
            .collect();
        let r = calcul(&Courbe::default(), [4.0; 7], &p, &rep);
        let z = zone_de(&r, Zone::Graves);
        assert_eq!(z.gain_db, 4.0);
        assert_eq!(z.confiance, Confiance::Probable);
        assert_eq!((z.duels, z.pour_variante, z.pour_reference), (5, 2, 3));
        assert!(!z.contradictoire);
    }

    #[test]
    fn gagner_dans_les_deux_sens_est_contradictoire_et_ne_change_rien() {
        let p = vec![
            duel_zone(Zone::Aigus, Sens::Plus, true),
            duel_zone(Zone::Aigus, Sens::Moins, true),
        ];
        let rep: Vec<_> = p.iter().map(pour_variante).collect();
        let r = calcul(&Courbe::default(), [4.0; 7], &p, &rep);
        let z = zone_de(&r, Zone::Aigus);
        assert!(z.contradictoire);
        assert_eq!(z.gain_db, 0.0);
        assert!(r.nouvelle_courbe.filtres.is_empty());
        // Aucun changement retenu dans une zone testée : le pas est divisé.
        assert_eq!(r.nouveaux_pas[Zone::Aigus.index()], 2.0);
        assert!(!r.a_des_changements());
    }

    #[test]
    fn les_pas_se_divisent_sans_changement_et_restent_avec_changement_ou_sans_test() {
        let p = vec![
            duel_zone(Zone::Graves, Sens::Plus, true), // changement retenu
            duel_zone(Zone::Mediums, Sens::Plus, true), // zone testée, la référence gagne
            duel_zone(Zone::Presence, Sens::Plus, true), // tout « Pareil »
            duel_zone(Zone::Aigus, Sens::Plus, true),  // passé
        ];
        let rep = [
            pour_variante(&p[0]),
            pour_reference(&p[1]),
            Some(Reponse::Pareil),
            None,
        ];
        let r = calcul(&Courbe::default(), [4.0; 7], &p, &rep);
        let pas = r.nouveaux_pas;
        assert_eq!(
            pas[Zone::Graves.index()],
            4.0,
            "changement retenu : inchangé"
        );
        assert_eq!(
            pas[Zone::Mediums.index()],
            2.0,
            "testée sans changement : divisé"
        );
        assert_eq!(pas[Zone::Presence.index()], 2.0, "pareil partout : divisé");
        assert_eq!(
            pas[Zone::Aigus.index()],
            4.0,
            "duel passé : zone non testée"
        );
        assert_eq!(pas[Zone::Air.index()], 4.0, "pas dans le plan");
        assert!(r.zones.iter().all(|z| z.zone != Zone::Aigus));
    }

    #[test]
    fn le_pas_ne_descend_pas_sous_le_plancher() {
        let p = vec![duel_zone(Zone::Graves, Sens::Plus, true)];
        let mut pas = [4.0; 7];
        pas[Zone::Graves.index()] = 1.0;
        let r = calcul(&Courbe::default(), pas, &p, &[pour_reference(&p[0])]);
        assert_eq!(r.nouveaux_pas[Zone::Graves.index()], 1.0);
        pas[Zone::Graves.index()] = 1.5;
        let r = calcul(&Courbe::default(), pas, &p, &[pour_reference(&p[0])]);
        assert_eq!(r.nouveaux_pas[Zone::Graves.index()], 1.0);
    }

    #[test]
    fn un_filtre_identique_dans_la_reference_est_fusionne() {
        let existant = filtre_de_zone(Zone::Graves, 2.0);
        let reference = Courbe {
            preampli_db: 0.0,
            filtres: vec![existant],
        };
        let p = vec![duel_zone(Zone::Graves, Sens::Plus, true)];
        let r = calcul(&reference, [4.0; 7], &p, &[pour_variante(&p[0])]);
        assert_eq!(r.nouvelle_courbe.filtres.len(), 1);
        assert_eq!(r.nouvelle_courbe.filtres[0].gain_db, 6.0);
    }

    #[test]
    fn une_fusion_qui_s_annule_retire_le_filtre() {
        let reference = Courbe {
            preampli_db: 0.0,
            filtres: vec![filtre_de_zone(Zone::SubGraves, 4.0)],
        };
        let p = vec![duel_zone(Zone::SubGraves, Sens::Moins, true)];
        let r = calcul(&reference, [4.0; 7], &p, &[pour_variante(&p[0])]);
        assert!(r.nouvelle_courbe.filtres.is_empty());
    }

    #[test]
    fn un_filtre_different_est_ajoute_a_cote() {
        let autre = Filtre {
            type_filtre: TypeFiltre::Cloche,
            frequence_hz: 1000.0,
            gain_db: 1.0,
            q: 1.41,
        };
        let reference = Courbe {
            preampli_db: 0.0,
            filtres: vec![autre],
        };
        let p = vec![duel_zone(Zone::Mediums, Sens::Plus, true)]; // 1 kHz mais Q 0,67
        let r = calcul(&reference, [4.0; 7], &p, &[pour_variante(&p[0])]);
        assert_eq!(r.nouvelle_courbe.filtres.len(), 2);
        assert_eq!(r.nouvelle_courbe.filtres[0], autre);
    }

    #[test]
    fn la_nouvelle_courbe_a_le_preampli_automatique() {
        let p = vec![
            duel_zone(Zone::Graves, Sens::Plus, true),
            duel_zone(Zone::Presence, Sens::Plus, false),
        ];
        let rep: Vec<_> = p.iter().map(pour_variante).collect();
        let r = calcul(&Courbe::default(), [4.0; 7], &p, &rep);
        let c = &r.nouvelle_courbe;
        assert_eq!(c.filtres.len(), 2);
        assert!(c.preampli_db < -3.0);
        assert_eq!(c.preampli_db, c.clone().avec_preampli_auto().preampli_db);
        for hz in frequences_log(300) {
            assert!(c.reponse_totale_db(hz) <= 0.01);
        }
    }

    #[test]
    fn duels_passes_et_zone_jamais_repondue_ne_comptent_pas() {
        let p = vec![
            duel_zone(Zone::Graves, Sens::Plus, true),
            duel_zone(Zone::Graves, Sens::Moins, true),
        ];
        let r = calcul(&Courbe::default(), [4.0; 7], &p, &[None, None]);
        assert!(r.zones.is_empty());
        assert_eq!(r.nouveaux_pas, [4.0; 7]);
        assert_eq!(r.confiance, Confiance::Indice);
        // Des réponses manquantes en fin de liste valent « passé ».
        let r = calcul(&Courbe::default(), [4.0; 7], &p, &[]);
        assert!(r.zones.is_empty());
    }

    fn plan_avec_controles(n_controles: usize) -> Vec<DuelPrevu> {
        let mut p = vec![duel_zone(Zone::Graves, Sens::Plus, true)];
        p.extend((0..5).map(|i| duel_zone(Zone::Graves, Sens::Plus, i % 2 == 0)));
        p.extend((0..n_controles).map(|i| duel(Sujet::Controle, i % 2 == 0)));
        p
    }

    #[test]
    fn la_moitie_des_controles_a_ou_b_baisse_la_confiance_d_un_cran() {
        // 5 victoires sur 5 : Sûr sans pénalité.
        let mut p = plan_avec_controles(2);
        let mut rep: Vec<_> = p[..6].iter().map(pour_variante).collect();
        rep.extend([Some(Reponse::A), Some(Reponse::Pareil)]);
        let r = calcul(&Courbe::default(), [4.0; 7], &p, &rep[..]);
        // 6 victoires sur 6 → Sûr, baissé en Probable : 1 contrôle sur 2 = la moitié.
        assert!(r.controles.penalise);
        assert_eq!(zone_de(&r, Zone::Graves).confiance, Confiance::Probable);
        assert_eq!(r.confiance, Confiance::Probable);
        assert_eq!(r.controles.total, 2);
        assert_eq!(r.controles.repondus, 2);
        assert_eq!(r.controles.a_ou_b, 1);
        assert_eq!(r.controles.pareil, 1);

        // Aucun contrôle trompé : pas de pénalité.
        rep[6] = Some(Reponse::Pareil);
        let r = calcul(&Courbe::default(), [4.0; 7], &p, &rep);
        assert!(!r.controles.penalise);
        assert_eq!(zone_de(&r, Zone::Graves).confiance, Confiance::Sur);
        assert_eq!(r.confiance, Confiance::Sur);

        // Un seul contrôle sur 3 trompé : moins de la moitié, pas de pénalité.
        p = plan_avec_controles(3);
        let mut rep: Vec<_> = p[..6].iter().map(pour_variante).collect();
        rep.extend([
            Some(Reponse::B),
            Some(Reponse::Pareil),
            Some(Reponse::Pareil),
        ]);
        let r = calcul(&Courbe::default(), [4.0; 7], &p, &rep);
        assert!(!r.controles.penalise);
        assert_eq!(r.confiance, Confiance::Sur);
    }

    #[test]
    fn les_controles_passes_ne_comptent_pas_dans_la_penalite() {
        let p = plan_avec_controles(2);
        let mut rep: Vec<_> = p[..6].iter().map(pour_variante).collect();
        rep.extend([Some(Reponse::Pareil), None]);
        let r = calcul(&Courbe::default(), [4.0; 7], &p, &rep);
        assert_eq!((r.controles.total, r.controles.repondus), (2, 1));
        assert!(!r.controles.penalise);
        // Tous les contrôles passés : pas de pénalité non plus.
        let mut rep: Vec<_> = p[..6].iter().map(pour_variante).collect();
        rep.extend([None, None]);
        let r = calcul(&Courbe::default(), [4.0; 7], &p, &rep);
        assert!(!r.controles.penalise);
    }

    #[test]
    fn les_controles_ne_changent_aucune_zone_ni_aucun_pas() {
        let p = vec![duel(Sujet::Controle, true), duel(Sujet::Controle, false)];
        let r = calcul(
            &Courbe::default(),
            [4.0; 7],
            &p,
            &[Some(Reponse::A), Some(Reponse::B)],
        );
        assert!(r.zones.is_empty());
        assert_eq!(r.nouveaux_pas, [4.0; 7]);
        assert!(r.controles.penalise);
    }

    #[test]
    fn la_confiance_du_test_est_la_plus_basse_des_zones_changees() {
        let mut p = Vec::new();
        for i in 0..5 {
            p.push(duel_zone(Zone::Graves, Sens::Plus, i % 2 == 0)); // 5/5 : Sûr
        }
        p.push(duel_zone(Zone::Aigus, Sens::Plus, true)); // 1/1 : Indice
        let rep: Vec<_> = p.iter().map(pour_variante).collect();
        let r = calcul(&Courbe::default(), [4.0; 7], &p, &rep);
        assert_eq!(zone_de(&r, Zone::Graves).confiance, Confiance::Sur);
        assert_eq!(zone_de(&r, Zone::Aigus).confiance, Confiance::Indice);
        assert_eq!(r.confiance, Confiance::Indice);
    }

    #[test]
    fn sans_zone_changee_la_confiance_du_test_est_indice() {
        let p = vec![duel_zone(Zone::Graves, Sens::Plus, true)];
        let r = calcul(&Courbe::default(), [4.0; 7], &p, &[pour_reference(&p[0])]);
        assert_eq!(r.confiance, Confiance::Indice);
    }

    #[test]
    fn la_verification_donne_la_confiance_du_test_et_se_traduit_selon_l_orientation() {
        // Cinq victoires : toutes les zones sont Sûr, mais la vérification (1 sur 1) fait Indice.
        let p: Vec<_> = (0..5)
            .map(|i| duel_zone(Zone::Graves, Sens::Plus, i % 2 == 0))
            .collect();
        let rep: Vec<_> = p.iter().map(pour_variante).collect();
        let v = version([4.0; 7]);
        for (ref_est_a, reponse, attendu) in [
            (true, Reponse::B, Preference::Variante),
            (true, Reponse::A, Preference::Reference),
            (false, Reponse::A, Preference::Variante),
            (false, Reponse::B, Preference::Reference),
            (false, Reponse::Pareil, Preference::Aucune),
        ] {
            let d = duel(Sujet::Verification, ref_est_a);
            let r = calculer_resultat(
                &Courbe::default(),
                &v,
                &ReglagesVariantes::default(),
                &p,
                &rep,
                Some((&d, reponse)),
                &vec![None; 5],
            );
            assert_eq!(r.verification, Some(reponse));
            assert_eq!(r.verification_preference, Some(attendu));
            assert_eq!(r.confiance, Confiance::Indice);
            assert_eq!(zone_de(&r, Zone::Graves).confiance, Confiance::Sur);
        }
    }

    #[test]
    fn par_appli_compte_les_duels_et_la_preference() {
        let p = vec![
            duel_zone(Zone::Graves, Sens::Plus, true),
            duel_zone(Zone::Graves, Sens::Plus, false),
            duel_zone(Zone::Aigus, Sens::Plus, true),
            duel(Sujet::Controle, true),
            duel_zone(Zone::Aigus, Sens::Moins, true),
            duel_zone(Zone::Aigus, Sens::Moins, true),
        ];
        let rep = [
            pour_variante(&p[0]),
            pour_variante(&p[1]),
            pour_reference(&p[2]),
            Some(Reponse::A),
            Some(Reponse::Pareil),
            None,
        ];
        let applis = [
            Some("Jeu.exe".to_string()),
            Some("Jeu.exe".to_string()),
            Some("Musique.exe".to_string()),
            Some("Jeu.exe".to_string()), // le contrôle ne compte pas
            Some("Musique.exe".to_string()),
            Some("Jeu.exe".to_string()), // passé : ne compte pas
        ];
        let r = calculer_resultat(
            &Courbe::default(),
            &version([4.0; 7]),
            &ReglagesVariantes::default(),
            &p,
            &rep,
            None,
            &applis,
        );
        assert_eq!(r.par_appli.len(), 2);
        let jeu = &r.par_appli[0];
        assert_eq!(jeu.appli, "Jeu.exe");
        assert_eq!((jeu.duels, jeu.pour_variante), (2, 2));
        assert_eq!(jeu.preference, Preference::Variante);
        let musique = &r.par_appli[1];
        assert_eq!(musique.appli, "Musique.exe");
        assert_eq!(
            (musique.duels, musique.pour_reference, musique.pareil),
            (2, 1, 1)
        );
        assert_eq!(musique.preference, Preference::Reference);
    }

    #[test]
    fn profil_contre_profil_donne_un_bilan_et_ne_touche_pas_aux_zones() {
        let mut p: Vec<_> = (0..4)
            .map(|i| duel(Sujet::AutreProfil, i % 2 == 0))
            .collect();
        p.push(duel(Sujet::Controle, true));
        let mut rep: Vec<_> = p[..4].iter().map(pour_variante).collect();
        rep.push(Some(Reponse::Pareil));
        let r = calcul(&Courbe::default(), [4.0; 7], &p, &rep);
        let b = r.autre_profil.as_ref().unwrap();
        assert_eq!(b.duels, 4);
        assert_eq!(b.pour_variante, 4);
        assert_eq!(b.preference, Preference::Variante);
        assert_eq!(b.confiance, Confiance::Probable); // 4/4 : 0,0625
        assert_eq!(r.confiance, Confiance::Probable);
        assert!(r.zones.is_empty());
        assert_eq!(r.nouveaux_pas, [4.0; 7]);
        // Sans profil contre profil, pas de bilan.
        let r = calcul(&Courbe::default(), [4.0; 7], &[], &[]);
        assert!(r.autre_profil.is_none());
    }

    #[test]
    fn profil_contre_profil_a_egalite_ou_avec_controles_rates() {
        let p = vec![
            duel(Sujet::AutreProfil, true),
            duel(Sujet::AutreProfil, true),
            duel(Sujet::Controle, true),
        ];
        let rep = [
            pour_variante(&p[0]),
            pour_reference(&p[1]),
            Some(Reponse::B),
        ];
        let r = calcul(&Courbe::default(), [4.0; 7], &p, &rep);
        let b = r.autre_profil.unwrap();
        assert_eq!(b.preference, Preference::Aucune);
        assert_eq!(b.confiance, Confiance::Indice);

        // 5 victoires sur 5 pour l'autre profil, mais le contrôle (1 sur 1) est raté : baisse.
        let mut p: Vec<_> = (0..5).map(|_| duel(Sujet::AutreProfil, true)).collect();
        p.push(duel(Sujet::Controle, true));
        let mut rep: Vec<_> = p[..5].iter().map(pour_variante).collect();
        rep.push(Some(Reponse::A));
        let r = calcul(&Courbe::default(), [4.0; 7], &p, &rep);
        assert_eq!(r.autre_profil.unwrap().confiance, Confiance::Probable);
        assert_eq!(r.confiance, Confiance::Probable);
    }

    #[test]
    fn le_resultat_passe_par_json() {
        let p = vec![duel_zone(Zone::Graves, Sens::Plus, true)];
        let r = calcul(&Courbe::default(), [4.0; 7], &p, &[pour_variante(&p[0])]);
        let j = serde_json::to_string(&r).unwrap();
        assert_eq!(serde_json::from_str::<ResultatTest>(&j).unwrap(), r);
    }
}
