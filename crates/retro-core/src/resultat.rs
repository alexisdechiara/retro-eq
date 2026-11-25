//! Le résultat d'un test : ce qu'on retient des réponses, et avec quelle confiance.
//!
//! Tout est ici une fonction pure : mêmes réponses, même résultat. Les règles viennent de
//! l'ADR-0005 (variantes par zone) et de la spec (« Confiance et résultat »).

use crate::confiance::{risque_hasard, Confiance};
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
    /// Confiance du changement (au moins 2 victoires sans défaite : jamais moins que
    /// Probable) ; `Indice` quand il n'y a pas de changement.
    pub confiance: Confiance,
    /// La variante a gagné dans les deux sens : on ne change rien et on le signale.
    pub contradictoire: bool,
    /// Un indice : dans ce sens, la variante a gagné un seul duel sans en perdre. Rien ne
    /// change, le pas de la zone est gardé, et ce couple passera en tête du test suivant.
    #[serde(default)]
    pub indice: Option<Sens>,
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
    /// Au moins la moitié des contrôles répondus ont reçu A ou B. Toujours égal à
    /// `rien_retenu` (gardé pour les journaux et le code écrits avant cette règle).
    pub penalise: bool,
    /// Les réponses ne distinguent pas deux versions identiques : **aucun changement
    /// n'est retenu** pour ce test (courbe et pas inchangés). À expliquer à l'écran.
    #[serde(default)]
    pub rien_retenu: bool,
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
    /// Confiance dans la préférence, sur un risque bilatéral (le gagnant est désigné
    /// après coup) ; `Indice` à égalité ou si les contrôles ont bloqué le test.
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
    /// Les couples (zone, sens) restés à l'état d'indice : à rejouer en tête du prochain
    /// test (voir [`crate::plan::HistoriquePlan`]).
    #[serde(default)]
    pub indices: Vec<(Zone, Sens)>,
    /// Faut-il proposer d'activer la nouvelle version ? Vrai si au moins une zone change
    /// et que le duel de vérification n'a pas préféré la référence (« Pareil », ou pas de
    /// vérification, ne s'y opposent pas). Faux quand il n'y a rien à activer.
    #[serde(default)]
    pub recommandee: bool,
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
        rien_retenu: false,
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
    // La barrière des contrôles : si la moitié au moins des contrôles répondus ont reçu A
    // ou B, les réponses ne distinguent pas deux versions identiques. On ne retient rien.
    let rien_retenu = controles.repondus > 0 && controles.a_ou_b * 2 >= controles.repondus;
    controles.penalise = rien_retenu;
    controles.rien_retenu = rien_retenu;

    // 2. Une conclusion par zone testée.
    // Un sens « gagne » avec au moins deux victoires et aucune défaite ; une seule
    // victoire sans défaite n'est qu'un indice. « Pareil » ne compte pour personne.
    let gagne = |c: Comptes| !rien_retenu && c.variante >= 2 && c.reference == 0;
    let est_indice = |c: Comptes| !rien_retenu && c.variante == 1 && c.reference == 0;
    let mut zones = Vec::new();
    let mut nouveaux_pas = version.pas_par_zone;
    let mut filtres = reference.filtres.clone();
    let mut confiances_changees = Vec::new();
    let mut indices = Vec::new();
    for zone in Zone::TOUTES {
        let [plus, moins] = par_zone[zone.index()];
        let repondus = plus.total() + moins.total();
        if repondus == 0 {
            continue; // zone non testée : son pas ne bouge pas
        }
        let contradictoire = gagne(plus) && gagne(moins);
        let retenu = match (gagne(plus), gagne(moins)) {
            (true, false) => Some((Sens::Plus, plus)),
            (false, true) => Some((Sens::Moins, moins)),
            _ => None,
        };
        let indice = match (retenu, contradictoire, est_indice(plus), est_indice(moins)) {
            (None, false, true, false) => Some(Sens::Plus),
            (None, false, false, true) => Some(Sens::Moins),
            _ => None,
        };
        let pas = version.pas_par_zone[zone.index()];
        let (gain_db, confiance) = match retenu {
            Some((sens, c)) => {
                let gain = sens.signe() * pas;
                ajouter_fusionne(&mut filtres, filtre_de_zone(zone, gain));
                let confiance = Confiance::depuis_victoires(c.variante, c.variante + c.reference);
                confiances_changees.push(confiance);
                (gain, confiance)
            }
            None => {
                match indice {
                    // Zone encore à l'étude : son pas reste tel quel.
                    Some(sens) => indices.push((zone, sens)),
                    None if rien_retenu => {}
                    None => nouveaux_pas[zone.index()] = reglages.pas_reduit(pas),
                }
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
            indice,
        });
    }

    let nouvelle_courbe = Courbe {
        preampli_db: reference.preampli_db,
        filtres,
    }
    .avec_preampli_auto();

    // 3. Profil contre profil. Le gagnant est désigné après avoir vu les réponses : le
    // risque est donc bilatéral (deux fois la queue d'un seul côté, au plus 1).
    let autre_profil = autre_profil.map(|c| {
        let preference = c.preference();
        let gagnant = c.variante.max(c.reference);
        let confiance = if preference == Preference::Aucune || rien_retenu {
            Confiance::Indice
        } else {
            let risque = 2.0 * risque_hasard(gagnant, c.variante + c.reference);
            Confiance::depuis_risque(risque.min(1.0))
        };
        BilanDuels {
            duels: c.total(),
            pour_variante: c.variante,
            pour_reference: c.reference,
            pareil: c.pareil,
            preference,
            confiance,
        }
    });

    // 4. Vérification et confiance du test.
    let verification_preference = verification.map(|(duel, rep)| duel.preference(rep));
    let confiance = if let Some(p) = verification_preference {
        // La vérification est un seul duel : « préférée 1 fois sur 1 » ne dépasse jamais Indice.
        let victoires = u32::from(p == Preference::Variante);
        let total = u32::from(p != Preference::Aucune);
        Confiance::depuis_victoires(victoires, total)
    } else if let Some(plus_basse) = confiances_changees.iter().copied().min() {
        plus_basse
    } else {
        // Aucune zone changée : en profil contre profil, c'est le bilan de l'autre profil ;
        // sinon rien n'est établi.
        autre_profil
            .as_ref()
            .map_or(Confiance::Indice, |b| b.confiance)
    };
    let recommandee = zones.iter().any(ResultatZone::est_changee)
        && verification_preference != Some(Preference::Reference);

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
        indices,
        recommandee,
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

    /// Deux duels du même couple, une fois dans chaque orientation.
    fn paire(zone: Zone, sens: Sens) -> Vec<DuelPrevu> {
        vec![duel_zone(zone, sens, true), duel_zone(zone, sens, false)]
    }

    fn tous_pour_variante(p: &[DuelPrevu]) -> Vec<Option<Reponse>> {
        p.iter().map(pour_variante).collect()
    }

    #[test]
    fn la_reponse_est_traduite_dans_les_deux_orientations() {
        // Référence = A : répondre B veut dire « la variante ».
        let p = vec![duel_zone(Zone::Graves, Sens::Plus, true)];
        let r = calcul(&Courbe::default(), [4.0; 7], &p, &[Some(Reponse::B)]);
        assert_eq!(zone_de(&r, Zone::Graves).pour_variante, 1);
        let r = calcul(&Courbe::default(), [4.0; 7], &p, &[Some(Reponse::A)]);
        assert_eq!(zone_de(&r, Zone::Graves).pour_reference, 1);

        // Référence = B : répondre A veut dire « la variante ».
        let p = vec![duel_zone(Zone::Graves, Sens::Plus, false)];
        let r = calcul(&Courbe::default(), [4.0; 7], &p, &[Some(Reponse::A)]);
        assert_eq!(zone_de(&r, Zone::Graves).pour_variante, 1);
        let r = calcul(&Courbe::default(), [4.0; 7], &p, &[Some(Reponse::B)]);
        assert_eq!(zone_de(&r, Zone::Graves).pour_reference, 1);
    }

    #[test]
    fn deux_victoires_sans_defaite_changent_la_zone() {
        let p = paire(Zone::Graves, Sens::Plus);
        let r = calcul(&Courbe::default(), [4.0; 7], &p, &tous_pour_variante(&p));
        let z = zone_de(&r, Zone::Graves);
        assert_eq!(z.gain_db, 4.0);
        assert_eq!(z.indice, None);
        assert_eq!(z.confiance, Confiance::Probable);
        assert!(r.a_des_changements());
        assert!(r.indices.is_empty());
    }

    #[test]
    fn une_seule_victoire_est_un_indice_qui_ne_change_rien_et_garde_le_pas() {
        let p = paire(Zone::Graves, Sens::Moins);
        let rep = [pour_variante(&p[0]), Some(Reponse::Pareil)];
        let r = calcul(&Courbe::default(), [4.0; 7], &p, &rep);
        let z = zone_de(&r, Zone::Graves);
        assert_eq!(z.gain_db, 0.0);
        assert_eq!(z.indice, Some(Sens::Moins));
        assert!(!r.a_des_changements());
        assert_eq!(r.indices, vec![(Zone::Graves, Sens::Moins)]);
        assert_eq!(
            r.nouveaux_pas[Zone::Graves.index()],
            4.0,
            "zone encore à l'étude"
        );
        assert!(r.nouvelle_courbe.filtres.is_empty());
        // Un seul duel joué et gagné : indice aussi.
        let p = vec![duel_zone(Zone::Air, Sens::Plus, true)];
        let r = calcul(&Courbe::default(), [4.0; 7], &p, &tous_pour_variante(&p));
        assert_eq!(r.indices, vec![(Zone::Air, Sens::Plus)]);
    }

    #[test]
    fn une_defaite_empeche_le_changement_et_l_indice() {
        // 1 victoire, 1 défaite.
        let p = paire(Zone::Mediums, Sens::Plus);
        let rep = [pour_variante(&p[0]), pour_reference(&p[1])];
        let r = calcul(&Courbe::default(), [4.0; 7], &p, &rep);
        let z = zone_de(&r, Zone::Mediums);
        assert_eq!((z.gain_db, z.indice), (0.0, None));
        assert_eq!(r.nouveaux_pas[Zone::Mediums.index()], 2.0);
        // 3 victoires, 1 défaite : toujours rien.
        let mut p = paire(Zone::Mediums, Sens::Plus);
        p.extend(paire(Zone::Mediums, Sens::Plus));
        let mut rep = tous_pour_variante(&p);
        rep[3] = pour_reference(&p[3]);
        let r = calcul(&Courbe::default(), [4.0; 7], &p, &rep);
        let z = zone_de(&r, Zone::Mediums);
        assert_eq!((z.gain_db, z.indice), (0.0, None));
        assert!(r.indices.is_empty());
    }

    #[test]
    fn un_indice_dans_chaque_sens_s_annule() {
        let p = vec![
            duel_zone(Zone::Aigus, Sens::Plus, true),
            duel_zone(Zone::Aigus, Sens::Moins, true),
        ];
        let r = calcul(&Courbe::default(), [4.0; 7], &p, &tous_pour_variante(&p));
        let z = zone_de(&r, Zone::Aigus);
        assert_eq!((z.gain_db, z.indice, z.contradictoire), (0.0, None, false));
        assert!(r.indices.is_empty());
        assert_eq!(r.nouveaux_pas[Zone::Aigus.index()], 2.0);
    }

    #[test]
    fn un_changement_retenu_efface_l_indice_de_l_autre_sens() {
        let mut p = paire(Zone::Aigus, Sens::Plus);
        p.push(duel_zone(Zone::Aigus, Sens::Moins, true));
        let r = calcul(&Courbe::default(), [4.0; 7], &p, &tous_pour_variante(&p));
        let z = zone_de(&r, Zone::Aigus);
        assert_eq!((z.gain_db, z.indice), (4.0, None));
        assert!(r.indices.is_empty());
    }

    #[test]
    fn une_zone_gagnante_en_moins_baisse_le_gain() {
        let p = paire(Zone::Aigus, Sens::Moins);
        let r = calcul(&Courbe::default(), [4.0; 7], &p, &tous_pour_variante(&p));
        assert_eq!(zone_de(&r, Zone::Aigus).gain_db, -4.0);
        let f = r.nouvelle_courbe.filtres.last().unwrap();
        assert_eq!(f.gain_db, -4.0);
        assert_eq!(f.type_filtre, TypeFiltre::Cloche);
    }

    #[test]
    fn le_gain_retenu_est_le_pas_de_la_zone_dans_la_version() {
        let mut pas = [4.0; 7];
        pas[Zone::Presence.index()] = 2.0;
        let p = paire(Zone::Presence, Sens::Plus);
        let r = calcul(&Courbe::default(), pas, &p, &tous_pour_variante(&p));
        assert_eq!(zone_de(&r, Zone::Presence).gain_db, 2.0);
    }

    #[test]
    fn pareil_ne_compte_pour_personne_et_pas_dans_le_total_binomial() {
        let mut p = paire(Zone::Graves, Sens::Plus);
        p.extend(paire(Zone::Graves, Sens::Plus));
        let rep = [
            pour_variante(&p[0]),
            Some(Reponse::Pareil),
            pour_variante(&p[2]),
            Some(Reponse::Pareil),
        ];
        let r = calcul(&Courbe::default(), [4.0; 7], &p, &rep);
        let z = zone_de(&r, Zone::Graves);
        assert_eq!(
            (z.duels, z.pour_variante, z.pour_reference, z.pareil),
            (4, 2, 0, 2)
        );
        assert_eq!(z.gain_db, 4.0);
        // 2 victoires sur 2 duels hors Pareil : risque 0,25, Probable.
        assert_eq!(z.confiance, Confiance::Probable);
    }

    #[test]
    fn confiance_d_une_zone_selon_les_victoires_dans_le_sens_retenu() {
        for (victoires, attendu) in [
            (2, Confiance::Probable),
            (4, Confiance::Probable),
            (5, Confiance::Sur),
        ] {
            let p: Vec<_> = (0..victoires)
                .map(|i| duel_zone(Zone::Graves, Sens::Plus, i % 2 == 0))
                .collect();
            let r = calcul(&Courbe::default(), [4.0; 7], &p, &tous_pour_variante(&p));
            assert_eq!(zone_de(&r, Zone::Graves).confiance, attendu, "{victoires}");
        }
    }

    #[test]
    fn la_confiance_ignore_l_autre_sens() {
        // Plus : 2 pour la variante. Moins : 3 pour la référence (sans effet sur le risque).
        let mut p = paire(Zone::Graves, Sens::Plus);
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
        let mut p = paire(Zone::Aigus, Sens::Plus);
        p.extend(paire(Zone::Aigus, Sens::Moins));
        let r = calcul(&Courbe::default(), [4.0; 7], &p, &tous_pour_variante(&p));
        let z = zone_de(&r, Zone::Aigus);
        assert!(z.contradictoire);
        assert_eq!(z.gain_db, 0.0);
        assert_eq!(z.indice, None);
        assert!(r.nouvelle_courbe.filtres.is_empty());
        // Aucun changement retenu dans une zone testée : le pas est divisé.
        assert_eq!(r.nouveaux_pas[Zone::Aigus.index()], 2.0);
        assert!(!r.a_des_changements());
    }

    #[test]
    fn les_pas_se_divisent_sans_changement_et_restent_avec_changement_indice_ou_sans_test() {
        let mut p = paire(Zone::Graves, Sens::Plus); // changement retenu
        p.extend([
            duel_zone(Zone::Mediums, Sens::Plus, true), // zone testée, la référence gagne
            duel_zone(Zone::Presence, Sens::Plus, true), // tout « Pareil »
            duel_zone(Zone::Aigus, Sens::Plus, true),   // passé
            duel_zone(Zone::SubGraves, Sens::Plus, true), // une victoire : indice
        ]);
        let rep = [
            pour_variante(&p[0]),
            pour_variante(&p[1]),
            pour_reference(&p[2]),
            Some(Reponse::Pareil),
            None,
            pour_variante(&p[5]),
        ];
        let r = calcul(&Courbe::default(), [4.0; 7], &p, &rep);
        let pas = r.nouveaux_pas;
        assert_eq!(pas[Zone::Graves.index()], 4.0, "changement retenu");
        assert_eq!(pas[Zone::Mediums.index()], 2.0, "testée sans changement");
        assert_eq!(pas[Zone::Presence.index()], 2.0, "pareil partout : divisé");
        assert_eq!(pas[Zone::Aigus.index()], 4.0, "duel passé : non testée");
        assert_eq!(pas[Zone::SubGraves.index()], 4.0, "indice : à l'étude");
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
        let reference = Courbe {
            preampli_db: 0.0,
            filtres: vec![filtre_de_zone(Zone::Graves, 2.0)],
        };
        let p = paire(Zone::Graves, Sens::Plus);
        let r = calcul(&reference, [4.0; 7], &p, &tous_pour_variante(&p));
        assert_eq!(r.nouvelle_courbe.filtres.len(), 1);
        assert_eq!(r.nouvelle_courbe.filtres[0].gain_db, 6.0);
    }

    #[test]
    fn une_fusion_qui_s_annule_retire_le_filtre() {
        let reference = Courbe {
            preampli_db: 0.0,
            filtres: vec![filtre_de_zone(Zone::SubGraves, 4.0)],
        };
        let p = paire(Zone::SubGraves, Sens::Moins);
        let r = calcul(&reference, [4.0; 7], &p, &tous_pour_variante(&p));
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
        let p = paire(Zone::Mediums, Sens::Plus); // 1 kHz mais Q 0,67
        let r = calcul(&reference, [4.0; 7], &p, &tous_pour_variante(&p));
        assert_eq!(r.nouvelle_courbe.filtres.len(), 2);
        assert_eq!(r.nouvelle_courbe.filtres[0], autre);
    }

    #[test]
    fn la_nouvelle_courbe_a_le_preampli_automatique() {
        let mut p = paire(Zone::Graves, Sens::Plus);
        p.extend(paire(Zone::Presence, Sens::Plus));
        let r = calcul(&Courbe::default(), [4.0; 7], &p, &tous_pour_variante(&p));
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
        let mut p: Vec<_> = (0..6)
            .map(|i| duel_zone(Zone::Graves, Sens::Plus, i % 2 == 0))
            .collect();
        p.push(duel_zone(Zone::Air, Sens::Moins, true));
        p.extend((0..n_controles).map(|i| duel(Sujet::Controle, i % 2 == 0)));
        p
    }

    #[test]
    fn si_la_moitie_des_controles_recoit_a_ou_b_rien_n_est_retenu() {
        let reference = Courbe {
            preampli_db: 0.0,
            filtres: vec![filtre_de_zone(Zone::Presence, 3.0)],
        };
        let mut p = plan_avec_controles(2);
        let mut rep = tous_pour_variante(&p[..7]);
        rep.extend([Some(Reponse::A), Some(Reponse::Pareil)]);
        let r = calcul(&reference, [4.0; 7], &p, &rep);
        // 1 contrôle sur 2 = la moitié : les réponses ne distinguent pas deux versions
        // identiques, donc on ne retient rien, même 6 victoires sur 6.
        assert!(r.controles.rien_retenu);
        assert!(r.controles.penalise);
        assert_eq!(
            (
                r.controles.total,
                r.controles.repondus,
                r.controles.a_ou_b,
                r.controles.pareil
            ),
            (2, 2, 1, 1)
        );
        assert!(!r.a_des_changements());
        assert!(r.indices.is_empty());
        assert_eq!(r.nouvelle_courbe, reference.clone().avec_preampli_auto());
        assert_eq!(r.nouveaux_pas, [4.0; 7], "pas inchangés");
        assert_eq!(r.confiance, Confiance::Indice);
        assert!(!r.recommandee);
        let z = zone_de(&r, Zone::Graves);
        assert_eq!((z.gain_db, z.pour_variante, z.indice), (0.0, 6, None));
        assert_eq!(z.confiance, Confiance::Indice);

        // Aucun contrôle trompé : tout est retenu, sans baisse de confiance.
        rep[7] = Some(Reponse::Pareil);
        let r = calcul(&reference, [4.0; 7], &p, &rep);
        assert!(!r.controles.rien_retenu);
        assert_eq!(zone_de(&r, Zone::Graves).gain_db, 4.0);
        assert_eq!(zone_de(&r, Zone::Graves).confiance, Confiance::Sur);
        assert_eq!(r.indices, vec![(Zone::Air, Sens::Moins)]);
        assert_eq!(r.confiance, Confiance::Sur);

        // Un seul contrôle sur 3 trompé : moins de la moitié, rien ne bloque.
        p = plan_avec_controles(3);
        let mut rep = tous_pour_variante(&p[..7]);
        rep.extend([
            Some(Reponse::B),
            Some(Reponse::Pareil),
            Some(Reponse::Pareil),
        ]);
        let r = calcul(&reference, [4.0; 7], &p, &rep);
        assert!(!r.controles.rien_retenu);
        assert_eq!(r.confiance, Confiance::Sur);
    }

    #[test]
    fn les_controles_passes_ne_comptent_pas() {
        let p = plan_avec_controles(2);
        let mut rep = tous_pour_variante(&p[..7]);
        rep.extend([Some(Reponse::Pareil), None]);
        let r = calcul(&Courbe::default(), [4.0; 7], &p, &rep);
        assert_eq!((r.controles.total, r.controles.repondus), (2, 1));
        assert!(!r.controles.rien_retenu);
        // Tous les contrôles passés : rien ne bloque non plus.
        let mut rep = tous_pour_variante(&p[..7]);
        rep.extend([None, None]);
        let r = calcul(&Courbe::default(), [4.0; 7], &p, &rep);
        assert!(!r.controles.rien_retenu);
        assert!(r.a_des_changements());
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
        assert!(r.controles.rien_retenu);
    }

    #[test]
    fn la_confiance_du_test_est_la_plus_basse_des_zones_changees() {
        let mut p: Vec<_> = (0..5)
            .map(|i| duel_zone(Zone::Graves, Sens::Plus, i % 2 == 0)) // 5/5 : Sûr
            .collect();
        p.extend(paire(Zone::Aigus, Sens::Plus)); // 2/2 : Probable
        let r = calcul(&Courbe::default(), [4.0; 7], &p, &tous_pour_variante(&p));
        assert_eq!(zone_de(&r, Zone::Graves).confiance, Confiance::Sur);
        assert_eq!(zone_de(&r, Zone::Aigus).confiance, Confiance::Probable);
        assert_eq!(r.confiance, Confiance::Probable);
    }

    #[test]
    fn sans_zone_changee_la_confiance_du_test_est_indice() {
        let p = vec![duel_zone(Zone::Graves, Sens::Plus, true)];
        let r = calcul(&Courbe::default(), [4.0; 7], &p, &[pour_reference(&p[0])]);
        assert_eq!(r.confiance, Confiance::Indice);
        assert!(!r.recommandee, "rien à activer");
    }

    #[test]
    fn la_verification_donne_la_confiance_du_test_et_dit_si_la_version_est_recommandee() {
        // Cinq victoires : la zone est Sûr, mais la vérification (1 sur 1) fait Indice.
        let p: Vec<_> = (0..5)
            .map(|i| duel_zone(Zone::Graves, Sens::Plus, i % 2 == 0))
            .collect();
        let rep = tous_pour_variante(&p);
        let v = version([4.0; 7]);
        for (ref_est_a, reponse, attendu, recommandee) in [
            (true, Reponse::B, Preference::Variante, true),
            (true, Reponse::A, Preference::Reference, false),
            (false, Reponse::A, Preference::Variante, true),
            (false, Reponse::B, Preference::Reference, false),
            (false, Reponse::Pareil, Preference::Aucune, true),
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
            assert_eq!(r.recommandee, recommandee, "{reponse:?}");
            // La nouvelle version est calculée dans tous les cas.
            assert!(r.a_des_changements());
        }
        // Sans vérification (duel passé) : recommandée dès qu'il y a un changement.
        let r = calcul(&Courbe::default(), [4.0; 7], &p, &rep);
        assert!(r.recommandee);
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

    fn profil_contre_profil(victoires: usize, controle: Reponse) -> ResultatTest {
        let mut p: Vec<_> = (0..victoires)
            .map(|i| duel(Sujet::AutreProfil, i % 2 == 0))
            .collect();
        p.push(duel(Sujet::Controle, true));
        let mut rep = tous_pour_variante(&p[..victoires]);
        rep.push(Some(controle));
        calcul(&Courbe::default(), [4.0; 7], &p, &rep)
    }

    #[test]
    fn profil_contre_profil_donne_un_bilan_et_ne_touche_pas_aux_zones() {
        let r = profil_contre_profil(4, Reponse::Pareil);
        let b = r.autre_profil.as_ref().unwrap();
        assert_eq!(b.duels, 4);
        assert_eq!(b.pour_variante, 4);
        assert_eq!(b.preference, Preference::Variante);
        assert_eq!(r.confiance, b.confiance);
        assert!(r.zones.is_empty());
        assert_eq!(r.nouveaux_pas, [4.0; 7]);
        assert!(!r.recommandee, "aucune nouvelle version dans ce mode");
        // Sans profil contre profil, pas de bilan.
        let r = calcul(&Courbe::default(), [4.0; 7], &[], &[]);
        assert!(r.autre_profil.is_none());
    }

    #[test]
    fn profil_contre_profil_a_une_confiance_bilaterale() {
        // Le gagnant est désigné après coup : le risque est doublé.
        // 4/4 : 2 × 0,0625 = 0,125 ; 5/5 : 2 × 0,031 = 0,0625 (pas < 0,06) ; 6/6 : 0,031.
        for (victoires, attendu) in [
            (2, Confiance::Indice),
            (3, Confiance::Probable),
            (4, Confiance::Probable),
            (5, Confiance::Probable),
            (6, Confiance::Sur),
        ] {
            let r = profil_contre_profil(victoires, Reponse::Pareil);
            assert_eq!(r.autre_profil.unwrap().confiance, attendu, "{victoires}");
        }
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
            Some(Reponse::Pareil),
        ];
        let r = calcul(&Courbe::default(), [4.0; 7], &p, &rep);
        let b = r.autre_profil.unwrap();
        assert_eq!(b.preference, Preference::Aucune);
        assert_eq!(b.confiance, Confiance::Indice);

        // 6 victoires sur 6, mais le contrôle est raté : la préférence est comptée, sans
        // aucune confiance.
        let r = profil_contre_profil(6, Reponse::A);
        assert!(r.controles.rien_retenu);
        let b = r.autre_profil.unwrap();
        assert_eq!(b.preference, Preference::Variante);
        assert_eq!(b.confiance, Confiance::Indice);
        assert_eq!(r.confiance, Confiance::Indice);
    }

    #[test]
    fn le_resultat_passe_par_json() {
        let mut p = paire(Zone::Graves, Sens::Plus);
        p.push(duel_zone(Zone::Air, Sens::Moins, true));
        let r = calcul(&Courbe::default(), [4.0; 7], &p, &tous_pour_variante(&p));
        assert!(r.recommandee && !r.indices.is_empty());
        let j = serde_json::to_string(&r).unwrap();
        assert_eq!(serde_json::from_str::<ResultatTest>(&j).unwrap(), r);
    }

    #[test]
    fn un_resultat_ecrit_avant_ces_regles_se_relit() {
        let p = paire(Zone::Graves, Sens::Plus);
        let r = calcul(&Courbe::default(), [4.0; 7], &p, &tous_pour_variante(&p));
        let mut v = serde_json::to_value(&r).unwrap();
        let o = v.as_object_mut().unwrap();
        o.remove("recommandee");
        o.remove("indices");
        o["controles"]
            .as_object_mut()
            .unwrap()
            .remove("rien_retenu");
        o["zones"][0].as_object_mut().unwrap().remove("indice");
        let ancien: ResultatTest = serde_json::from_value(v).unwrap();
        assert!(!ancien.recommandee);
        assert!(ancien.indices.is_empty());
    }

    // ----- Régression statistique (ADR-0005) -----
    //
    // Avec un duel par couple et un changement dès la première victoire, quelqu'un qui
    // répondait au hasard obtenait un profil modifié dans 97 % des tests rapides. Ces
    // tests simulent des milliers de tests pour vérifier que ce n'est plus le cas.

    use crate::hasard::{Hasard, HasardGraine};
    use crate::plan::{plan_avec_historique, HistoriquePlan, Mode};

    /// Joue `n` tests rapides avec un auditeur donné et rend la part de tests qui
    /// modifient la courbe.
    fn part_de_tests_modifies(
        n: u64,
        mut auditeur: impl FnMut(&DuelPrevu, &mut HasardGraine) -> Reponse,
    ) -> f64 {
        let v = version([4.0; 7]);
        let mut modifies = 0;
        for graine in 0..n {
            let mut h = HasardGraine::new(graine);
            let p = plan_avec_historique(
                &Mode::DuelsEnJeu { precis: false },
                &v,
                &HistoriquePlan::default(),
                &mut h,
            );
            let rep: Vec<_> = p.iter().map(|d| Some(auditeur(d, &mut h))).collect();
            let r = calcul(&Courbe::default(), [4.0; 7], &p, &rep);
            if r.a_des_changements() {
                assert!(!r.nouvelle_courbe.filtres.is_empty());
                modifies += 1;
            } else {
                assert!(r.nouvelle_courbe.filtres.is_empty());
            }
        }
        modifies as f64 / n as f64
    }

    #[test]
    fn repondre_au_hasard_a_pareil_b_modifie_moins_d_un_test_sur_dix() {
        let part = part_de_tests_modifies(4_000, |_, h| {
            [Reponse::A, Reponse::Pareil, Reponse::B][h.entier(3) as usize]
        });
        assert!(part < 0.10, "{part}");
        // Le garde-fou n'est pas un verrou : un test au hasard sur vingt passe encore.
        assert!(part > 0.0, "{part}");
    }

    #[test]
    fn repondre_au_hasard_a_ou_b_modifie_moins_de_deux_tests_sur_cent() {
        let part = part_de_tests_modifies(4_000, |_, h| {
            if h.pile_ou_face() {
                Reponse::A
            } else {
                Reponse::B
            }
        });
        assert!(part < 0.02, "{part}");
    }

    #[test]
    fn un_auditeur_parfait_obtient_son_changement_des_que_le_couple_est_au_plan() {
        // Il préfère toujours « plus de graves », et entend « Pareil » partout ailleurs.
        let parfait = |d: &DuelPrevu| match d.sujet {
            Sujet::Zone {
                zone: Zone::Graves,
                sens: Sens::Plus,
            } => pour_variante(d),
            Sujet::Zone {
                zone: Zone::Graves,
                sens: Sens::Moins,
            } => pour_reference(d),
            _ => Some(Reponse::Pareil),
        };
        let v = version([4.0; 7]);
        for graine in 0..300 {
            let mut h = HasardGraine::new(graine);
            // Un historique où les graves viennent d'être testés : le couple n'arrive
            // qu'au deuxième ou troisième test.
            let mut historique = HistoriquePlan::depuis_zones(&[Zone::Graves]);
            let mut tests = 0;
            loop {
                tests += 1;
                assert!(tests <= 3, "le couple n'est jamais planifié");
                let p = plan_avec_historique(
                    &Mode::DuelsEnJeu { precis: false },
                    &v,
                    &historique,
                    &mut h,
                );
                let rep: Vec<_> = p.iter().map(parfait).collect();
                let r = calcul(&Courbe::default(), [4.0; 7], &p, &rep);
                let au_plan = p.iter().any(|d| {
                    d.sujet
                        == Sujet::Zone {
                            zone: Zone::Graves,
                            sens: Sens::Plus,
                        }
                });
                if au_plan {
                    assert_eq!(zone_de(&r, Zone::Graves).gain_db, 4.0);
                    assert_eq!(zone_de(&r, Zone::Graves).confiance, Confiance::Probable);
                    assert_eq!(r.zones.iter().filter(|z| z.est_changee()).count(), 1);
                    assert!(r.recommandee);
                    break;
                }
                assert!(!r.a_des_changements());
                historique.ajouter_test(&p, &r.indices);
            }
            assert!(tests >= 2);
        }
    }
}
