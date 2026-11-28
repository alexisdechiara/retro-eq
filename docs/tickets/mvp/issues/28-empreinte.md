# Empreinte : préférences par activité

Status: resolved
Type: task

PRODUCT.md : « Chaque passage note l'appli au premier plan : l'Empreinte montre les préférences par activité (jeu, vidéo, Discord). »

Trois manques :

- Le premier passage de chaque test n'enregistre aucune appli : elle n'est relevée qu'une fois par seconde, après la création du test.
- Retro EQ lui-même n'est pas écarté des applis relevées.
- Il n'existe qu'une liste par test sur l'écran Résultat. L'Empreinte regroupe les réponses de tous les tests d'un profil par appli.

## Comments

Fait (worktree des tickets 26 à 28).

- **Premier passage** : le pont relève l'appli au premier plan chaque seconde, test ou pas ; le contrôleur garde la dernière connue et la donne à chaque test dès sa création (et à un test repris). Le premier passage la note donc.
- **Retro EQ écarté** : `retro_audio::appli_a_noter` rend « aucune appli » quand la fenêtre au premier plan appartient au processus de Retro EQ (fenêtre principale ou mini-fenêtre) ; l'implémentation Windows l'appelle avec `std::process::id()`. Un passage passé dans Retro EQ compte comme « appli inconnue » et ne prend pas la place de l'activité.
- **Empreinte** : une carte « Empreinte de « profil » » dans **Mes profils**, sous les versions du profil ouvert (c'est là que la maquette « Mon empreinte sonore » la range, section « Selon ce que tu faisais »). Une ligne par appli : nombre de duels, au plus trois tendances (« Plus de présence · 3 sur 3 ») et la confiance de la plus nette.
- **Règles** (`retro-core/src/empreinte.rs`, `duels_pour_empreinte`) : seuls comptent les duels **sur une zone**, répondus, avec une appli connue, dans les tests **terminés** dont les duels de contrôle n'ont pas tout bloqué. Contrôles, vérifications, duels contre un autre profil ou une autre version, tests abandonnés : ignorés. Une tendance est un couple (zone, sens) où la variante a gagné plus souvent que la référence ; sa confiance se calcule comme sur l'écran Résultat (victoires sur duels tranchés).
- Les duels sont relus dans les journaux avec les règles de la reprise (nouveaux tirages, annulations), sans nouveau fichier.
- Pas fait (hors ticket) : regrouper les exécutables par activité (jeu, vidéo, Discord) et « Créer un profil pour cette appli » de la maquette.
