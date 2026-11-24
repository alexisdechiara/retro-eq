//! Les presets livrés avec l'appli : des points de départ en lecture seule. Pour en faire un
//! profil, l'appli crée un `Profil` dont la première version a l'origine `Origine::Preset`.

use retro_dsp::Courbe;

/// Un point de départ proposé à l'utilisateur.
#[derive(Debug, Clone, PartialEq)]
pub struct Preset {
    /// Nom affiché (en français, langue source des textes).
    pub nom: &'static str,
    pub courbe: Courbe,
}

/// Les presets de départ. Pour l'instant : « À plat ». Ceux du H9 II viendront quand leurs
/// valeurs seront fournies (ticket 14).
pub fn presets() -> Vec<Preset> {
    vec![Preset {
        nom: "À plat",
        courbe: Courbe::default(),
    }]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pour_l_instant_un_seul_preset_a_plat() {
        let p = presets();
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].nom, "À plat");
        assert!(p[0].courbe.filtres.is_empty());
        assert_eq!(p[0].courbe.preampli_db, 0.0);
    }
}
