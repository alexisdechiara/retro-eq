//! Les modes « sans fenêtre » : l'appli fait une seule chose, écrit une ligne et rend un code
//! de sortie. Aujourd'hui, un seul : `--rendre-son-origine`, appelé par le désinstalleur
//! (`installateur/retro-eq.iss`) avant de supprimer les fichiers.
//!
//! Ce mode fait exactement ce que le bouton de Réglages fait, par le même chemin
//! (`Controleur::rendre_son_origine`, donc `Moteur::retirer`), pour qu'il n'y ait qu'une
//! seule façon de rendre le son d'origine.

use crate::controleur::{Controleur, ErreurControleur, Options};
use crate::textes;
use retro_core::Langue;
use retro_engine::Moteur;
use std::path::Path;

/// L'option de la ligne de commande qui déclenche ce mode.
pub const RENDRE_SON_ORIGINE: &str = "--rendre-son-origine";

/// Ce qu'il faut dire à l'utilisateur, et si ça a marché.
#[derive(Debug, PartialEq, Eq)]
pub struct Bilan {
    pub reussi: bool,
    pub message: String,
}

impl Bilan {
    /// Le code de sortie du processus : 0 si réussi, 1 sinon. Le désinstalleur le lit.
    pub fn code_sortie(&self) -> i32 {
        if self.reussi {
            0
        } else {
            1
        }
    }
}

/// Rend le son d'origine : le moteur est retiré des sorties audio.
///
/// - Si l'appli a des données (`reglages.json`), on passe par le contrôleur, comme le bouton
///   de Réglages : il retire aussi l'accord d'écriture, pour une éventuelle réinstallation.
/// - Sinon (jamais lancée, ou données effacées), on ne crée rien dans le dossier de données :
///   on demande seulement au moteur de se retirer.
/// - Si le contrôleur ne peut pas s'ouvrir (dossier de données abîmé ou protégé), on se
///   rabat sur ce second chemin : le désinstalleur doit pouvoir rendre le son d'origine
///   quoi qu'il arrive aux données.
///
/// `fabriquer_moteur` donne un moteur neuf : le contrôleur garde le premier, même quand son
/// ouverture échoue.
pub fn rendre_son_origine(
    donnees: &Path,
    mut fabriquer_moteur: impl FnMut() -> Box<dyn Moteur>,
    options: Options,
    maintenant_ms: u64,
) -> Bilan {
    let langue = options.langue_systeme;
    let moteur_absent = options.moteur_absent;
    if !donnees.join("reglages.json").is_file() {
        return direct(fabriquer_moteur(), moteur_absent, langue);
    }
    let mut controleur =
        match Controleur::ouvrir(donnees, fabriquer_moteur(), options, maintenant_ms) {
            Ok(c) => c,
            // L'accord d'écriture reste noté dans les réglages : sans conséquence, l'appli
            // est en train d'être désinstallée.
            Err(_) => return direct(fabriquer_moteur(), moteur_absent, langue),
        };
    // Ce que l'ouverture a pu annoncer (test repris, profil ignoré...) n'est pas le résultat.
    let _ = controleur.prendre_annonce();
    controleur.rendre_son_origine(maintenant_ms);
    // `tenter` laisse toujours une annonce : confirmation ou erreur.
    match controleur.prendre_annonce() {
        Some(a) => Bilan {
            reussi: !a.erreur,
            message: a.texte,
        },
        None => Bilan {
            reussi: true,
            message: textes::annonce_son_origine(None, langue),
        },
    }
}

fn direct(mut moteur: Box<dyn Moteur>, moteur_absent: bool, langue: Langue) -> Bilan {
    if !moteur_absent {
        if let Err(e) = moteur.retirer() {
            return echec(&ErreurControleur::from(e), langue);
        }
    }
    Bilan {
        reussi: true,
        message: textes::annonce_son_origine(None, langue),
    }
}

fn echec(erreur: &ErreurControleur, langue: Langue) -> Bilan {
    Bilan {
        reussi: false,
        message: textes::erreur(erreur, langue),
    }
}

/// Le point d'entrée appelé par `main` : monte le moteur et le dossier de données réels,
/// écrit le bilan sur la console et rend le code de sortie.
pub fn lancer_rendre_son_origine() -> i32 {
    rattacher_console();
    let donnees = crate::dossier_donnees();
    let (moteur, dossier_moteur) = crate::moteur();
    // Le premier moteur sert d'abord ; un second est monté si le contrôleur a gardé le premier.
    let mut premier = Some(moteur);
    let fabriquer_moteur = move || premier.take().unwrap_or_else(|| crate::moteur().0);
    let options = Options {
        dossier_export: crate::dossier_export(&donnees),
        moteur_absent: dossier_moteur.is_none(),
        decalage_minutes: 0,
        graine: None,
        langue_systeme: textes::langue_depuis_locale(sys_locale::get_locale().as_deref()),
        langue_imposee: None,
    };
    let bilan = rendre_son_origine(&donnees, fabriquer_moteur, options, crate::maintenant_ms());
    if bilan.reussi {
        println!("{}", bilan.message);
    } else {
        eprintln!("{}", bilan.message);
    }
    bilan.code_sortie()
}

/// L'appli est compilée « sans console » (`windows_subsystem = "windows"`) : lancée depuis
/// un terminal, elle n'y écrirait rien. On s'accroche à la console du processus parent
/// s'il en a une ; sinon (lancée par le désinstalleur) l'appel échoue et on ignore.
#[cfg(windows)]
fn rattacher_console() {
    use windows::Win32::System::Console::{AttachConsole, ATTACH_PARENT_PROCESS};
    // SAFETY: appel sans pointeur ; l'échec est sans conséquence.
    let _ = unsafe { AttachConsole(ATTACH_PARENT_PROCESS) };
}

