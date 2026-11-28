//! Le duel d'essai du premier lancement (PRODUCT.md, ticket 25).
//!
//! Un seul duel, avec une différence exprès énorme : la référence contre elle-même avec
//! 6 dB de graves en plus. Il vérifie de bout en bout que le son change vraiment sur le
//! casque, avec les mêmes touches que les vrais duels. Il ne crée aucune version et n'entre
//! dans aucun résultat.
//!
//! Il se joue avec la même machine à états qu'un vrai test ([`crate::Deroulement`]), donc
//! avec les mêmes garanties : la référence est remise à chaque pause, à la fin, à l'abandon.
//! Pour cela, il prend la forme d'un mode existant : « profil contre profil » avec un seul
//! duel, l'« autre profil » étant la variante aux graves. Ce mode n'ajoute jamais de duel de
//! vérification, et le duel d'essai se termine donc dès la réponse.

use crate::deroulement::DuelFini;
use crate::hasard::Hasard;
use crate::plan::{retirer_duel, Cote, DuelPrevu, Mode, Preference, Reponse, Sujet};
use crate::zone::{variante, Sens, Zone};
use retro_dsp::Courbe;

/// Ce que la variante du duel d'essai ajoute dans les graves, en dB.
pub const GRAVES_EN_PLUS_DB: f64 = 6.0;

/// Le mode et le plan d'un duel d'essai, prêts à donner à [`crate::ParametresTest`].
#[derive(Debug, Clone, PartialEq)]
pub struct DuelDEssai {
    pub mode: Mode,
    pub plan: Vec<DuelPrevu>,
}

/// Le duel d'essai pour cette référence : qui est A et l'ordre des passages sont tirés au
/// hasard, comme pour un vrai duel.
pub fn duel_d_essai(reference: &Courbe, hasard: &mut impl Hasard) -> DuelDEssai {
    let graves = variante(reference, Zone::Graves, Sens::Plus, GRAVES_EN_PLUS_DB);
    // `retirer_duel` tire l'orientation et l'ordre des passages sans toucher au sujet : le
    // duel de départ ne sert qu'à porter ce sujet.
    let gabarit = DuelPrevu {
        sujet: Sujet::AutreProfil,
        reference_est_a: true,
        ordre_passages: [Cote::A, Cote::B, Cote::A, Cote::B, Cote::A, Cote::B],
    };
    DuelDEssai {
        mode: Mode::ProfilContreProfil {
            autre: graves,
            duels: 1,
        },
        plan: vec![retirer_duel(&gabarit, hasard)],
    }
}

/// Ce que la réponse au duel d'essai dit du son.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IssueEssai {
    /// La version aux graves a été choisie : le son change, et on l'entend.
    Reconnue,
    /// L'autre version a été choisie : une différence a été entendue, sans reconnaître la
    /// version aux graves. Le son change quand même.
    Confondue,
    /// « Pareil » : aucune différence entendue. Le son ne change peut-être pas : c'est le
    /// cas à renvoyer vers le Diagnostic.
    Pareil,
    /// Duel passé sans réponse.
    Passe,
}

/// Le bilan d'un duel d'essai fini : l'issue, et de quoi la raconter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BilanEssai {
    pub issue: IssueEssai,
    /// Le côté qui jouait la version aux graves (révélé : l'essai ne compte pas).
    pub cote_graves: Cote,
    /// La réponse donnée, `None` si le duel a été passé.
    pub reponse: Option<Reponse>,
}

impl BilanEssai {
    /// Le bilan d'après le duel clos. « Préférer la variante » veut dire ici « reconnaître
    /// la version aux graves » : c'est la question posée pendant l'essai.
    pub fn depuis(duel: &DuelFini) -> BilanEssai {
        let issue = match duel.preference {
            Some(Preference::Variante) => IssueEssai::Reconnue,
            Some(Preference::Reference) => IssueEssai::Confondue,
            Some(Preference::Aucune) => IssueEssai::Pareil,
            None => IssueEssai::Passe,
        };
        BilanEssai {
            issue,
            cote_graves: duel.reference.autre(),
            reponse: duel.reponse,
        }
    }

