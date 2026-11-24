# Installateur : Inno Setup

L'installateur est écrit avec Inno Setup : un seul script lisible, gratuit, capable de lancer l'installateur d'Equalizer APO puis notre désinstallation « rendre le son d'origine », et déjà utilisé par de nombreux projets signés par SignPath Foundation. WiX produit des MSI plus propres pour les entreprises mais demande beaucoup plus de XML pour le même résultat ; NSIS est équivalent, avec un langage de script moins lisible.

Equalizer APO (GPLv2) n'est pas mélangé à notre code : son installateur officiel est lancé par le nôtre. S'il était déjà installé, on le garde tel quel et la désinstallation ne le retire pas.

## À faire sur le PC

Rien de tout cela ne peut être essayé hors de Windows : le script est livré avec le dépôt et construit par l'intégration continue, mais sa première exécution réelle demande l'accord d'Alexis.
