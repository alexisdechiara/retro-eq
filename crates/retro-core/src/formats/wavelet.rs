//! Wavelet : une ligne `GraphicEQ:` sur la grille de 127 fréquences d'AutoEQ.
//!
//! Wavelet (Android) lit la ligne `GraphicEQ: 20 -1.2; 21 -1.1; …` d'Equalizer APO.
//! À l'export, comme AutoEQ, on y plie le préampli, et on empêche le premier point de monter
//! au-dessus de 0 dB (pas de renfort sous 20 Hz). À la lecture ([`lire`]), la courbe de
//! points est ramenée aux 10 bandes de l'égaliseur : c'est une approximation, dont
//! l'écart est rendu avec l'import.

use super::{
    lire_nombre, ErreurFormat, Import, LigneIgnoree, RaisonIgnoree, GAIN_MAX_DB, PREAMPLI_MAX_DB,
};
use crate::egaliseur::{courbe_10_bandes, BANDES_HZ, Q_BANDES};
use retro_dsp::{frequences_log, Courbe, Filtre, TypeFiltre};

/// Plus basse fréquence où l'écart est mesuré (celle de la grille d'AutoEQ).
const FREQUENCE_MIN_HZ: f64 = 20.0;
/// Plus haute fréquence où l'écart est mesuré.
const FREQUENCE_MAX_HZ: f64 = 20_000.0;

/// Les 127 fréquences (Hz) de la grille GraphicEQ d'AutoEQ.
///
/// Source : `eqapo_graphic_eq` d'AutoEQ, pas de 1,0563 depuis 20 Hz, valeurs
/// tronquées à l'entier : la dernière est 19871 Hz.
pub const FREQUENCES_HZ: [u32; 127] = [
    20, 21, 22, 23, 24, 26, 27, 29, 30, 32, 34, 36, 38, 40, 43, 45, 48, 50, 53, 56, 59, 63, 66, 70,
    74, 78, 83, 87, 92, 97, 103, 109, 115, 121, 128, 136, 143, 151, 160, 169, 178, 188, 199, 210,
    222, 235, 248, 262, 277, 292, 309, 326, 345, 364, 385, 406, 429, 453, 479, 506, 534, 565, 596,
    630, 665, 703, 743, 784, 829, 875, 924, 977, 1032, 1090, 1151, 1216, 1284, 1357, 1433, 1514,
    1599, 1689, 1784, 1885, 1991, 2103, 2221, 2347, 2479, 2618, 2766, 2921, 3086, 3260, 3443, 3637,
    3842, 4058, 4287, 4528, 4783, 5052, 5337, 5637, 5955, 6290, 6644, 7018, 7414, 7831, 8272, 8738,
    9230, 9749, 10298, 10878, 11490, 12137, 12821, 13543, 14305, 15110, 15961, 16860, 17809, 18812,
    19871,
];

/// Écrit la ligne `GraphicEQ:` d'une courbe.
///
/// `reponse_db(f)` donne la réponse de la courbe à `f` Hz, préampli exclu ;
/// le préampli est ajouté à chaque point. Gains avec une décimale, une seule
/// ligne terminée par un retour à la ligne.
pub fn exporter(reponse_db: impl Fn(f64) -> f64, preampli_db: f64) -> String {
    let paires: Vec<String> = FREQUENCES_HZ
        .iter()
        .enumerate()
        .map(|(rang, &f)| {
            let mut gain = reponse_db(f64::from(f)) + preampli_db;
            if rang == 0 && gain > 0.0 {
                gain = 0.0;
            }
            format!("{f} {}", un_chiffre(gain))
        })
        .collect();
    format!("GraphicEQ: {}\n", paires.join("; "))
}

// ---------------------------------------------------------------------------
// Lecture : une courbe de points ramenée aux 10 bandes de l'égaliseur.
// ---------------------------------------------------------------------------

/// Gain d'un point `GraphicEQ:` le plus fort admis, en valeur absolue (dB). Un point contient
/// le préampli replié dans la courbe : la limite est celle du préampli, plus large que celle
/// d'un filtre.
pub const GAIN_POINT_MAX_DB: f64 = PREAMPLI_MAX_DB;

