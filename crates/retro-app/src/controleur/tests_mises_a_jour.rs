//! La vérification des mises à jour, de bout en bout, avec un faux client HTTP et une
//! horloge passée à la main : aucun réseau, aucun fil.

#![allow(clippy::unwrap_used)]

use super::*;
use crate::http::factice::ClientFactice;
use crate::http::{ErreurHttp, ReponseHttp};
use retro_core::mises_a_jour::{DELAI_ENTRE_VERIFICATIONS_MS as JOUR, PAGE_VERSIONS};
use retro_engine::MoteurSimule;
use tempfile::TempDir;

/// 24 novembre 2025, 20:40 UTC (21:40 à Paris : décalage de 60 min dans `options`).
const T0: u64 = 1_764_016_800_000;

fn options() -> Options {
    Options {
        dossier_export: PathBuf::from("exports"),
        moteur_absent: false,
        decalage_minutes: 60,
        graine: Some(7),
        langue_systeme: Langue::Fr,
        langue_imposee: None,
    }
}

fn ouvrir(dossier: &TempDir, t: u64) -> Controleur {
    Controleur::ouvrir(
        dossier.path().join("donnees"),
        Box::new(MoteurSimule::nouveau()),
        options(),
        t,
    )
    .unwrap()
}

/// Ce que fait le pont, sans le fil : demander, envoyer, rendre la réponse.
/// Rend `true` si une requête est partie.
fn verifier(c: &mut Controleur, client: &ClientFactice, automatique: bool, t: u64) -> bool {
    match c.demander_verification(automatique, t) {
        Some(demande) => {
            c.recevoir_verification(demande.envoyer(client), t);
            true
        }
        None => false,
    }
}

fn reponse(statut: u16, entetes: &[(&str, &str)], corps: &str) -> Result<ReponseHttp, ErreurHttp> {
    Ok(ReponseHttp {
        statut,
        entetes: entetes
            .iter()
            .map(|(n, v)| (n.to_string(), v.to_string()))
            .collect(),
        corps: corps.as_bytes().to_vec(),
    })
}

fn publiee(tag: &str) -> Result<ReponseHttp, ErreurHttp> {
    reponse(
        200,
        &[("x-ratelimit-remaining", "58")],
        &format!(
            r#"{{"tag_name":"{tag}","html_url":"https://github.com/alexisdechiara/retro-eq/releases/tag/{tag}",
            "body":"Notes de {tag}","draft":false,"prerelease":false,"published_at":"2026-01-02T10:30:00Z",
            "assets":[{{"name":"Retro-EQ-Installateur-{tag}.exe","size":12345678,
            "browser_download_url":"https://github.com/alexisdechiara/retro-eq/releases/download/{tag}/Retro-EQ-Installateur-{tag}.exe"}}]}}"#
        ),
    )
}

#[test]
fn au_premier_lancement_rien_n_est_verifie_et_la_verification_est_due() {
    let dossier = TempDir::new().unwrap();
    let c = ouvrir(&dossier, T0);
    let vue = c.vue_mise_a_jour(T0);
    assert_eq!((vue.etat, vue.en_cours), (0, false));
    assert_eq!(vue.verification, "");
    assert_eq!(vue.probleme, "");
    assert!(!vue.disponible());
    assert!(c.vue_reglages().verifier_mises_a_jour, "actif par défaut");
    assert!(c.verification_automatique_due(T0));
}

