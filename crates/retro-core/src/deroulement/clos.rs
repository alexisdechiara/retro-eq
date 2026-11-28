//! Les duels clos d'un test, relus après coup : pour l'écran Résultat (écart de volume
//! mesuré, ticket 26) et pour l'Empreinte (réponses regroupées par appli, ticket 28).
//!
//! Le test est fini quand on lit ceci : rien ici n'a besoin de rester à l'aveugle.

use super::{ContexteReprise, Deroulement, ErreurReprise};
use crate::journal::Entree;
use crate::plan::{Cote, DuelPrevu, Preference, Reponse, Sujet};
use crate::profil::{Origine, Version};
use crate::zone::ReglagesVariantes;
use retro_dsp::{Courbe, Spectre};

/// Au-delà de cet écart (dB, dans un sens ou dans l'autre) entre le volume mesuré de la
/// variante et celui de la référence, le duel est signalé sur l'écran Résultat : le côté le
/// plus fort a pu gagner parce qu'il était plus fort.
///
/// 2 dB plutôt que le seuil de perception (≈ 1 dB) : la mesure se fait sur ce qui joue, et
/// en jeu le contenu change d'un passage à l'autre (ADR-0006). Avec 3 passages de chaque
/// côté, un écart de 1 dB peut venir du contenu seul ; au-delà de 2 dB, il mérite qu'on
/// regarde. Valeur de départ, à revoir avec les premiers vrais tests sur le PC.
pub const SEUIL_ECART_VOLUME_DB: f64 = 2.0;

/// Un duel clos, avec tout ce qu'on en sait une fois le test fini.
#[derive(Debug, Clone, PartialEq)]
pub struct DuelClos {
    /// Numéro du duel, à partir de 1.
    pub numero: u32,
    pub sujet: Sujet,
    /// La réponse ; `None` si le duel a été passé.
    pub reponse: Option<Reponse>,
    /// La réponse traduite (variante, référence ou aucune) ; `None` si le duel a été passé.
    pub preference: Option<Preference>,
    /// L'appli vue le plus souvent au premier plan pendant ses passages.
    pub appli: Option<String>,
    /// Volume mesuré de la variante moins celui de la référence (dB), s'il a été mesuré.
    pub ecart_volume_db: Option<f64>,
}

impl DuelClos {
    /// L'écart de volume mesuré dépasse-t-il le seuil ?
    pub fn ecart_signale(&self) -> bool {
        self.ecart_volume_db
            .is_some_and(|e| e.abs() > SEUIL_ECART_VOLUME_DB)
    }
}

impl Deroulement {
    /// Les duels clos jusqu'ici, du premier au dernier.
    pub fn duels_clos(&self) -> Vec<DuelClos> {
        self.plan
            .iter()
            .zip(&self.issues)
            .enumerate()
            .map(|(i, (duel, issue))| duel_clos(i, duel, *issue, &self.applis, &self.ecarts))
            .collect()
    }
}

fn duel_clos(
    i: usize,
    duel: &DuelPrevu,
    issue: Option<Reponse>,
    applis: &[Option<String>],
    ecarts: &[Option<f64>],
) -> DuelClos {
    DuelClos {
        numero: i as u32 + 1,
        sujet: duel.sujet,
        reponse: issue,
        preference: issue.map(|r| duel.preference(r)),
        appli: applis.get(i).cloned().flatten(),
        ecart_volume_db: ecarts.get(i).copied().flatten(),
    }
}

/// Les duels clos d'un journal, test fini ou non, relus avec les règles de la reprise
/// (nouveaux tirages, annulations, duel de vérification).
pub fn duels_clos_du_journal(journal: &[Entree]) -> Result<Vec<DuelClos>, ErreurReprise> {
    let Some(Entree::TestDemarre {
        version_reference, ..
    }) = journal.first()
    else {
        return Err(if journal.is_empty() {
            ErreurReprise::JournalVide
        } else {
            ErreurReprise::SansDemarrage
        });
    };
    // La relecture ne joue aucun son : la version ne sert qu'à passer le contrôle du
    // numéro, la courbe est relue dans le journal et le spectre n'est jamais utilisé.
    let contexte = ContexteReprise {
        reference: Version {
            numero: *version_reference,
            creee_le: String::new(),
            courbe: Courbe::default(),
            origine: Origine::Manuel,
            pas_par_zone: [ReglagesVariantes::default().pas_initial_db; 7],
        },
        variantes: ReglagesVariantes::default(),
        spectre: Spectre::rose(8),
        verification: DuelPrevu {
            sujet: Sujet::Verification,
            reference_est_a: true,
            ordre_passages: [Cote::A, Cote::B, Cote::A, Cote::B, Cote::A, Cote::B],
        },
    };
    let (d, _) = Deroulement::rejouer(journal, contexte, true)?;
    Ok(d.duels_clos())
}

/// Les duels d'un journal qui comptent pour l'Empreinte (ticket 28) : ceux d'un test
/// **terminé** dont les duels de contrôle n'ont pas bloqué le résultat. Un test abandonné
/// ou interrompu n'a pas de bilan des contrôles, et un test bloqué a montré que ses
/// réponses ne distinguaient pas deux versions identiques : ni l'un ni l'autre ne dit
/// quelque chose de sûr sur les goûts. Vide pour tout journal illisible.
pub fn duels_pour_empreinte(journal: &[Entree]) -> Vec<DuelClos> {
    let bloque = journal.iter().find_map(|e| match e {
        Entree::TestTermine { resultat, .. } => Some(resultat.controles.rien_retenu),
        _ => None,
    });
    match bloque {
        Some(false) => duels_clos_du_journal(journal).unwrap_or_default(),
        _ => Vec::new(),
    }
}
