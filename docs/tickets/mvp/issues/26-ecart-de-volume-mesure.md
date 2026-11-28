# Vérifier la compensation du volume sur ce qui joue

Status: resolved
Type: task

PRODUCT.md : « compensation calculée et vérifiée sur ce qui joue ». ADR-0006 : la capture sert « à afficher l'écart mesuré ».

Aujourd'hui le spectre capté ne sert qu'au calcul. Cumuler le volume perçu mesuré pendant les passages de A et de B d'un duel, en déduire l'écart mesuré en dB et le garder avec le duel dans le journal. L'afficher sur l'écran Résultat et signaler un duel dont l'écart dépasse un seuil. Testable avec la capture simulée ; la validation réelle demande le PC (ticket 16).

## Comments

Fait (worktree des tickets 26 à 28).

- **Mesure** : une fois par seconde pendant un test, le pont donne au contrôleur le volume perçu de ce qui joue (niveau RMS + écart de pondération K du spectre : le même chiffre que le « volume » du visualiseur). Le déroulement le cumule pour le côté qui joue (`Evenement::VolumeMesure`), en moyenne de puissances comme une mesure LUFS intégrée (`retro_dsp::VolumeIntegre`). Les mesures prises moins de **2 s** après une bascule ne comptent pas (fondu, rechargement d'Equalizer APO, lissage de la mesure), le silence (sous −70 dB) non plus. Il faut au moins **3 mesures retenues de chaque côté** pour donner un écart.
- **Journal** : nouvelle entrée `ecart_volume_mesure { duel, ecart_db }` (variante moins référence, en dB), écrite juste après `reponse_donnee`. Rien pour un duel passé ou mal mesuré. Les anciens journaux se relisent : leurs duels n'ont pas d'écart et l'écran Résultat dit « pas mesuré ». La reprise après plantage retrouve les écarts.
- **Seuil : 2 dB** (`SEUIL_ECART_VOLUME_DB`, `retro-core/src/deroulement/clos.rs`). Plus haut que le seuil de perception (≈ 1 dB) parce que la mesure se fait sur ce qui joue et qu'en jeu le contenu change d'un passage à l'autre (ADR-0006) : sous 2 dB, l'écart peut venir du contenu seul. Valeur de départ, à revoir avec les vrais tests.
- **Écran Résultat** : sous la vérification, une phrase (écart moyen, maximum, nombre de duels au-delà du seuil), puis chaque duel signalé avec son écart en pastille rouge et ce qu'il veut dire (« la variante sonnait 2,6 dB plus fort que la référence »). `ResultatTest` n'est pas modifié : les écarts sont relus avec les duels clos (`duels_clos_du_journal`).
- À vérifier sur le PC : que le chiffre mesuré suit un changement de préampli connu, et que 2 s suffisent après une bascule.
