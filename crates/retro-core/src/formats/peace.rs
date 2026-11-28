//! Préréglages `.peace` de Peace, l'interface graphique d'Equalizer APO.
//!
//! # Ce que le format est (observé sur des fichiers réels)
//!
//! Peace n'a pas de spécification publique. Ce module est écrit d'après la
//! structure observée sur des préréglages Peace publiquement disponibles ; les
//! fichiers d'essai de `fixtures/` sont écrits à la main pour Retro EQ (voir
//! `fixtures/README.md`).
//!
//! C'est un fichier INI (souvent en fins de ligne CRLF, sans BOM). Les bandes
//! du réglage « Tous les haut-parleurs » sont réparties sur plusieurs sections,
//! une entrée par bande, numérotées à partir de 1 :
//!
//! - `[Frequencies]` : `Frequency1=1000` (Hz)
//! - `[Gains]` : `Gain1=-3` (dB). Une bande sans `Gain` est à plat (0 dB).
//! - `[Qualities]` : `Quality1=1.41`
//! - `[Filters]` : `Filter1=4` : code du type, absent pour une cloche (0)
//! - `[Disabled]` : `Disabled1=1` : bande éteinte
//! - `[General]` : `PreAmp=-3.5`
//!
//! Codes de type (ordre de la liste déroulante de Peace, vérifié sur les
//! presets officiels « Filter low pass 1 kHz », « Filter notch 1 kHz »…) :
//! 0 cloche, 1 passe-bas, 2 passe-haut, 3 passe-bande, 4 plateau bas,
//! 5 plateau haut, 6 notch, 7 passe-tout, 8 et 9 plateaux bas/haut réglés par
//! une pente en dB, 10 à 13 passe-bas/haut Butterworth et Linkwitz-Riley,
//! 14 et 15 plateaux bas/haut dont Q est une pente, 16 et 17 idem avec
//! fréquence de coin.
//!
//! # Ce qu'on lit
//!
//! Codes 0 (cloche), 4 (plateau bas), 5 (plateau haut), et 14 et 15 (plateaux
//! « Q comme pente », traités comme des plateaux ordinaires : approximation
//! sur la forme exacte du plateau, signalée au rapport par
//! [`RaisonIgnoree::Approximation`]). Les autres types vont au rapport d'ignorés
//! sous la forme `Peace <code>`. Les bandes éteintes ou sans gain y vont aussi, comme
//! celles à 22 050 Hz ou plus ([`RaisonIgnoree::FrequenceTropHaute`]).
//!
//! Les sections propres à un haut-parleur (`[Gains1]`…`[Gains8]`, et leurs
//! équivalents `[Frequencies1]`…) ne sont lues que pour être comparées au réglage
//! commun : identiques, elles ne sont pas signalées ; différentes, leurs lignes vont
//! au rapport ([`RaisonIgnoree::HautParleurParticulier`]) et [`Import::partiel`] est vrai.
//!
//! # Ce qui n'est pas géré
//!
//! Réglages par haut-parleur, mode « GraphicEQ » (convolution), commandes
//! Equalizer APO ajoutées, graves/aigus dédiés, automatisation, MIDI.
//!
//! # À vérifier
//!
//! L'écriture produit un fichier conforme à ce qu'on a observé, mais elle n'a
//! pas été essayée dans Peace lui-même : elle n'écrit pas les sections par
//! haut-parleur (`[Frequencies1]`…) que Peace ajoute lui-même.

use super::{
    ecrire_nombre, lire_nombre, ErreurFormat, Import, LigneIgnoree, RaisonIgnoree, FREQUENCE_MAX_HZ,
};
use retro_dsp::{Courbe, Filtre, TypeFiltre};

/// Q d'une bande dont le fichier ne donne pas de `Quality` (valeur par défaut de Peace).
const Q_PAR_DEFAUT: f64 = 1.41;

/// Une ligne `clé=valeur` du fichier.
struct Entree {
    numero: usize,
    cle: String,
    valeur: String,
    texte: String,
}

