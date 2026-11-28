# Suivi des tickets : fichiers Markdown locaux

Les specs et les tickets de ce dépôt vivent dans `docs/tickets/`. Pas de GitHub Issues pour l'instant.

## Conventions

- Un dossier par fonctionnalité ou par chantier : `docs/tickets/<slug>/`
- La spec est `docs/tickets/<slug>/spec.md`
- Un fichier par ticket : `docs/tickets/<slug>/issues/<NN>-<slug>.md`, numérotés à partir de `01`. Jamais un seul fichier qui regroupe tous les tickets.
- L'état de tri est une ligne `Status:` en haut du fichier (voir `triage-labels.md`).
- Les dépendances sont une ligne `Blocked by: NN, NN` en haut du fichier. Un ticket est débloqué quand tous les tickets cités sont `resolved`.
- Les commentaires s'ajoutent en bas du fichier, sous un titre `## Comments`.
- Les tickets sont rédigés en français, avec le vocabulaire de `GLOSSARY.md`.

## Quand un skill dit « publier dans le suivi des tickets »

Créer un fichier sous `docs/tickets/<slug>/` (créer le dossier si besoin).

## Quand un skill dit « récupérer le ticket »

Lire le fichier au chemin indiqué. L'utilisateur donne en général le chemin ou le numéro.

## Carte des décisions (`/wayfinder`)

- **Carte** : `docs/tickets/<chantier>/map.md` (Notes, Décisions prises, Brouillard).
- **Ticket enfant** : `docs/tickets/<chantier>/issues/NN-<slug>.md`, avec la question dans le corps. Une ligne `Type:` donne le type (`research`, `prototype`, `grilling`, `task`), une ligne `Status:` donne `claimed` ou `resolved`.
- **Frontière** : les tickets ouverts, débloqués et non pris ; le plus petit numéro d'abord.
- **Prendre** : écrire `Status: claimed` et enregistrer avant tout travail.
- **Résoudre** : ajouter la réponse sous `## Answer`, écrire `Status: resolved`, puis ajouter un renvoi (résumé + lien) dans les Décisions prises de `map.md`.
