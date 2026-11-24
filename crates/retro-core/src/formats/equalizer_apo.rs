//! Format texte d'Equalizer APO : `Preamp: -6 dB` et `Filter 1: ON PK Fc 1000 Hz Gain -3 dB Q 1.41`.
//!
//! Types lus : `PK`/`PEQ` (cloche), `LSC`/`LS` (plateau bas), `HSC`/`HS` (plateau haut).
//! Un plateau sans `Q` prend 0,707. La largeur `BW Oct` est convertie en Q.
//! Tout le reste (commentaires, `OFF`, `Include`, `Channel`, `LP`, `HP`, `NO`…)
//! est mis de côté sans erreur et listé dans [`Import::ignores`].
//! On n'accepte que la syntaxe d'Equalizer APO : pas de `1k` pour 1000 Hz.

use super::{ecrire_nombre, lire_nombre, ErreurFormat, Import, LigneIgnoree, RaisonIgnoree};
use retro_dsp::{Courbe, Filtre, TypeFiltre};

/// Q des plateaux écrits sans Q (`LS`, `HS`).
const Q_PAR_DEFAUT_PLATEAU: f64 = 0.707;

/// Lit un fichier Equalizer APO.
///
/// Rend une erreur seulement s'il n'y a ni préampli ni filtre utilisable.
pub fn lire(texte: &str) -> Result<Import, ErreurFormat> {
    let texte = texte.trim_start_matches('\u{feff}');
    let mut courbe = Courbe::default();
    let mut ignores = Vec::new();
    let mut preampli_vu = false;

    for (index, ligne) in texte.lines().enumerate() {
        let ligne = ligne.trim();
        if ligne.is_empty() {
            continue;
        }
        let mut ignorer = |raison| {
            ignores.push(LigneIgnoree {
                numero: index + 1,
                texte: ligne.to_string(),
                raison,
            });
        };
        if ligne.starts_with('#') {
            ignorer(RaisonIgnoree::Commentaire);
            continue;
        }
        let Some((mot, reste)) = ligne.split_once(':') else {
            ignorer(RaisonIgnoree::Illisible);
            continue;
        };
        let mot = mot.trim().to_lowercase();
        if mot == "preamp" {
            match valeur_en_db(reste) {
                Some(db) => {
                    courbe.preampli_db += db;
                    preampli_vu = true;
                }
                None => ignorer(RaisonIgnoree::Illisible),
            }
        } else if est_un_mot_filtre(&mot) {
            match lire_filtre(reste) {
                Ok(Some(filtre)) => courbe.filtres.push(filtre),
                Ok(None) => ignorer(RaisonIgnoree::Desactive),
                Err(raison) => ignorer(raison),
            }
        } else {
            ignorer(RaisonIgnoree::CommandeNonGeree);
        }
    }

    if !preampli_vu && courbe.filtres.is_empty() {
        return Err(ErreurFormat::AucuneLigneUtilisable);
    }
    Ok(Import {
        courbe,
        nom: None,
        ignores,
    })
}

/// Vrai pour `filter`, `filter 3`, `filter3`.
fn est_un_mot_filtre(mot: &str) -> bool {
    match mot.strip_prefix("filter") {
        Some(suite) => suite.trim().chars().all(|c| c.is_ascii_digit()),
        None => false,
    }
}

/// Lit `-6 dB` (l'unité est facultative).
fn valeur_en_db(texte: &str) -> Option<f64> {
    let mut mots = texte.split_whitespace();
    let valeur = lire_nombre(mots.next()?)?;
    match mots.next() {
        None => Some(valeur),
        Some(unite) if unite.eq_ignore_ascii_case("db") && mots.next().is_none() => Some(valeur),
        Some(_) => None,
    }
}

