//! Profils et versions.
//!
//! Un profil est un réglage nommé ; chaque test lui ajoute une version. **Une version n'est
//! jamais modifiée ni supprimée** : `Profil` n'expose que de quoi en ajouter et choisir
//! laquelle est active. C'est pourquoi ses champs sont privés (en Rust, on ne peut pas
//! empêcher la mutation d'un champ public).

use crate::zone::ReglagesVariantes;
use retro_dsp::Courbe;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// D'où vient une version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Origine {
    /// Un preset livré avec l'appli.
    Preset,
    /// Un fichier importé (`format` : « equalizer_apo », « peace », …).
    Import { format: String },
    /// Réglée à la main dans l'égaliseur.
    Manuel,
    /// Issue d'un test.
    Test { test_id: String },
}

/// Un état figé d'un profil.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Version {
    /// Numéro, à partir de 1, croissant.
    pub numero: u32,
    /// Date de création, au format RFC 3339, fournie par l'appelant.
    pub creee_le: String,
    pub courbe: Courbe,
    pub origine: Origine,
    /// Le pas (dB) de la prochaine variante dans chaque zone, dans l'ordre de `Zone::TOUTES`.
    pub pas_par_zone: [f64; 7],
}

/// Ce qui peut mal tourner avec un profil.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ErreurProfil {
    #[error("le profil n'a pas de version {0}")]
    VersionInconnue(u32),
    #[error("profil invalide : {0}")]
    Invalide(String),
}

/// Un réglage d'égaliseur nommé, avec toutes ses versions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "ProfilFichier")]
pub struct Profil {
    id: String,
    nom: String,
    versions: Vec<Version>,
    active: u32,
    reduction_bruit: Option<bool>,
}

/// Le profil tel qu'il est lu sur le disque, avant vérification (voir `TryFrom`).
#[derive(Deserialize)]
struct ProfilFichier {
    id: String,
    nom: String,
    versions: Vec<Version>,
    active: u32,
    #[serde(default)]
    reduction_bruit: Option<bool>,
}

impl TryFrom<ProfilFichier> for Profil {
    type Error = ErreurProfil;

    /// Un fichier modifié à la main ne doit pas pouvoir casser les règles du profil.
    fn try_from(f: ProfilFichier) -> Result<Self, ErreurProfil> {
        if f.versions.is_empty() {
            return Err(ErreurProfil::Invalide("aucune version".into()));
        }
        if !f.versions.windows(2).all(|w| w[0].numero < w[1].numero) {
            return Err(ErreurProfil::Invalide(
                "les numéros de version ne sont pas strictement croissants".into(),
            ));
        }
        if !f.versions.iter().any(|v| v.numero == f.active) {
            return Err(ErreurProfil::VersionInconnue(f.active));
        }
        Ok(Profil {
            id: f.id,
            nom: f.nom,
            versions: f.versions,
            active: f.active,
            reduction_bruit: f.reduction_bruit,
        })
    }
}

