//! Les 7 zones du spectre, le sens d'une variante et la fabrication des variantes.
//!
//! Une variante n'est jamais qu'une référence plus un seul filtre posé sur une zone
//! (ADR-0005) : ce module sait fabriquer ce filtre.

use retro_dsp::{Courbe, Filtre, TypeFiltre};
use serde::{Deserialize, Serialize};

/// Une plage de fréquences nommée du spectre. Ne pas confondre avec une « bande »
/// (un fader de l'égaliseur) : une zone peut en couvrir plusieurs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Zone {
    SubGraves,
    Graves,
    BasMediums,
    Mediums,
    Presence,
    Aigus,
    Air,
}

impl Zone {
    /// Les 7 zones, des plus graves aux plus aiguës. L'ordre est aussi l'ordre des cases
    /// de `Version::pas_par_zone` (voir [`Zone::index`]).
    pub const TOUTES: [Zone; 7] = [
        Zone::SubGraves,
        Zone::Graves,
        Zone::BasMediums,
        Zone::Mediums,
        Zone::Presence,
        Zone::Aigus,
        Zone::Air,
    ];

    /// Position de la zone dans [`Zone::TOUTES`] : sert d'indice dans les tableaux à 7 cases.
    pub fn index(self) -> usize {
        match self {
            Zone::SubGraves => 0,
            Zone::Graves => 1,
            Zone::BasMediums => 2,
            Zone::Mediums => 3,
            Zone::Presence => 4,
            Zone::Aigus => 5,
            Zone::Air => 6,
        }
    }

    /// Bornes de la zone en Hz : (basse, haute).
    pub fn bornes_hz(self) -> (f64, f64) {
        match self {
            Zone::SubGraves => (20.0, 60.0),
            Zone::Graves => (60.0, 250.0),
            Zone::BasMediums => (250.0, 500.0),
            Zone::Mediums => (500.0, 2_000.0),
            Zone::Presence => (2_000.0, 5_000.0),
            Zone::Aigus => (5_000.0, 10_000.0),
            Zone::Air => (10_000.0, 20_000.0),
        }
    }

    /// Centre géométrique de la zone : le « milieu » à l'oreille, où chaque octave compte autant.
    pub fn centre_hz(self) -> f64 {
        let (bas, haut) = self.bornes_hz();
        (bas * haut).sqrt()
    }

    /// Largeur de la zone en Hz (haut moins bas).
    pub fn largeur_hz(self) -> f64 {
        let (bas, haut) = self.bornes_hz();
        haut - bas
    }

    /// Nom français affiché, à passer à `@tr(...)` côté interface : le français est la
    /// langue source des textes.
    pub fn nom(self) -> &'static str {
        match self {
            Zone::SubGraves => "Sub-graves",
            Zone::Graves => "Graves",
            Zone::BasMediums => "Bas-médiums",
            Zone::Mediums => "Médiums",
            Zone::Presence => "Présence",
            Zone::Aigus => "Aigus",
            Zone::Air => "Air",
        }
    }
}

/// Le sens d'une variante : on ajoute du gain (`Plus`) ou on en retire (`Moins`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Sens {
    Plus,
    Moins,
}

impl Sens {
    /// +1 pour `Plus`, -1 pour `Moins` : pratique pour signer un gain.
    pub fn signe(self) -> f64 {
        match self {
            Sens::Plus => 1.0,
            Sens::Moins => -1.0,
        }
    }

    /// Le sens à l'envers.
    pub fn oppose(self) -> Sens {
        match self {
            Sens::Plus => Sens::Moins,
            Sens::Moins => Sens::Plus,
        }
    }
}

/// Les valeurs de départ des variantes. Regroupées ici pour pouvoir les ajuster à l'oreille
/// après les premiers vrais tests (ADR-0005), sans chercher dans le code.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ReglagesVariantes {
    /// Pas (en dB) d'une variante au premier test d'un profil.
    pub pas_initial_db: f64,
    /// Plus petit pas possible : en dessous, la différence ne s'entend plus.
    pub pas_plancher_db: f64,
}

