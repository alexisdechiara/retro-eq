//! Lecture du registre Windows (compilé seulement sous Windows).

use std::path::PathBuf;

use windows_registry::LOCAL_MACHINE;

use crate::{ErreurMoteur, SortieAudio};

const CLE_EQUALIZER_APO: &str = r"SOFTWARE\EqualizerAPO";

/// Le dossier d'installation d'Equalizer APO d'après le registre, s'il y est.
pub(crate) fn dossier_installation() -> Option<PathBuf> {
    let cle = LOCAL_MACHINE.open(CLE_EQUALIZER_APO).ok()?;
    let chemin = cle.get_string("InstallPath").ok()?;
    Some(PathBuf::from(chemin))
}

/// Les sorties sur lesquelles Equalizer APO est installé.
pub(crate) fn sorties_installees() -> Result<Vec<SortieAudio>, ErreurMoteur> {
    // A VERIFIER SUR LE PC : la disposition « Child APOs » sous la clé
    // d'Equalizer APO n'a pas pu être contrôlée hors Windows. Si la clé
    // n'existe pas, on répond « aucune sortie » plutôt qu'une erreur.
    let Ok(cle) = LOCAL_MACHINE.open(format!(r"{CLE_EQUALIZER_APO}\Child APOs")) else {
        return Ok(Vec::new());
    };
    let sous_cles = cle
        .keys()
        .map_err(|e| ErreurMoteur::Registre(e.to_string()))?;
    Ok(sous_cles
        .map(|id| SortieAudio {
            // A VERIFIER SUR LE PC : le nom lisible n'est sans doute pas le nom
            // de la sous-clé ; pour l'instant on affiche l'identifiant.
            nom: id.clone(),
            id,
            moteur_installe: true,
        })
        .collect())
}
