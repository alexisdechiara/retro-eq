//! Sonde d'essai pour Equalizer APO sur un vrai PC Windows (tickets 15 et 16).
//!
//! Elle écoute la sortie par défaut en loopback pendant que le vrai moteur `EqualizerApo`
//! change la courbe, et mesure ce qui arrive vraiment au son :
//!
//! - `wav <fichier> <sinus|bruit>` : fabrique un son d'essai de 60 s (niveau modéré) ;
//! - `delai <dossier config>` : bascule le préampli entre 0 et −12 dB et mesure le délai
//!   entre l'écriture du fichier et le changement de niveau capté ;
//! - `fondu <dossier config> <paliers> <pause ms>` : même chose avec le fondu par paliers,
//!   et affiche le niveau capté toutes les 20 ms autour de chaque bascule ;
//! - `reponse <dossier config>` : compare le spectre capté à plat et avec un plateau bas de
//!   +6 dB à 100 Hz, Q 0,71, et le confronte à la courbe calculée par `retro-dsp`.
//!
//! Un son doit jouer sur la sortie par défaut pendant la mesure (le `.wav` fabriqué plus
//! haut, en boucle). À la fin, le moteur est retiré (`retirer`), même en cas d'erreur.
//!
//! Lancer : `cargo run -p retro-engine --example sonde_apo -- delai "C:\Program Files\EqualizerAPO\config"`

#[cfg(windows)]
fn main() {
    if let Err(e) = windows::lancer() {
        eprintln!("erreur : {e}");
        std::process::exit(1);
    }
}

#[cfg(not(windows))]
fn main() {
    eprintln!("Cette sonde ne sert que sous Windows, avec Equalizer APO installé.");
}

#[cfg(windows)]
mod windows {
    use retro_audio::windows_impl::CaptureLoopback;
    use retro_audio::{Capture, Trame};
    use retro_dsp::{Courbe, Filtre, TypeFiltre};
    use retro_engine::{EqualizerApo, Moteur};
    use std::error::Error;
    use std::f64::consts::PI;
    use std::io::Write;
    use std::path::PathBuf;
    use std::sync::mpsc::Receiver;
    use std::time::{Duration, Instant};

    type Issue<T> = Result<T, Box<dyn Error>>;

    pub fn lancer() -> Issue<()> {
        let args: Vec<String> = std::env::args().skip(1).collect();
        match args.first().map(String::as_str) {
            Some("wav") => fabriquer_wav(arg(&args, 1)?, arg(&args, 2)?),
            Some("delai") => avec_moteur(arg(&args, 1)?, None, mesurer_bascules),
            Some("fondu") => {
                let paliers: u32 = arg(&args, 2)?.parse()?;
                let pause: u64 = arg(&args, 3)?.parse()?;
                avec_moteur(
                    arg(&args, 1)?,
                    Some((paliers, Duration::from_millis(pause))),
                    mesurer_bascules,
                )
            }
            Some("reponse") => avec_moteur(arg(&args, 1)?, None, mesurer_reponse),
            Some("clic") => {
                let paliers: u32 = args.get(2).map_or(Ok(0), |a| a.parse())?;
                let pause: u64 = args.get(3).map_or(Ok(20), |a| a.parse())?;
                avec_moteur(
                    arg(&args, 1)?,
                    Some((paliers, Duration::from_millis(pause))),
                    mesurer_clics,
                )
            }
            _ => Err("usage : wav <fichier> <sinus|bruit> | delai <dossier> | fondu <dossier> <paliers> <pause ms> | reponse <dossier>".into()),
        }
    }

    fn arg(args: &[String], i: usize) -> Issue<&str> {
        args.get(i)
            .map(String::as_str)
            .ok_or_else(|| "argument manquant".into())
    }

    /// Lance la mesure, puis retire toujours le moteur, pour rendre `config.txt`.
    fn avec_moteur(
        dossier: &str,
        fondu: Option<(u32, Duration)>,
        mesure: fn(&mut EqualizerApo, &Receiver<Trame>) -> Issue<()>,
    ) -> Issue<()> {
        let mut moteur = EqualizerApo::nouveau(PathBuf::from(dossier));
        if let Some((paliers, pause)) = fondu {
            moteur = moteur.avec_fondu(paliers, pause);
        }
        let mut capture = CaptureLoopback::new();
        let trames = capture.demarrer()?;
        let resultat = mesure(&mut moteur, &trames);
        capture.arreter();
        let retrait = moteur.retirer();
        println!(
            "moteur retiré : {}",
            if retrait.is_ok() { "oui" } else { "NON" }
        );
        if let Some(copie) = moteur.copie_de_securite() {
            println!("ancienne copie gardée : {}", copie.display());
        }
        resultat?;
        retrait?;
        Ok(())
    }