impl Default for ReglagesVariantes {
    fn default() -> Self {
        ReglagesVariantes {
            pas_initial_db: 4.0,
            pas_plancher_db: 1.0,
        }
    }
}

impl ReglagesVariantes {
    /// Le pas d'une zone au test suivant quand aucun changement n'y a été retenu :
    /// la moitié, sans descendre sous le plancher.
    pub fn pas_reduit(&self, pas_db: f64) -> f64 {
        (pas_db / 2.0).max(self.pas_plancher_db)
    }

    /// Les 7 pas d'un profil qui n'a encore jamais été testé.
    pub fn pas_initiaux(&self) -> [f64; 7] {
        [self.pas_initial_db; 7]
    }
}

/// Q des plateaux (sub-graves et air) : 0,71 est le plateau « sans bosse » du Cookbook.
const Q_PLATEAU: f64 = 0.71;
/// Fréquence du plateau bas des sub-graves.
const FREQUENCE_PLATEAU_BAS_HZ: f64 = 60.0;
/// Fréquence du plateau haut de l'air.
const FREQUENCE_PLATEAU_HAUT_HZ: f64 = 10_000.0;

/// Le filtre qui agit sur une zone : plateau bas pour les sub-graves, plateau haut pour l'air,
/// cloche centrée (au centre géométrique, Q = centre / largeur) pour les autres.
pub fn filtre_de_zone(zone: Zone, gain_db: f64) -> Filtre {
    match zone {
        Zone::SubGraves => Filtre {
            type_filtre: TypeFiltre::PlateauBas,
            frequence_hz: FREQUENCE_PLATEAU_BAS_HZ,
            gain_db,
            q: Q_PLATEAU,
        },
        Zone::Air => Filtre {
            type_filtre: TypeFiltre::PlateauHaut,
            frequence_hz: FREQUENCE_PLATEAU_HAUT_HZ,
            gain_db,
            q: Q_PLATEAU,
        },
        _ => Filtre {
            type_filtre: TypeFiltre::Cloche,
            frequence_hz: zone.centre_hz(),
            gain_db,
            q: zone.centre_hz() / zone.largeur_hz(),
        },
    }
}