/// Lit ce qui suit `Filter n:`.
///
/// `Ok(None)` : le filtre est `OFF`.
fn lire_filtre(reste: &str) -> Result<Option<Filtre>, RaisonIgnoree> {
    let mots: Vec<&str> = reste.split_whitespace().collect();
    let mut position = 0;
    match mots.first().map(|m| m.to_uppercase()).as_deref() {
        Some("OFF") => return Ok(None),
        Some("ON") => position = 1,
        _ => {}
    }
    let code = mots
        .get(position)
        .ok_or(RaisonIgnoree::Illisible)?
        .to_uppercase();
    let type_filtre = match code.as_str() {
        "PK" | "PEQ" => TypeFiltre::Cloche,
        "LSC" | "LS" => TypeFiltre::PlateauBas,
        "HSC" | "HS" => TypeFiltre::PlateauHaut,
        _ => return Err(RaisonIgnoree::TypeFiltreNonGere(code)),
    };

    let mut frequence = None;
    let mut gain = None;
    let mut q = None;
    let mut i = position + 1;
    while i < mots.len() {
        let mot = mots[i].to_lowercase();
        let suivant = mots.get(i + 1).copied();
        match mot.as_str() {
            "fc" => {
                frequence = suivant.and_then(lire_nombre);
                i += 2;
            }
            "gain" => {
                gain = suivant.and_then(lire_nombre);
                i += 2;
            }
            "q" => {
                q = suivant.and_then(lire_nombre);
                i += 2;
            }
            "bw" => {
                // `BW Oct 0.5` : largeur en octaves.
                let valeur = match suivant {
                    Some(m) if m.eq_ignore_ascii_case("oct") => mots.get(i + 2).copied(),
                    _ => None,
                };
                q = valeur.and_then(lire_nombre).and_then(q_depuis_octaves);
                i += 3;
            }
            _ => i += 1,
        }
    }

    let frequence_hz = frequence
        .filter(|f| *f > 0.0)
        .ok_or(RaisonIgnoree::Illisible)?;
    let gain_db = gain.ok_or(RaisonIgnoree::Illisible)?;
    let q = match (q, type_filtre) {
        (Some(q), _) if q > 0.0 => q,
        (None, TypeFiltre::PlateauBas | TypeFiltre::PlateauHaut) => Q_PAR_DEFAUT_PLATEAU,
        _ => return Err(RaisonIgnoree::Illisible),
    };
    Ok(Some(Filtre {
        type_filtre,
        frequence_hz,
        gain_db,
        q,
    }))
}

/// Passe d'une largeur en octaves à un Q : `Q = sqrt(2^bw) / (2^bw - 1)`.
fn q_depuis_octaves(octaves: f64) -> Option<f64> {
    if octaves <= 0.0 {
        return None;
    }
    let p = 2f64.powf(octaves);
    Some(p.sqrt() / (p - 1.0))
}

