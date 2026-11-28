# Interface en Slint, sans WebView ni HTML

Alexis vient du web (Nuxt, Tailwind) et la maquette est en HTML, donc Tauri ou une WebView auraient été le chemin évident. On choisit pourtant une appli 100 % Rust avec une interface Slint : pas de moteur web à embarquer ni à garder en mémoire à côté d'un jeu, un langage d'interface déclaratif avec aperçu en direct, et un seul langage du moteur audio jusqu'à l'écran. La maquette HTML reste une référence visuelle et n'est pas réutilisée comme code.

## Options écartées

- **Tauri ou WebView** : aurait permis de reprendre la maquette, écarté par choix du tout-Rust.
- **iced** : gardé comme plan B si Slint bloque (rendu des appareils dessinés, fenêtre sans bordure), mais encore expérimental avant sa version 1.0.

## Conséquences

- Slint est utilisé sous sa licence GPLv3, ce qui impose que Retro EQ soit lui-même sous GPLv3 (voir ADR-0003).
- Les textes passent par `@tr(...)`, français en référence et anglais traduit.