    fn preampli(db: f64) -> Courbe {
        Courbe {
            preampli_db: db,
            filtres: Vec::new(),
        }
    }

    fn rms_db(echantillons: &[f32]) -> f64 {
        let somme: f64 = echantillons.iter().map(|&x| f64::from(x).powi(2)).sum();
        let rms = (somme / echantillons.len().max(1) as f64).sqrt();
        20.0 * rms.max(1e-9).log10()
    }

    /// Vide le canal et rend (instant de réception, niveau) pour chaque trame arrivée.
    fn lire(trames: &Receiver<Trame>, pendant: Duration) -> Vec<(Instant, f64)> {
        let fin = Instant::now() + pendant;
        let mut niveaux = Vec::new();
        while Instant::now() < fin {
            if let Ok(t) = trames.recv_timeout(Duration::from_millis(50)) {
                niveaux.push((Instant::now(), rms_db(&t.echantillons)));
            }
        }
        niveaux
    }

    fn mesurer_bascules(moteur: &mut EqualizerApo, trames: &Receiver<Trame>) -> Issue<()> {
        moteur.appliquer(&preampli(0.0))?;
        let depart = lire(trames, Duration::from_secs(2));
        let base = mediane(depart.iter().map(|n| n.1));
        println!("niveau capté à plat : {base:.1} dB");
        if base < -70.0 {
            return Err("rien ne joue sur la sortie par défaut : lance le son d'essai".into());
        }
        let mut delais = Vec::new();
        for i in 0..8 {
            let cible = if i % 2 == 0 { -12.0 } else { 0.0 };
            let avant = Instant::now();
            moteur.appliquer(&preampli(cible))?;
            let ecrit = Instant::now();
            let niveaux = lire(trames, Duration::from_millis(2500));
            let attendu = base + cible;
            // Premier bloc à moins de 3 dB du niveau attendu.
            let arrive = niveaux.iter().find(|(_, n)| (n - attendu).abs() < 3.0);
            let fin = mediane(niveaux.iter().rev().take(20).map(|n| n.1));
            match arrive {
                Some((t, _)) => {
                    let ms = t.duration_since(avant).as_millis();
                    delais.push(ms);
                    println!(
                        "bascule {} → {cible:+.0} dB : écrit en {} ms, entendu après {ms} ms, niveau final {fin:.1} dB",
                        i + 1,
                        ecrit.duration_since(avant).as_millis()
                    );
                }
                None => println!(
                    "bascule {} → {cible:+.0} dB : AUCUN changement capté en 2,5 s (niveau final {fin:.1} dB)",
                    i + 1
                ),
            }
            if std::env::var_os("SONDE_DETAIL").is_some() {
                let t0 = avant;
                let ligne: Vec<String> = niveaux
                    .iter()
                    .take(40)
                    .map(|(t, n)| format!("{}:{n:.0}", t.duration_since(t0).as_millis()))
                    .collect();
                println!("   détail (ms:dB) {}", ligne.join(" "));
            }
        }
        if !delais.is_empty() {
            delais.sort_unstable();
            println!(
                "délai : min {} ms, médian {} ms, max {} ms (précision ≈ 20 ms, taille d'un bloc capté)",
                delais[0],
                delais[delais.len() / 2],
                delais[delais.len() - 1]
            );
        }
        moteur.appliquer(&preampli(0.0))?;
        Ok(())
    }

    /// Le plus grand « saut » d'un bloc : la différence seconde |x[n] − 2x[n−1] + x[n−2]|.
    /// Sur un sinus pur de 1 kHz elle reste petite (≈ 1,7 % de l'amplitude) ; un clic, c'est
    /// une cassure de la forme d'onde, qui la fait bondir.
    fn saut_max(e: &[f32]) -> f64 {
        e.windows(3)
            .map(|w| f64::from(w[2] - 2.0 * w[1] + w[0]).abs())
            .fold(0.0, f64::max)
    }

