//! Import et export des réglages d'égaliseur dans les formats que les gens s'échangent.
//!
//! Un module par format : [`equalizer_apo`], [`autoeq`], [`peace`] (lecture et
//! écriture), [`wavelet`] (écriture seulement). Chaque lecture rend un [`Import`] :
//! la courbe, un nom de profil éventuel et le rapport des lignes ignorées.
//! [`detecter_format`] devine le format d'un fichier.

pub mod autoeq;
pub mod equalizer_apo;
pub mod peace;
pub mod wavelet;

use retro_dsp::Courbe;
use thiserror::Error;

/// Les formats de fichier que l'appli sait reconnaître.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// Fichier texte d'Equalizer APO (`Preamp:` et `Filter n:`).
    EqualizerApo,
    /// `ParametricEQ.txt` d'AutoEQ : même grammaire qu'Equalizer APO.
    AutoEq,
    /// Préréglage `.peace` de l'interface Peace.
    Peace,
    /// Ligne `GraphicEQ:` (export pour Wavelet).
    Wavelet,
}

/// Pourquoi une ligne n'a pas été prise en compte.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RaisonIgnoree {
    /// Ligne de commentaire (`#`).
    Commentaire,
    /// Filtre ou bande désactivé (`OFF`, ou case « désactivé » de Peace).
    Desactive,
    /// Commande qu'on ne gère pas (`Include`, `Channel`, `Device`, `GraphicEQ`…).
    CommandeNonGeree,
    /// Type de filtre qu'on ne sait pas représenter (`LP`, `HP`, `NO`…). Contient le code lu.
    TypeFiltreNonGere(String),
    /// Bande Peace sans gain : elle est à plat, elle ne change rien.
    BandeSansGain,
    /// Réglage propre à un haut-parleur précis de Peace : on ne lit que « Tous ».
    HautParleurParticulier,
    /// Ligne qu'on n'arrive pas à lire (valeur manquante, nombre invalide…).
    Illisible,
}

/// Une ligne laissée de côté, pour que l'utilisateur sache ce qui n'a pas été importé.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LigneIgnoree {
    /// Numéro de la ligne dans le fichier, en partant de 1.
    pub numero: usize,
    /// Texte de la ligne, sans espaces autour.
    pub texte: String,
    pub raison: RaisonIgnoree,
}

/// Le résultat d'une lecture : la courbe et ce qui a été écarté.
#[derive(Debug, Clone, PartialEq)]
pub struct Import {
    pub courbe: Courbe,
    /// Nom de profil déduit du fichier, quand le format permet d'en trouver un.
    pub nom: Option<String>,
    pub ignores: Vec<LigneIgnoree>,
}

/// Ce qui peut empêcher de lire un fichier.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ErreurFormat {
    /// Le fichier se lit, mais ne contient ni filtre ni préampli utilisable.
    #[error("aucune ligne utilisable : ni filtre ni préampli reconnu dans ce fichier")]
    AucuneLigneUtilisable,
    /// Le contenu n'a pas la forme attendue pour ce format.
    #[error("contenu invalide : {0}")]
    ContenuInvalide(String),
}

/// Devine le format d'un fichier d'après son contenu et son nom.
///
/// Rend `None` quand rien ne ressemble à un format connu.
pub fn detecter_format(contenu: &str, nom_de_fichier: &str) -> Option<Format> {
    let contenu = contenu.trim_start_matches('\u{feff}');
    let nom = nom_de_fichier.to_lowercase();
    let lignes: Vec<&str> = contenu.lines().map(str::trim).collect();

    let a_sections_peace = lignes.iter().any(|l| {
        let l = l.to_lowercase();
        l == "[frequencies]" || l == "[gains]" || l == "[speakers]"
    });
    if nom.ends_with(".peace") || a_sections_peace {
        return Some(Format::Peace);
    }

    let commence_par = |l: &str, mot: &str| {
        l.get(..mot.len())
            .is_some_and(|d| d.eq_ignore_ascii_case(mot))
    };
    let a_parametrique = lignes
        .iter()
        .any(|l| commence_par(l, "filter") && l.contains(':') || commence_par(l, "preamp"));
    let a_graphique = lignes.iter().any(|l| commence_par(l, "graphiceq"));

    if a_parametrique {
        let est_autoeq = nom.ends_with("parametriceq.txt");
        return Some(if est_autoeq {
            Format::AutoEq
        } else {
            Format::EqualizerApo
        });
    }
    if a_graphique {
        return Some(Format::Wavelet);
    }
    None
}

// ---------------------------------------------------------------------------
// Petits outils partagés par les modules de format.
// ---------------------------------------------------------------------------

/// Lit un nombre écrit à la française ou à l'anglaise (`-3,5` ou `-3.5`).
pub(crate) fn lire_nombre(texte: &str) -> Option<f64> {
    let n: f64 = texte.trim().replace(',', ".").parse().ok()?;
    n.is_finite().then_some(n)
}

