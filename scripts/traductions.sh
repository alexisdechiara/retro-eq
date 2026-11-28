#!/usr/bin/env bash
# Prépare la mise à jour de la traduction anglaise après un changement de texte dans les .slint.
# Usage : scripts/traductions.sh [fichier.pot]
#
# 1. extrait tous les @tr(...) des .slint dans un modèle (.pot, par défaut dans /tmp) ;
# 2. si `msgmerge` (gettext) est installé, fusionne ce modèle dans le .po anglais : les
#    phrases nouvelles ou changées y arrivent avec une traduction vide, à remplir ;
# 3. sinon, `cargo test -p retro-app traductions` liste les entrées à ajouter à la main.
# Voir crates/retro-app/lang/README.md.
set -euo pipefail
cd "$(dirname "$0")/../crates/retro-app"
pot="${1:-${TMPDIR:-/tmp}/retro-app.pot}"
po="lang/en/LC_MESSAGES/retro-app.po"

if ! command -v slint-tr-extractor >/dev/null; then
  echo "slint-tr-extractor manque : cargo install slint-tr-extractor --version '^1.19'" >&2
  exit 1
fi

# Sans contexte par défaut : c'est ce que build.rs demande aussi au compilateur Slint.
slint-tr-extractor --no-default-translation-context \
  --package-name retro-app --package-version "$(cargo pkgid | sed 's/.*[#@]//')" \
  -o "$pot" ui/*.slint ui/ecrans/*.slint
echo "Modèle écrit : $pot ($(grep -c '^msgid ' "$pot") entrées, en-tête compris)"

if command -v msgmerge >/dev/null; then
  msgmerge --update --backup=none --no-fuzzy-matching "$po" "$pot"
  echo "Fusionné dans $po : cherche les msgstr \"\" (à traduire) et les entrées #~ (à retirer)."
else
  echo "msgmerge absent : lance « cargo test -p retro-app traductions » pour voir ce qu'il faut ajouter à $po."
fi
