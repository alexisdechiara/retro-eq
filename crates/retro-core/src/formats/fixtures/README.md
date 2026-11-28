# Fichiers d'essai du module `formats`

Ces fichiers `.peace` ont été écrits à la main pour Retro EQ, d'après la structure observée
sur des préréglages Peace publiquement disponibles. Aucun fichier de tiers n'est inclus :
toutes les valeurs (fréquences, gains, Q, descriptions) sont inventées pour ce projet.
Seuls la structure et les noms de clés du format sont repris.

| Fichier | Fins de ligne | Particularité du format couverte |
|---|---|---|
| `peace_exemple_minimal.peace` | LF | Un seul haut-parleur, 4 bandes, pas de `[General]` donc pas de préampli ; `Filter2=2` (passe-haut) et `Filter4=3` (passe-bande), types non gérés. |
| `peace_exemple_bandes_eteintes.peace` | CRLF | 9 bandes dont 6 listées dans `[Disabled]` ; `[General]` avec `Description`, `Configuration Icon`, `PreAmp` et `Off4` ; sections numérotées par haut-parleur (`[Frequencies1]`…`[Qualities8]`) ; `[Speakers]` en fin de fichier. |
| `peace_exemple_plateaux.peace` | LF | `[Speakers]` en tête, 10 bandes toutes avec un gain ; `Filter2=4` (plateau bas) et `Filter3=5` (plateau haut) ; pas de `[General]` ; fichier terminé par `HotKey` sans `=` ni fin de ligne. |
| `peace_exemple_plateau_bas.peace` | CRLF | Préampli négatif, grille de 13 bandes dont une seule a un `Gain` (les 12 autres sont à plat, sans `Gain`) ; `Filter8=4` ; `HotKey=` vide. |
| `peace_exemple_pente.peace` | CRLF | Codes 8 et 9 (plateaux réglés par une pente en dB, sans Q utile) ; sections par haut-parleur ; `[General]` avec `Description` et `PreAmp` avant `[Speakers]`. |
| `peace_exemple_graphic_eq_vide.peace` | CRLF | Mode `GraphicEQ=1` (convolution) : des fréquences et des Q mais aucune section `[Gains]`, donc rien d'utilisable. |

La table des codes de type (`Filter<n>=<code>`) est décrite dans l'en-tête de `../peace.rs`.