impl Profil {
    /// Un nouveau profil, avec sa première version (numéro 1, active). Ses pas de zone sont
    /// les pas initiaux.
    pub fn nouveau(
        id: impl Into<String>,
        nom: impl Into<String>,
        courbe: Courbe,
        origine: Origine,
        creee_le: impl Into<String>,
    ) -> Profil {
        Profil {
            id: id.into(),
            nom: nom.into(),
            versions: vec![Version {
                numero: 1,
                creee_le: creee_le.into(),
                courbe,
                origine,
                pas_par_zone: ReglagesVariantes::default().pas_initiaux(),
            }],
            active: 1,
            reduction_bruit: None,
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn nom(&self) -> &str {
        &self.nom
    }

    /// Renomme le profil. Le nom n'appartient à aucune version : il peut changer.
    pub fn renommer(&mut self, nom: impl Into<String>) {
        self.nom = nom.into();
    }

    /// Toutes les versions, de la plus ancienne à la plus récente.
    pub fn versions(&self) -> &[Version] {
        &self.versions
    }

    /// Une version par son numéro.
    pub fn version(&self, numero: u32) -> Option<&Version> {
        self.versions.iter().find(|v| v.numero == numero)
    }

    /// Le numéro de la version active.
    pub fn numero_actif(&self) -> u32 {
        self.active
    }

    /// La version active : c'est la référence d'un test.
    pub fn version_active(&self) -> &Version {
        // Invariant garanti à la construction : `active` désigne toujours une version.
        self.version(self.active).unwrap_or(&self.versions[0])
    }

    /// Ajoute une version à la suite des autres et rend son numéro. Elle n'est **pas**
    /// activée : c'est un choix de l'utilisateur (après la vérification d'un test).
    pub fn ajouter_version(
        &mut self,
        courbe: Courbe,
        origine: Origine,
        pas_par_zone: [f64; 7],
        creee_le: impl Into<String>,
    ) -> u32 {
        let numero = self.versions.last().map_or(1, |v| v.numero + 1);
        self.versions.push(Version {
            numero,
            creee_le: creee_le.into(),
            courbe,
            origine,
            pas_par_zone,
        });
        numero
    }

    /// Rend active la version `numero`. Refuse un numéro qui n'existe pas.
    pub fn activer(&mut self, numero: u32) -> Result<(), ErreurProfil> {
        if self.version(numero).is_none() {
            return Err(ErreurProfil::VersionInconnue(numero));
        }
        self.active = numero;
        Ok(())
    }

    /// Réglage « réduction de bruit » propre au profil ; `None` tant que l'utilisateur n'a rien choisi.
    pub fn reduction_bruit(&self) -> Option<bool> {
        self.reduction_bruit
    }

    pub fn definir_reduction_bruit(&mut self, valeur: Option<bool>) {
        self.reduction_bruit = valeur;
    }

    /// Le profil est « stable » quand toutes les zones de la version active sont au pas
    /// plancher (ADR-0005). C'est une approximation : le pas ne dit pas si un changement
    /// a été retenu au dernier test.
    pub fn est_stable(&self, reglages: &ReglagesVariantes) -> bool {
        self.version_active()
            .pas_par_zone
            .iter()
            .all(|&p| p <= reglages.pas_plancher_db)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use retro_dsp::{Filtre, TypeFiltre};

    fn profil() -> Profil {
        Profil::nouveau(
            "p1",
            "Mon casque",
            Courbe::default(),
            Origine::Manuel,
            "2025-01-01T10:00:00Z",
        )
    }

    fn courbe_gain(g: f64) -> Courbe {
        Courbe {
            preampli_db: 0.0,
            filtres: vec![Filtre {
                type_filtre: TypeFiltre::Cloche,
                frequence_hz: 1000.0,
                gain_db: g,
                q: 1.0,
            }],
        }
    }

    #[test]
    fn un_nouveau_profil_a_une_version_active() {
        let p = profil();
        assert_eq!(p.versions().len(), 1);
        assert_eq!(p.numero_actif(), 1);
        assert_eq!(p.version_active().numero, 1);
        assert_eq!(p.version_active().pas_par_zone, [4.0; 7]);
        assert_eq!(p.id(), "p1");
        assert_eq!(p.nom(), "Mon casque");
        assert_eq!(p.reduction_bruit(), None);
    }

    #[test]
    fn ajouter_une_version_la_numerote_sans_l_activer_ni_toucher_aux_autres() {
        let mut p = profil();
        let avant = p.versions()[0].clone();
        let n = p.ajouter_version(
            courbe_gain(2.0),
            Origine::Test {
                test_id: "t1".into(),
            },
            [2.0; 7],
            "2025-01-02T10:00:00Z",
        );
        assert_eq!(n, 2);
        assert_eq!(p.versions().len(), 2);
        assert_eq!(p.versions()[0], avant);
        assert_eq!(p.numero_actif(), 1);
        let n3 = p.ajouter_version(courbe_gain(3.0), Origine::Manuel, [1.0; 7], "d");
        assert_eq!(n3, 3);
    }

    #[test]
    fn activer_choisit_une_version_et_refuse_un_numero_inconnu() {
        let mut p = profil();
        p.ajouter_version(courbe_gain(2.0), Origine::Manuel, [4.0; 7], "d");
        p.activer(2).unwrap();
        assert_eq!(p.version_active().numero, 2);
        p.activer(1).unwrap();
        assert_eq!(p.version_active().numero, 1);
        assert_eq!(p.activer(0), Err(ErreurProfil::VersionInconnue(0)));
        assert_eq!(p.activer(9), Err(ErreurProfil::VersionInconnue(9)));
        assert_eq!(p.numero_actif(), 1, "l'échec ne change rien");
    }

    #[test]
    fn renommer_et_reduction_de_bruit() {
        let mut p = profil();
        p.renommer("Autre nom");
        p.definir_reduction_bruit(Some(true));
        assert_eq!(p.nom(), "Autre nom");
        assert_eq!(p.reduction_bruit(), Some(true));
        assert_eq!(p.versions().len(), 1);
    }

    #[test]
    fn stable_quand_tous_les_pas_sont_au_plancher() {
        let r = ReglagesVariantes::default();
        let mut p = profil();
        assert!(!p.est_stable(&r));
        let n = p.ajouter_version(Courbe::default(), Origine::Manuel, [1.0; 7], "d");
        p.activer(n).unwrap();
        assert!(p.est_stable(&r));
    }

    #[test]
    fn le_profil_passe_par_json_sans_perte() {
        let mut p = profil();
        p.ajouter_version(
            courbe_gain(2.0),
            Origine::Import {
                format: "peace".into(),
            },
            [2.0; 7],
            "d",
        );
        p.definir_reduction_bruit(Some(false));
        let j = serde_json::to_string_pretty(&p).unwrap();
        assert_eq!(serde_json::from_str::<Profil>(&j).unwrap(), p);
    }

    #[test]
    fn un_fichier_sans_reduction_de_bruit_se_charge() {
        let mut v = serde_json::to_value(profil()).unwrap();
        v.as_object_mut().unwrap().remove("reduction_bruit");
        let p: Profil = serde_json::from_value(v).unwrap();
        assert_eq!(p.reduction_bruit(), None);
    }

    #[test]
    fn un_fichier_incoherent_est_refuse() {
        let base = serde_json::to_value(profil()).unwrap();

        let mut v = base.clone();
        v["active"] = 7.into();
        assert!(serde_json::from_value::<Profil>(v).is_err());

        let mut v = base.clone();
        v["versions"] = serde_json::json!([]);
        assert!(serde_json::from_value::<Profil>(v).is_err());

        let mut v = base;
        let ligne = v["versions"][0].clone();
        v["versions"] = serde_json::json!([ligne, ligne]);
        assert!(serde_json::from_value::<Profil>(v).is_err());
    }
}
