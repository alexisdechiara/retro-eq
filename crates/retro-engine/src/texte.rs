//! Génération du fichier texte lu par Equalizer APO.

use retro_dsp::{Courbe, Filtre, TypeFiltre};

/// Arrondit à `decimales` et évite le « -0.0 », que le moteur lirait de travers
/// pour rien et qui fait du bruit dans les diffs.
fn arrondi(x: f64, decimales: i32) -> f64 {
    let echelle = 10f64.powi(decimales);
    (x * echelle).round() / echelle + 0.0
}

fn code_filtre(type_filtre: TypeFiltre) -> &'static str {
    match type_filtre {
        TypeFiltre::Cloche => "PK",
        TypeFiltre::PlateauBas => "LSC",
        TypeFiltre::PlateauHaut => "HSC",
    }
}

fn ligne_filtre(numero: usize, f: &Filtre) -> String {
    // `{}` sur un f64 écrit toujours un point décimal, quelle que soit la
    // langue de Windows : c'est exactement ce que veut Equalizer APO.
    format!(
        "Filter {numero}: ON {} Fc {} Hz Gain {:.2} dB Q {:.3}",
        code_filtre(f.type_filtre),
        // Fréquence non arrondie : `{}` rend le plus court texte qui se relit
        // à l'identique. `+ 0.0` évite « -0 ».
        f.frequence_hz + 0.0,
        arrondi(f.gain_db, 2),
        arrondi(f.q, 3),
    )
}

/// Le contenu de `retro-eq.txt` pour une courbe.
///
/// Fonction pure (pas de fichier, pas d'horloge) : facile à tester.
pub fn texte_config(courbe: &Courbe) -> String {
    let mut texte = String::from(
        "# Fichier genere par Retro EQ. Ne pas modifier a la main : il est reecrit a chaque changement.\n",
    );
    texte.push_str(&format!(
        "Preamp: {:.2} dB\n",
        arrondi(courbe.preampli_db, 2)
    ));
    for (i, filtre) in courbe.filtres.iter().enumerate() {
        texte.push_str(&ligne_filtre(i + 1, filtre));
        texte.push('\n');
    }
    texte
}

#[cfg(test)]
mod tests {
    use super::*;

    fn filtre(type_filtre: TypeFiltre, f: f64, g: f64, q: f64) -> Filtre {
        Filtre {
            type_filtre,
            frequence_hz: f,
            gain_db: g,
            q,
        }
    }

    #[test]
    fn courbe_plate_donne_preampli_zero_et_aucun_filtre() {
        let texte = texte_config(&Courbe::default());
        assert!(texte.starts_with("# Fichier genere par Retro EQ."));
        assert!(texte.contains("Preamp: 0.00 dB\n"));
        assert!(!texte.contains("Filter"));
    }

    #[test]
    fn cloche_donne_pk() {
        let c = Courbe {
            preampli_db: -6.0,
            filtres: vec![filtre(TypeFiltre::Cloche, 1000.0, -3.0, 1.41)],
        };
        let texte = texte_config(&c);
        assert!(texte.contains("Preamp: -6.00 dB\n"));
        assert!(texte.contains("Filter 1: ON PK Fc 1000 Hz Gain -3.00 dB Q 1.410\n"));
    }

    #[test]
    fn plateaux_donnent_lsc_et_hsc_numerotes() {
        let c = Courbe {
            preampli_db: 0.0,
            filtres: vec![
                filtre(TypeFiltre::PlateauBas, 60.0, 4.0, 0.71),
                filtre(TypeFiltre::PlateauHaut, 10000.0, -2.5, 0.71),
            ],
        };
        let texte = texte_config(&c);
        assert!(texte.contains("Filter 1: ON LSC Fc 60 Hz Gain 4.00 dB Q 0.710\n"));
        assert!(texte.contains("Filter 2: ON HSC Fc 10000 Hz Gain -2.50 dB Q 0.710\n"));
    }

    #[test]
    fn separateur_decimal_est_toujours_le_point_et_zero_negatif_disparait() {
        let c = Courbe {
            preampli_db: -0.0,
            filtres: vec![filtre(TypeFiltre::Cloche, 62.5, -0.004, 1.0)],
        };
        let texte = texte_config(&c);
        assert!(texte.contains("Preamp: 0.00 dB"));
        assert!(texte.contains("Fc 62.5 Hz Gain 0.00 dB Q 1.000"));
        assert!(!texte.contains(','));
    }

    #[test]
    fn le_prea_et_les_gains_gardent_deux_decimales() {
        // 0,1 dB d'écart de niveau entre deux côtés d'un duel s'entendrait : on n'arrondit pas à 0,1.
        let c = Courbe {
            preampli_db: -4.37,
            filtres: vec![filtre(TypeFiltre::Cloche, 1000.0, 2.456, 1.3142)],
        };
        let texte = texte_config(&c);
        assert!(texte.contains("Preamp: -4.37 dB\n"), "{texte}");
        assert!(texte.contains("Gain 2.46 dB Q 1.314\n"), "{texte}");
    }

    #[test]
    fn la_frequence_fait_l_aller_retour_sans_perte() {
        let c = Courbe {
            preampli_db: 0.0,
            filtres: vec![filtre(TypeFiltre::Cloche, 1234.5678, 1.0, 1.0)],
        };
        let texte = texte_config(&c);
        assert!(texte.contains("Fc 1234.5678 Hz"), "{texte}");
        let lue: f64 = texte
            .split("Fc ")
            .nth(1)
            .and_then(|s| s.split(' ').next())
            .and_then(|s| s.parse().ok())
            .unwrap();
        assert_eq!(lue, 1234.5678);
    }

    #[test]
    fn pas_de_zero_negatif_meme_apres_arrondi() {
        let c = Courbe {
            preampli_db: -0.004,
            filtres: vec![filtre(TypeFiltre::PlateauBas, 80.0, -0.004, 0.7)],
        };
        let texte = texte_config(&c);
        assert!(!texte.contains("-0.00"), "{texte}");
        assert!(texte.contains("Preamp: 0.00 dB"), "{texte}");
    }
}