/// Une section `[Nom]` avec ses entrées.
struct Section {
    /// Nom en minuscules, pour comparer sans tenir compte de la casse.
    nom: String,
    entrees: Vec<Entree>,
}

/// Découpe le texte en sections. Rend `None` s'il n'y a aucun en-tête `[...]`.
fn decouper(texte: &str) -> Option<Vec<Section>> {
    let mut sections: Vec<Section> = Vec::new();
    for (index, ligne) in texte.lines().enumerate() {
        let ligne = ligne.trim();
        if ligne.is_empty() || ligne.starts_with(';') {
            continue;
        }
        if let Some(nom) = ligne.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            sections.push(Section {
                nom: nom.trim().to_lowercase(),
                entrees: Vec::new(),
            });
        } else if let (Some(section), Some((cle, valeur))) =
            (sections.last_mut(), ligne.split_once('='))
        {
            section.entrees.push(Entree {
                numero: index + 1,
                cle: cle.trim().to_lowercase(),
                valeur: valeur.trim().to_string(),
                texte: ligne.to_string(),
            });
        }
    }
    (!sections.is_empty()).then_some(sections)
}

/// Toutes les entrées des sections de ce nom.
fn entrees<'a>(sections: &'a [Section], nom: &'a str) -> impl Iterator<Item = &'a Entree> {
    sections
        .iter()
        .filter(move |s| s.nom == nom)
        .flat_map(|s| s.entrees.iter())
}

/// La valeur de `cle` (en minuscules) dans la section `nom`.
fn chercher<'a>(sections: &'a [Section], nom: &str, cle: &str) -> Option<&'a Entree> {
    sections
        .iter()
        .filter(|s| s.nom == nom)
        .flat_map(|s| s.entrees.iter())
        .find(|e| e.cle == cle)
}

/// Lit un fichier `.peace`.
///
/// Rend [`ErreurFormat::ContenuInvalide`] si le texte n'est pas un INI, et
/// [`ErreurFormat::AucuneLigneUtilisable`] s'il n'a ni bande ni préampli.
pub fn lire(texte: &str) -> Result<Import, ErreurFormat> {
    let texte = texte.trim_start_matches('\u{feff}');
    let sections = decouper(texte).ok_or_else(|| {
        ErreurFormat::ContenuInvalide(
            "pas de section INI `[...]`, ce n'est pas un fichier .peace".into(),
        )
    })?;

    let mut courbe = Courbe::default();
    let mut ignores = Vec::new();
    let mut preampli_vu = false;

    if let Some(entree) = chercher(&sections, "general", "preamp") {
        match lire_nombre(&entree.valeur) {
            Some(db) => {
                courbe.preampli_db = db;
                preampli_vu = true;
            }
            None => ignores.push(ignoree(entree, RaisonIgnoree::Illisible)),
        }
    }

    // Les bandes, dans l'ordre de leur numéro.
    let mut bandes: Vec<(u32, &Entree)> = entrees(&sections, "frequencies")
        .filter_map(|e| Some((numero_de(&e.cle, "frequency")?, e)))
        .collect();
    bandes.sort_by_key(|(n, _)| *n);

    for (n, entree) in bandes {
        match lire_bande(&sections, n, entree) {
            Ok((filtre, approche)) => {
                courbe.filtres.push(filtre);
                if approche {
                    ignores.push(ignoree(entree, RaisonIgnoree::Approximation));
                }
            }
            Err(raison) => ignores.push(ignoree(entree, raison)),
        }
    }

    // Réglages propres à un haut-parleur : `[Gains1]`, `[Frequencies2]`… On ne les
    // signale que s'ils diffèrent du réglage commun.
    for section in &sections {
        let Some((defaut, genre)) = genre_de_section(&section.nom) else {
            continue;
        };
        if resume(section.entrees.iter(), defaut) != resume(entrees(&sections, genre), defaut) {
            for entree in &section.entrees {
                ignores.push(ignoree(entree, RaisonIgnoree::HautParleurParticulier));
            }
        }
    }
    ignores.sort_by_key(|l| l.numero);

    if !preampli_vu && courbe.filtres.is_empty() {
        return Err(ErreurFormat::AucuneLigneUtilisable);
    }
    Ok(Import::nouveau(courbe, ignores))
}

