//! Les phrases du début d'un test et du duel d'essai, dans les deux langues (tickets 25
//! et 30). Les titres et les boutons fixes sont dans les fichiers `.slint` (`@tr`) ; ici, ce
//! qui dépend d'une liste de touches ou de la réponse donnée.

use retro_core::Langue;

/// Les touches de réponse qu'une autre appli garde pour elle, dites au démarrage du test.
/// `touches` n'est jamais vide (le contrôleur ne l'appelle pas sinon).
pub fn annonce_touches_mortes(touches: &[String], l: Langue) -> String {
    let liste = touches.join(", ");
    match (touches.len() > 1, l) {
        (false, Langue::Fr) => format!(
            "{liste} est déjà prise par une autre appli : cette touche ne répondra pas pendant le test. Ferme l’appli qui la garde, ou réponds depuis la fenêtre."
        ),
        (true, Langue::Fr) => format!(
            "{liste} sont déjà prises par une autre appli : ces touches ne répondront pas pendant le test. Ferme l’appli qui les garde, ou réponds depuis la fenêtre."
        ),
        (false, Langue::En) => format!(
            "{liste} is already taken by another app: this key won’t answer during the test. Close the app holding it, or answer from the window."
        ),
        (true, Langue::En) => format!(
            "{liste} are already taken by another app: these keys won’t answer during the test. Close the app holding them, or answer from the window."
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn une_ou_plusieurs_touches_mortes_s_accordent() {
        let une = annonce_touches_mortes(&["F8".to_string()], Langue::Fr);
        assert!(une.starts_with("F8 est déjà prise"), "{une}");
        let deux = annonce_touches_mortes(&["F8".to_string(), "F10".to_string()], Langue::En);
        assert!(deux.starts_with("F8, F10 are already taken"), "{deux}");
    }
}
