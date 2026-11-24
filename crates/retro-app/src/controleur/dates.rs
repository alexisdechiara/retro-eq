//! Dates sans dépendance : le contrôleur reçoit l'heure en millisecondes et le décalage
//! horaire local, et en tire le texte RFC 3339 rangé dans chaque version.

/// Jour civil (année, mois, jour) à partir d'un nombre de jours depuis le 1er janvier 1970.
/// Algorithme de Howard Hinnant (« days from civil »), exact pour tout le calendrier grégorien.
fn jour_civil(jours: i64) -> (i64, u32, u32) {
    let z = jours + 719_468;
    let ere = z.div_euclid(146_097);
    let jour_ere = z.rem_euclid(146_097);
    let annee_ere = (jour_ere - jour_ere / 1_460 + jour_ere / 36_524 - jour_ere / 146_096) / 365;
    let jour_annee = jour_ere - (365 * annee_ere + annee_ere / 4 - annee_ere / 100);
    let mp = (5 * jour_annee + 2) / 153;
    let jour = (jour_annee - (153 * mp + 2) / 5 + 1) as u32;
    let mois = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let annee = annee_ere + ere * 400 + i64::from(mois <= 2);
    (annee, mois, jour)
}

/// L'instant `ms` (millisecondes depuis 1970, UTC) écrit en RFC 3339 à l'heure locale :
/// `2025-11-24T21:40:05+01:00`.
pub fn rfc3339(ms: u64, decalage_minutes: i32) -> String {
    let secondes = (ms / 1000) as i64 + i64::from(decalage_minutes) * 60;
    let (annee, mois, jour) = jour_civil(secondes.div_euclid(86_400));
    let reste = secondes.rem_euclid(86_400);
    let signe = if decalage_minutes < 0 { '-' } else { '+' };
    let d = decalage_minutes.unsigned_abs();
    format!(
        "{annee:04}-{mois:02}-{jour:02}T{:02}:{:02}:{:02}{signe}{:02}:{:02}",
        reste / 3600,
        reste % 3600 / 60,
        reste % 60,
        d / 60,
        d % 60
    )
}

/// « 24/11 » à partir d'une date RFC 3339. Rend le texte tel quel s'il n'a pas la forme attendue
/// (un fichier de profil modifié à la main, par exemple).
pub fn jour_mois(rfc: &str) -> String {
    match (rfc.get(8..10), rfc.get(5..7)) {
        (Some(jour), Some(mois)) if rfc.as_bytes().get(4) == Some(&b'-') => format!("{jour}/{mois}"),
        _ => rfc.to_string(),
    }
}

/// « 24/11 21:40 » à partir d'une date RFC 3339.
pub fn jour_heure(rfc: &str) -> String {
    match rfc.get(11..16) {
        Some(heure) if rfc.as_bytes().get(10) == Some(&b'T') => {
            format!("{} {heure}", jour_mois(rfc))
        }
        _ => jour_mois(rfc),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn l_epoch_est_le_premier_janvier_1970() {
        assert_eq!(rfc3339(0, 0), "1970-01-01T00:00:00+00:00");
    }

    #[test]
    fn une_date_connue_avec_decalage() {
        // 24 novembre 2025, 20:40:05 UTC.
        let ms = 1_764_016_805_000;
        assert_eq!(rfc3339(ms, 0), "2025-11-24T20:40:05+00:00");
        assert_eq!(rfc3339(ms, 60), "2025-11-24T21:40:05+01:00");
        assert_eq!(rfc3339(ms, -300), "2025-11-24T15:40:05-05:00");
        // Le décalage peut faire changer de jour.
        assert_eq!(rfc3339(ms, 240), "2025-11-25T00:40:05+04:00");
    }

    #[test]
    fn un_29_fevrier() {
        assert_eq!(rfc3339(1_709_164_800_000, 0), "2024-02-29T00:00:00+00:00");
    }

    #[test]
    fn les_formes_courtes() {
        assert_eq!(jour_mois("2025-11-24T21:40:05+01:00"), "24/11");
        assert_eq!(jour_heure("2025-11-24T21:40:05+01:00"), "24/11 21:40");
        assert_eq!(jour_heure("hier"), "hier");
    }
}
