//! L'autotest : ce que l'appli vérifie elle-même, et ce qu'elle demande de vérifier.
//!
//! Règle : une coche verte seulement pour ce que l'appli a réellement constaté. Les
//! réglages qu'elle ne peut pas lire (INZONE Hub, son spatial, améliorations audio de
//! Windows) sont montrés en « attention », avec l'endroit où regarder.

use crate::textes::{self, TexteControle};
use retro_audio::InfoSortie;
use retro_core::Langue;
use retro_engine::SortieAudio;
use std::path::{Path, PathBuf};

/// L'état d'un contrôle, sans type Slint (le pont le traduit).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Etat {
    Ok,
    Attention,
    Erreur,
}

/// Ce que fait le bouton d'action d'un contrôle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Cible {
    Lien(&'static str),
    Dossier(PathBuf),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Controle {
    pub titre: String,
    pub detail: String,
    pub etat: Etat,
    pub action: String,
    pub cible: Option<Cible>,
}

/// Ce que l'autotest a besoin de savoir, rassemblé par le pont.
pub struct Contexte<'a> {
    /// Nom de la sortie audio par défaut, si le système en voit une.
    pub sortie: Option<&'a str>,
    /// 0 absent, 1 inactif, 2 actif.
    pub etat_moteur: i32,
    pub moteur_autorise: bool,
    /// Le dossier `config` d'Equalizer APO, s'il est connu.
    pub dossier_moteur: Option<&'a Path>,
    pub conflits: &'a [String],
    pub raccourcis_disponibles: bool,
    /// La langue des phrases.
    pub langue: Langue,
}

pub const LIEN_EQUALIZER_APO: &str = "https://sourceforge.net/projects/equalizerapo/";
/// Ouvre la page « Son » des Paramètres de Windows.
pub const LIEN_REGLAGES_SON: &str = "ms-settings:sound";

/// Peut-on écrire dans ce dossier ? On le constate en y créant puis en y supprimant un
/// petit fichier : c'est le seul test fiable des droits sous Windows.
pub fn dossier_inscriptible(dossier: &Path) -> bool {
    let essai = dossier.join(format!(".retro-eq-essai-{}.tmp", std::process::id()));
    let ecrit = std::fs::write(&essai, b"").is_ok();
    let _ = std::fs::remove_file(&essai);
    ecrit
}

fn ligne(texte: TexteControle, etat: Etat, cible: Option<Cible>) -> Controle {
    Controle {
        titre: texte.titre,
        detail: texte.detail,
        etat,
        action: if cible.is_some() {
            texte.action
        } else {
            String::new()
        },
        cible,
    }
}

/// Les contrôles, dans l'ordre d'affichage, et la phrase de bilan.
pub fn controles(contexte: &Contexte) -> (Vec<Controle>, String) {
    let mut liste = Vec::new();

    liste.push(ligne(
        textes::controle_sortie(contexte.sortie, contexte.langue),
        if contexte.sortie.is_some() {
            Etat::Ok
        } else {
            Etat::Erreur
        },
        None,
    ));

    // Avant l'accord de l'utilisateur, on n'écrit rien chez Equalizer APO, pas même un
    // fichier d'essai : le contrôle des droits attend.
    let inscriptible = match (contexte.dossier_moteur, contexte.moteur_autorise) {
        (Some(dossier), true) if contexte.etat_moteur != 0 => Some(dossier_inscriptible(dossier)),
        _ => None,
    };
    let (etat, cible) = match (contexte.etat_moteur, contexte.moteur_autorise, inscriptible) {
        (0, _, _) => (Etat::Erreur, Some(Cible::Lien(LIEN_EQUALIZER_APO))),
        (_, _, Some(false)) => (
            Etat::Erreur,
            contexte
                .dossier_moteur
                .map(|d| Cible::Dossier(d.to_path_buf())),
        ),
        (2, true, _) => (Etat::Ok, None),
        _ => (Etat::Attention, None),
    };
    liste.push(ligne(
        textes::controle_moteur(
            contexte.etat_moteur,
            contexte.moteur_autorise,
            inscriptible,
            contexte.langue,
        ),
        etat,
        cible,
    ));

    for (i, texte) in textes::controles_a_verifier_soi_meme(contexte.langue)
        .into_iter()
        .enumerate()
    {
        let cible = (i > 0).then_some(Cible::Lien(LIEN_REGLAGES_SON));
        liste.push(ligne(texte, Etat::Attention, cible));
    }

    let raccourcis_bons = contexte.raccourcis_disponibles && contexte.conflits.is_empty();
    liste.push(ligne(
        textes::controle_raccourcis(
            contexte.conflits,
            contexte.raccourcis_disponibles,
            contexte.langue,
        ),
        if raccourcis_bons {
            Etat::Ok
        } else {
            Etat::Erreur
        },
        None,
    ));

    let compte = |etat| liste.iter().filter(|c| c.etat == etat).count();
    let bilan = textes::bilan_autotest(
        compte(Etat::Ok),
        compte(Etat::Attention),
        compte(Etat::Erreur),
        contexte.langue,
    );
    (liste, bilan)
}

