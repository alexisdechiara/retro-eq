# Stockage en fichiers JSON, avec un journal par test

Les données tiennent dans le dossier de l'utilisateur (`%APPDATA%\Retro EQ`) : un fichier JSON par profil avec toutes ses versions, un fichier de réglages, et pour chaque test un journal `.jsonl` où chaque événement (duel commencé, passage, réponse, pause) est ajouté sur une ligne et écrit sur le disque aussitôt. L'état d'un test se reconstruit en relisant son journal : c'est ce qui permet de garder chaque réponse dès l'appui sur la touche et de reprendre après un plantage. Les autres fichiers sont remplacés de façon atomique (écriture à côté, puis renommage).

SQLite a été écarté : il apporte une dépendance C à compiler et des requêtes dont on n'a pas besoin pour quelques profils et quelques centaines de réponses, et des fichiers texte restent lisibles et réparables à la main. À revoir si l'Empreinte demande un jour des croisements lourds sur des milliers de passages.

## Conséquences

- Un fichier `etat-moteur.json` dit quelle courbe devrait être active. Au démarrage, l'appli la réécrit toujours dans le moteur : une variante restée active après un arrêt brutal est ainsi remplacée par la référence.
- Rien n'est supprimé : une version n'est jamais modifiée, un test en ajoute une.
