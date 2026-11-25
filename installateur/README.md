# Installateur de Retro EQ

Le script `retro-eq.iss` est écrit pour [Inno Setup](https://jrsoftware.org/isinfo.php) 6.3 ou plus (ADR-0009). `construire.ps1` le compile. L'intégration continue fait la même chose : à chaque push, le job Windows de `.github/workflows/ci.yml` produit l'artefact `retro-eq-installateur-x64` ; sur une étiquette `v*`, `.github/workflows/release.yml` en fait un brouillon de publication.

> **Rien de tout ceci n'a été exécuté sur un vrai PC.** Le script a été écrit sur Linux, d'après la documentation d'Inno Setup. Voir « Jamais essayé » plus bas.

## Le construire à la main, sous Windows

1. Installer Inno Setup 6.3+ (<https://jrsoftware.org/isdl.php>), ou `choco install innosetup`.
2. Compiler l'appli : `cargo build --release -p retro-app`.
3. Lancer `.\installateur\construire.ps1` depuis la racine du dépôt.

Le résultat est `installateur\sortie\retro-eq-<version>-installateur-x64.exe`. La version est lue dans `Cargo.toml` (`[workspace.package]`), seul endroit où elle s'écrit ; le script d'Inno la reçoit par `/DAppVersion=...`. Pour compiler sans PowerShell : `ISCC.exe /DAppVersion=0.1.0 installateur\retro-eq.iss`.

## Ce que fait l'installateur

1. Demande les droits d'administrateur (installation pour tous les utilisateurs, dans `C:\Program Files\Retro EQ`). Ils servent aussi à lancer l'installateur d'Equalizer APO et, plus tard, à retirer le moteur de sa configuration.
2. Refuse de s'installer hors Windows 10 22H2 ou plus récent, et hors 64 bits. Si Retro EQ est ouvert, demande de le fermer (mutex `RetroEQ.EnCours`, créé par l'appli).
3. Langue française ou anglaise, selon la langue de Windows. Page de licence : le `LICENSE` du dépôt (GPLv3).
4. Tâches proposées, toutes décochées : raccourci sur le Bureau, lancement au démarrage de Windows (raccourci dans le dossier Démarrage commun, l'appli s'ouvre alors avec sa fenêtre : l'appli n'a pas encore d'option « démarrer réduite »).
5. **Page « Moteur audio : Equalizer APO »**, seulement si `HKLM\SOFTWARE\EqualizerAPO` / `InstallPath` n'existe pas (ou pointe vers un dossier absent). Elle explique en clair ce qu'est Equalizer APO, qu'il est gratuit et fait par d'autres, que son installateur demandera sur quelle sortie agir et un redémarrage. Une case, décochée, propose de télécharger et lancer son installateur officiel. Sans la case cochée, rien n'est téléchargé. Si Equalizer APO est déjà là, la page n'apparaît pas et on n'y touche pas.
6. Au clic sur « Installer », si la case est cochée : téléchargement de `EqualizerAPO-x64-1.4.2.exe` depuis SourceForge (page de progression d'Inno), vérifié par son empreinte SHA-256 (un fichier différent est refusé). Un échec (réseau, empreinte) n'empêche pas l'installation de Retro EQ : on propose d'ouvrir la page officielle pour l'installer soi-même.
7. Copie des fichiers, raccourcis, puis lancement de l'installateur d'Equalizer APO **dans sa propre fenêtre, visible** (jamais en silencieux, jamais avec `/S`). Si Equalizer APO est bien présent ensuite, la valeur `HKLM\SOFTWARE\Retro EQ` / `EqualizerApoInstalleParRetroEq` = 1 est écrite : « installé par nous ».
8. Une installation silencieuse (`/SILENT`) n'affiche pas la page : jamais de téléchargement ni d'installateur lancé sans que quelqu'un ait coché la case.

### Pourquoi cette adresse pour Equalizer APO

Equalizer APO est sur SourceForge (projet `equalizerapo`). Le script utilise l'adresse **de la version** (`https://downloads.sourceforge.net/project/equalizerapo/1.4.2/EqualizerAPO-x64-1.4.2.exe`, qui redirige vers un miroir), pas « latest » : elle ne change pas quand une nouvelle version sort, et l'empreinte SHA-256 épinglée dans le script correspond à ce fichier. Vérifié le 25/11/2026 depuis Linux : l'adresse répond, le fichier fait 11 980 366 octets, c'est un installateur NSIS pour Windows, et son empreinte est `7403be74…0afa`. Cette empreinte est celle que nous avons calculée nous-mêmes à ce moment-là : SourceForge n'en publie pas pour ce fichier. Pour passer à une version plus récente : changer les trois constantes en tête de `[Code]`, recalculer l'empreinte, et vérifier la version.

## Ce que fait le désinstalleur (« Rendre le son d'origine »)

Lancé depuis Paramètres › Applications. Pour Inno Setup 6, un désinstalleur ne peut pas avoir de vraies pages ni de cases à cocher : les options de la maquette (`maquette/x-desinstaller.png`) sont donc posées sous forme de questions oui/non, dans cet ordre :

1. **Rendre le son d'origine** (obligatoire, pas de case) : explique que le moteur sera retiré des sorties audio, que le son peut se couper une seconde, et dit où sont les profils. Oui continue, Non annule.
2. **Retirer Equalizer APO**, posée seulement s'il a été installé par nous (valeur de registre ci-dessus) et qu'il est encore là. Jamais proposée s'il était là avant.
3. **Supprimer aussi mes réglages et l'historique des tests** : Non par défaut. Si Oui : une dernière question, « supprimer aussi mes profils ? », Non par défaut.

Puis, dans l'ordre :

- attend que Retro EQ soit fermé ;
- lance `retro-eq.exe --rendre-son-origine` (sans fenêtre : même chemin que le bouton de Réglages, `Moteur::retirer`). Code 0 = réussi. **Sinon, la désinstallation est annulée** et rien n'est supprimé : mieux vaut garder Retro EQ que laisser le moteur actif sans lui ;
- supprime les fichiers de Retro EQ ;
- supprime les données choisies, puis lance le désinstalleur d'Equalizer APO dans sa fenêtre, si demandé ;
- supprime la clé `HKLM\SOFTWARE\Retro EQ`.

Les **profils restent dans `%APPDATA%\Retro EQ\profils`** (et les exports dans `Documents\Retro EQ`, que le désinstalleur ne touche jamais). Il n'y a pas d'option `--exporter-profils` : les profils restant en place, elle n'a pas d'utilité ici.

En désinstallation silencieuse (`/SILENT`) : aucune question, le moteur est retiré, Equalizer APO et toutes les données sont gardés ; si Retro EQ est ouvert ou si le moteur ne se retire pas, la désinstallation est annulée.

## Jamais essayé sur une vraie machine

- Compilation du script par Inno Setup : sera prouvée par le premier passage du job Windows de la CI (le script est écrit pour 6.3+ ; la CI installe la dernière version par Chocolatey, ce qui peut révéler un changement de syntaxe).
- Présence de `ISCC.exe` sur `windows-latest` : inconnue ; `construire.ps1` le cherche et sinon l'installe avec Chocolatey.
- La langue française d'Inno (`compiler:Languages\French.isl`) : supposée fournie avec Inno Setup.
- `MinVersion=10.0.19045` : le numéro de build de Windows 10 22H2 est confirmé par la documentation d'Inno, mais pas l'écriture avec build pour Windows 10 ; la compilation le dira.
- Le téléchargement par la page d'Inno depuis SourceForge (redirections vers un miroir) : l'adresse a été testée avec `curl`, pas avec Inno.
- La page « Equalizer APO » : son aspect (retours à la ligne, hauteur de la case) n'a jamais été vu.
- Le lancement de l'installateur d'Equalizer APO par un installateur élevé, et son code de sortie.
- Le chemin du désinstalleur d'Equalizer APO (`<InstallPath>\Uninstall.exe`) : supposé d'après l'usage de NSIS, jamais vérifié. S'il est absent, un message renvoie vers Paramètres Windows.
- Détection de Retro EQ ouvert par le mutex (`CheckForMutexes`), côté installateur et désinstalleur.
- `retro-eq.exe --rendre-son-origine` sous Windows avec un vrai Equalizer APO : testé seulement avec le moteur simulé (Linux) ; sous Windows la CI vérifie seulement qu'il rend 0 sur une machine sans Equalizer APO.
- Désinstallation par un compte standard qui saisit les identifiants d'un autre administrateur : `%APPDATA%` est alors celui de l'administrateur, pas de l'utilisateur. Cas non géré.
- Le raccourci du dossier Démarrage commun.
- Mise à jour par-dessus une version déjà installée.
- Aucune signature de code : Windows SmartScreen affichera un avertissement. Voir `docs/signature-signpath.md`.
