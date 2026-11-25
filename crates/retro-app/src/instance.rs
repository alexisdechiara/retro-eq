//! Le mutex nommé qui dit à l'installateur « Retro EQ est en cours d'exécution ».
//!
//! Inno Setup (`AppMutex` dans `installateur/retro-eq.iss`) le cherche au début d'une mise à
//! jour ou d'une désinstallation : s'il existe, il demande de fermer l'appli avant de
//! toucher à ses fichiers. Windows le libère tout seul quand le processus se termine.

/// Le nom du mutex. Il doit être identique à `AppMutex` dans le script de l'installateur
/// (la casse compte).
#[cfg_attr(not(windows), allow(dead_code))]
pub const NOM_MUTEX: &str = "RetroEQ.EnCours";

/// Crée le mutex pour toute la durée du processus. Sans effet hors Windows.
#[cfg(windows)]
pub fn poser() {
    use windows::core::PCWSTR;
    use windows::Win32::System::Threading::CreateMutexW;
    let nom: Vec<u16> = NOM_MUTEX.encode_utf16().chain(Some(0)).collect();
    // SAFETY: `nom` se termine par un zéro et vit jusqu'à la fin de l'appel. La poignée
    // renvoyée est volontairement gardée ouverte : la fermer ferait disparaître le mutex.
    let _ = unsafe { CreateMutexW(None, false, PCWSTR(nom.as_ptr())) };
}

#[cfg(not(windows))]
pub fn poser() {}

#[cfg(test)]
mod tests {
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
