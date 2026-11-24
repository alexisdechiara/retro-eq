# Retro EQ

Appli de bureau Windows, 100 % Rust, interface Slint. Elle trouve le réglage d'égaliseur préféré à l'oreille, par duels A/B à l'aveugle pendant l'usage réel du PC.

## À lire d'abord

- `PRODUCT.md` : source de vérité sur le produit.
- `docs/BRIEF.md` : décisions techniques prises, règles produit à respecter dans le code, méthode de travail.
- `GLOSSARY.md` : vocabulaire du domaine. `docs/adr/` : décisions.
- `maquette/` : captures des écrans validés (style « Station Pilleur »). Référence visuelle seulement : la maquette est en HTML, l'appli est en Slint, on ne copie pas le code.

## Règles de travail

- Répondre à Alexis en français. Il est développeur front-end et designer, débutant en Rust : expliquer tout choix Rust qui n'est pas évident.
- Commits petits et clairs, en français.
- Licence GPLv3 : n'ajouter que des dépendances MIT, Apache 2.0, BSD ou GPL. Signaler toute autre licence avant de l'ajouter.
- Aucun texte d'interface en dur : tout passe par `@tr(...)`. Le français est la langue de référence, l'anglais est traduit.
- Ne jamais toucher à la configuration audio réelle du PC d'Alexis sans le prévenir. N'exécuter aucun installeur et ne modifier aucun réglage Windows sans son accord explicite.
- Logique pure (DSP, déroulement d'un test, confiance, import et export) : écrite en TDD, testable sans Windows.

## Où le code peut tourner

Les sessions cloud tournent sous Linux : on peut y écrire et tester `retro-dsp` et `retro-core`, pas ce qui touche à Windows (Equalizer APO, WASAPI, raccourcis globaux, barre des tâches, fenêtre sans bordure). Tout ce qui exige le PC Windows et le casque devient un ticket `ready-for-human` ou se fait dans une session locale sur le PC.

## Agent skills

### Issue tracker

Fichiers Markdown locaux dans `docs/tickets/`, pas de GitHub Issues. Voir `docs/agents/issue-tracker.md`.

### Triage labels

Les cinq rôles par défaut, écrits dans la ligne `Status:` de chaque ticket. Voir `docs/agents/triage-labels.md`.

### Domain docs

Contexte unique : `GLOSSARY.md` à la racine et `docs/adr/`. Voir `docs/agents/domain.md`.
