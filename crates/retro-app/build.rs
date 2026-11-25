fn main() {
    // Compile les fichiers .slint en code Rust au moment de la compilation, avec les
    // traductions du dossier `lang` : un fichier `.po` par langue, dans
    // `lang/<langue>/LC_MESSAGES/retro-app.po` (le nom de la crate). Le français, écrit dans
    // les `.slint`, est la langue de référence : il n'a pas de fichier.
    //
    // Sans contexte par défaut : un `@tr("texte")` a pour seule clé son texte français, pas
    // le nom du composant qui le contient. Un texte repris dans deux écrans n'a qu'une entrée.
    //
    // En version de développement (donc pour `cargo test`), on garde les informations de mise au
    // point : les tests de clic retrouvent les éléments par leur nom grâce à elles. Elles
    // alourdissent un peu le code, donc pas en version `release`.
    let developpement = std::env::var("PROFILE").is_ok_and(|profil| profil == "debug");
    if developpement {
        // Pour la macro `slint!` des tests, qui lit cette variable.
        println!("cargo:rustc-env=SLINT_EMIT_DEBUG_INFO=1");
    }
    let configuration = slint_build::CompilerConfiguration::new()
        .with_debug_info(developpement)
        .with_bundled_translations("lang")
        .with_default_translation_context(slint_build::DefaultTranslationContext::None);
    if let Err(erreur) = slint_build::compile_with_config("ui/app.slint", configuration) {
        eprintln!("{erreur}");
        std::process::exit(1);
    }
}
