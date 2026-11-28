//! L'Empreinte : les réponses de tous les tests d'un profil, regroupées par appli au premier
//! plan (PRODUCT.md : « l'Empreinte montre les préférences par activité »). Ticket 28.
//!
//! Seuls les duels sur une zone comptent : c'est là qu'une réponse dit un goût (« plus de
//! graves »). Un duel de contrôle, de vérification ou contre un autre profil ne dit rien
//! d'une zone. Les duels sans appli connue, ou passés, ne comptent pas non plus.

use crate::confiance::Confiance;
use crate::deroulement::DuelClos;
use crate::plan::{Preference, Sujet};
use crate::zone::{Sens, Zone};
use std::collections::BTreeMap;

/// Ce que les réponses données dans une appli disent d'un couple (zone, sens).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TendanceAppli {
    pub zone: Zone,
    pub sens: Sens,
    pub pour_variante: u32,
    pub pour_reference: u32,
    pub pareil: u32,
    /// Confiance de « la variante est préférée » (victoires sur duels tranchés), comme
    /// pour une zone de l'écran Résultat.
    pub confiance: Confiance,
}

/// Les goûts relevés dans une appli.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoutsAppli {
    pub appli: String,
    /// Duels sur une zone répondus dans cette appli, tous tests confondus.
    pub duels: u32,
    /// Les couples où la variante a été préférée plus souvent que la référence, du plus
    /// net au moins net. Vide : rien ne se dégage encore.
    pub tendances: Vec<TendanceAppli>,
}

/// Regroupe des duels clos (de un ou plusieurs tests) par appli. Les applis viennent de la
/// plus utilisée à la moins utilisée (à égalité, par ordre alphabétique).
pub fn empreinte<'a>(duels: impl IntoIterator<Item = &'a DuelClos>) -> Vec<GoutsAppli> {
    // `BTreeMap` : un dictionnaire trié par clé, pour un résultat qui ne dépend pas de
    // l'ordre de lecture des fichiers.
    let mut par_appli: BTreeMap<&str, BTreeMap<(Zone, u8), [u32; 3]>> = BTreeMap::new();
    for duel in duels {
        let (Sujet::Zone { zone, sens }, Some(preference), Some(appli)) =
            (duel.sujet, duel.preference, duel.appli.as_deref())
        else {
            continue;
        };
        let cle = (zone, if sens == Sens::Plus { 0 } else { 1 });
        let comptes = par_appli.entry(appli).or_default().entry(cle).or_default();
        let place = match preference {
            Preference::Variante => 0,
            Preference::Reference => 1,
            Preference::Aucune => 2,
        };
        comptes[place] += 1;
    }
    let mut gouts: Vec<GoutsAppli> = par_appli
        .into_iter()
        .map(|(appli, couples)| {
            let duels = couples.values().map(|c| c.iter().sum::<u32>()).sum();
            let mut tendances: Vec<TendanceAppli> = couples
                .into_iter()
                .filter(|(_, [variante, reference, _])| variante > reference)
                .map(
                    |((zone, sens), [variante, reference, pareil])| TendanceAppli {
                        zone,
                        sens: if sens == 0 { Sens::Plus } else { Sens::Moins },
                        pour_variante: variante,
                        pour_reference: reference,
                        pareil,
                        confiance: Confiance::depuis_victoires(variante, variante + reference),
                    },
                )
                .collect();
            // Le plus net d'abord : la plus grande avance de la variante. `sort_by_key` est
            // stable : à égalité, l'ordre du spectre (celui du `BTreeMap`) est gardé.
            tendances.sort_by_key(|t| std::cmp::Reverse(t.pour_variante - t.pour_reference));
            GoutsAppli {
                appli: appli.to_string(),
                duels,
                tendances,
            }
        })
        .collect();
    gouts.sort_by(|a, b| b.duels.cmp(&a.duels).then_with(|| a.appli.cmp(&b.appli)));
    gouts
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::Reponse;

    fn duel(sujet: Sujet, preference: Option<Preference>, appli: Option<&str>) -> DuelClos {
        DuelClos {
            numero: 1,
            sujet,
            reponse: preference.map(|_| Reponse::A),
            preference,
            appli: appli.map(str::to_string),
            ecart_volume_db: None,
        }
    }

    fn zone(z: Zone, s: Sens, p: Preference, appli: &str) -> DuelClos {
        duel(Sujet::Zone { zone: z, sens: s }, Some(p), Some(appli))
    }

    #[test]
    fn sans_duel_l_empreinte_est_vide() {
        assert!(empreinte(&[]).is_empty());
    }

    #[test]
    fn les_reponses_sont_regroupees_par_appli_tous_tests_confondus() {
        // Deux « tests » : on passe simplement leurs duels à la suite.
        let test1 = [
            zone(Zone::Graves, Sens::Plus, Preference::Variante, "Valorant"),
            zone(Zone::Presence, Sens::Plus, Preference::Variante, "Valorant"),
            zone(Zone::Graves, Sens::Plus, Preference::Reference, "chrome"),
        ];
        let test2 = [
            zone(Zone::Graves, Sens::Plus, Preference::Variante, "Valorant"),
            zone(Zone::Graves, Sens::Moins, Preference::Aucune, "Valorant"),
        ];
        let e = empreinte(test1.iter().chain(&test2));
        assert_eq!(e.len(), 2);
        assert_eq!(e[0].appli, "Valorant");
        assert_eq!(e[0].duels, 4);
        let graves = e[0].tendances[0];
        assert_eq!((graves.zone, graves.sens), (Zone::Graves, Sens::Plus));
        assert_eq!(graves.pour_variante, 2);
        assert_eq!(graves.confiance, Confiance::Probable);
        assert_eq!(
            e[0].tendances.len(),
            2,
            "« moins de graves » : rien ne se dégage"
        );
        assert_eq!(e[1].appli, "chrome");
        assert_eq!(e[1].duels, 1);
        assert!(
            e[1].tendances.is_empty(),
            "la référence a gagné : pas de tendance"
        );
    }

    #[test]
    fn seuls_les_duels_de_zone_repondus_dans_une_appli_connue_comptent() {
        let duels = [
            duel(Sujet::Controle, Some(Preference::Variante), Some("Jeu")),
            duel(Sujet::Verification, Some(Preference::Variante), Some("Jeu")),
            duel(Sujet::AutreProfil, Some(Preference::Variante), Some("Jeu")),
            duel(
                Sujet::Zone {
                    zone: Zone::Air,
                    sens: Sens::Plus,
                },
                None,
                Some("Jeu"),
            ),
            duel(
                Sujet::Zone {
                    zone: Zone::Air,
                    sens: Sens::Plus,
                },
                Some(Preference::Variante),
                None,
            ),
        ];
        assert!(empreinte(&duels).is_empty());
    }

    #[test]
    fn la_tendance_la_plus_nette_vient_en_premier_et_les_applis_les_plus_utilisees_aussi() {
        let mut duels = vec![zone(Zone::Graves, Sens::Plus, Preference::Variante, "B")];
        for _ in 0..3 {
            duels.push(zone(Zone::Air, Sens::Moins, Preference::Variante, "A"));
        }
        duels.push(zone(Zone::Graves, Sens::Plus, Preference::Variante, "A"));
        let e = empreinte(&duels);
        assert_eq!(e[0].appli, "A");
        assert_eq!(e[0].tendances[0].zone, Zone::Air);
        assert_eq!(e[0].tendances[1].zone, Zone::Graves);
        assert_eq!(e[1].appli, "B");
    }
}
