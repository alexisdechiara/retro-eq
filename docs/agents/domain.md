# Documentation de domaine

Dépôt à contexte unique.

## À lire avant d'explorer

- **`GLOSSARY.md`** à la racine : le vocabulaire du produit.
- **`docs/adr/`** : les décisions qui touchent la zone de travail.
- **`PRODUCT.md`** à la racine : la source de vérité sur le produit.

Si un fichier manque, continuer sans le signaler. `/domain-modeling` (via `/grill-with-docs`) les crée quand un terme ou une décision est tranché.

## Employer le vocabulaire du glossaire

Tout nom de concept (titre de ticket, nom de test, type Rust, texte d'interface) reprend le terme de `GLOSSARY.md`, jamais un synonyme listé sous « À éviter ». Un concept absent du glossaire est un signal : soit on invente un mot que le projet n'emploie pas, soit il y a un vrai manque à noter pour `/domain-modeling`.

Les identifiants Rust du domaine sont en français, comme le glossaire (`Duel`, `Passage`, `Moteur`). Le reste du code suit les conventions Rust habituelles.

## Signaler les conflits avec un ADR

Si une proposition contredit un ADR, le dire explicitement plutôt que de passer outre :

> _Contredit l'ADR-0002 (Equalizer APO derrière un trait), mais mérite d'être rouvert parce que…_
