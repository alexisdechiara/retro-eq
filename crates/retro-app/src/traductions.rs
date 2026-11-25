//! Garde-fou des traductions : si une phrase de l'interface est ajoutée ou changée en
//! français sans sa traduction anglaise, `cargo test` échoue et dit quoi ajouter.
//!
//! Le test lit les fichiers `.slint` (tous les `@tr(...)`) et le fichier
//! `lang/en/LC_MESSAGES/retro-app.po`, sans passer par Slint. Voir `lang/README.md`.

#![cfg(test)]

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

/// La clé d'une phrase : son contexte (vide en général) et son texte français.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Cle {
    contexte: String,
    texte: String,
    /// Le texte du pluriel pour `@tr("un" | "plusieurs" % n)`.
    pluriel: Option<String>,
}

/// Ce que le `.po` dit d'une phrase : une traduction par forme (une seule sans pluriel).
type Formes = Vec<String>;

/// Lit une chaîne entre guillemets à partir de `debut` (qui doit pointer sur le guillemet
/// ouvrant). Rend la chaîne et la position après le guillemet fermant.
fn lire_chaine(texte: &[char], debut: usize) -> Option<(String, usize)> {
    if texte.get(debut) != Some(&'"') {
        return None;
    }
    let mut chaine = String::new();
    let mut i = debut + 1;
    while i < texte.len() {
        match texte[i] {
            '"' => return Some((chaine, i + 1)),
            '\\' => {
                i += 1;
                match texte.get(i)? {
                    'n' => chaine.push('\n'),
                    't' => chaine.push('\t'),
                    autre => chaine.push(*autre),
                }
            }
            autre => chaine.push(autre),
        }
        i += 1;
    }
    None
}

fn sauter_blancs(texte: &[char], mut i: usize) -> usize {
    while texte.get(i).is_some_and(|c| c.is_whitespace()) {
        i += 1;
    }
    i
}

/// Retire les commentaires `// ...` (hors chaînes) : un `@tr` commenté n'est pas affiché.
fn sans_commentaires(source: &str) -> Vec<char> {
    let caracteres: Vec<char> = source.chars().collect();
    let mut sortie = Vec::with_capacity(caracteres.len());
    let (mut i, mut dans_chaine) = (0, false);
    while i < caracteres.len() {
        let c = caracteres[i];
        if dans_chaine {
            sortie.push(c);
            if c == '\\' {
                if let Some(suivant) = caracteres.get(i + 1) {
                    sortie.push(*suivant);
                    i += 1;
                }
            } else if c == '"' {
                dans_chaine = false;
            }
        } else if c == '"' {
            dans_chaine = true;
            sortie.push(c);
        } else if c == '/' && caracteres.get(i + 1) == Some(&'/') {
            while i < caracteres.len() && caracteres[i] != '\n' {
                i += 1;
            }
            continue;
        } else {
            sortie.push(c);
        }
        i += 1;
    }
    sortie
}

/// Tous les `@tr(...)` d'un fichier `.slint`.
fn cles_du_fichier(source: &str) -> Vec<Cle> {
    let texte = sans_commentaires(source);
    let motif: Vec<char> = "@tr(".chars().collect();
    let mut cles = Vec::new();
    let mut i = 0;
    while i + motif.len() <= texte.len() {
        if texte[i..i + motif.len()] != motif[..] {
            i += 1;
            continue;
        }
        i = sauter_blancs(&texte, i + motif.len());
        let Some((premiere, suite)) = lire_chaine(&texte, i) else {
            panic!("@tr( sans texte littéral près du caractère {i} : {source:.80}");
        };
        i = sauter_blancs(&texte, suite);
        let (contexte, texte_fr) = if texte[i..].starts_with(&['=', '>']) {
            let debut = sauter_blancs(&texte, i + 2);
            let Some((seconde, suite)) = lire_chaine(&texte, debut) else {
                panic!("@tr(\"{premiere}\" => ...) sans texte");
            };
            i = sauter_blancs(&texte, suite);
            (premiere, seconde)
        } else {
            (String::new(), premiere)
        };
        let pluriel = if texte.get(i) == Some(&'|') {
            let debut = sauter_blancs(&texte, i + 1);
            let (pluriel, suite) =
                lire_chaine(&texte, debut).expect("pluriel entre guillemets après « | »");
            i = suite;
            Some(pluriel)
        } else {
            None
        };
        cles.push(Cle {
            contexte,
            texte: texte_fr,
            pluriel,
        });
    }
    cles
}