/// Pour une section propre à un haut-parleur (`gains1`), rend la valeur par défaut d'une
/// entrée absente et le nom de la section commune (`gains`).
fn genre_de_section(nom: &str) -> Option<(f64, &'static str)> {
    const GENRES: [(&str, f64); 5] = [
        ("frequencies", f64::NAN),
        ("gains", 0.0),
        ("qualities", Q_PAR_DEFAUT),
        ("filters", 0.0),
        ("disabled", 0.0),
    ];
    GENRES.into_iter().find_map(|(genre, defaut)| {
        let suite = nom.strip_prefix(genre)?;
        (!suite.is_empty() && suite.chars().all(|c| c.is_ascii_digit())).then_some((defaut, genre))
    })
}

/// Les entrées d'une section réduites à ce qui compte pour comparer deux réglages :
/// clé et valeur (lue comme nombre quand c'est possible), sans les valeurs par défaut,
/// qui n'ajoutent rien à une entrée absente.
fn resume<'a>(
    entrees: impl Iterator<Item = &'a Entree>,
    defaut: f64,
) -> std::collections::BTreeMap<String, String> {
    entrees
        .filter(|e| lire_nombre(&e.valeur).is_none_or(|v| v != defaut))
        .map(|e| {
            let valeur = lire_nombre(&e.valeur).map_or_else(|| e.valeur.clone(), |v| v.to_string());
            (e.cle.clone(), valeur)
        })
        .collect()
}

fn ignoree(entree: &Entree, raison: RaisonIgnoree) -> LigneIgnoree {
    LigneIgnoree {
        numero: entree.numero,
        texte: entree.texte.clone(),
        raison,
    }
}

/// `numero_de("frequency12", "frequency")` donne `Some(12)`.
fn numero_de(cle: &str, prefixe: &str) -> Option<u32> {
    let suite = cle.strip_prefix(prefixe)?;
    if suite.is_empty() || !suite.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    suite.parse().ok()
}

/// Rassemble les entrées de la bande `n` en un filtre, ou dit pourquoi on l'écarte.
/// Le booléen dit si le filtre est une approximation (codes 14 et 15).
fn lire_bande(
    sections: &[Section],
    n: u32,
    frequence: &Entree,
) -> Result<(Filtre, bool), RaisonIgnoree> {
    let frequence_hz = lire_nombre(&frequence.valeur)
        .filter(|f| *f > 0.0)
        .ok_or(RaisonIgnoree::Illisible)?;

    let cle = |prefixe: &str| format!("{prefixe}{n}");
    let eteinte = chercher(sections, "disabled", &cle("disabled"))
        .is_some_and(|e| !matches!(e.valeur.as_str(), "" | "0"));
    if eteinte {
        return Err(RaisonIgnoree::Desactive);
    }
    if frequence_hz >= FREQUENCE_MAX_HZ {
        return Err(RaisonIgnoree::FrequenceTropHaute);
    }

    let code = match chercher(sections, "filters", &cle("filter")) {
        Some(e) => e
            .valeur
            .parse::<u32>()
            .map_err(|_| RaisonIgnoree::Illisible)?,
        None => 0,
    };
    let type_filtre = match code {
        0 => TypeFiltre::Cloche,
        4 | 14 => TypeFiltre::PlateauBas,
        5 | 15 => TypeFiltre::PlateauHaut,
        autre => return Err(RaisonIgnoree::TypeFiltreNonGere(format!("Peace {autre}"))),
    };

    let gain_db = match chercher(sections, "gains", &cle("gain")) {
        Some(e) => lire_nombre(&e.valeur).ok_or(RaisonIgnoree::Illisible)?,
        None => return Err(RaisonIgnoree::BandeSansGain),
    };
    let q = match chercher(sections, "qualities", &cle("quality")) {
        Some(e) => lire_nombre(&e.valeur)
            .filter(|q| *q > 0.0)
            .ok_or(RaisonIgnoree::Illisible)?,
        None => Q_PAR_DEFAUT,
    };
    Ok((
        Filtre {
            type_filtre,
            frequence_hz,
            gain_db,
            q,
        },
        matches!(code, 14 | 15),
    ))
}

