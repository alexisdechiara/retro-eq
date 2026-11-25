//! Retro EQ : le point d'entrée.
//!
//! Sans option, l'appli démarre pour de vrai : dossier de données, moteur audio, contrôleur,
//! interface. Les options servent aux captures d'écran et aux essais :
//!
//! - `--demo` : données d'exemple de la maquette, sans moteur ni dossier ;
//! - `--ecran <nom>` : ouvre directement cet écran ;
//! - `--capture <fichier.png>` (avec `--hauteur <px>`) : enregistre l'écran et quitte ;
//! - `--rail` : navigation dans le rail latéral.
//!
//! Variables d'environnement : `RETRO_EQ_DONNEES` (dossier de données),
//! `RETRO_EQ_CONFIG_APO` (dossier `config` d'Equalizer APO, au lieu de le chercher),
//! `RETRO_EQ_SCENARIO` (suite d'actions d'essai, voir `Pont::jouer_scenario`) et, hors
//! Windows, `RETRO_EQ_IMPORT` (fichier que « Importer » lit, faute de boîte de dialogue).

// Pas de console noire derrière la fenêtre sous Windows (sauf en version de mise au point).
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

// Le code Rust que Slint génère depuis les fichiers .slint. Nos règles de lint ne s'y appliquent pas.
#[allow(clippy::all, clippy::unwrap_used)]
mod ui {
    slint::include_modules!();
}
pub use ui::*;

mod autotest;
mod barre;
mod controleur;
mod courbes;
mod demo;
mod fenetre;
mod mini;
mod pont;
mod raccourcis;
mod textes;

use controleur::{Controleur, Options};
use retro_engine::{EqualizerApo, Moteur, MoteurSimule};
use slint::ComponentHandle;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// Nom d'écran passé à `--ecran`, pour ouvrir l'appli directement dessus.
fn ecran_depuis(nom: &str) -> Option<Ecran> {
    Some(match nom {
        "demarrage" => Ecran::Demarrage,
        "accueil" => Ecran::Accueil,
        "tester" => Ecran::Tester,
        "nouveau-test" => Ecran::NouveauTest,
        "duels" => Ecran::Duels,
        "resultat" => Ecran::Resultat,
        "profils" => Ecran::Profils,
        "egaliseur" => Ecran::Egaliseur,
        "visualiseur" => Ecran::Visualiseur,
        "reglages" => Ecran::Reglages,
        "diagnostic" => Ecran::Diagnostic,
        "a-propos" => Ecran::APropos,
        _ => return None,
    })
}

fn argument(arguments: &[String], nom: &str) -> Option<String> {
    let position = arguments.iter().position(|a| a == nom)?;
    arguments.get(position + 1).cloned()
}