/// Nombre de fréquences, de 20 Hz à 20 kHz en échelle logarithmique, sur lesquelles la
/// courbe des 10 bandes est ajustée à celle du fichier (24 par octave).
const POINTS_D_AJUSTEMENT: usize = 240;

/// Itérations de Gauss-Newton : la réponse d'une cloche en dB n'est pas tout à fait
/// proportionnelle à son gain, quelques corrections suffisent à converger.
const ITERATIONS: usize = 8;

/// Une ligne `GraphicEQ:` lue : ses points, triés par fréquence.
struct CourbePoints {
    points: Vec<(f64, f64)>,
}

impl CourbePoints {
    /// Gain (dB) à `f_hz` : interpolation linéaire en dB sur une échelle de fréquence
    /// logarithmique, prolongée à plat avant le premier point et après le dernier.
    fn gain_db_a(&self, f_hz: f64) -> f64 {
        let p = &self.points;
        let apres = p.partition_point(|&(f, _)| f <= f_hz);
        match apres {
            0 => p[0].1,
            n if n == p.len() => p[n - 1].1,
            n => {
                let ((f0, g0), (f1, g1)) = (p[n - 1], p[n]);
                let t = (f_hz / f0).ln() / (f1 / f0).ln();
                g0 + t * (g1 - g0)
            }
        }
    }
}

/// Lit un point `20 -1.2`. `Err` : la raison de l'écarter.
fn lire_point(texte: &str) -> Result<(f64, f64), RaisonIgnoree> {
    let mots: Vec<&str> = texte.split_whitespace().collect();
    let [f, g] = mots[..] else {
        return Err(RaisonIgnoree::Illisible);
    };
    let f = lire_nombre(f)
        .filter(|f| *f > 0.0)
        .ok_or(RaisonIgnoree::Illisible)?;
    let g = lire_nombre(g).ok_or(RaisonIgnoree::Illisible)?;
    if g.abs() > GAIN_POINT_MAX_DB {
        return Err(RaisonIgnoree::HorsLimites);
    }
    Ok((f, g))
}