/// La variante de `reference` pour une zone : la référence plus un seul filtre de `pas_db`
/// dans le sens demandé. Le préampli de la référence est gardé tel quel : c'est
/// `retro_dsp::egaliser_volumes` qui règle les volumes au moment d'écouter.
pub fn variante(reference: &Courbe, zone: Zone, sens: Sens, pas_db: f64) -> Courbe {
    let mut courbe = reference.clone();
    courbe
        .filtres
        .push(filtre_de_zone(zone, sens.signe() * pas_db.abs()));
    courbe
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sept_zones_contigues_de_20_hz_a_20_khz() {
        assert_eq!(Zone::TOUTES.len(), 7);
        assert_eq!(Zone::TOUTES[0].bornes_hz().0, 20.0);
        assert_eq!(Zone::TOUTES[6].bornes_hz().1, 20_000.0);
        for paire in Zone::TOUTES.windows(2) {
            assert_eq!(paire[0].bornes_hz().1, paire[1].bornes_hz().0);
        }
        for (i, z) in Zone::TOUTES.iter().enumerate() {
            assert_eq!(z.index(), i);
        }
    }

    #[test]
    fn bornes_exactes_de_la_spec() {
        let attendu = [
            (20.0, 60.0),
            (60.0, 250.0),
            (250.0, 500.0),
            (500.0, 2_000.0),
            (2_000.0, 5_000.0),
            (5_000.0, 10_000.0),
            (10_000.0, 20_000.0),
        ];
        for (z, b) in Zone::TOUTES.iter().zip(attendu) {
            assert_eq!(z.bornes_hz(), b);
        }
    }

    #[test]
    fn noms_francais_distincts_et_non_vides() {
        let mut noms: Vec<_> = Zone::TOUTES.iter().map(|z| z.nom()).collect();
        assert!(noms.iter().all(|n| !n.is_empty()));
        noms.sort();
        noms.dedup();
        assert_eq!(noms.len(), 7);
    }

    #[test]
    fn reglages_par_defaut_4_db_et_plancher_1_db() {
        let r = ReglagesVariantes::default();
        assert_eq!(r.pas_initial_db, 4.0);
        assert_eq!(r.pas_plancher_db, 1.0);
        assert_eq!(r.pas_reduit(4.0), 2.0);
        assert_eq!(r.pas_reduit(2.0), 1.0);
        assert_eq!(r.pas_reduit(1.0), 1.0);
        assert_eq!(r.pas_initiaux(), [4.0; 7]);
    }

    #[test]
    fn le_filtre_des_sub_graves_est_un_plateau_bas_a_60_hz() {
        let f = filtre_de_zone(Zone::SubGraves, 4.0);
        assert_eq!(f.type_filtre, TypeFiltre::PlateauBas);
        assert_eq!(f.frequence_hz, 60.0);
        assert_eq!(f.q, 0.71);
        assert_eq!(f.gain_db, 4.0);
    }

    #[test]
    fn le_filtre_de_l_air_est_un_plateau_haut_a_10_khz() {
        let f = filtre_de_zone(Zone::Air, -4.0);
        assert_eq!(f.type_filtre, TypeFiltre::PlateauHaut);
        assert_eq!(f.frequence_hz, 10_000.0);
        assert_eq!(f.q, 0.71);
        assert_eq!(f.gain_db, -4.0);
    }

    #[test]
    fn les_autres_zones_sont_des_cloches_au_centre_geometrique() {
        for zone in [
            Zone::Graves,
            Zone::BasMediums,
            Zone::Mediums,
            Zone::Presence,
            Zone::Aigus,
        ] {
            let (bas, haut) = zone.bornes_hz();
            let f = filtre_de_zone(zone, 3.0);
            assert_eq!(f.type_filtre, TypeFiltre::Cloche);
            assert!((f.frequence_hz - (bas * haut).sqrt()).abs() < 1e-9);
            assert!((f.q - f.frequence_hz / (haut - bas)).abs() < 1e-9);
        }
        // Valeur à la main : médiums, centre 1 kHz, largeur 1,5 kHz.
        let m = filtre_de_zone(Zone::Mediums, 1.0);
        assert!((m.frequence_hz - 1000.0).abs() < 1e-9);
        assert!((m.q - 1000.0 / 1500.0).abs() < 1e-9);
    }

    #[test]
    fn la_variante_ajoute_un_seul_filtre_signe() {
        let reference = Courbe {
            preampli_db: -2.0,
            filtres: vec![filtre_de_zone(Zone::Graves, 1.0)],
        };
        let plus = variante(&reference, Zone::Aigus, Sens::Plus, 4.0);
        let moins = variante(&reference, Zone::Aigus, Sens::Moins, 4.0);
        assert_eq!(plus.filtres.len(), 2);
        assert_eq!(plus.filtres[0], reference.filtres[0]);
        assert_eq!(plus.filtres[1].gain_db, 4.0);
        assert_eq!(moins.filtres[1].gain_db, -4.0);
        assert_eq!(plus.preampli_db, -2.0);
        // La référence n'est pas touchée.
        assert_eq!(reference.filtres.len(), 1);
    }

    #[test]
    fn sens_signe_et_oppose() {
        assert_eq!(Sens::Plus.signe(), 1.0);
        assert_eq!(Sens::Moins.signe(), -1.0);
        assert_eq!(Sens::Plus.oppose(), Sens::Moins);
    }

    #[test]
    fn les_types_passent_par_json() {
        let j = serde_json::to_string(&Zone::BasMediums).unwrap();
        assert_eq!(serde_json::from_str::<Zone>(&j).unwrap(), Zone::BasMediums);
        let r = ReglagesVariantes::default();
        let j = serde_json::to_string(&r).unwrap();
        assert_eq!(serde_json::from_str::<ReglagesVariantes>(&j).unwrap(), r);
    }
}
