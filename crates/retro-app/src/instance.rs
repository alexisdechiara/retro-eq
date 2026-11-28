//! Une seule Retro EQ à la fois.
//!
//! Deux instances ouvertes sur le même dossier de données se marcheraient dessus : la
//! seconde réécrirait la courbe du moteur pendant un duel de la première et reprendrait son
//! journal de test, encore ouvert. Deux gardes, prises au lancement avant de toucher au
//! moteur :
//!
//! - **Le mutex nommé** `RetroEQ.EnCours` (Windows seulement). Il dit aussi à l'installateur
//!   que l'appli tourne : Inno Setup (`AppMutex` dans `installateur/retro-eq.iss`) le
//!   cherche au début d'une mise à jour ou d'une désinstallation et demande de fermer
//!   l'appli avant de toucher à ses fichiers. Windows le libère tout seul à la fin du
//!   processus.
//! - **Le verrou du dossier de données** (`instance.verrou`), sur tous les systèmes : un
//!   fichier tenu verrouillé tant que l'appli tourne. Le système lève le verrou à la fin du
//!   processus, même après un arrêt brutal : le fichier qui reste ne bloque rien.

use std::fs::{File, OpenOptions, TryLockError};
use std::path::Path;

/// Le nom du mutex. Il doit être identique à `AppMutex` dans le script de l'installateur
/// (la casse compte).
#[cfg_attr(not(windows), allow(dead_code))]
pub const NOM_MUTEX: &str = "RetroEQ.EnCours";

/// Le fichier verrouillé dans le dossier de données.
pub const FICHIER_VERROU: &str = "instance.verrou";

/// Crée le mutex pour toute la durée du processus. Rend vrai s'il existait déjà, c'est-à-dire
/// si une autre Retro EQ tourne. Sans effet hors Windows (rend faux).
// A VERIFIER SUR LE PC : jamais compilé ni lancé sous Windows depuis l'ajout du test
// d'existence (ERROR_ALREADY_EXISTS).
#[cfg(windows)]
pub fn poser() -> bool {
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS};
    use windows::Win32::System::Threading::CreateMutexW;
    let nom: Vec<u16> = NOM_MUTEX.encode_utf16().chain(Some(0)).collect();
    // SAFETY: `nom` se termine par un zéro et vit jusqu'à la fin de l'appel. La poignée
    // renvoyée est volontairement gardée ouverte : la fermer ferait disparaître le mutex.
    let poignee = unsafe { CreateMutexW(None, false, PCWSTR(nom.as_ptr())) };
    // Quand le mutex existe déjà, `CreateMutexW` réussit quand même (il rend une poignée vers
    // l'existant) et le dit par `GetLastError`, à lire juste après l'appel.
    // SAFETY: appel sans argument, qui ne fait que lire l'erreur du fil courant.
    let deja = unsafe { GetLastError() } == ERROR_ALREADY_EXISTS;
    poignee.is_ok() && deja
}

#[cfg(not(windows))]
pub fn poser() -> bool {
    false
}

/// Ce qui empêche de prendre le verrou du dossier de données.
#[derive(Debug)]
pub enum ErreurVerrou {
    /// Une autre Retro EQ tient déjà le verrou.
    DejaOuverte,
    /// Le fichier n'a pas pu être créé ou verrouillé (dossier protégé, disque plein...).
    Fichier(std::io::Error),
}

/// Le verrou du dossier de données : tant qu'il vit, aucune autre Retro EQ ne peut s'ouvrir
/// sur ce dossier. À garder jusqu'à la fin de `main`.
#[derive(Debug)]
pub struct VerrouInstance {
    // Le verrou tient tant que le fichier est ouvert ; il est levé à sa fermeture.
    _fichier: File,
}

/// Prend le verrou du dossier de données (créé s'il n'existe pas encore). Ne bloque jamais :
/// si une autre instance le tient, rend tout de suite `DejaOuverte`.
pub fn verrouiller(dossier: &Path) -> Result<VerrouInstance, ErreurVerrou> {
    std::fs::create_dir_all(dossier).map_err(ErreurVerrou::Fichier)?;
    // Ni `truncate` ni écriture : le contenu ne compte pas, seul le verrou compte.
    let fichier = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(dossier.join(FICHIER_VERROU))
        .map_err(ErreurVerrou::Fichier)?;
    match fichier.try_lock() {
        Ok(()) => Ok(VerrouInstance { _fichier: fichier }),
        Err(TryLockError::WouldBlock) => Err(ErreurVerrou::DejaOuverte),
        Err(TryLockError::Error(e)) => Err(ErreurVerrou::Fichier(e)),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn une_seconde_instance_sur_le_meme_dossier_est_refusee() {
        let t = TempDir::new().unwrap();
        let premiere = verrouiller(t.path()).unwrap();
        assert!(matches!(
            verrouiller(t.path()),
            Err(ErreurVerrou::DejaOuverte)
        ));
        // La première fermée, l'appli peut de nouveau s'ouvrir.
        drop(premiere);
        assert!(verrouiller(t.path()).is_ok());
    }

    #[test]
    fn le_fichier_laisse_par_un_arret_brutal_ne_bloque_pas() {
        let t = TempDir::new().unwrap();
        std::fs::write(t.path().join(FICHIER_VERROU), "").unwrap();
        assert!(verrouiller(t.path()).is_ok());
    }

    #[test]
    fn le_dossier_de_donnees_est_cree_s_il_manque() {
        let t = TempDir::new().unwrap();
        let dossier = t.path().join("Retro EQ");
        let _verrou = verrouiller(&dossier).unwrap();
        assert!(dossier.join(FICHIER_VERROU).is_file());
    }

    /// Le script de l'installateur et l'appli doivent parler du même mutex : sinon
    /// l'installateur ne verrait jamais l'appli ouverte.
    #[test]
    fn le_script_de_l_installateur_attend_le_meme_mutex() {
        let script = include_str!("../../../installateur/retro-eq.iss");
        assert!(
            script.contains(&format!("AppMutex={}", super::NOM_MUTEX)),
            "AppMutex de retro-eq.iss différent de NOM_MUTEX"
        );
    }
}