/// Lit un fichier qui ne contient qu'une courbe `GraphicEQ:` : un export pour Wavelet, ou
/// un fichier Equalizer APO sans autre réglage.
///
/// L'appli ne sait pas représenter une courbe de points : elle est **ramenée aux 10 bandes**
/// de l'égaliseur (fréquences [`BANDES_HZ`] et Q [`Q_BANDES`] fixes) plus un préampli, par
/// moindres carrés sur une grille logarithmique de 20 Hz à 20 kHz. L'import est donc
/// approché : la ligne est notée [`RaisonIgnoree::Approximation`] et
/// [`Import::ecart_max_db`] donne le plus grand écart entre les points du fichier (de 20 Hz
/// à 20 kHz) et la courbe importée.
///
/// Plusieurs lignes `GraphicEQ:` s'additionnent, comme dans Equalizer APO. Un point mal
/// formé (pas deux nombres, fréquence nulle ou négative, gain non fini ou au-delà de
/// ±[`GAIN_POINT_MAX_DB`]) est signalé et laissé de côté ; toute autre commande aussi.
pub fn lire(texte: &str) -> Result<Import, ErreurFormat> {
    let texte = texte.trim_start_matches('\u{feff}');
    let mut ignores = Vec::new();
    let mut courbes: Vec<CourbePoints> = Vec::new();
    let mut lignes_lues: Vec<LigneIgnoree> = Vec::new();
    for (index, ligne) in texte.lines().enumerate() {
        let (numero, ligne) = (index + 1, ligne.trim());
        let note = |raison: RaisonIgnoree, texte: &str| LigneIgnoree {
            numero,
            texte: texte.to_string(),
            raison,
        };
        if ligne.is_empty() {
            continue;
        }
        if ligne.starts_with('#') {
            ignores.push(note(RaisonIgnoree::Commentaire, ligne));
            continue;
        }
        let utile = ligne.split_once('#').map_or(ligne, |(avant, _)| avant);
        let corps = utile.split_once(':').and_then(|(mot, corps)| {
            mot.trim()
                .eq_ignore_ascii_case("graphiceq")
                .then_some(corps)
        });
        let Some(corps) = corps else {
            ignores.push(note(RaisonIgnoree::CommandeNonGeree, ligne));
            continue;
        };
        let mut points = Vec::new();
        for morceau in corps.split(';').map(str::trim).filter(|m| !m.is_empty()) {
            match lire_point(morceau) {
                Ok(point) => points.push(point),
                Err(raison) => ignores.push(note(raison, morceau)),
            }
        }
        if points.is_empty() {
            ignores.push(note(RaisonIgnoree::Illisible, ligne));
            continue;
        }
        points.sort_by(|a, b| a.0.total_cmp(&b.0));
        courbes.push(CourbePoints { points });
        lignes_lues.push(note(RaisonIgnoree::Approximation, ligne));
    }
    if courbes.is_empty() {
        return Err(ErreurFormat::AucuneLigneUtilisable);
    }
    let cible = |f: f64| courbes.iter().map(|c| c.gain_db_a(f)).sum::<f64>();

    let courbe = ajuster_10_bandes(&cible);

    // L'écart se mesure sur les points du fichier eux-mêmes, pas sur la grille d'ajustement.
    let ecart_max_db = courbes
        .iter()
        .flat_map(|c| c.points.iter().map(|&(f, _)| f))
        .filter(|f| (FREQUENCE_MIN_HZ..=FREQUENCE_MAX_HZ).contains(f))
        .map(|f| (cible(f) - courbe.reponse_totale_db(f)).abs())
        .fold(0.0, f64::max);

    ignores.extend(lignes_lues);
    ignores.sort_by_key(|l| l.numero);
    let mut import = Import::nouveau(courbe, ignores);
    import.ecart_max_db = Some(ecart_max_db);
    Ok(import)
}

/// La courbe à 10 bandes (plus un préampli) la plus proche de `cible` au sens des moindres
/// carrés, sur [`POINTS_D_AJUSTEMENT`] fréquences de 20 Hz à 20 kHz.
fn ajuster_10_bandes(cible: &dyn Fn(f64) -> f64) -> Courbe {
    const N: usize = BANDES_HZ.len() + 1; // 10 gains, puis le préampli
    let grille = frequences_log(POINTS_D_AJUSTEMENT);
    let voulu: Vec<f64> = grille.iter().map(|&f| cible(f)).collect();
    let bande = |i: usize, gain_db: f64, f: f64| {
        Filtre {
            type_filtre: TypeFiltre::Cloche,
            frequence_hz: BANDES_HZ[i],
            gain_db,
            q: Q_BANDES,
        }
        .gain_db_a(f)
    };
    let modele = |x: &[f64; N], f: f64| -> f64 {
        x[N - 1] + (0..N - 1).map(|i| bande(i, x[i], f)).sum::<f64>()
    };

    let mut x = [0.0; N];
    for _ in 0..ITERATIONS {
        // Jacobienne : dérivée de chaque point par rapport à chaque gain (différence
        // centrée), et 1 pour le préampli.
        let pas = 0.01;
        let mut jtj = [[0.0; N]; N];
        let mut jtr = [0.0; N];
        for (&f, &v) in grille.iter().zip(&voulu) {
            let mut ligne = [1.0; N];
            for (i, d) in ligne.iter_mut().take(N - 1).enumerate() {
                *d = (bande(i, x[i] + pas, f) - bande(i, x[i] - pas, f)) / (2.0 * pas);
            }
            let r = v - modele(&x, f);
            for a in 0..N {
                jtr[a] += ligne[a] * r;
                for b in 0..N {
                    jtj[a][b] += ligne[a] * ligne[b];
                }
            }
        }
        // Un léger amortissement garde le système inversible même pour une cible plate.
        for (a, ligne) in jtj.iter_mut().enumerate() {
            ligne[a] += 1e-9;
        }
        let Some(delta) = resoudre(jtj, jtr) else {
            break;
        };
        for (xi, di) in x.iter_mut().zip(delta) {
            *xi += di;
        }
        for g in x.iter_mut().take(N - 1) {
            *g = g.clamp(-GAIN_MAX_DB, GAIN_MAX_DB);
        }
        x[N - 1] = x[N - 1].clamp(-PREAMPLI_MAX_DB, PREAMPLI_MAX_DB);
    }

    // Au centième de dB : une bande qui ne bouge pas vraiment est à plat (pas de filtre).
    let centieme = |v: f64| {
        let r = (v * 100.0).round() / 100.0;
        if r == 0.0 {
            0.0
        } else {
            r
        }
    };
    let mut gains = [0.0; N - 1];
    for (g, xi) in gains.iter_mut().zip(x) {
        *g = centieme(xi);
    }
    let mut courbe = courbe_10_bandes(gains);
    courbe.preampli_db = centieme(x[N - 1]);
    courbe
}