    /// Avec le sinus de 1 kHz en lecture : bascule le préampli 0 / −12 dB et compare le plus
    /// grand saut capté juste après chaque bascule au saut ordinaire.
    fn mesurer_clics(moteur: &mut EqualizerApo, trames: &Receiver<Trame>) -> Issue<()> {
        moteur.appliquer(&preampli(0.0))?;
        std::thread::sleep(Duration::from_millis(500));
        let lire_sauts = |pendant: Duration| -> Vec<f64> {
            let fin = Instant::now() + pendant;
            let mut sauts = Vec::new();
            while Instant::now() < fin {
                if let Ok(t) = trames.recv_timeout(Duration::from_millis(50)) {
                    sauts.push(saut_max(&t.echantillons));
                }
            }
            sauts
        };
        while trames.try_recv().is_ok() {}
        let ordinaire = mediane(lire_sauts(Duration::from_secs(2)).into_iter());
        println!("saut ordinaire à plat : {ordinaire:.5}");
        // Pour chaque bascule, une fenêtre témoin de même durée sans bascule, au même niveau :
        // on compare le pire saut des deux. Sans clic, les deux se valent.
        let pire_de = |sauts: Vec<f64>| sauts.into_iter().fold(0.0, f64::max);
        let (mut avec, mut temoin) = (Vec::new(), Vec::new());
        for i in 0..10 {
            let cible = if i % 2 == 0 { -12.0 } else { 0.0 };
            while trames.try_recv().is_ok() {}
            moteur.appliquer(&preampli(cible))?;
            let bascule = pire_de(lire_sauts(Duration::from_millis(400)));
            let calme = pire_de(lire_sauts(Duration::from_millis(400))).max(1e-9);
            println!(
                "bascule {} → {cible:+.0} dB : pire saut {:.1} × celui de la fenêtre témoin",
                i + 1,
                bascule / calme
            );
            avec.push(bascule / calme);
            temoin.push(calme / ordinaire);
        }
        avec.sort_by(f64::total_cmp);
        println!(
            "rapport bascule / témoin : médiane {:.1} ×, pire {:.1} × (≈ 1 : pas de cassure ; nettement > 2 : clic)",
            avec[avec.len() / 2],
            avec[avec.len() - 1]
        );
        moteur.appliquer(&preampli(0.0))?;
        Ok(())
    }

    fn mediane(valeurs: impl Iterator<Item = f64>) -> f64 {
        let mut v: Vec<f64> = valeurs.collect();
        if v.is_empty() {
            return f64::NEG_INFINITY;
        }
        v.sort_by(f64::total_cmp);
        v[v.len() / 2]
    }

    /// Puissance moyenne à chaque fréquence (Goertzel sur des blocs de 8192 échantillons).
    fn spectre(trames: &Receiver<Trame>, pendant: Duration, frequences: &[f64]) -> Vec<f64> {
        let mut tampon: Vec<f32> = Vec::new();
        let mut fe = 48_000.0;
        let fin = Instant::now() + pendant;
        while Instant::now() < fin {
            if let Ok(t) = trames.recv_timeout(Duration::from_millis(50)) {
                fe = f64::from(t.frequence_echantillonnage);
                tampon.extend_from_slice(&t.echantillons);
            }
        }
        // 32 768 échantillons (≈ 0,7 s) : assez fin pour séparer 20 Hz de 30 Hz.
        const N: usize = 32_768;
        let blocs: Vec<&[f32]> = tampon.chunks_exact(N).collect();
        frequences
            .iter()
            .map(|&f| {
                let k = 2.0 * (2.0 * PI * f / fe).cos();
                let somme: f64 = blocs
                    .iter()
                    .map(|bloc| {
                        let (mut s1, mut s2) = (0.0f64, 0.0f64);
                        for (i, &x) in bloc.iter().enumerate() {
                            // Fenêtre de Hann : limite les fuites entre fréquences voisines.
                            let w = 0.5 - 0.5 * (2.0 * PI * i as f64 / N as f64).cos();
                            let s = f64::from(x) * w + k * s1 - s2;
                            s2 = s1;
                            s1 = s;
                        }
                        s1 * s1 + s2 * s2 - k * s1 * s2
                    })
                    .sum();
                somme / blocs.len().max(1) as f64
            })
            .collect()
    }

    /// Les fréquences regardées par `reponse` (et jouées par le son `multi`).
    const FREQUENCES: [f64; 26] = [
        20.0, 30.0, 40.0, 50.0, 63.0, 80.0, 100.0, 125.0, 160.0, 200.0, 250.0, 315.0, 400.0, 500.0,
        630.0, 800.0, 1000.0, 2000.0, 3000.0, 4000.0, 5000.0, 6300.0, 8000.0, 10000.0, 12500.0,
        16000.0,
    ];