/// Écrit un fichier `.peace` (fins de ligne CRLF, comme Peace).
///
/// Seul le réglage « Tous les haut-parleurs » est écrit. Chaque filtre a une
/// entrée `Gain` explicite, même à 0 dB, pour qu'il survive à la relecture.
pub fn ecrire(courbe: &Courbe) -> String {
    let mut lignes: Vec<String> = vec![
        "[Speakers]".into(),
        "SpeakerId0=0".into(),
        "SpeakerTargets0=all".into(),
        "SpeakerName0=All".into(),
    ];
    lignes.push("[Frequencies]".into());
    for (i, f) in courbe.filtres.iter().enumerate() {
        lignes.push(format!(
            "Frequency{}={}",
            i + 1,
            ecrire_nombre(f.frequence_hz)
        ));
    }
    lignes.push("[Gains]".into());
    for (i, f) in courbe.filtres.iter().enumerate() {
        lignes.push(format!("Gain{}={}", i + 1, ecrire_nombre(f.gain_db)));
    }
    lignes.push("[Qualities]".into());
    for (i, f) in courbe.filtres.iter().enumerate() {
        lignes.push(format!("Quality{}={}", i + 1, ecrire_nombre(f.q)));
    }
    let types: Vec<String> = courbe
        .filtres
        .iter()
        .enumerate()
        .filter_map(|(i, f)| match f.type_filtre {
            TypeFiltre::Cloche => None,
            TypeFiltre::PlateauBas => Some(format!("Filter{}=4", i + 1)),
            TypeFiltre::PlateauHaut => Some(format!("Filter{}=5", i + 1)),
        })
        .collect();
    if !types.is_empty() {
        lignes.push("[Filters]".into());
        lignes.extend(types);
    }
    lignes.push("[General]".into());
    lignes.push(format!("PreAmp={}", ecrire_nombre(courbe.preampli_db)));

    let mut sortie = lignes.join("\r\n");
    sortie.push_str("\r\n");
    sortie
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formats::outils_de_test::{courbe_melangee, proches};
    use crate::formats::RaisonIgnoree;
    use retro_dsp::TypeFiltre;

    const MINIMAL: &str = include_str!("fixtures/peace_exemple_minimal.peace");
    const GRADO: &str = include_str!("fixtures/peace_exemple_bandes_eteintes.peace");
    const PLATEAUX: &str = include_str!("fixtures/peace_exemple_plateaux.peace");
    const PLATEAU_BAS: &str = include_str!("fixtures/peace_exemple_plateau_bas.peace");
    const PENTE: &str = include_str!("fixtures/peace_exemple_pente.peace");
    const GRAPHIC_VIDE: &str = include_str!("fixtures/peace_exemple_graphic_eq_vide.peace");

    #[test]
    fn lit_le_fichier_minimal_et_signale_les_types_non_geres() {
        // Filter2=2 (passe-haut) et Filter4=3 (passe-bande) ne sont pas représentables.
        let i = lire(MINIMAL).expect("lecture");
        assert_eq!(i.courbe.filtres.len(), 2);
        assert_eq!(i.courbe.filtres[0].frequence_hz, 45.0);
        assert_eq!(i.courbe.filtres[0].gain_db, 1.5);
        assert_eq!(i.courbe.filtres[0].q, 0.9);
        assert_eq!(i.courbe.filtres[1].frequence_hz, 3500.0);
        assert_eq!(i.courbe.filtres[1].q, 2.2);
        let raisons: Vec<_> = i.ignores.iter().map(|l| l.raison.clone()).collect();
        assert_eq!(
            raisons,
            [
                RaisonIgnoree::TypeFiltreNonGere("Peace 2".into()),
                RaisonIgnoree::TypeFiltreNonGere("Peace 3".into()),
            ]
        );
        assert_eq!(i.ignores[0].texte, "Frequency2=220");
        assert_eq!(i.courbe.preampli_db, 0.0);
    }

    #[test]
    fn les_bandes_desactivees_sont_ignorees() {
        let i = lire(GRADO).expect("lecture");
        // 9 bandes, 6 désactivées (1, 2, 4, 6, 8, 9) : restent 250 Hz, 900 Hz, 2600 Hz.
        let freqs: Vec<_> = i.courbe.filtres.iter().map(|f| f.frequence_hz).collect();
        assert_eq!(freqs, [250.0, 900.0, 2600.0]);
        assert_eq!(
            i.ignores
                .iter()
                .filter(|l| l.raison == RaisonIgnoree::Desactive)
                .count(),
            6
        );
    }

    #[test]
    fn lit_les_plateaux_bas_et_haut() {
        let i = lire(PLATEAUX).expect("lecture");
        assert_eq!(i.courbe.filtres[0].type_filtre, TypeFiltre::Cloche);
        assert_eq!(i.courbe.filtres[1].type_filtre, TypeFiltre::PlateauBas);
        assert_eq!(i.courbe.filtres[1].frequence_hz, 95.0);
        assert_eq!(i.courbe.filtres[2].type_filtre, TypeFiltre::PlateauHaut);
        assert_eq!(i.courbe.filtres[2].frequence_hz, 640.0);
        assert_eq!(i.courbe.filtres.len(), 10);
    }

    #[test]
    fn lit_un_preset_a_un_seul_plateau_avec_preampli() {
        let i = lire(PLATEAU_BAS).expect("lecture");
        assert_eq!(i.courbe.preampli_db, -6.0);
        assert_eq!(i.courbe.filtres.len(), 1);
        let f = i.courbe.filtres[0];
        assert_eq!(f.type_filtre, TypeFiltre::PlateauBas);
        assert_eq!(f.frequence_hz, 2500.0);
        assert_eq!(f.gain_db, 6.0);
        assert_eq!(f.q, 0.85);
        // Les 12 autres bandes sont à plat.
        assert_eq!(
            i.ignores
                .iter()
                .filter(|l| l.raison == RaisonIgnoree::BandeSansGain)
                .count(),
            12
        );
    }

    #[test]
    fn les_plateaux_a_pente_en_db_du_preset_pente_sont_ignores() {
        // Types 8 et 9 : plateaux réglés par une pente en dB, sans Q : non représentables.
        let i = lire(PENTE).expect("lecture");
        assert!(i.courbe.filtres.is_empty());
        let nb = i
            .ignores
            .iter()
            .filter(|l| matches!(l.raison, RaisonIgnoree::TypeFiltreNonGere(_)))
            .count();
        assert_eq!(nb, 2);
    }

    #[test]
    fn un_fichier_graphic_eq_sans_filtre_donne_une_erreur() {
        assert_eq!(lire(GRAPHIC_VIDE), Err(ErreurFormat::AucuneLigneUtilisable));
    }

    #[test]
    fn un_texte_sans_section_ini_est_invalide() {
        assert!(matches!(
            lire("Preamp: -3 dB\n"),
            Err(ErreurFormat::ContenuInvalide(_))
        ));
        assert!(matches!(lire(""), Err(ErreurFormat::ContenuInvalide(_))));
    }

    #[test]
    fn tolere_bom_lf_casse_et_virgule() {
        let texte = "\u{feff}[frequencies]\nFrequency1=1000\n[gains]\nGain1=-2,5\n[qualities]\nQuality1=1,41\n[general]\npreamp=-3,5\n";
        let i = lire(texte).expect("lecture");
        assert_eq!(i.courbe.preampli_db, -3.5);
        assert_eq!(i.courbe.filtres[0].gain_db, -2.5);
        assert_eq!(i.courbe.filtres[0].q, 1.41);
    }

    #[test]
    fn le_code_14_et_15_donnent_des_plateaux() {
        let texte = "[Frequencies]\nFrequency1=100\nFrequency2=8000\n[Gains]\nGain1=2\nGain2=-2\n[Qualities]\nQuality1=0.5\nQuality2=0.4\n[Filters]\nFilter1=14\nFilter2=15\n";
        let i = lire(texte).expect("lecture");
        assert_eq!(i.courbe.filtres[0].type_filtre, TypeFiltre::PlateauBas);
        assert_eq!(i.courbe.filtres[1].type_filtre, TypeFiltre::PlateauHaut);
    }

    #[test]
    fn les_codes_14_et_15_sont_signales_comme_approximation() {
        let texte = "[Frequencies]\nFrequency1=100\nFrequency2=8000\nFrequency3=500\n[Gains]\nGain1=2\nGain2=-2\nGain3=1\n[Qualities]\nQuality1=0.5\nQuality2=0.4\n[Filters]\nFilter1=14\nFilter2=15\n";
        let i = lire(texte).expect("lecture");
        assert_eq!(i.courbe.filtres.len(), 3, "les bandes restent importées");
        let notes: Vec<_> = i
            .ignores
            .iter()
            .map(|l| (l.texte.as_str(), l.raison.clone()))
            .collect();
        assert_eq!(
            notes,
            [
                ("Frequency1=100", RaisonIgnoree::Approximation),
                ("Frequency2=8000", RaisonIgnoree::Approximation),
            ]
        );
        assert!(!i.partiel, "une approximation n'est pas une perte");
    }

    #[test]
    fn un_reglage_haut_parleur_identique_au_commun_est_silencieux() {
        // Gain absent = 0 dB : l'égalité tient aussi avec les valeurs par défaut omises.
        let texte = "[Frequencies]\nFrequency1=1000\nFrequency2=2000\n[Gains]\nGain1=1\nGain2=0\n[Gains1]\nGain1=1.0\n[Gains2]\nGain1=1\nGain2=0\n[Frequencies1]\nFrequency1=1000\nFrequency2=2000\n";
        let i = lire(texte).expect("lecture");
        assert!(i.ignores.is_empty(), "{:?}", i.ignores);
        assert!(!i.partiel);
    }

    #[test]
    fn un_reglage_haut_parleur_different_rend_l_import_partiel() {
        let texte =
            "[Frequencies]\nFrequency1=1000\n[Gains]\nGain1=1\n[Frequencies1]\nFrequency1=1500\n";
        let i = lire(texte).expect("lecture");
        assert_eq!(i.ignores.len(), 1);
        assert_eq!(i.ignores[0].raison, RaisonIgnoree::HautParleurParticulier);
        assert!(i.partiel);
    }

    #[test]
    fn les_reglages_d_un_haut_parleur_particulier_sont_signales() {
        let texte = "[Frequencies]\nFrequency1=1000\n[Gains]\nGain1=1\n[Gains1]\nGain1=5\n";
        let i = lire(texte).expect("lecture");
        assert_eq!(i.courbe.filtres.len(), 1);
        assert_eq!(i.ignores.len(), 1);
        assert_eq!(i.ignores[0].raison, RaisonIgnoree::HautParleurParticulier);
        assert_eq!(i.ignores[0].numero, 6);
        assert!(i.partiel);
    }

    #[test]
    fn une_bande_a_ou_au_dela_de_nyquist_est_signalee_au_lieu_d_etre_importee() {
        let texte = "[Frequencies]\nFrequency1=1000\nFrequency2=30000\n[Gains]\nGain1=1\nGain2=6\n";
        let i = lire(texte).expect("lecture");
        assert_eq!(i.courbe.filtres.len(), 1);
        assert_eq!(i.courbe.filtres[0].frequence_hz, 1000.0);
        assert_eq!(i.ignores.len(), 1);
        assert_eq!(i.ignores[0].raison, RaisonIgnoree::FrequenceTropHaute);
        assert_eq!(i.ignores[0].texte, "Frequency2=30000");
        assert!(i.partiel);
    }

    #[test]
    fn ecrit_un_fichier_ini_lisible() {
        let texte = ecrire(&courbe_melangee());
        assert!(texte.contains("[Frequencies]\r\nFrequency1=105\r\n"));
        assert!(texte.contains("Filter1=4\r\n"));
        assert!(texte.contains("Filter4=5\r\n"));
        assert!(!texte.contains("Filter2="));
        assert!(texte.contains("PreAmp=-6.5\r\n"));
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