    /// Une différence a-t-elle été entendue ? Sinon, renvoyer vers le Diagnostic.
    pub fn son_change(&self) -> bool {
        matches!(self.issue, IssueEssai::Reconnue | IssueEssai::Confondue)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::deroulement::{Deroulement, Effet, Evenement, ParametresTest};
    use crate::hasard::HasardGraine;
    use crate::plan::duel_verification;
    use crate::profil::{Origine, Version};
    use crate::reglages::ReglagesTest;
    use crate::zone::ReglagesVariantes;
    use retro_dsp::Spectre;

    fn reference() -> Courbe {
        Courbe {
            preampli_db: -3.0,
            filtres: Vec::new(),
        }
    }

    #[test]
    fn un_seul_duel_contre_la_reference_avec_6_db_de_graves_en_plus() {
        let essai = duel_d_essai(&reference(), &mut HasardGraine::new(3));
        assert_eq!(essai.plan.len(), 1);
        assert_eq!(essai.plan[0].sujet, Sujet::AutreProfil);
        let Mode::ProfilContreProfil { autre, duels } = &essai.mode else {
            panic!("mode inattendu : {:?}", essai.mode);
        };
        assert_eq!(*duels, 1);
        // Le préampli de la référence est gardé : seul un filtre de graves s'ajoute.
        assert_eq!(autre.preampli_db, -3.0);
        let ecart = |f: f64| autre.reponse_db(f) - reference().reponse_db(f);
        assert!((ecart(Zone::Graves.centre_hz()) - 6.0).abs() < 0.1);
        assert!(ecart(3_000.0).abs() < 0.5, "les aigus ne bougent pas");
    }

    #[test]
    fn qui_est_a_et_l_ordre_des_passages_sont_tires_au_hasard() {
        let orientations: Vec<bool> = (0..20)
            .map(|g| duel_d_essai(&reference(), &mut HasardGraine::new(g)).plan[0].reference_est_a)
            .collect();
        assert!(orientations.contains(&true) && orientations.contains(&false));
        for g in 0..20 {
            let ordre =
                duel_d_essai(&reference(), &mut HasardGraine::new(g)).plan[0].ordre_passages;
            assert_eq!(ordre.iter().filter(|c| **c == Cote::A).count(), 3);
        }
    }

    fn fini(reference: Cote, reponse: Option<Reponse>) -> DuelFini {
        let prevu = DuelPrevu {
            sujet: Sujet::AutreProfil,
            reference_est_a: reference == Cote::A,
            ordre_passages: [Cote::A; 6],
        };
        DuelFini {
            numero: 1,
            sujet: Sujet::AutreProfil,
            reponse,
            reference,
            preference: reponse.map(|r| prevu.preference(r)),
        }
    }

    #[test]
    fn la_reponse_dit_si_la_version_aux_graves_a_ete_reconnue() {
        // La référence est A : les graves sont en B.
        let bilan = BilanEssai::depuis(&fini(Cote::A, Some(Reponse::B)));
        assert_eq!(bilan.issue, IssueEssai::Reconnue);
        assert_eq!(bilan.cote_graves, Cote::B);
        assert!(bilan.son_change());

        let bilan = BilanEssai::depuis(&fini(Cote::A, Some(Reponse::A)));
        assert_eq!(bilan.issue, IssueEssai::Confondue);
        assert!(bilan.son_change());

        let bilan = BilanEssai::depuis(&fini(Cote::B, Some(Reponse::Pareil)));
        assert_eq!(bilan.issue, IssueEssai::Pareil);
        assert_eq!(bilan.cote_graves, Cote::A);
        assert!(!bilan.son_change());

        let bilan = BilanEssai::depuis(&fini(Cote::B, None));
        assert_eq!(bilan.issue, IssueEssai::Passe);
        assert!(!bilan.son_change());
    }

    #[test]
    fn le_duel_d_essai_finit_des_la_reponse_sur_la_reference() {
        let version = Version {
            numero: 1,
            creee_le: "2025-11-24T21:40:00+01:00".into(),
            courbe: reference(),
            origine: Origine::Preset,
            pas_par_zone: [4.0; 7],
        };
        let mut hasard = HasardGraine::new(9);
        let essai = duel_d_essai(&version.courbe, &mut hasard);
        let mut d = Deroulement::nouveau(ParametresTest {
            test_id: "essai".into(),
            profil_id: "p".into(),
            reference: version.clone(),
            mode: essai.mode,
            plan: essai.plan,
            reglages: ReglagesTest::nouveau(10_000, false),
            variantes: ReglagesVariantes::default(),
            spectre: Spectre::rose(32),
            verification: duel_verification(&mut hasard),
        });
        d.traiter(Evenement::Demarrer, 0);
        for i in 1..=6 {
            d.traiter(Evenement::Tic, i * 10_000);
        }
        let effets = d.traiter(Evenement::Reponse(Reponse::B), 61_000);
        // Pas de duel de vérification : le test se termine tout de suite…
        assert!(effets.iter().any(|e| matches!(e, Effet::Termine(_))));
        // …et la dernière courbe donnée au moteur est la référence telle quelle.
        let derniere = effets.iter().rev().find_map(|e| match e {
            Effet::Appliquer { cote, courbe } => Some((*cote, courbe.clone())),
            _ => None,
        });
        assert_eq!(derniere, Some((None, version.courbe)));
    }
}
