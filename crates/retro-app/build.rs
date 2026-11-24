fn main() {
    // Compile les fichiers .slint en code Rust au moment de la compilation.
    if let Err(erreur) = slint_build::compile("ui/app.slint") {
        eprintln!("{erreur}");
        std::process::exit(1);
    }
}
