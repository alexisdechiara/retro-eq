//! Les phrases des tickets 26 à 28, dans les deux langues : l'écart de volume mesuré
//! (écran Résultat), la comparaison de deux versions d'un profil, et l'Empreinte.

use super::{choisir, db, duels, fois, ligne_journal, sens_de_zone};
use retro_core::{
    BilanDuels, Cote, DuelClos, DuelFini, GoutsAppli, Langue, Preference, Sujet,
    SEUIL_ECART_VOLUME_DB,
};

/// Une valeur en dB sans signe : « 2,6 dB ».
fn db_sans_signe(valeur: f64, l: Langue) -> String {
    db(valeur.abs(), l).trim_start_matches('+').to_string()
}

/// Le bilan de l'écart de volume mesuré sur les duels d'un test : moyenne et maximum (en
/// valeur absolue), et combien dépassent le seuil.
pub fn volume_mesure(duels_clos: &[DuelClos], l: Langue) -> String {
    let ecarts: Vec<f64> = duels_clos
        .iter()
        .filter_map(|d| d.ecart_volume_db)
        .map(f64::abs)
        .collect();
    if ecarts.is_empty() {
        return choisir(
            l,
            "Écart de volume entre les deux côtés : pas mesuré, faute de son capté pendant les duels.",
            "Volume gap between the two sides: not measured, no sound was captured during the duels.",
        )
        .to_string();
    }
    let n = ecarts.len() as u32;
    let moyenne = ecarts.iter().sum::<f64>() / f64::from(n);
    let max = ecarts.iter().copied().fold(0.0, f64::max);
    let signales = duels_clos.iter().filter(|d| d.ecart_signale()).count() as u32;
    let (moyenne, max, seuil) = (
        db_sans_signe(moyenne, l),
        db_sans_signe(max, l),
        db_sans_signe(SEUIL_ECART_VOLUME_DB, l),
    );
    let mesure = match l {
        Langue::Fr => format!(
            "Écart de volume mesuré entre les deux côtés : {moyenne} en moyenne, au plus {max}, sur {}.",
            duels(n, l)
        ),
        Langue::En => format!(
            "Volume gap measured between the two sides: {moyenne} on average, at most {max}, over {}.",
            duels(n, l)
        ),
    };
    let suite = match (signales, l) {
        (0, Langue::Fr) => format!("Aucun ne dépasse {seuil}."),
        (0, Langue::En) => format!("None goes over {seuil}."),
        (1, Langue::Fr) => format!(
            "1 dépasse {seuil} : le côté le plus fort a pu plaire parce qu’il était plus fort."
        ),
        (1, Langue::En) => {
            format!("1 goes over {seuil}: the louder side may have won because it was louder.")
        }
        (k, Langue::Fr) => format!(
            "{k} dépassent {seuil} : le côté le plus fort a pu plaire parce qu’il était plus fort."
        ),
        (k, Langue::En) => {
            format!("{k} go over {seuil}: the louder side may have won because it was louder.")
        }
    };
    format!("{mesure} {suite}")
}

/// Un duel signalé : son titre (« Duel 4 · graves en plus »), l'écart signé et une phrase.
pub fn ecart_volume(duel: &DuelClos, l: Langue) -> (String, String, String) {
    let ecart = duel.ecart_volume_db.unwrap_or(0.0);
    // Le titre est celui du journal une fois le test fini (`revele`) : seul le titre sert,
    // et il ne dépend ni du côté de la référence ni de la réponse.
    let (titre, _) = ligne_journal(
        &DuelFini {
            numero: duel.numero,
            sujet: duel.sujet,
            reponse: duel.reponse,
            reference: Cote::A,
            preference: duel.preference,
        },
        true,
        l,
    );
    let ecart_sans_signe = db_sans_signe(ecart, l);
    let plus_fort = ecart > 0.0;
    let detail = match (duel.sujet, l) {
        (Sujet::Controle, Langue::Fr) => format!(
            "deux versions identiques, mais un côté a été mesuré {ecart_sans_signe} plus fort : le son a changé entre les passages"
        ),
        (Sujet::Controle, Langue::En) => format!(
            "two identical versions, yet one side measured {ecart_sans_signe} louder: the sound changed between passes"
        ),
        (sujet, Langue::Fr) => {
            let cote = match sujet {
                Sujet::Verification => "la nouvelle version",
                Sujet::AutreProfil => "l’autre réglage",
                _ => "la variante",
            };
            let sens = if plus_fort { "plus fort" } else { "moins fort" };
            format!("{cote} sonnait {ecart_sans_signe} {sens} que la référence")
        }
        (sujet, Langue::En) => {
            let cote = match sujet {
                Sujet::Verification => "the new version",
                Sujet::AutreProfil => "the other setting",
                _ => "the variant",
            };
            let sens = if plus_fort { "louder" } else { "quieter" };
            format!("{cote} sounded {ecart_sans_signe} {sens} than the reference")
        }
    };
    (titre, db(ecart, l), detail)
}

/// Le titre d'un résultat « version contre version » : « Mon profil : version 3 contre version 2 ».
pub fn titre_version_contre_version(profil: &str, reference: u32, autre: u32, l: Langue) -> String {
    match l {
        Langue::Fr => format!("{profil} : version {reference} contre version {autre}"),
        Langue::En => format!("{profil}: version {reference} vs version {autre}"),
    }
}