/// Un dossier donné par une variable d'environnement, si elle existe et n'est pas vide.
fn dossier_env(nom: &str) -> Option<PathBuf> {
    std::env::var_os(nom)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

/// Où l'appli range ses données : `%APPDATA%\Retro EQ` sous Windows (ADR-0007), le dossier
/// de données de l'utilisateur ailleurs.
fn dossier_donnees() -> PathBuf {
    if let Some(dossier) = dossier_env("RETRO_EQ_DONNEES") {
        return dossier;
    }
    if cfg!(windows) {
        if let Some(appdata) = dossier_env("APPDATA") {
            return appdata.join("Retro EQ");
        }
    }
    if let Some(xdg) = dossier_env("XDG_DATA_HOME") {
        return xdg.join("retro-eq");
    }
    match dossier_env("HOME").or_else(|| dossier_env("USERPROFILE")) {
        Some(maison) => maison.join(".local/share/retro-eq"),
        None => PathBuf::from("retro-eq-donnees"),
    }
}

/// Où vont les exports : `Documents\Retro EQ` sous Windows, le dossier de données ailleurs.
// A VERIFIER SUR LE PC : si « Documents » est redirigé (OneDrive), ce chemin n'est pas celui
// que l'Explorateur montre ; l'annonce affiche le chemin complet pour qu'on le retrouve.
fn dossier_export(donnees: &std::path::Path) -> PathBuf {
    match dossier_env("USERPROFILE") {
        Some(profil) if cfg!(windows) => profil.join("Documents").join("Retro EQ"),
        _ => donnees.join("exports"),
    }
}

/// Le moteur audio : Equalizer APO s'il est installé (ou si `RETRO_EQ_CONFIG_APO` donne son
/// dossier), sinon un moteur simulé qui ne fait rien, et l'appli le dit.
fn moteur() -> (Box<dyn Moteur>, Option<PathBuf>) {
    let dossier = dossier_env("RETRO_EQ_CONFIG_APO")
        .filter(|d| d.is_dir())
        .or_else(EqualizerApo::detecter);
    match dossier {
        Some(dossier) => (Box::new(EqualizerApo::nouveau(dossier.clone())), Some(dossier)),
        None => (Box::new(MoteurSimule::nouveau()), None),
    }
}

fn maintenant_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arguments: Vec<String> = std::env::args().collect();
    let capture = argument(&arguments, "--capture");
    let fenetre = FenetrePrincipale::new()?;
    let ecran = argument(&arguments, "--ecran").and_then(|nom| ecran_depuis(&nom));

    // `_vivant` garde le pont et son minuteur en vie jusqu'à la fin de `main`.
    let mut _vivant = None;
    if arguments.iter().any(|a| a == "--demo") {
        demo::remplir(&fenetre);
    } else {
        let donnees = dossier_donnees();
        let (moteur, dossier_moteur) = moteur();
        let options = Options {
            dossier_export: dossier_export(&donnees),
            moteur_absent: dossier_moteur.is_none(),
            // L'heure locale ne sert qu'à dater les versions de façon lisible.
            decalage_minutes: chrono::Local::now().offset().local_minus_utc() / 60,
            graine: None,
        };
        let controleur = Controleur::ouvrir(&donnees, moteur, options, maintenant_ms())?;
        let premier_lancement = !controleur.moteur_autorise();

        // Pour une capture d'écran : ni raccourcis globaux ni mini-fenêtre.
        let (raccourcis, mini) = if capture.is_some() {
            (raccourcis::Raccourcis::inertes(), None)
        } else {
            let mut mini = mini::Mini::nouvelle().ok();
            if let Some(mini) = &mut mini {
                mini.initialiser();
            }
            (raccourcis::Raccourcis::nouveau(), mini)
        };
        let pont = pont::Pont::nouveau(
            controleur,
            pont::Systeme::reel(),
            raccourcis,
            mini,
            dossier_moteur,
        );
        let (pont, minuteur) = pont::brancher(&fenetre, pont);
        if let Ok(scenario) = std::env::var("RETRO_EQ_SCENARIO") {
            if let Ok(mut pont) = pont.try_borrow_mut() {
                pont.jouer_scenario(&fenetre, &scenario);
            }
        }
        if ecran.is_none() && premier_lancement {
            fenetre.global::<Appli>().set_ecran(Ecran::Demarrage);
        }
        _vivant = Some((pont, minuteur));
    }

    if let Some(ecran) = ecran {
        fenetre.global::<Appli>().set_ecran(ecran);
    }
    if arguments.iter().any(|a| a == "--rail") {
        fenetre.global::<Appli>().set_rail(true);
    }
    if let Some(chemin) = capture {
        let hauteur = argument(&arguments, "--hauteur")
            .and_then(|h| h.parse().ok())
            .unwrap_or(800);
        return capturer(&fenetre, chemin, hauteur);
    }
    fenetre::brancher(&fenetre);
    fenetre.show()?;
    // `run_event_loop_until_quit` et non `run` : la boucle continue quand la fenêtre est
    // cachée dans la barre des tâches. Elle s'arrête sur « Quitter ».
    slint::run_event_loop_until_quit()?;
    Ok(())
}

/// Affiche la fenêtre, enregistre ce qu'elle montre dans un PNG, puis quitte.
/// Sert à comparer l'appli à la maquette sans écran.
fn capturer(
    fenetre: &FenetrePrincipale,
    chemin: String,
    hauteur: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    fenetre
        .window()
        .set_size(slint::PhysicalSize::new(1280, hauteur));
    fenetre.show()?;
    let faible = fenetre.as_weak();
    slint::Timer::single_shot(std::time::Duration::from_millis(500), move || {
        if let Some(fenetre) = faible.upgrade() {
            let resultat = fenetre
                .window()
                .take_snapshot()
                .map_err(|e| e.to_string())
                .and_then(|pixels| {
                    image::save_buffer(
                        &chemin,
                        pixels.as_bytes(),
                        pixels.width(),
                        pixels.height(),
                        image::ColorType::Rgba8,
                    )
                    .map_err(|e| e.to_string())
                });
            if let Err(erreur) = resultat {
                eprintln!("capture impossible : {erreur}");
            }
        }
        let _ = slint::quit_event_loop();
    });
    slint::run_event_loop()?;
    Ok(())
}