#[cfg(not(windows))]
fn rattacher_console() {}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    use retro_engine::{Appel, MoteurSimule};
    use tempfile::TempDir;

    const T0: u64 = 1_764_016_800_000;

    fn options(dossier: &Path, moteur_absent: bool) -> Options {
        Options {
            dossier_export: dossier.join("exports"),
            moteur_absent,
            decalage_minutes: 0,
            graine: Some(7),
            langue_systeme: Langue::Fr,
            langue_imposee: None,
        }
    }

    /// Un dossier de données où l'appli a déjà tourné et reçu l'accord d'écrire.
    fn donnees_autorisees(racine: &Path, moteur: &MoteurSimule) -> std::path::PathBuf {
        let donnees = racine.join("donnees");
        let mut c = Controleur::ouvrir(
            &donnees,
            Box::new(moteur.clone()),
            options(racine, false),
            T0,
        )
        .unwrap();
        c.autoriser_moteur();
        donnees
    }

    #[test]
    fn retire_le_moteur_par_le_controleur_et_reussit() {
        let t = TempDir::new().unwrap();
        let moteur = MoteurSimule::nouveau();
        let donnees = donnees_autorisees(t.path(), &moteur);

        let bilan = rendre_son_origine(
            &donnees,
            || Box::new(moteur.clone()),
            options(t.path(), false),
            T0 + 1,
        );

        assert!(bilan.reussi, "{bilan:?}");
        assert_eq!(bilan.code_sortie(), 0);
        assert!(bilan.message.contains("Son d’origine rendu"));
        assert_eq!(moteur.appels().last(), Some(&Appel::Retirer));
        // L'accord d'écriture est retiré, comme avec le bouton.
        let c = Controleur::ouvrir(
            &donnees,
            Box::new(MoteurSimule::nouveau()),
            options(t.path(), false),
            T0 + 2,
        )
        .unwrap();
        assert!(!c.moteur_autorise());
    }

    /// Un moteur qui accepte tout sauf de se retirer (droits refusés sur le dossier d'APO).
    struct RetirerRefuse;

    impl Moteur for RetirerRefuse {
        fn appliquer(&mut self, _: &retro_dsp::Courbe) -> Result<(), retro_engine::ErreurMoteur> {
            Ok(())
        }
        fn couper(&mut self) -> Result<(), retro_engine::ErreurMoteur> {
            Ok(())
        }
        fn etat(&self) -> retro_engine::EtatMoteur {
            retro_engine::EtatMoteur::Actif
        }
        fn sorties(&self) -> Result<Vec<retro_engine::SortieAudio>, retro_engine::ErreurMoteur> {
            Ok(Vec::new())
        }
        fn retirer(&mut self) -> Result<(), retro_engine::ErreurMoteur> {
            Err(retro_engine::ErreurMoteur::Simule("droits refusés".into()))
        }
    }

    #[test]
    fn un_echec_du_moteur_donne_un_code_non_nul_et_un_message() {
        let t = TempDir::new().unwrap();
        let donnees = donnees_autorisees(t.path(), &MoteurSimule::nouveau());

        let bilan = rendre_son_origine(
            &donnees,
            || Box::new(RetirerRefuse),
            options(t.path(), false),
            T0 + 1,
        );

        assert!(!bilan.reussi);
        assert_eq!(bilan.code_sortie(), 1);
        assert!(bilan.message.contains("droits refusés"), "{bilan:?}");
    }

    #[test]
    fn un_dossier_de_donnees_inutilisable_n_empeche_pas_de_retirer_le_moteur() {
        let t = TempDir::new().unwrap();
        let moteur = MoteurSimule::nouveau();
        let donnees = donnees_autorisees(t.path(), &moteur);
        // Le dossier des profils est remplacé par un fichier : le contrôleur ne s'ouvre plus.
        let profils = donnees.join("profils");
        std::fs::remove_dir_all(&profils).unwrap();
        std::fs::write(&profils, "").unwrap();
        assert!(Controleur::ouvrir(
            &donnees,
            Box::new(MoteurSimule::nouveau()),
            options(t.path(), false),
            T0
        )
        .is_err());

        let bilan = rendre_son_origine(
            &donnees,
            || Box::new(moteur.clone()),
            options(t.path(), false),
            T0 + 1,
        );

        assert!(bilan.reussi, "{bilan:?}");
        assert_eq!(bilan.code_sortie(), 0);
        assert_eq!(moteur.appels().last(), Some(&Appel::Retirer));
    }

    #[test]
    fn sans_donnees_le_moteur_est_retire_sans_rien_creer() {
        let t = TempDir::new().unwrap();
        let donnees = t.path().join("jamais-cree");
        let moteur = MoteurSimule::nouveau();

        let bilan = rendre_son_origine(
            &donnees,
            || Box::new(moteur.clone()),
            options(t.path(), false),
            T0,
        );

        assert!(bilan.reussi);
        assert_eq!(moteur.appels(), vec![Appel::Retirer]);
        assert!(
            !donnees.exists(),
            "le dossier de données ne doit pas être créé"
        );
    }

    #[test]
    fn sans_moteur_installe_il_n_y_a_rien_a_rendre() {
        let t = TempDir::new().unwrap();
        let moteur = MoteurSimule::nouveau();

        let bilan = rendre_son_origine(
            &t.path().join("vide"),
            || Box::new(moteur.clone()),
            options(t.path(), true),
            T0,
        );

        assert!(bilan.reussi);
        assert!(moteur.appels().is_empty());
    }
}