/// La sortie par défaut est-elle une sortie où le moteur n'est pas installé ? Rend son nom
/// dans ce cas (dongle branché sur un autre port, par exemple).
///
/// Si le moteur ne sait pas dire où il est installé (liste vide), on ne dit rien plutôt
/// que d'alerter à tort.
pub fn nouvelle_sortie(defaut: Option<&InfoSortie>, moteur: &[SortieAudio]) -> Option<String> {
    let defaut = defaut?;
    if moteur.is_empty() {
        return None;
    }
    // Windows et Equalizer APO n'écrivent pas l'identifiant tout à fait pareil (préfixe,
    // casse) : on compare sans la casse, et l'un peut contenir l'autre.
    let id = defaut.id.to_lowercase();
    let connue = moteur.iter().filter(|s| s.moteur_installe).any(|s| {
        let autre = s.id.to_lowercase();
        !autre.is_empty() && (id.contains(&autre) || autre.contains(&id))
    });
    (!connue).then(|| defaut.nom.clone())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    fn contexte<'a>(dossier: Option<&'a Path>, etat: i32, autorise: bool) -> Contexte<'a> {
        Contexte {
            sortie: Some("Casque"),
            etat_moteur: etat,
            moteur_autorise: autorise,
            dossier_moteur: dossier,
            conflits: &[],
            raccourcis_disponibles: true,
            langue: Langue::Fr,
        }
    }

    #[test]
    fn ce_que_l_appli_ne_peut_pas_lire_n_est_jamais_vert() {
        let dossier = tempfile::tempdir().unwrap();
        let (liste, bilan) = controles(&contexte(Some(dossier.path()), 2, true));
        assert_eq!(liste.len(), 6);
        assert_eq!(liste[0].etat, Etat::Ok);
        assert_eq!(liste[1].etat, Etat::Ok);
        for controle in &liste[2..5] {
            assert_eq!(controle.etat, Etat::Attention, "{}", controle.titre);
        }
        assert_eq!(liste[5].etat, Etat::Ok);
        assert_eq!(bilan, "3 bons · 3 à vérifier toi-même");
    }

    #[test]
    fn avant_l_accord_rien_n_est_ecrit_dans_le_dossier_du_moteur() {
        // Un dossier qui n'existe pas : si l'essai d'écriture avait lieu, il échouerait et
        // le contrôle passerait en erreur.
        let absent = Path::new("/dossier/qui/n/existe/pas");
        let (liste, _) = controles(&contexte(Some(absent), 1, false));
        assert_eq!(liste[1].etat, Etat::Attention);
        assert!(liste[1].titre.contains("pas encore activé"));
    }

    #[test]
    fn un_dossier_protege_est_une_erreur_avec_un_bouton_pour_l_ouvrir() {
        let absent = Path::new("/dossier/qui/n/existe/pas");
        let (liste, _) = controles(&contexte(Some(absent), 1, true));
        assert_eq!(liste[1].etat, Etat::Erreur);
        assert_eq!(liste[1].cible, Some(Cible::Dossier(absent.to_path_buf())));
    }

    #[test]
    fn l_essai_d_ecriture_ne_laisse_rien_derriere_lui() {
        let dossier = tempfile::tempdir().unwrap();
        assert!(dossier_inscriptible(dossier.path()));
        assert_eq!(std::fs::read_dir(dossier.path()).unwrap().count(), 0);
    }

    #[test]
    fn moteur_absent_et_raccourcis_pris_sont_des_erreurs() {
        let conflits = vec!["F10".to_string()];
        let (liste, bilan) = controles(&Contexte {
            sortie: None,
            conflits: &conflits,
            ..contexte(None, 0, false)
        });
        assert_eq!(liste[0].etat, Etat::Erreur);
        assert_eq!(liste[1].etat, Etat::Erreur);
        assert_eq!(liste[1].cible, Some(Cible::Lien(LIEN_EQUALIZER_APO)));
        assert_eq!(liste[5].etat, Etat::Erreur);
        assert!(liste[5].detail.contains("F10"));
        assert!(bilan.contains("3 à régler"));
    }

    #[test]
    fn une_sortie_inconnue_du_moteur_est_signalee() {
        let sortie = |id: &str| InfoSortie {
            id: id.to_string(),
            nom: "Dongle".to_string(),
            par_defaut: true,
        };
        let moteur = vec![SortieAudio {
            id: "{ABC-1}".to_string(),
            nom: "x".to_string(),
            moteur_installe: true,
        }];
        assert_eq!(
            nouvelle_sortie(Some(&sortie("{0.0.0}.{abc-1}")), &moteur),
            None
        );
        assert_eq!(
            nouvelle_sortie(Some(&sortie("{0.0.0}.{def-2}")), &moteur),
            Some("Dongle".to_string())
        );
        assert_eq!(nouvelle_sortie(Some(&sortie("{def-2}")), &[]), None);
        assert_eq!(nouvelle_sortie(None, &moteur), None);
    }

    #[test]
    fn l_autotest_parle_la_langue_demandee() {
        let dossier = tempfile::tempdir().unwrap();
        let en = Contexte {
            langue: Langue::En,
            ..contexte(Some(dossier.path()), 2, true)
        };
        let (liste, bilan) = controles(&en);
        assert_eq!(liste[0].titre, "Audio output detected");
        assert_eq!(bilan, "3 good · 3 to check yourself");
    }
}
