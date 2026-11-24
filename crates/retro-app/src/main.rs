slint::include_modules!();

mod demo;
mod fenetre;

/// Nom d'écran passé à `--ecran`, pour ouvrir l'appli directement dessus.
fn ecran_depuis(nom: &str) -> Option<Ecran> {
    Some(match nom {
        "demarrage" => Ecran::Demarrage,
        "accueil" => Ecran::Accueil,
        "tester" => Ecran::Tester,
        "nouveau-test" => Ecran::NouveauTest,
        "duels" => Ecran::Duels,
        "resultat" => Ecran::Resultat,
        "profils" => Ecran::Profils,
        "egaliseur" => Ecran::Egaliseur,
        "visualiseur" => Ecran::Visualiseur,
        "reglages" => Ecran::Reglages,
        "diagnostic" => Ecran::Diagnostic,
        "a-propos" => Ecran::APropos,
        _ => return None,
    })
}

fn argument(arguments: &[String], nom: &str) -> Option<String> {
    let position = arguments.iter().position(|a| a == nom)?;
    arguments.get(position + 1).cloned()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arguments: Vec<String> = std::env::args().collect();
    let fenetre = FenetrePrincipale::new()?;
    if let Some(ecran) = argument(&arguments, "--ecran").and_then(|nom| ecran_depuis(&nom)) {
        fenetre.global::<Appli>().set_ecran(ecran);
    }
    fenetre.global::<Appli>().set_rail(arguments.iter().any(|a| a == "--rail"));
    if arguments.iter().any(|a| a == "--demo") {
        demo::remplir(&fenetre);
    }
    if let Some(chemin) = argument(&arguments, "--capture") {
        let hauteur = argument(&arguments, "--hauteur").and_then(|h| h.parse().ok()).unwrap_or(900);
        return capturer(&fenetre, chemin, hauteur);
    }
    fenetre::brancher(&fenetre);
    fenetre.run()?;
    Ok(())
}

/// Affiche la fenêtre, enregistre ce qu'elle montre dans un PNG, puis quitte.
/// Sert à comparer l'appli à la maquette sans écran.
fn capturer(fenetre: &FenetrePrincipale, chemin: String, hauteur: u32) -> Result<(), Box<dyn std::error::Error>> {
    fenetre.window().set_size(slint::PhysicalSize::new(1440, hauteur));
    fenetre.show()?;
    let faible = fenetre.as_weak();
    slint::Timer::single_shot(std::time::Duration::from_millis(500), move || {
        if let Some(fenetre) = faible.upgrade() {
            let resultat = fenetre.window().take_snapshot().map_err(|e| e.to_string()).and_then(|pixels| {
                image::save_buffer(&chemin, pixels.as_bytes(), pixels.width(), pixels.height(), image::ColorType::Rgba8)
                    .map_err(|e| e.to_string())
            });
            if let Err(erreur) = resultat {
                eprintln!("capture impossible : {erreur}");
            }
        }
        let _ = slint::quit_event_loop();
    });
    slint::run_event_loop()?;
    Ok(())
}
