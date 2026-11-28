//! Les phrases de la vérification des mises à jour, dans les deux langues.
//!
//! Les titres et les boutons de la section sont dans `a_propos.slint` (`@tr`) ; ici, ce qui
//! dépend d'une date, d'une taille ou d'une raison d'échec.

use super::choisir;
use retro_core::mises_a_jour::RaisonInconnue;
use retro_core::Langue;

/// Une phrase calme qui explique pourquoi on n'a pas pu vérifier, et ce qui se passe ensuite.
/// Jamais de code d'erreur brut seul, jamais d'alarme : l'appli marche très bien hors ligne.
pub fn probleme_de_verification(raison: &RaisonInconnue, l: Langue) -> String {
    match raison {
        RaisonInconnue::Reseau => choisir(
            l,
            "Retro EQ n’a pas pu joindre GitHub. Vérifie ta connexion : il réessaiera plus tard.",
            "Retro EQ couldn’t reach GitHub. Check your connection; it will try again later.",
        )
        .to_string(),
        RaisonInconnue::LimiteAtteinte => choisir(
            l,
            "GitHub a reçu trop de demandes depuis ton adresse. Réessaie dans une heure environ.",
            "GitHub has had too many requests from your address. Try again in about an hour.",
        )
        .to_string(),
        RaisonInconnue::ReponseIllisible | RaisonInconnue::VersionIllisible => choisir(
            l,
            "GitHub a répondu quelque chose que Retro EQ ne comprend pas. Réessaie plus tard.",
            "GitHub sent a reply Retro EQ doesn’t understand. Try again later.",
        )
        .to_string(),
        RaisonInconnue::Http(code) => match l {
            Langue::Fr => format!("GitHub a répondu par une erreur ({code}). Réessaie plus tard."),
            Langue::En => format!("GitHub replied with an error ({code}). Try again later."),
        },
        // Pas une panne : l'écran a sa propre phrase. Gardée ici pour que le `match` reste complet.
        RaisonInconnue::AucuneVersionPubliee => choisir(
            l,
            "Aucune version n’est publiée pour l’instant.",
            "No version has been published yet.",
        )
        .to_string(),
    }
}

const MOIS_FR: [&str; 12] = [
    "janvier",
    "février",
    "mars",
    "avril",
    "mai",
    "juin",
    "juillet",
    "août",
    "septembre",
    "octobre",
    "novembre",
    "décembre",
];

const MOIS_EN: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

/// « 2 janvier 2026 » à partir de `2026-01-02`. Rend le texte tel quel s'il n'a pas cette forme.
pub fn date_de_version(date: &str, l: Langue) -> String {
    let lu = (|| {
        let annee: u32 = date.get(0..4)?.parse().ok()?;
        let mois: usize = date.get(5..7)?.parse().ok()?;
        let jour: u32 = date.get(8..10)?.parse().ok()?;
        (1..=12).contains(&mois).then_some((annee, mois, jour))
    })();
    match (lu, l) {
        (Some((a, m, j)), Langue::Fr) => {
            format!(
                "{j}{} {} {a}",
                if j == 1 { "er" } else { "" },
                MOIS_FR[m - 1]
            )
        }
        (Some((a, m, j)), Langue::En) => format!("{j} {} {a}", MOIS_EN[m - 1]),
        (None, _) => date.to_string(),
    }
}

/// « vérifié aujourd’hui à 19:40 », ou « vérifié le 24/11 à 19:40 ».
pub fn verification_faite(aujourd_hui: bool, jour_mois: &str, heure: &str, l: Langue) -> String {
    match (l, aujourd_hui) {
        (Langue::Fr, true) => format!("vérifié aujourd’hui à {heure}"),
        (Langue::Fr, false) => format!("vérifié le {jour_mois} à {heure}"),
        (Langue::En, true) => format!("checked today at {heure}"),
        (Langue::En, false) => format!("checked on {jour_mois} at {heure}"),
    }
}

/// « Retro-EQ-Installateur-0.2.0.exe · 11,8 Mo » : le nom et la taille de l'installateur.
pub fn installateur(nom: &str, taille_octets: u64, l: Langue) -> String {
    if taille_octets == 0 {
        return nom.to_string();
    }
    let mo = taille_octets as f64 / (1024.0 * 1024.0);
    let nombre = format!("{mo:.1}");
    match l {
        Langue::Fr => format!("{nom} · {} Mo", nombre.replace('.', ",")),
        Langue::En => format!("{nom} · {nombre} MB"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn les_dates_de_version_s_ecrivent_dans_la_langue() {
        assert_eq!(date_de_version("2026-01-02", Langue::Fr), "2 janvier 2026");
        assert_eq!(
            date_de_version("2026-02-01", Langue::Fr),
            "1er février 2026"
        );
        assert_eq!(
            date_de_version("2026-12-25", Langue::En),
            "25 December 2026"
        );
        assert_eq!(
            date_de_version("n’importe quoi", Langue::Fr),
            "n’importe quoi"
        );
        assert_eq!(date_de_version("2026-13-01", Langue::En), "2026-13-01");
    }

    #[test]
    fn la_verification_dit_quand() {
        assert_eq!(
            verification_faite(true, "24/11", "19:40", Langue::Fr),
            "vérifié aujourd’hui à 19:40"
        );
        assert_eq!(
            verification_faite(false, "24/11", "19:40", Langue::En),
            "checked on 24/11 at 19:40"
        );
    }

    #[test]
    fn la_taille_de_l_installateur_est_en_mo() {
        assert_eq!(
            installateur("setup.exe", 12_345_678, Langue::Fr),
            "setup.exe · 11,8 Mo"
        );
        assert_eq!(
            installateur("setup.exe", 12_345_678, Langue::En),
            "setup.exe · 11.8 MB"
        );
        assert_eq!(installateur("setup.exe", 0, Langue::Fr), "setup.exe");
    }

    #[test]
    fn chaque_raison_a_une_phrase_dans_chaque_langue() {
        for raison in [
            RaisonInconnue::Reseau,
            RaisonInconnue::LimiteAtteinte,
            RaisonInconnue::ReponseIllisible,
            RaisonInconnue::VersionIllisible,
            RaisonInconnue::Http(503),
            RaisonInconnue::AucuneVersionPubliee,
        ] {
            for l in Langue::TOUTES {
                assert!(!probleme_de_verification(&raison, l).is_empty());
            }
        }
        assert!(probleme_de_verification(&RaisonInconnue::Http(503), Langue::Fr).contains("503"));
    }
}