fn fichiers_slint(dossier: &Path, trouves: &mut Vec<PathBuf>) {
    let mut entrees: Vec<_> = fs::read_dir(dossier)
        .expect("dossier ui lisible")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .collect();
    entrees.sort();
    for chemin in entrees {
        if chemin.is_dir() {
            fichiers_slint(&chemin, trouves);
        } else if chemin.extension().and_then(|e| e.to_str()) == Some("slint") {
            trouves.push(chemin);
        }
    }
}

/// Tous les `@tr` de l'interface, avec les fichiers où ils sont.
fn cles_de_l_interface() -> BTreeMap<Cle, Vec<String>> {
    let racine = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut fichiers = Vec::new();
    fichiers_slint(&racine.join("ui"), &mut fichiers);
    assert!(!fichiers.is_empty(), "aucun fichier .slint trouvé");
    let mut cles: BTreeMap<Cle, Vec<String>> = BTreeMap::new();
    for fichier in fichiers {
        let source = fs::read_to_string(&fichier).expect("fichier .slint lisible");
        let nom = fichier
            .strip_prefix(racine)
            .unwrap_or(&fichier)
            .display()
            .to_string();
        for cle in cles_du_fichier(&source) {
            cles.entry(cle).or_default().push(nom.clone());
        }
    }
    cles
}

/// Lit un fichier `.po` : (contexte, msgid, msgid_plural) -> les `msgstr`.
fn lire_po(contenu: &str) -> BTreeMap<Cle, Formes> {
    let mut entrees = BTreeMap::new();
    for bloc in contenu.split("\n\n") {
        let (mut contexte, mut texte, mut pluriel) = (String::new(), String::new(), None);
        let mut formes: Vec<String> = Vec::new();
        // Le champ en cours de lecture, pour les lignes de suite (`"..."` seules).
        let mut champ = String::new();
        for ligne in bloc.lines() {
            let ligne = ligne.trim();
            if ligne.starts_with('#') || ligne.is_empty() {
                continue;
            }
            let (mot, reste) = match ligne.find(char::is_whitespace) {
                Some(position) if !ligne.starts_with('"') => {
                    (&ligne[..position], ligne[position..].trim())
                }
                _ => ("", ligne),
            };
            let caracteres: Vec<char> = reste.chars().collect();
            let Some((valeur, _)) = lire_chaine(&caracteres, 0) else {
                panic!("ligne .po illisible : {ligne}");
            };
            if !mot.is_empty() {
                champ = mot.to_string();
            }
            match champ.as_str() {
                "msgctxt" => contexte.push_str(&valeur),
                "msgid" => texte.push_str(&valeur),
                "msgid_plural" => pluriel.get_or_insert_with(String::new).push_str(&valeur),
                "msgstr" => match (mot.is_empty(), formes.last_mut()) {
                    (true, Some(derniere)) => derniere.push_str(&valeur),
                    _ => formes.push(valeur),
                },
                forme if forme.starts_with("msgstr[") => {
                    if mot.is_empty() {
                        if let Some(derniere) = formes.last_mut() {
                            derniere.push_str(&valeur);
                        }
                    } else {
                        formes.push(valeur);
                    }
                }
                autre => panic!("mot-clé .po inconnu « {autre} » dans : {ligne}"),
            }
        }
        // L'en-tête (msgid vide) décrit le fichier : ce n'est pas une phrase.
        if texte.is_empty() && contexte.is_empty() {
            continue;
        }
        entrees.insert(
            Cle {
                contexte,
                texte,
                pluriel,
            },
            formes,
        );
    }
    entrees
}

/// Les `{...}` d'une phrase, triés : la traduction doit en avoir les mêmes.
fn remplacements(texte: &str) -> Vec<String> {
    let mut trouves = Vec::new();
    let mut courant: Option<String> = None;
    for c in texte.chars() {
        match (c, courant.as_mut()) {
            ('{', _) => courant = Some(String::from("{")),
            ('}', Some(en_cours)) => {
                en_cours.push('}');
                trouves.push(courant.take().unwrap_or_default());
            }
            (autre, Some(en_cours)) => en_cours.push(autre),
            _ => {}
        }
    }
    trouves.sort();
    trouves
}

fn chemin_po() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("lang/en/LC_MESSAGES/retro-app.po")
}

fn po_anglais() -> BTreeMap<Cle, Formes> {
    let contenu =
        fs::read_to_string(chemin_po()).expect("lang/en/LC_MESSAGES/retro-app.po lisible");
    lire_po(&contenu)
}

