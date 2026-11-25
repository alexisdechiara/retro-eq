# Traductions de l'interface

Le **français est la langue de référence** : les textes sont écrits en français dans les
fichiers `.slint`, derrière `@tr("…")`. L'anglais est une traduction, dans
`en/LC_MESSAGES/retro-app.po` (format gettext). Slint la compile dans l'appli
(`build.rs`, `with_bundled_translations("lang")`) : aucun fichier à livrer à côté.

Le nom du fichier, `retro-app`, est celui de la crate. Le dossier `en` est le code de la
langue, celui que `slint::select_bundled_translation` reçoit. Le français, lui, se choisit
avec une chaîne vide : c'est la langue « d'origine » de Slint, celle des `.slint`.

## Quand un texte change ou qu'on en ajoute un

1. Écrire (ou modifier) le texte en français dans le `.slint`, toujours avec `@tr`.
2. `cargo test -p retro-app traductions` échoue et affiche les entrées `.po` à ajouter,
   prêtes à copier.
3. Les coller dans `en/LC_MESSAGES/retro-app.po` et traduire. Retirer l'ancienne entrée
   si le français a changé (le test le signale aussi).
4. Relancer le test. Il vérifie aussi que la traduction garde les mêmes `{}` / `{n}` que le
   français.

Pour extraire proprement tous les textes (utile si beaucoup ont changé) :
`scripts/traductions.sh`. Il lance l'extracteur officiel (`cargo install
slint-tr-extractor --version '^1.19'`, un outil à part, pas une dépendance de l'appli) et,
si `msgmerge` est installé, met le `.po` à jour avec les phrases nouvelles, vides à remplir.

## Règles

- Pas de contexte par défaut (`--no-default-translation-context` dans l'extracteur,
  `DefaultTranslationContext::None` dans `build.rs`) : la clé d'une phrase est son texte
  français, quel que soit le composant. Un texte répété dans deux écrans n'a qu'une entrée.
- Un contexte explicite, `@tr("séparateur décimal" => ",")`, devient `msgctxt`. Celui-ci se
  traduit par `"."` en anglais.
- Un pluriel, `@tr("{n} version" | "{n} versions" % n)`, devient `msgid_plural` avec deux
  `msgstr[…]` (l'en-tête du `.po` dit `nplurals=2; plural=(n != 1);`).
- Vocabulaire à garder partout : passage = *pass*, duel = *duel*, test = *test*, profil =
  *profile*, version = *version*, référence = *reference*, variante = *variant*, zone =
  *zone*, bande = *band*, sortie audio = *audio output*, moteur = *engine*, duel de contrôle =
  *control duel*, Indice / Probable / Sûr = *Hint* / *Likely* / *Sure*, Pareil = *Same*.
  Orthographe britannique.

## Les phrases fabriquées par Rust

Celles qui dépendent de chiffres, de noms ou d'erreurs (« 2 zones changent », annonces,
journal d'un test) ne sont pas dans les `.slint` : elles sont dans `src/textes.rs`, une
fonction par phrase, avec un `match` sur `Langue`. Pas de `.po` pour celles-là.

## Ajouter une langue

Une variante de `retro_core::Langue`, un dossier `lang/<code>/LC_MESSAGES/retro-app.po`, et
le compilateur montre chaque `match` de `textes.rs` à compléter. Le test sur le `.po` est
écrit pour l'anglais : l'élargir à la nouvelle langue.
