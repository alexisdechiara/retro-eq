//! `ParametricEQ.txt` d'AutoEQ : la même grammaire qu'Equalizer APO.
//!
//! Ce module ne fait que déléguer à [`super::equalizer_apo`] et déduire le nom
//! du profil du nom de fichier (`Sony INZONE H9 II ParametricEQ.txt` donne `Sony INZONE H9 II`).

use super::{equalizer_apo, ErreurFormat, Import};
use retro_dsp::Courbe;

/// Lit un `ParametricEQ.txt` et nomme le profil d'après le nom du fichier.
pub fn lire(texte: &str, nom_de_fichier: &str) -> Result<Import, ErreurFormat> {
    let mut import = equalizer_apo::lire(texte)?;
    import.nom = nom_de_profil(nom_de_fichier);
    Ok(import)
}

/// Nom de profil déduit du nom de fichier : sans dossier, sans `.txt`,
/// sans le suffixe ` ParametricEQ`. `None` s'il ne reste rien.
pub fn nom_de_profil(nom_de_fichier: &str) -> Option<String> {
    let fichier = nom_de_fichier
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(nom_de_fichier);
    let sans_extension = match fichier.rsplit_once('.') {
        Some((debut, ext)) if ext.eq_ignore_ascii_case("txt") => debut,
        _ => fichier,
    };
    let suffixe = "parametriceq";
    let minuscules = sans_extension.to_lowercase();
    let nom = if minuscules.ends_with(suffixe) {
        // Le suffixe est en ASCII : on peut couper sur sa longueur en octets.
        &sans_extension[..sans_extension.len() - suffixe.len()]
    } else {
        sans_extension
    };
    let nom = nom.trim();
    (!nom.is_empty()).then(|| nom.to_string())
}

/// Écrit une courbe au format AutoEQ (identique à Equalizer APO).
pub fn ecrire(courbe: &Courbe) -> String {
    equalizer_apo::ecrire(courbe)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formats::outils_de_test::{courbe_melangee, proches};

    #[test]
    fn nom_sans_le_suffixe_parametriceq() {
        assert_eq!(
            nom_de_profil("Sony INZONE H9 II ParametricEQ.txt").as_deref(),
            Some("Sony INZONE H9 II")
        );
    }

    #[test]
    fn nom_ignore_le_dossier_les_deux_separateurs_et_la_casse() {
        assert_eq!(
            nom_de_profil("C:\\eq\\AKG K371 parametriceq.TXT").as_deref(),
            Some("AKG K371")
        );
        assert_eq!(
            nom_de_profil("/tmp/AKG K371 ParametricEQ.txt").as_deref(),
            Some("AKG K371")
        );
    }

    #[test]
    fn nom_d_un_fichier_ordinaire_garde_son_nom_sans_extension() {
        assert_eq!(
            nom_de_profil("Mon casque.txt").as_deref(),
            Some("Mon casque")
        );
    }

    #[test]
    fn nom_vide_quand_il_ne_reste_rien() {
        assert_eq!(nom_de_profil("ParametricEQ.txt"), None);
        assert_eq!(nom_de_profil(""), None);
    }

    #[test]
    fn lire_delegue_et_nomme_le_profil() {
        let i = lire(
            "Preamp: -6.2 dB\nFilter 1: ON LSC Fc 105 Hz Gain 5.5 dB Q 0.70\nFilter 2: ON PK Fc 21 Hz Gain 4.6 dB Q 1.40\n",
            "Sony INZONE H9 II ParametricEQ.txt",
        )
        .expect("lecture");
        assert_eq!(i.nom.as_deref(), Some("Sony INZONE H9 II"));
        assert_eq!(i.courbe.filtres.len(), 2);
        assert_eq!(i.courbe.preampli_db, -6.2);
    }

    #[test]
    fn lire_sans_ligne_utilisable_donne_une_erreur() {
        assert!(lire("rien", "x ParametricEQ.txt").is_err());
    }

    #[test]
    fn aller_retour_garde_la_courbe() {
        let c = courbe_melangee();
        let relue = lire(&ecrire(&c), "x ParametricEQ.txt")
            .expect("relecture")
            .courbe;
        assert!(proches(&c, &relue));
    }
}
