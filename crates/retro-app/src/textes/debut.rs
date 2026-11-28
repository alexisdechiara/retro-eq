//! Les phrases du début d'un test et du duel d'essai, dans les deux langues (tickets 25
//! et 30). Les titres et les boutons fixes sont dans les fichiers `.slint` (`@tr`) ; ici, ce
//! qui dépend d'une liste de touches ou de la réponse donnée.

use super::choisir;
use retro_core::{BilanEssai, Cote, Langue, Reponse};

/// La lettre d'un côté.
fn lettre(cote: Cote) -> &'static str {
    match cote {
        Cote::A => "A",
        Cote::B => "B",
    }
}

/// Ce qui s'est passé au duel d'essai, en une phrase : qui jouait les graves (l'essai ne
/// compte pas, on peut le révéler) et ce qui a été répondu.
pub fn detail_essai(bilan: &BilanEssai, l: Langue) -> String {
    let graves = lettre(bilan.cote_graves);
    match (bilan.reponse, l) {
        (Some(Reponse::Pareil), Langue::Fr) => {
            format!("La version aux graves était {graves} ; tu as répondu « Pareil ».")
        }
        (Some(Reponse::Pareil), Langue::En) => {
            format!("The bass-heavy version was {graves}; you answered “Same”.")
        }
        (Some(r), Langue::Fr) => {
            let r = lettre(if r == Reponse::A { Cote::A } else { Cote::B });
            format!("La version aux graves était {graves} ; tu as répondu {r}.")
        }
        (Some(r), Langue::En) => {
            let r = lettre(if r == Reponse::A { Cote::A } else { Cote::B });
            format!("The bass-heavy version was {graves}; you answered {r}.")
        }
        (None, Langue::Fr) => {
            format!("La version aux graves était {graves} ; tu n’as pas répondu.")
        }
        (None, Langue::En) => format!("The bass-heavy version was {graves}; you didn’t answer."),
    }
}

pub fn annonce_essai_arrete(l: Langue) -> &'static str {
    choisir(
        l,
        "Essai arrêté. Ta référence est remise.",
        "Trial stopped. Your reference is back.",
    )
}

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
    fn le_detail_de_l_essai_dit_qui_jouait_les_graves_et_la_reponse() {
        use retro_core::IssueEssai;
        let bilan = BilanEssai {
            issue: IssueEssai::Confondue,
            cote_graves: Cote::B,
            reponse: Some(Reponse::A),
        };
        assert_eq!(
            detail_essai(&bilan, Langue::Fr),
            "La version aux graves était B ; tu as répondu A."
        );
        let pareil = BilanEssai {
            reponse: Some(Reponse::Pareil),
            ..bilan
        };
        assert!(detail_essai(&pareil, Langue::En).ends_with("you answered “Same”."));
    }

    #[test]
    fn une_ou_plusieurs_touches_mortes_s_accordent() {
        let une = annonce_touches_mortes(&["F8".to_string()], Langue::Fr);
        assert!(une.starts_with("F8 est déjà prise"), "{une}");
        let deux = annonce_touches_mortes(&["F8".to_string(), "F10".to_string()], Langue::En);
        assert!(deux.starts_with("F8, F10 are already taken"), "{deux}");
    }
}