/// Une entrée `.po` vide, à compléter, pour la copier dans le fichier.
fn modele_po(cle: &Cle) -> String {
    let mut sortie = String::new();
    if !cle.contexte.is_empty() {
        sortie.push_str(&format!("msgctxt {:?}\n", cle.contexte));
    }
    sortie.push_str(&format!("msgid {:?}\n", cle.texte));
    match &cle.pluriel {
        Some(pluriel) => sortie.push_str(&format!(
            "msgid_plural {pluriel:?}\nmsgstr[0] \"\"\nmsgstr[1] \"\"\n"
        )),
        None => sortie.push_str("msgstr \"\"\n"),
    }
    sortie
}

#[test]
fn chaque_phrase_de_l_interface_a_sa_traduction_anglaise() {
    let interface = cles_de_l_interface();
    let po = po_anglais();

    let manquantes: Vec<&Cle> = interface
        .keys()
        .filter(|cle| {
            let attendu = if cle.pluriel.is_some() { 2 } else { 1 };
            !po.get(*cle)
                .is_some_and(|f| f.len() == attendu && f.iter().all(|t| !t.trim().is_empty()))
        })
        .collect();
    assert!(
        manquantes.is_empty(),
        "{} phrase(s) de l'interface sans traduction anglaise dans {}. \
         À ajouter (voir lang/README.md) :\n\n{}",
        manquantes.len(),
        chemin_po().display(),
        manquantes
            .iter()
            .map(|c| format!("# {}\n{}", interface[*c].join(", "), modele_po(c)))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn le_fichier_anglais_ne_garde_pas_de_phrases_disparues() {
    let interface = cles_de_l_interface();
    let po = po_anglais();
    let orphelines: Vec<String> = po
        .keys()
        .filter(|cle| !interface.contains_key(*cle))
        .map(|cle| format!("{:?}", cle.texte))
        .collect();
    assert!(
        orphelines.is_empty(),
        "phrases du .po qui ne sont plus dans l'interface (français modifié ou supprimé ?), \
         à retirer ou à mettre à jour : {}",
        orphelines.join(" ; ")
    );
}

#[test]
fn les_traductions_gardent_leurs_remplacements() {
    let po = po_anglais();
    for (cle, formes) in &po {
        let originaux = match &cle.pluriel {
            Some(pluriel) => vec![cle.texte.as_str(), pluriel.as_str()],
            None => vec![cle.texte.as_str()],
        };
        for (original, traduit) in originaux.iter().zip(formes) {
            assert_eq!(
                remplacements(original),
                remplacements(traduit),
                "les {{}} de « {original} » ne sont pas ceux de « {traduit} »"
            );
        }
    }
}

#[test]
fn le_separateur_decimal_anglais_est_le_point() {
    let po = po_anglais();
    let cle = Cle {
        contexte: "séparateur décimal".to_string(),
        texte: ",".to_string(),
        pluriel: None,
    };
    assert_eq!(po.get(&cle), Some(&vec![".".to_string()]));
}

#[test]
fn l_en_tete_du_po_annonce_deux_formes_de_pluriel() {
    let contenu = fs::read_to_string(chemin_po()).expect("fichier .po lisible");
    assert!(contenu.contains("Plural-Forms: nplurals=2; plural=(n != 1);"));
    assert!(contenu.contains("Language: en"));
}

#[test]
fn le_lecteur_de_slint_comprend_les_trois_formes_de_tr() {
    let source = r#"
        // @tr("commenté") ne compte pas
        a: @tr("Simple \"cité\"");
        b: @tr("Duel {}", Test.duel);
        c: @tr("séparateur décimal" => ",");
        d: @tr("{n} version" | "{n} versions" % p.versions);
    "#;
    let cles = cles_du_fichier(source);
    assert_eq!(cles.len(), 4);
    assert_eq!(cles[0].texte, "Simple \"cité\"");
    assert_eq!(cles[1].texte, "Duel {}");
    assert_eq!(
        (cles[2].contexte.as_str(), cles[2].texte.as_str()),
        ("séparateur décimal", ",")
    );
    assert_eq!(cles[3].pluriel.as_deref(), Some("{n} versions"));
}

#[test]
fn le_lecteur_de_po_comprend_les_lignes_de_suite_et_les_pluriels() {
    let po = lire_po(
        "msgid \"\"\nmsgstr \"\"\n\"Language: en\\n\"\n\n\
         msgid \"\"\n\"Un texte \"\n\"coupé\"\nmsgstr \"\"\n\"A split \"\n\"text\"\n\n\
         msgid \"{n} a\"\nmsgid_plural \"{n} as\"\nmsgstr[0] \"{n} x\"\nmsgstr[1] \"{n} xs\"\n",
    );
    let coupe = Cle {
        contexte: String::new(),
        texte: "Un texte coupé".to_string(),
        pluriel: None,
    };
    assert_eq!(po.get(&coupe), Some(&vec!["A split text".to_string()]));
    assert_eq!(po.len(), 2);
}