/// Écrit une courbe au format Equalizer APO, une ligne par filtre.
pub fn ecrire(courbe: &Courbe) -> String {
    let mut sortie = format!("Preamp: {} dB\n", ecrire_nombre(courbe.preampli_db));
    for (i, f) in courbe.filtres.iter().enumerate() {
        let code = match f.type_filtre {
            TypeFiltre::Cloche => "PK",
            TypeFiltre::PlateauBas => "LSC",
            TypeFiltre::PlateauHaut => "HSC",
        };
        sortie.push_str(&format!(
            "Filter {}: ON {} Fc {} Hz Gain {} dB Q {}\n",
            i + 1,
            code,
            ecrire_nombre(f.frequence_hz),
            ecrire_nombre(f.gain_db),
            ecrire_nombre(f.q),
        ));
    }
    sortie
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formats::outils_de_test::{courbe_melangee, proches};
    use crate::formats::RaisonIgnoree;
    use retro_dsp::TypeFiltre;

    fn lire_ok(texte: &str) -> Import {
        lire(texte).expect("le texte doit se lire")
    }

    #[test]
    fn lit_le_cas_simple() {
        let i = lire_ok("Preamp: -6 dB\nFilter 1: ON PK Fc 1000 Hz Gain -3 dB Q 1.41\n");
        assert_eq!(i.courbe.preampli_db, -6.0);
        assert_eq!(i.courbe.filtres.len(), 1);
        let f = i.courbe.filtres[0];
        assert_eq!(f.type_filtre, TypeFiltre::Cloche);
        assert_eq!(f.frequence_hz, 1000.0);
        assert_eq!(f.gain_db, -3.0);
        assert_eq!(f.q, 1.41);
        assert!(i.ignores.is_empty());
    }

    #[test]
    fn lit_les_types_de_plateaux() {
        let i = lire_ok(
            "Filter 1: ON LSC Fc 100 Hz Gain 2 dB Q 0.5\n\
             Filter 2: ON HSC Fc 9000 Hz Gain -2 dB Q 0.6\n\
             Filter 3: ON PEQ Fc 500 Hz Gain 1 dB Q 2\n",
        );
        let types: Vec<_> = i.courbe.filtres.iter().map(|f| f.type_filtre).collect();
        assert_eq!(
            types,
            [
                TypeFiltre::PlateauBas,
                TypeFiltre::PlateauHaut,
                TypeFiltre::Cloche
            ]
        );
    }

    #[test]
    fn ls_et_hs_sans_q_prennent_0_707() {
        let i =
            lire_ok("Filter 1: ON LS Fc 100 Hz Gain 2 dB\nFilter 2: ON HS Fc 9000 Hz Gain -2 dB\n");
        assert_eq!(i.courbe.filtres[0].type_filtre, TypeFiltre::PlateauBas);
        assert_eq!(i.courbe.filtres[0].q, 0.707);
        assert_eq!(i.courbe.filtres[1].type_filtre, TypeFiltre::PlateauHaut);
        assert_eq!(i.courbe.filtres[1].q, 0.707);
    }

    #[test]
    fn ls_avec_q_garde_le_q() {
        let i = lire_ok("Filter 1: ON LS Fc 100 Hz Gain 2 dB Q 0.9\n");
        assert_eq!(i.courbe.filtres[0].q, 0.9);
    }

    #[test]
    fn tolere_filter_sans_numero_espaces_casse_et_crlf() {
        let i = lire_ok(
            "preamp:   -4.5   db\r\n\
             FILTER:  on   pk   fc   200   hz   gain   3   db   q   1.0\r\n\
             filter 12 : ON Pk Fc 2000 HZ Gain -1 DB Q 2\r\n",
        );
        assert_eq!(i.courbe.preampli_db, -4.5);
        assert_eq!(i.courbe.filtres.len(), 2);
        assert_eq!(i.courbe.filtres[0].frequence_hz, 200.0);
        assert_eq!(i.courbe.filtres[1].gain_db, -1.0);
    }

    #[test]
    fn tolere_le_bom_utf8() {
        let i = lire_ok("\u{feff}Preamp: -3 dB\nFilter 1: ON PK Fc 100 Hz Gain 1 dB Q 1\n");
        assert_eq!(i.courbe.preampli_db, -3.0);
        assert_eq!(i.courbe.filtres.len(), 1);
    }

    #[test]
    fn tolere_la_virgule_decimale() {
        let i = lire_ok("Preamp: -3,5 dB\nFilter 1: ON PK Fc 1000,5 Hz Gain -2,25 dB Q 1,41\n");
        assert_eq!(i.courbe.preampli_db, -3.5);
        let f = i.courbe.filtres[0];
        assert_eq!(f.frequence_hz, 1000.5);
        assert_eq!(f.gain_db, -2.25);
        assert_eq!(f.q, 1.41);
    }

    #[test]
    fn lit_la_largeur_en_octaves() {
        // Q = sqrt(2^bw) / (2^bw - 1) ; bw = 1 octave donne Q = sqrt(2).
        let i = lire_ok("Filter 1: ON PK Fc 1000 Hz Gain 3 dB BW Oct 1\n");
        assert!((i.courbe.filtres[0].q - 2f64.sqrt()).abs() < 1e-9);
    }

    #[test]
    fn les_lignes_off_sont_ignorees_et_listees() {
        let i = lire_ok(
            "Filter 1: OFF PK Fc 1000 Hz Gain -3 dB Q 1.41\nFilter 2: ON PK Fc 2000 Hz Gain 1 dB Q 1\n",
        );
        assert_eq!(i.courbe.filtres.len(), 1);
        assert_eq!(i.ignores.len(), 1);
        assert_eq!(i.ignores[0].numero, 1);
        assert_eq!(i.ignores[0].raison, RaisonIgnoree::Desactive);
        assert!(i.ignores[0].texte.starts_with("Filter 1: OFF"));
    }

    #[test]
    fn les_commentaires_sont_ignores_et_listes() {
        let i = lire_ok("# mon profil\nFilter 1: ON PK Fc 1000 Hz Gain 1 dB Q 1\n");
        assert_eq!(i.ignores.len(), 1);
        assert_eq!(i.ignores[0].raison, RaisonIgnoree::Commentaire);
        assert_eq!(i.ignores[0].numero, 1);
    }

    #[test]
    fn les_commandes_inconnues_sont_ignorees_sans_erreur() {
        let i = lire_ok(
            "Include: autre.txt\nChannel: L\nDevice: Casque\nGraphicEQ: 20 0; 100 1\n\
             Filter 1: ON PK Fc 1000 Hz Gain 1 dB Q 1\n",
        );
        assert_eq!(i.courbe.filtres.len(), 1);
        assert_eq!(i.ignores.len(), 4);
        assert!(i
            .ignores
            .iter()
            .all(|l| l.raison == RaisonIgnoree::CommandeNonGeree));
    }

    #[test]
    fn les_autres_types_de_filtre_sont_ignores_avec_leur_code() {
        let i = lire_ok(
            "Filter 1: ON LP Fc 15000 Hz\nFilter 2: ON HP Fc 20 Hz\nFilter 3: ON NO Fc 50 Hz\n\
             Filter 4: ON AP Fc 50 Hz Q 1\nFilter 5: ON PK Fc 1000 Hz Gain 1 dB Q 1\n",
        );
        assert_eq!(i.courbe.filtres.len(), 1);
        let codes: Vec<_> = i.ignores.iter().map(|l| l.raison.clone()).collect();
        assert_eq!(
            codes,
            [
                RaisonIgnoree::TypeFiltreNonGere("LP".into()),
                RaisonIgnoree::TypeFiltreNonGere("HP".into()),
                RaisonIgnoree::TypeFiltreNonGere("NO".into()),
                RaisonIgnoree::TypeFiltreNonGere("AP".into()),
            ]
        );
    }

    #[test]
    fn une_ligne_filtre_cassee_est_ignoree_comme_illisible() {
        let i = lire_ok(
            "Filter 1: ON PK Fc abc Hz Gain 1 dB Q 1\nFilter 2: ON PK Fc 1000 Hz Q 1\n\
             Filter 3: ON PK Fc 1000 Hz Gain 1 dB Q 0\nFilter 4: ON PK Fc 1000 Hz Gain 1 dB Q 1\n",
        );
        assert_eq!(i.courbe.filtres.len(), 1);
        assert_eq!(i.ignores.len(), 3);
        assert!(i
            .ignores
            .iter()
            .all(|l| l.raison == RaisonIgnoree::Illisible));
    }

    #[test]
    fn n_invente_pas_la_syntaxe_1k() {
        let i = lire_ok("Preamp: 0 dB\nFilter 1: ON PK Fc 1k Hz Gain 1 dB Q 1\n");
        assert!(i.courbe.filtres.is_empty());
        assert_eq!(i.ignores[0].raison, RaisonIgnoree::Illisible);
    }

    #[test]
    fn les_preamps_s_additionnent_comme_dans_equalizer_apo() {
        let i = lire_ok("Preamp: -2 dB\nPreamp: -1.5 dB\n");
        assert_eq!(i.courbe.preampli_db, -3.5);
    }

    #[test]
    fn un_fichier_sans_ligne_utilisable_donne_une_erreur() {
        for texte in [
            "",
            "\n\n",
            "# rien\n",
            "Include: x.txt\n",
            "Filter 1: OFF PK Fc 1 Hz Gain 1 dB Q 1\n",
            "n'importe quoi",
        ] {
            assert_eq!(
                lire(texte),
                Err(ErreurFormat::AucuneLigneUtilisable),
                "{texte:?}"
            );
        }
    }

    #[test]
    fn un_preampli_seul_est_une_courbe_plate_valide() {
        let i = lire_ok("Preamp: -2 dB\n");
        assert!(i.courbe.filtres.is_empty());
        assert_eq!(i.courbe.preampli_db, -2.0);
    }

    #[test]
    fn ecrit_la_grammaire_attendue() {
        let texte = ecrire(&courbe_melangee());
        let lignes: Vec<_> = texte.lines().collect();
        assert_eq!(lignes[0], "Preamp: -6.5 dB");
        assert_eq!(lignes[1], "Filter 1: ON LSC Fc 105 Hz Gain 5.5 dB Q 0.7");
        assert_eq!(lignes[2], "Filter 2: ON PK Fc 1000 Hz Gain -3 dB Q 1.41");
        assert_eq!(lignes[3], "Filter 3: ON PK Fc 3150.5 Hz Gain 0 dB Q 4");
        assert_eq!(
            lignes[4],
            "Filter 4: ON HSC Fc 8000 Hz Gain -2.25 dB Q 0.71"
        );
        assert!(texte.ends_with('\n'));
    }

    #[test]
    fn aller_retour_garde_la_courbe() {
        let c = courbe_melangee();
        let relue = lire(&ecrire(&c)).expect("relecture").courbe;
        assert!(proches(&c, &relue), "{c:?} != {relue:?}");
    }

    #[test]
    fn aller_retour_d_une_courbe_plate() {
        let c = retro_dsp::Courbe::default();
        assert_eq!(lire(&ecrire(&c)).expect("relecture").courbe, c);
    }
}
