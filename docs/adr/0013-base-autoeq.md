# La base AutoEQ : téléchargée à la demande, jamais embarquée

PRODUCT.md promet un point de départ « depuis AutoEQ (recherche du casque dans la base, gardée en local) ». La recherche (`docs/recherche-autoeq.md`) montre que le code d'AutoEQ est sous licence MIT, mais que le dépôt ne dit rien de la licence des **mesures** (oratory1990, crinacle, Rtings, Filk…) dont les réglages découlent. Rien n'autorise clairement à les redistribuer dans un dépôt GPLv3 ni dans l'installateur.

Décision : on n'embarque aucune donnée AutoEQ. L'appli les télécharge sur demande, depuis le dépôt d'AutoEQ, et les garde dans le dossier de données de l'utilisateur.

- **L'index** (`results/INDEX.md`, environ 0,85 Mo, 8 850 entrées, environ 0,11 Mo en gzip) est téléchargé à la première recherche, ou par « Mettre à jour la base ». Il est gardé dans `…/Retro EQ/autoeq/index.json`, dans notre propre format compact (nom, source, dossier plateforme/type). C'est la « base gardée en local ».
- **Un réglage** (`<Nom> ParametricEQ.txt`, environ 0,5 Ko) est téléchargé quand l'utilisateur choisit un résultat, et gardé dans `…/autoeq/fichiers/`. Il est alors importable hors ligne.
- **Hors ligne** : la recherche marche sur l'index gardé. Un casque jamais téléchargé ne peut pas être importé sans réseau, et l'appli le dit simplement.
- **Aucun choix silencieux** : un casque mesuré par plusieurs sources donne plusieurs lignes, chacune avec sa source et son banc de mesure.
- **Origine affichée** : le profil créé porte `Origine::Import { format: "autoeq|<source>|<plateforme>" }` ; l'interface et l'annonce disent « AutoEQ », la source et le banc. Le nom du profil ajoute la source entre parenthèses.
- **Honnêteté** (principe 5) : l'écran rappelle que les bancs diffèrent et que l'aigu au-delà d'environ 8 kHz est peu fiable.

## Ce qu'on n'utilise pas

L'API GitHub (arbre du dépôt, 60 appels par heure sans identifiant) : `INDEX.md` suffit, servi par `raw.githubusercontent.com`, sans limite documentée. Un seul client HTTP, `ureq`, avec des délais courts et un `User-Agent` générique `retro-eq/<version>`.

## Conséquences

- L'index et les fichiers viennent de la branche `master` d'AutoEQ : si le dépôt change de disposition, le téléchargement échoue calmement et l'utilisateur garde ce qu'il avait. Le format de l'index est lu de façon tolérante (une ligne illisible est sautée) et un index qui ne donne aucune entrée est refusé.
- Les actions réseau ne tournent jamais sur le fil de l'interface. La logique pure (lecture de l'index, recherche) est dans `retro_core::autoeq_base` et se teste sans réseau ; le réseau est derrière le trait `Telechargeur` (réel : `ureq`, tests : un faux).
- Dépendance ajoutée : `ureq` 3 (MIT ou Apache 2.0) avec `rustls` (Apache 2.0, ISC ou MIT ; `ring` : Apache 2.0 et ISC ; `webpki-roots` : CDLA-Permissive-2.0, des données de certificats). Cette dernière licence est à valider avant la livraison de l'installateur.
- Si un jour un accord clair d'AutoEQ ou des mesureurs le permet, une liste de casques courants pourrait être livrée ; ce n'est pas le cas aujourd'hui.