/// Écrit un nombre au plus court, sans exposant, et sans « -0 ».
pub(crate) fn ecrire_nombre(valeur: f64) -> String {
    if valeur == 0.0 {
        "0".to_string()
    } else {
        format!("{valeur}")
    }
}

#[cfg(test)]
pub(crate) mod outils_de_test {
    use retro_dsp::{Courbe, Filtre, TypeFiltre};

    /// Une courbe qui mélange les trois types de filtre.
    pub fn courbe_melangee() -> Courbe {
        Courbe {
            preampli_db: -6.5,
            filtres: vec![
                Filtre {
                    type_filtre: TypeFiltre::PlateauBas,
                    frequence_hz: 105.0,
                    gain_db: 5.5,
                    q: 0.7,
                },
                Filtre {
                    type_filtre: TypeFiltre::Cloche,
                    frequence_hz: 1000.0,
                    gain_db: -3.0,
                    q: 1.41,
                },
                Filtre {
                    type_filtre: TypeFiltre::Cloche,
                    frequence_hz: 3150.5,
                    gain_db: 0.0,
                    q: 4.0,
                },
                Filtre {
                    type_filtre: TypeFiltre::PlateauHaut,
                    frequence_hz: 8000.0,
                    gain_db: -2.25,
                    q: 0.71,
                },
            ],
        }
    }

    /// Deux courbes égales à 0,01 près sur tous les nombres.
    pub fn proches(a: &Courbe, b: &Courbe) -> bool {
        let pres = |x: f64, y: f64| (x - y).abs() <= 0.01;
        pres(a.preampli_db, b.preampli_db)
            && a.filtres.len() == b.filtres.len()
            && a.filtres.iter().zip(&b.filtres).all(|(f, g)| {
                f.type_filtre == g.type_filtre
                    && pres(f.frequence_hz, g.frequence_hz)
                    && pres(f.gain_db, g.gain_db)
                    && pres(f.q, g.q)
            })
    }
}

#[cfg(test)]
mod tests {
    use super::outils_de_test::courbe_melangee;
    use super::*;

    #[test]
    fn reconnait_equalizer_apo() {
        let texte = "Preamp: -6 dB\nFilter 1: ON PK Fc 1000 Hz Gain -3 dB Q 1.41\n";
        assert_eq!(
            detecter_format(texte, "retro-eq.txt"),
            Some(Format::EqualizerApo)
        );
        assert_eq!(detecter_format(texte, ""), Some(Format::EqualizerApo));
    }

    #[test]
    fn reconnait_autoeq_par_le_nom_du_fichier() {
        let texte = "Preamp: -6 dB\nFilter 1: ON PK Fc 1000 Hz Gain -3 dB Q 1.41\n";
        assert_eq!(
            detecter_format(texte, "Sony INZONE H9 II ParametricEQ.txt"),
            Some(Format::AutoEq)
        );
        assert_eq!(
            detecter_format(texte, "x parametriceq.TXT"),
            Some(Format::AutoEq)
        );
    }

    #[test]
    fn reconnait_peace_par_l_extension_ou_les_sections() {
        assert_eq!(detecter_format("", "casque.peace"), Some(Format::Peace));
        assert_eq!(
            detecter_format("[Frequencies]\r\nFrequency1=100\r\n", "inconnu.ini"),
            Some(Format::Peace)
        );
        assert_eq!(
            detecter_format("\u{feff}[Gains]\nGain1=1\n", "x"),
            Some(Format::Peace)
        );
    }

    #[test]
    fn reconnait_wavelet() {
        assert_eq!(
            detecter_format("GraphicEQ: 20 -1.2; 21 -1.1\n", "graphic.txt"),
            Some(Format::Wavelet)
        );
        assert_eq!(
            detecter_format(&wavelet::exporter(|_| 0.0, 0.0), "x.txt"),
            Some(Format::Wavelet)
        );
    }

    #[test]
    fn un_texte_quelconque_n_a_pas_de_format() {
        assert_eq!(detecter_format("bonjour", "note.txt"), None);
        assert_eq!(detecter_format("", "vide.txt"), None);
    }

    #[test]
    fn reconnait_ce_que_nous_ecrivons() {
        let c = courbe_melangee();
        assert_eq!(
            detecter_format(&equalizer_apo::ecrire(&c), "a.txt"),
            Some(Format::EqualizerApo)
        );
        assert_eq!(
            detecter_format(&autoeq::ecrire(&c), "a ParametricEQ.txt"),
            Some(Format::AutoEq)
        );
        assert_eq!(
            detecter_format(&peace::ecrire(&c), "a.peace"),
            Some(Format::Peace)
        );
        assert_eq!(
            detecter_format(&peace::ecrire(&c), "sans-extension"),
            Some(Format::Peace)
        );
    }

    #[test]
    fn les_erreurs_ont_un_message_clair() {
        assert!(ErreurFormat::AucuneLigneUtilisable
            .to_string()
            .contains("aucune ligne utilisable"));
    }
}