/// Le résumé d'une comparaison de la version active avec la version `autre` du même profil.
pub fn resume_autre_version(bilan: &BilanDuels, autre: u32, l: Langue) -> String {
    let departages = bilan.pour_variante + bilan.pour_reference;
    match (bilan.preference, l) {
        (Preference::Variante, Langue::Fr) => format!(
            "La version {autre} préférée {} sur {departages}.",
            fois(bilan.pour_variante, l)
        ),
        (Preference::Variante, Langue::En) => format!(
            "Version {autre} preferred {} out of {departages}.",
            fois(bilan.pour_variante, l)
        ),
        (Preference::Reference, Langue::Fr) => format!(
            "Ta version actuelle préférée {} sur {departages} face à la version {autre}.",
            fois(bilan.pour_reference, l)
        ),
        (Preference::Reference, Langue::En) => format!(
            "Your current version preferred {} out of {departages} against version {autre}.",
            fois(bilan.pour_reference, l)
        ),
        (Preference::Aucune, Langue::Fr) => {
            format!("Pas de préférence nette entre ta version actuelle et la version {autre}.")
        }
        (Preference::Aucune, Langue::En) => {
            format!("No clear preference between your current version and version {autre}.")
        }
    }
}

/// Ce qui se dégage des réponses données dans une appli : au plus trois tendances, la plus
/// nette d'abord, avec leurs victoires (« Plus de graves · 2 sur 3 »).
pub fn tendances_appli(gouts: &GoutsAppli, l: Langue) -> String {
    if gouts.tendances.is_empty() {
        return choisir(
            l,
            "Rien ne se dégage encore : la référence ou « Pareil » l’emportent.",
            "Nothing stands out yet: the reference or “Same” win.",
        )
        .to_string();
    }
    gouts
        .tendances
        .iter()
        .take(3)
        .map(|t| {
            let sur = t.pour_variante + t.pour_reference;
            let victoires = match l {
                Langue::Fr => format!("{} sur {sur}", t.pour_variante),
                Langue::En => format!("{} of {sur}", t.pour_variante),
            };
            format!("{} · {victoires}", sens_de_zone(t.zone, t.sens, l))
        })
        .collect::<Vec<_>>()
        .join(choisir(l, " ; ", "; "))
}

#[cfg(test)]
mod tests {
    use super::*;
    use retro_core::{Confiance, Reponse, Sens, TendanceAppli, Zone};

    fn duel(sujet: Sujet, ecart: Option<f64>) -> DuelClos {
        DuelClos {
            numero: 4,
            sujet,
            reponse: Some(Reponse::A),
            preference: Some(Preference::Reference),
            appli: None,
            ecart_volume_db: ecart,
        }
    }

    const GRAVES: Sujet = Sujet::Zone {
        zone: Zone::Graves,
        sens: Sens::Plus,
    };

    #[test]
    fn le_bilan_du_volume_dit_moyenne_maximum_et_depassements_dans_les_deux_langues() {
        let duels = [duel(GRAVES, Some(-2.5)), duel(Sujet::Controle, Some(0.5))];
        assert_eq!(
            volume_mesure(&duels, Langue::Fr),
            "Écart de volume mesuré entre les deux côtés : 1,5 dB en moyenne, au plus 2,5 dB, sur 2 duels. 1 dépasse 2 dB : le côté le plus fort a pu plaire parce qu’il était plus fort."
        );
        assert!(volume_mesure(&duels, Langue::En).starts_with("Volume gap measured"));
        assert!(volume_mesure(&[duel(GRAVES, None)], Langue::Fr).contains("pas mesuré"));
        assert!(volume_mesure(&[], Langue::En).contains("not measured"));
    }

    #[test]
    fn un_duel_signale_dit_quel_cote_sonnait_plus_ou_moins_fort() {
        let (titre, ecart, detail) = ecart_volume(&duel(GRAVES, Some(-2.5)), Langue::Fr);
        assert_eq!(titre, "Duel 4 · graves en plus");
        assert_eq!(ecart, "−2,5 dB");
        assert_eq!(
            detail,
            "la variante sonnait 2,5 dB moins fort que la référence"
        );
        let (_, _, detail) = ecart_volume(&duel(Sujet::Verification, Some(3.0)), Langue::En);
        assert_eq!(
            detail,
            "the new version sounded 3 dB louder than the reference"
        );
    }

    #[test]
    fn les_tendances_d_une_appli_suivent_la_ponctuation_de_chaque_langue() {
        let t = |zone, sens| TendanceAppli {
            zone,
            sens,
            pour_variante: 2,
            pour_reference: 1,
            pareil: 0,
            confiance: Confiance::Indice,
        };
        let gouts = GoutsAppli {
            appli: "Jeu".into(),
            duels: 6,
            tendances: vec![t(Zone::Graves, Sens::Plus), t(Zone::Air, Sens::Moins)],
        };
        assert_eq!(
            tendances_appli(&gouts, Langue::Fr),
            "Plus de graves · 2 sur 3 ; Moins d’air · 2 sur 3"
        );
        assert_eq!(
            tendances_appli(&gouts, Langue::En),
            "More bass · 2 of 3; Less air · 2 of 3"
        );
    }

    #[test]
    fn une_comparaison_de_versions_s_accorde_au_feminin() {
        let bilan = BilanDuels {
            duels: 6,
            pour_variante: 3,
            pour_reference: 1,
            pareil: 2,
            preference: Preference::Variante,
            confiance: Confiance::Indice,
        };
        assert_eq!(
            resume_autre_version(&bilan, 2, Langue::Fr),
            "La version 2 préférée 3 fois sur 4."
        );
        assert_eq!(
            titre_version_contre_version("Mon profil", 3, 2, Langue::En),
            "Mon profil: version 3 vs version 2"
        );
    }
}
