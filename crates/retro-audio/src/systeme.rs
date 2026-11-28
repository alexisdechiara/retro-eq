//! Point d'entrée pour l'appli : la vraie implémentation sous Windows, la simulée ailleurs.

use crate::{Bip, Capture, DetecteurAppel, PleinEcran, PremierPlan, SortiesAudio};

#[cfg(windows)]
use crate::windows_impl as impl_systeme;

pub fn capture() -> Box<dyn Capture> {
    #[cfg(windows)]
    return Box::new(impl_systeme::CaptureLoopback::new());
    #[cfg(not(windows))]
    return Box::new(crate::simule::CaptureSimulee::new());
}

pub fn premier_plan() -> Box<dyn PremierPlan> {
    #[cfg(windows)]
    return Box::new(impl_systeme::PremierPlanWindows);
    #[cfg(not(windows))]
    return Box::new(crate::simule::PremierPlanFixe(None));
}

pub fn detecteur_appel() -> Box<dyn DetecteurAppel> {
    #[cfg(windows)]
    return Box::new(impl_systeme::DetecteurAppelDiscord);
    #[cfg(not(windows))]
    return Box::new(crate::simule::DetecteurAppelSimule::default());
}

pub fn plein_ecran() -> Box<dyn PleinEcran> {
    #[cfg(windows)]
    return Box::new(impl_systeme::PleinEcranWindows);
    #[cfg(not(windows))]
    return Box::new(crate::simule::PleinEcranFixe(false));
}

pub fn sorties() -> Box<dyn SortiesAudio> {
    #[cfg(windows)]
    return Box::new(impl_systeme::SortiesWindows);
    #[cfg(not(windows))]
    return Box::new(crate::simule::SortiesFixes(Vec::new()));
}

pub fn bip() -> Box<dyn Bip> {
    #[cfg(windows)]
    return Box::new(impl_systeme::BipWindows);
    #[cfg(not(windows))]
    return Box::new(crate::simule::BipMuet::default());
}