/// Résout `a · x = b` par élimination de Gauss avec pivot partiel. `None` si le système est
/// singulier ou si un nombre n'est pas fini.
fn resoudre<const N: usize>(mut a: [[f64; N]; N], mut b: [f64; N]) -> Option<[f64; N]> {
    for col in 0..N {
        let pivot = (col..N).max_by(|&i, &j| a[i][col].abs().total_cmp(&a[j][col].abs()))?;
        // NaN compris : un pivot qui n'est pas nettement non nul arrête tout.
        if a[pivot][col].abs().partial_cmp(&1e-300) != Some(std::cmp::Ordering::Greater) {
            return None;
        }
        a.swap(col, pivot);
        b.swap(col, pivot);
        let (pivot_ligne, pivot_b) = (a[col], b[col]);
        for ligne in col + 1..N {
            let k = a[ligne][col] / pivot_ligne[col];
            for (case, p) in a[ligne].iter_mut().zip(pivot_ligne).skip(col) {
                *case -= k * p;
            }
            b[ligne] -= k * pivot_b;
        }
    }
    let mut x = [0.0; N];
    for ligne in (0..N).rev() {
        let somme: f64 = (ligne + 1..N).map(|c| a[ligne][c] * x[c]).sum();
        x[ligne] = (b[ligne] - somme) / a[ligne][ligne];
    }
    x.iter().all(|v| v.is_finite()).then_some(x)
}

