#!/usr/bin/env bash
# Capture un écran de l'appli dans un PNG, sans écran réel (Linux, Xvfb).
# Usage : scripts/capture.sh <ecran> <fichier.png> [hauteur] [options de l'appli, ex. --rail]
# Écrans : demarrage accueil tester nouveau-test duels resultat profils egaliseur visualiseur reglages diagnostic a-propos autoeq (Mes profils, panneau AutoEQ ouvert) mini (la mini-fenêtre seule)
# Alertes du test (écran duels et mini) : RETRO_EQ_DEMO_TEST=touches-mortes ou pause
set -euo pipefail
ecran="$1"; sortie="$2"; hauteur="${3:-800}"; shift; shift; [ $# -gt 0 ] && shift
cible="${CARGO_TARGET_DIR:-target}"
cargo build -q -p retro-app
SLINT_BACKEND=winit-femtovg xvfb-run -a -s "-screen 0 1600x$((hauteur + 100))x24" \
  "$cible/debug/retro-eq" --demo --ecran "$ecran" --hauteur "$hauteur" --capture "$sortie" "$@" 2> >(grep -v 'xdg desktop' >&2)