#[test]
fn la_requete_est_celle_de_la_doc_github_sans_rien_de_personnel() {
    let dossier = TempDir::new().unwrap();
    let mut c = ouvrir(&dossier, T0);
    let client = ClientFactice::nouveau();
    client.repondre(publiee("v0.1.0"));
    assert!(verifier(&mut c, &client, true, T0));
    let requetes = client.requetes.lock().unwrap();
    assert_eq!(requetes.len(), 1);
    let (url, entetes) = &requetes[0];
    assert_eq!(
        url,
        "https://api.github.com/repos/alexisdechiara/retro-eq/releases/latest"
    );
    let entete = |nom: &str| {
        entetes
            .iter()
            .find(|(n, _)| n == nom)
            .map(|(_, v)| v.as_str())
    };
    assert_eq!(entete("Accept"), Some("application/vnd.github+json"));
    assert_eq!(
        entete("User-Agent"),
        Some(concat!("retro-eq/", env!("CARGO_PKG_VERSION")))
    );
    // Pas d'identifiant, de jeton ni de cookie.
    assert!(entetes
        .iter()
        .all(|(n, _)| !matches!(n.as_str(), "Authorization" | "Cookie")));
}

#[test]
fn une_mise_a_jour_disponible_arrive_dans_la_vue_avec_tous_ses_details() {
    let dossier = TempDir::new().unwrap();
    let mut c = ouvrir(&dossier, T0);
    let client = ClientFactice::nouveau();
    client.repondre(publiee("v0.2.0"));
    verifier(&mut c, &client, true, T0);
    let vue = c.vue_mise_a_jour(T0);
    assert_eq!(vue.etat, 2);
    assert!(vue.disponible());
    assert_eq!(vue.version, "0.2.0");
    assert_eq!(vue.date, "2 janvier 2026");
    assert_eq!(vue.notes, "Notes de v0.2.0");
    assert_eq!(
        vue.page,
        "https://github.com/alexisdechiara/retro-eq/releases/tag/v0.2.0"
    );
    assert_eq!(
        vue.installateur,
        "Retro-EQ-Installateur-v0.2.0.exe · 11,8 Mo"
    );
    assert_eq!(vue.verification, "vérifié aujourd’hui à 21:40");
    assert_eq!(vue.probleme, "");
}

#[test]
fn les_textes_suivent_la_langue() {
    let dossier = TempDir::new().unwrap();
    let mut c = ouvrir(&dossier, T0);
    c.choisir_langue("en");
    let client = ClientFactice::nouveau();
    client.repondre(publiee("v0.2.0"));
    verifier(&mut c, &client, true, T0);
    let vue = c.vue_mise_a_jour(T0);
    assert_eq!(vue.date, "2 January 2026");
    assert_eq!(vue.verification, "checked today at 21:40");
    assert_eq!(
        vue.installateur,
        "Retro-EQ-Installateur-v0.2.0.exe · 11.8 MB"
    );
}

#[test]
fn a_jour_et_build_local_en_avance_arrivent_dans_la_vue() {
    for tag in ["v0.1.0", "v0.0.9"] {
        let dossier = TempDir::new().unwrap();
        let mut c = ouvrir(&dossier, T0);
        let client = ClientFactice::nouveau();
        client.repondre(publiee(tag));
        verifier(&mut c, &client, true, T0);
        let vue = c.vue_mise_a_jour(T0);
        assert_eq!(vue.etat, 1, "{tag}");
        assert!(!vue.disponible());
        assert_eq!(vue.verification, "vérifié aujourd’hui à 21:40");
    }
}