/// Une décimale, sans « -0.0 ».
fn un_chiffre(gain: f64) -> String {
    let texte = format!("{gain:.1}");
    if texte == "-0.0" {
        "0.0".to_string()
    } else {
        texte
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_grille_a_127_points_croissants_de_20_a_19871() {
        assert_eq!(FREQUENCES_HZ.len(), 127);
        assert_eq!(FREQUENCES_HZ[0], 20);
        assert_eq!(FREQUENCES_HZ[126], 19871);
        assert!(FREQUENCES_HZ.windows(2).all(|p| p[0] < p[1]));
    }

    #[test]
    fn la_grille_debute_comme_celle_d_autoeq() {
        assert_eq!(
            FREQUENCES_HZ[..22],
            [
                20, 21, 22, 23, 24, 26, 27, 29, 30, 32, 34, 36, 38, 40, 43, 45, 48, 50, 53, 56, 59,
                63
            ]
        );
    }

    #[test]
    fn la_grille_suit_la_formule_d_autoeq() {
        // AutoEQ : 20 * 1.0563^k, tronqué à l'entier, doublons retirés.
        let mut attendu: Vec<u32> = (0..)
            .map(|k| 20.0 * 1.0563_f64.powi(k))
            .take_while(|f| *f < 20000.0)
            .map(|f| f as u32)
            .collect();
        attendu.dedup();
        assert_eq!(FREQUENCES_HZ.to_vec(), attendu);
    }

    #[test]
    fn une_seule_ligne_avec_127_paires() {
        let texte = exporter(|_| 0.0, 0.0);
        assert_eq!(texte.lines().count(), 1);
        let ligne = texte.lines().next().expect("une ligne");
        let corps = ligne.strip_prefix("GraphicEQ: ").expect("préfixe");
        assert_eq!(corps.split("; ").count(), 127);
        assert!(corps.starts_with("20 0.0; 21 0.0; 22 0.0;"));
        assert!(corps.ends_with("19871 0.0"));
    }

    #[test]
    fn le_preampli_est_ajoute_et_une_decimale() {
        let texte = exporter(|f| if f < 100.0 { -1.0 } else { -2.04 }, -3.0);
        assert!(texte.starts_with("GraphicEQ: 20 -4.0; 21 -4.0;"));
        assert!(texte.contains("; 1032 -5.0;"));
    }

    #[test]
    fn pas_de_moins_zero() {
        let texte = exporter(|_| -0.01, 0.0);
        assert!(!texte.contains("-0.0"));
    }

    #[test]
    fn le_premier_point_ne_monte_pas_au_dessus_de_zero_comme_autoeq() {
        let texte = exporter(|f| if f < 21.0 { 5.0 } else { 1.0 }, 0.0);
        assert!(texte.starts_with("GraphicEQ: 20 0.0; 21 1.0;"));
    }

    // ----- Lecture -----

    use crate::egaliseur::{courbe_10_bandes, FadersEtSupplementaires};
    use crate::formats::{ErreurFormat, RaisonIgnoree};
    use retro_dsp::Courbe;

    fn exporter_courbe(c: &Courbe) -> String {
        exporter(|f| c.reponse_db(f), c.preampli_db)
    }

    /// Le plus grand écart (dB) entre les points d'une ligne `GraphicEQ:` et une courbe.
    fn ecart_reel(texte: &str, c: &Courbe) -> f64 {
        let corps = texte.trim().strip_prefix("GraphicEQ:").unwrap();
        corps
            .split(';')
            .map(|p| {
                let mut m = p.split_whitespace();
                let f: f64 = m.next().unwrap().parse().unwrap();
                let g: f64 = m.next().unwrap().parse().unwrap();
                (g - c.reponse_totale_db(f)).abs()
            })
            .fold(0.0, f64::max)
    }

    #[test]
    fn une_courbe_a_10_bandes_fait_l_aller_retour_a_peu_pres() {
        for gains in [
            [3.0, -2.0, 1.5, 0.0, -1.0, 2.0, -3.0, 4.0, -2.0, 1.0],
            [6.0, 4.0, 2.0, 0.0, 0.0, 0.0, 0.0, 2.0, 4.0, 6.0],
            [-6.0, 0.0, 6.0, -6.0, 0.0, 6.0, -6.0, 0.0, 6.0, -6.0],
        ] {
            let source = courbe_10_bandes(gains).avec_preampli_auto();
            let texte = exporter_courbe(&source);
            let i = lire(&texte).expect("lecture");
            let faders = FadersEtSupplementaires::depuis_courbe(&i.courbe);
            assert!(faders.supplementaires.is_empty(), "{:?}", i.courbe);
            for (lu, attendu) in faders.gains.iter().zip(gains) {
                assert!(
                    (lu - attendu).abs() < 0.25,
                    "{:?} au lieu de {gains:?}",
                    faders.gains
                );
            }
            let ecart = i.ecart_max_db.expect("import approché");
            assert!(ecart < 0.15, "écart {ecart} pour {gains:?}");
            assert!((ecart - ecart_reel(&texte, &i.courbe)).abs() < 1e-9);
            assert!(!i.partiel);
            assert_eq!(i.ignores.len(), 1);
            assert_eq!(i.ignores[0].raison, RaisonIgnoree::Approximation);
        }
    }

    #[test]
    fn une_courbe_plate_donne_une_courbe_plate() {
        let i = lire(&exporter(|_| 0.0, 0.0)).expect("lecture");
        assert!(i.courbe.filtres.is_empty(), "{:?}", i.courbe);
        assert_eq!(i.courbe.preampli_db, 0.0);
        assert!(i.ecart_max_db.expect("approché") < 1e-9);
        // Plate mais plus basse : c'est un préampli.
        let i = lire(&exporter(|_| 0.0, -3.0)).expect("lecture");
        assert!(i.courbe.filtres.is_empty(), "{:?}", i.courbe);
        assert!((i.courbe.preampli_db + 3.0).abs() < 1e-6);
    }

    #[test]
    fn l_ecart_dit_ce_que_les_10_bandes_ne_rendent_pas() {
        // Une marche brutale à 1 kHz : impossible à rendre exactement avec 10 cloches.
        let texte = exporter(|f| if f < 1000.0 { -6.0 } else { 0.0 }, 0.0);
        let i = lire(&texte).expect("lecture");
        let ecart = i.ecart_max_db.expect("approché");
        assert!(ecart > 0.5, "{ecart}");
        assert!((ecart - ecart_reel(&texte, &i.courbe)).abs() < 1e-9);
    }

    #[test]
    fn un_fichier_equalizer_apo_qui_n_a_qu_un_graphiceq_se_lit() {
        let texte = "# exporté\r\nGraphicEQ: 20 -2; 1000 -2; 2000 1; 20000 1\r\n";
        assert_eq!(
            crate::formats::detecter_format(texte, "config.txt"),
            Some(crate::formats::Format::Wavelet)
        );
        let i = lire(texte).expect("lecture");
        assert!(i.ecart_max_db.is_some());
        assert_eq!(i.ignores[0].raison, RaisonIgnoree::Commentaire);
        assert!(!i.partiel);
    }

    #[test]
    fn deux_lignes_graphiceq_s_additionnent_comme_dans_equalizer_apo() {
        let i = lire("GraphicEQ: 20 -2; 20000 -2\nGraphicEQ: 20 -1; 20000 -1\n").expect("lecture");
        assert!(i.courbe.filtres.is_empty());
        assert!((i.courbe.preampli_db + 3.0).abs() < 1e-6);
    }

    #[test]
    fn les_points_mal_formes_sont_signales_sans_planter() {
        let texte = "GraphicEQ: 20 0; abc 1; 100 x; -5 2; 0 1; 1000 nan; 2000 1e300; 4000; 8000 1 2; 16000 0\n";
        let i = lire(texte).expect("lecture");
        let signales: Vec<_> = i
            .ignores
            .iter()
            .filter(|l| l.raison.change_le_son())
            .map(|l| (l.texte.as_str(), l.raison.clone()))
            .collect();
        assert_eq!(
            signales,
            [
                ("abc 1", RaisonIgnoree::Illisible),
                ("100 x", RaisonIgnoree::Illisible),
                ("-5 2", RaisonIgnoree::Illisible),
                ("0 1", RaisonIgnoree::Illisible),
                ("1000 nan", RaisonIgnoree::Illisible),
                ("2000 1e300", RaisonIgnoree::HorsLimites),
                ("4000", RaisonIgnoree::Illisible),
                ("8000 1 2", RaisonIgnoree::Illisible),
            ]
        );
        assert!(i.partiel);
        assert!(i.courbe.filtres.is_empty());
        assert!(i.ignores.iter().all(|l| l.numero == 1));
    }

    #[test]
    fn sans_point_utilisable_c_est_une_erreur_pas_un_plantage() {
        for texte in [
            "",
            "GraphicEQ:",
            "GraphicEQ: ;;;",
            "GraphicEQ: x y; z",
            "# rien\n",
            "Preamp: -3 dB\n",
        ] {
            assert_eq!(
                lire(texte),
                Err(ErreurFormat::AucuneLigneUtilisable),
                "{texte:?}"
            );
        }
        // Des octets quelconques ne font jamais planter la lecture.
        let mut graine = 12345u64;
        for _ in 0..500 {
            let texte: String = (0..40)
                .map(|_| {
                    graine = graine.wrapping_mul(6364136223846793005).wrapping_add(1);
                    let alphabet = b"GraphicEQ: 0123456789.-e;,x \n";
                    alphabet[(graine >> 33) as usize % alphabet.len()] as char
                })
                .collect();
            let _ = lire(&texte);
        }
    }
}