    fn mesurer_reponse(moteur: &mut EqualizerApo, trames: &Receiver<Trame>) -> Issue<()> {
        let frequences = FREQUENCES;
        // `SONDE_HAUT` : plateau haut de +6 dB à 8 kHz (zone Air) au lieu du plateau bas.
        let (type_filtre, frequence_hz) = if std::env::var_os("SONDE_HAUT").is_some() {
            (TypeFiltre::PlateauHaut, 8000.0)
        } else {
            (TypeFiltre::PlateauBas, 100.0)
        };
        let plateau = Courbe {
            preampli_db: 0.0,
            filtres: vec![Filtre {
                type_filtre,
                frequence_hz,
                gain_db: 6.0,
                q: 0.71,
            }],
        };
        let mut reference = vec![0.0; frequences.len()];
        let mut avec = vec![0.0; frequences.len()];
        // Deux allers-retours, pour moyenner les variations du bruit.
        for tour in 0..2 {
            for (courbe, cumul) in [(&preampli(0.0), &mut reference), (&plateau, &mut avec)] {
                moteur.appliquer(courbe)?;
                std::thread::sleep(Duration::from_millis(800));
                while trames.try_recv().is_ok() {}
                for (c, p) in
                    cumul
                        .iter_mut()
                        .zip(spectre(trames, Duration::from_secs(8), &frequences))
                {
                    *c += p;
                }
                print!(".");
                let _ = std::io::stdout().flush();
            }
            let _ = tour;
        }
        println!();
        println!("  f (Hz)   capté (dB)   retro-dsp (dB)   écart");
        let mut ecart_max: f64 = 0.0;
        for (i, &f) in frequences.iter().enumerate() {
            let capte = 10.0 * (avec[i] / reference[i].max(1e-30)).log10();
            let calcule = plateau.reponse_db(f);
            ecart_max = ecart_max.max((capte - calcule).abs());
            println!(
                "{f:>8.0}   {capte:>9.2}   {calcule:>13.2}   {:>6.2}",
                capte - calcule
            );
        }
        println!("écart maximal : {ecart_max:.2} dB");
        moteur.appliquer(&preampli(0.0))?;
        Ok(())
    }

    /// Un son d'essai de 60 s, 48 kHz, 16 bits, stéréo : sinus 1 kHz à −20 dBFS, ou bruit
    /// blanc à environ −20 dBFS efficaces.
    fn fabriquer_wav(chemin: &str, genre: &str) -> Issue<()> {
        const FE: u32 = 48_000;
        let n = FE as usize * 60;
        let amplitude = 0.1f64; // −20 dBFS
        let mut graine: u64 = 0x2545_F491_4F6C_DD1D;
        let mut donnees = Vec::with_capacity(n * 4);
        for i in 0..n {
            let t = i as f64 / f64::from(FE);
            let x = match genre {
                "sinus" => amplitude * (2.0 * PI * 1000.0 * t).sin() * 2f64.sqrt(),
                // Un sinus à chaque fréquence mesurée par `reponse`, phases étalées pour
                // limiter les crêtes : chaque fréquence ressort très au-dessus du bruit.
                "multi" => FREQUENCES
                    .iter()
                    .enumerate()
                    .map(|(k, &f)| 0.02 * (2.0 * PI * f * t + k as f64 * 2.4).sin())
                    .sum(),
                _ => {
                    graine ^= graine << 13;
                    graine ^= graine >> 7;
                    graine ^= graine << 17;
                    // Uniforme sur [-1, 1] : efficace = 1/√3, ramené à `amplitude`.
                    let u = (graine >> 11) as f64 / (1u64 << 53) as f64 * 2.0 - 1.0;
                    u * amplitude * 3f64.sqrt()
                }
            };
            let s = (x * 32767.0).round().clamp(-32768.0, 32767.0) as i16;
            donnees.extend_from_slice(&s.to_le_bytes());
            donnees.extend_from_slice(&s.to_le_bytes());
        }
        let mut f = std::fs::File::create(chemin)?;
        let taille = donnees.len() as u32;
        f.write_all(b"RIFF")?;
        f.write_all(&(36 + taille).to_le_bytes())?;
        f.write_all(b"WAVEfmt ")?;
        f.write_all(&16u32.to_le_bytes())?;
        f.write_all(&1u16.to_le_bytes())?; // PCM
        f.write_all(&2u16.to_le_bytes())?; // stéréo
        f.write_all(&FE.to_le_bytes())?;
        f.write_all(&(FE * 4).to_le_bytes())?;
        f.write_all(&4u16.to_le_bytes())?;
        f.write_all(&16u16.to_le_bytes())?;
        f.write_all(b"data")?;
        f.write_all(&taille.to_le_bytes())?;
        f.write_all(&donnees)?;
        println!("écrit : {chemin}");
        Ok(())
    }
}
