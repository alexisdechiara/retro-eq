# Installateur Inno Setup et désinstallation qui rend le son d'origine

Status: ready-for-human
Type: task
Blocked by: 09, 15

ADR-0009. Installe aussi Equalizer APO. Signature par SignPath Foundation : demande à faire par Alexis (le projet doit être public).

## Comments

Écrit sans Windows : `installateur/retro-eq.iss`, `installateur/construire.ps1`, `--rendre-son-origine` (testé avec le moteur simulé), mutex `RetroEQ.EnCours`, CI et `release.yml`, `docs/signature-signpath.md`. Reste à faire par Alexis : voir « Jamais essayé sur une vraie machine » dans `installateur/README.md` (première compilation par la CI, première installation et désinstallation sur le PC, demande à SignPath).