#[test]
fn aucune_version_publiee_est_calme_et_compte_comme_une_verification() {
    let dossier = TempDir::new().unwrap();
    let mut c = ouvrir(&dossier, T0);
    let client = ClientFactice::nouveau();
    client.repondre(reponse(404, &[], r#"{"message":"Not Found"}"#));
    verifier(&mut c, &client, true, T0);
    let vue = c.vue_mise_a_jour(T0);
    assert_eq!(vue.etat, 3);
    assert_eq!(vue.probleme, "", "ce n'est pas une panne");
    assert_eq!(vue.verification, "vérifié aujourd’hui à 21:40");
    assert!(c.annonce.is_none(), "aucune annonce d'erreur");
    // Une version brouillon ou une pré-version ne changent rien à ce constat.
    let mut c = ouvrir(&TempDir::new().unwrap(), T0);
    client.repondre(reponse(
        200,
        &[],
        r#"{"tag_name":"v9.0.0","draft":true,"prerelease":false}"#,
    ));
    verifier(&mut c, &client, true, T0);
    assert_eq!(c.vue_mise_a_jour(T0).etat, 3);
    client.repondre(reponse(
        200,
        &[],
        r#"{"tag_name":"v9.0.0-beta.1","draft":false,"prerelease":true}"#,
    ));
    verifier(&mut c, &client, false, T0);
    assert_eq!(c.vue_mise_a_jour(T0).etat, 3);
}

#[test]
fn une_verification_par_jour_au_plus_meme_apres_un_redemarrage() {
    let dossier = TempDir::new().unwrap();
    let client = ClientFactice::nouveau();
    client.repondre(publiee("v0.1.0"));
    client.repondre(publiee("v0.1.0"));
    {
        let mut c = ouvrir(&dossier, T0);
        assert!(verifier(&mut c, &client, true, T0));
        // Dans la journée : rien.
        assert!(!verifier(&mut c, &client, true, T0 + 1_000));
        assert!(!verifier(&mut c, &client, true, T0 + JOUR - 1));
    }
    // L'appli redémarre dans la journée : le fichier se souvient.
    let mut c = ouvrir(&dossier, T0 + 3_600_000);
    assert!(!verifier(&mut c, &client, true, T0 + 3_600_000));
    assert_eq!(c.vue_mise_a_jour(T0 + 3_600_000).etat, 1);
    assert_eq!(client.nombre_de_requetes(), 1);
    // Le lendemain, la vérification repart.
    assert!(verifier(&mut c, &client, true, T0 + JOUR));
    assert_eq!(client.nombre_de_requetes(), 2);
    // Et la date affichée change de forme : plus « aujourd’hui ».
    let vue = c.vue_mise_a_jour(T0 + 2 * JOUR);
    assert_eq!(vue.verification, "vérifié le 25/11 à 21:40");
}

#[test]
fn le_bouton_verifier_maintenant_passe_toujours_mais_pas_deux_a_la_fois() {
    let dossier = TempDir::new().unwrap();
    let mut c = ouvrir(&dossier, T0);
    let client = ClientFactice::nouveau();
    client.repondre(publiee("v0.1.0"));
    client.repondre(publiee("v0.2.0"));
    assert!(verifier(&mut c, &client, true, T0));
    // Une minute plus tard, à la demande de l'utilisateur.
    assert!(verifier(&mut c, &client, false, T0 + 60_000));
    assert_eq!(c.vue_mise_a_jour(T0 + 60_000).etat, 2);
    // Une demande en vol : ni une autre manuelle, ni une automatique.
    let demande = c.demander_verification(false, T0 + 120_000).unwrap();
    assert!(c.vue_mise_a_jour(T0 + 120_000).en_cours);
    assert!(c.demander_verification(false, T0 + 120_001).is_none());
    assert!(c.demander_verification(true, T0 + 3 * JOUR).is_none());
    c.recevoir_verification(demande.envoyer(&client), T0 + 125_000);
    assert!(!c.vue_mise_a_jour(T0 + 125_000).en_cours);
}

#[test]
fn reglage_coupe_aucune_requete_toute_seule() {
    let dossier = TempDir::new().unwrap();
    let client = ClientFactice::nouveau();
    {
        let mut c = ouvrir(&dossier, T0);
        c.regler_verifier_mises_a_jour(false);
        assert!(!c.vue_reglages().verifier_mises_a_jour);
        for jours in [0, 1, 2, 30, 400] {
            assert!(!c.verification_automatique_due(T0 + jours * JOUR));
            assert!(!verifier(&mut c, &client, true, T0 + jours * JOUR));
        }
    }
    // Le choix survit au redémarrage.
    let mut c = ouvrir(&dossier, T0 + 5 * JOUR);
    assert!(!c.vue_reglages().verifier_mises_a_jour);
    assert!(!verifier(&mut c, &client, true, T0 + 5 * JOUR));
    assert_eq!(client.nombre_de_requetes(), 0, "zéro requête");
    // Mais le bouton « Vérifier maintenant » reste à la disposition de l'utilisateur.
    client.repondre(publiee("v0.1.0"));
    assert!(verifier(&mut c, &client, false, T0 + 5 * JOUR));
    assert_eq!(client.nombre_de_requetes(), 1);
    // Rallumé, la vérification automatique reprend sa règle d'un jour.
    c.regler_verifier_mises_a_jour(true);
    assert!(!c.verification_automatique_due(T0 + 5 * JOUR + 1));
    assert!(c.verification_automatique_due(T0 + 6 * JOUR + 1));
}

#[test]
fn une_erreur_reseau_laisse_l_etat_connu_visible() {
    let dossier = TempDir::new().unwrap();
    let mut c = ouvrir(&dossier, T0);
    let client = ClientFactice::nouveau();
    client.repondre(publiee("v0.2.0"));
    client.repondre(Err(ErreurHttp::Connexion("dns error".into())));
    verifier(&mut c, &client, true, T0);
    let avant = c.vue_mise_a_jour(T0);
    assert_eq!(avant.etat, 2);

    // Le lendemain, le réseau est coupé.
    let t = T0 + JOUR;
    assert!(verifier(&mut c, &client, true, t));
    let apres = c.vue_mise_a_jour(t);
    assert_eq!(apres.etat, 2, "la mise à jour connue reste affichée");
    assert_eq!(apres.version, "0.2.0");
    assert_eq!(apres.page, avant.page);
    assert!(apres.probleme.contains("GitHub"), "{}", apres.probleme);
    assert!(!apres.en_cours);
    // La date de dernière vérification réussie n'a pas bougé.
    assert_eq!(apres.verification, "vérifié le 24/11 à 21:40");
    assert!(c.annonce.is_none(), "pas d'alarme");
    // Hors ligne, rien n'est noté : le lancement suivant réessaie.
    assert!(c.verification_automatique_due(t + 1));
    // Et la vérification suivante, réussie, efface le problème.
    client.repondre(publiee("v0.2.0"));
    assert!(verifier(&mut c, &client, true, t + 1));
    assert_eq!(c.vue_mise_a_jour(t + 1).probleme, "");
}

#[test]
fn une_erreur_au_tout_premier_essai_laisse_pas_encore_verifie() {
    let dossier = TempDir::new().unwrap();
    let mut c = ouvrir(&dossier, T0);
    let client = ClientFactice::nouveau();
    client.repondre(Err(ErreurHttp::Connexion("timeout".into())));
    verifier(&mut c, &client, true, T0);
    let vue = c.vue_mise_a_jour(T0);
    assert_eq!(vue.etat, 0);
    assert_eq!(vue.verification, "");
    assert!(!vue.probleme.is_empty());
}

#[test]
fn limite_de_requetes_et_pannes_sont_dites_calmement_et_pas_reessayees_a_chaque_lancement() {
    let cas: [(Result<ReponseHttp, ErreurHttp>, &str); 4] = [
        (
            reponse(403, &[("x-ratelimit-remaining", "0")], "{}"),
            "trop de demandes",
        ),
        (reponse(429, &[], "{}"), "trop de demandes"),
        (reponse(503, &[], "<html>"), "(503)"),
        (reponse(200, &[], "pas du json"), "ne comprend pas"),
    ];
    for (reponse, attendu) in cas {
        let dossier = TempDir::new().unwrap();
        let mut c = ouvrir(&dossier, T0);
        let client = ClientFactice::nouveau();
        client.repondre(reponse);
        verifier(&mut c, &client, true, T0);
        let vue = c.vue_mise_a_jour(T0);
        assert!(
            vue.probleme.contains(attendu),
            "{attendu} : {}",
            vue.probleme
        );
        assert_eq!(vue.etat, 0);
        assert!(!c.annonce.as_ref().is_some_and(|a| a.erreur));
        // GitHub a répondu : pas de nouvelle tentative automatique avant demain,
        // même après un redémarrage.
        let mut c = ouvrir(&dossier, T0 + 60_000);
        assert!(!verifier(&mut c, &client, true, T0 + 60_000));
        assert!(c.verification_automatique_due(T0 + JOUR));
    }
}

#[test]
fn un_echec_n_efface_pas_une_mise_a_jour_connue() {
    let dossier = TempDir::new().unwrap();
    let mut c = ouvrir(&dossier, T0);
    let client = ClientFactice::nouveau();
    client.repondre(publiee("v0.2.0"));
    client.repondre(reponse(
        403,
        &[("x-ratelimit-remaining", "0")],
        r#"{"message":"API rate limit exceeded"}"#,
    ));
    verifier(&mut c, &client, true, T0);
    verifier(&mut c, &client, false, T0 + 1_000);
    let vue = c.vue_mise_a_jour(T0 + 1_000);
    assert_eq!(vue.etat, 2);
    assert!(vue.probleme.contains("trop de demandes"));
    // Redémarrage : la mise à jour est toujours là, sans reproche.
    let c = ouvrir(&dossier, T0 + 2_000);
    let vue = c.vue_mise_a_jour(T0 + 2_000);
    assert_eq!((vue.etat, vue.version.as_str()), (2, "0.2.0"));
    assert_eq!(vue.probleme, "");
}

#[test]
fn une_mise_a_jour_deja_installee_n_est_plus_proposee_au_lancement() {
    let dossier = TempDir::new().unwrap();
    {
        let mut c = ouvrir(&dossier, T0);
        let client = ClientFactice::nouveau();
        client.repondre(publiee("v0.2.0"));
        verifier(&mut c, &client, true, T0);
        assert_eq!(c.vue_mise_a_jour(T0).etat, 2);
    }
    // L'utilisateur installe la 0.2.0 (ici : la version qui tourne devient 0.2.0).
    let depot = Depot::ouvrir(dossier.path().join("donnees")).unwrap();
    let sauve = depot.charger_mises_a_jour().unwrap();
    let mut c = ouvrir(&dossier, T0 + 1_000);
    c.maj.version_courante = retro_core::mises_a_jour::Version::analyser("0.2.0").unwrap();
    c.maj.sauve = sauve;
    c.charger_mises_a_jour();
    assert_eq!(c.vue_mise_a_jour(T0 + 1_000).etat, 1);
}

#[test]
fn un_fichier_de_mises_a_jour_abime_ne_gene_pas_le_demarrage() {
    let dossier = TempDir::new().unwrap();
    let donnees = dossier.path().join("donnees");
    std::fs::create_dir_all(&donnees).unwrap();
    std::fs::write(donnees.join("mises-a-jour.json"), "{ pas du json").unwrap();
    let c = ouvrir(&dossier, T0);
    assert_eq!(c.vue_mise_a_jour(T0).etat, 0);
    assert!(c.verification_automatique_due(T0));
    // Un vieux fichier réduit à `{}` se charge aussi.
    std::fs::write(donnees.join("mises-a-jour.json"), "{}").unwrap();
    assert_eq!(ouvrir(&dossier, T0).vue_mise_a_jour(T0).etat, 0);
}

#[test]
fn le_lien_de_repli_est_la_page_des_versions() {
    let dossier = TempDir::new().unwrap();
    let mut c = ouvrir(&dossier, T0);
    let client = ClientFactice::nouveau();
    client.repondre(reponse(200, &[], r#"{"tag_name":"v0.2.0"}"#));
    verifier(&mut c, &client, true, T0);
    let vue = c.vue_mise_a_jour(T0);
    assert_eq!(vue.page, PAGE_VERSIONS);
    assert_eq!((vue.date.as_str(), vue.installateur.as_str()), ("", ""));
}
