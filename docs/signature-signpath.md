# Signer Retro EQ avec SignPath Foundation

Sans signature, Windows SmartScreen affiche « Windows a protégé votre ordinateur » à l'ouverture de l'installateur. La SignPath Foundation signe gratuitement les projets libres. **La demande est à faire par Alexis lui-même** : elle engage son nom et son dépôt, et l'équipe de SignPath lui écrira. Rien n'est signé aujourd'hui ; ce document dit quoi faire et où la signature se branchera.

Faits relevés le 25/11/2026 sur les pages officielles. Ce qui n'a pas pu être vérifié est marqué **[non vérifié]**.

## Ce que SignPath Foundation demande

D'après les conditions officielles (<https://signpath.org/terms>, page marquée « brouillon » au moment de la lecture) :

- Licence libre approuvée par l'OSI, sans double licence commerciale, aucun composant propriétaire. La GPLv3 de Retro EQ convient. Les dépendances de `Cargo.toml` sont MIT, Apache 2.0, BSD ou GPL (règle du projet). Equalizer APO (GPLv2) n'est pas dans nos binaires.
- Projet **maintenu** et **déjà publié** sous la forme à signer : il faut donc une première publication (non signée) avant de demander.
- Fonctionnalités **décrites sur la page de téléchargement** (le README ou la page des publications).
- On ne signe que **ses propres binaires, construits depuis ses propres sources**, par un build vérifiable et automatisé : c'est le rôle de `release.yml`. Chaque publication demande une approbation manuelle dans SignPath.
- Pas d'outil de piratage ; respect de la vie privée ; **les changements de configuration du système doivent être annoncés** (Retro EQ le fait : accord demandé au premier lancement, désinstalleur qui explique) ; **une désinstallation doit exister** (c'est l'objet de l'installateur).
- Authentification à deux facteurs pour SignPath et pour le dépôt, pour toute l'équipe.
- Rôles de signature définis : auteurs, relecteurs, approbateurs (pour un projet à une personne, Alexis tient les trois rôles : à indiquer tel quel).
- Le **site ou le dépôt doit avoir une section « Code signing policy »** (ce terme exact, sur l'accueil et sur la page de téléchargement), contenant : la phrase « Free code signing provided by SignPath.io, certificate by SignPath Foundation », les rôles et leurs membres, et une politique de confidentialité ou un lien vers elle. À écrire dans le `README.md` du dépôt avant de postuler.
- Métadonnées des binaires signés : le nom du produit doit être celui du projet et la version identique à chaque build. Les fichiers `.exe` doivent donc porter ces informations (nom de produit « Retro EQ », version de `Cargo.toml`) **[non vérifié : Retro EQ n'écrit pas encore ces métadonnées dans l'exécutable ; il faudra les ajouter, par exemple avec une ressource Windows dans `build.rs`]**.
- Un dépôt public n'est pas cité dans les conditions lues, mais le ticket 21 le suppose et la vérification « construit depuis le dépôt » le rend pratiquement nécessaire **[non vérifié]**.

La page d'inscription (<https://signpath.org/apply>) n'a pas pu être lue en détail (son contenu ne s'est pas affiché) : les champs du formulaire et le délai de réponse sont **[non vérifiés]**.

## Ce qu'Alexis doit faire

1. Publier la première version (étiquette `v0.1.0`, `release.yml` fait un brouillon : le relire et le publier).
2. Ajouter au `README.md` la section « Code signing policy » décrite ci-dessus, et une page de téléchargement qui décrit l'appli.
3. Activer l'authentification à deux facteurs sur son compte GitHub.
4. Déposer la demande : <https://signpath.org/apply>. Pages utiles : conditions <https://signpath.org/terms>, présentation <https://signpath.org/>.
5. Une fois accepté, dans SignPath (documentation : <https://docs.signpath.io/trusted-build-systems/github>) :
   - ajouter le système de build de confiance « GitHub.com » à l'organisation, et le lier au projet ;
   - installer l'application GitHub de SignPath sur le dépôt ;
   - créer la configuration d'artefact (racine `<zip-file>` : l'artefact envoyé par `actions/upload-artifact` est un zip) et une politique de signature, avec approbation manuelle ;
   - créer un jeton d'API (utilisateur ayant le droit de soumettre) et l'enregistrer dans les secrets du dépôt sous `SIGNPATH_API_TOKEN` ;
   - noter l'identifiant d'organisation, le « slug » du projet et celui de la politique.

## Où la signature se branche dans `release.yml`

Le fichier `.github/workflows/release.yml` contient un commentaire à l'endroit voulu : après `cargo build` et avant `installateur/construire.ps1`. Il y a deux signatures à faire, dans cet ordre :

1. `retro-eq.exe` (envoyer, recevoir le fichier signé, le remettre à `target/release/`) ;
2. puis l'installateur, une fois construit (`installateur/sortie/*.exe`), avant de calculer les empreintes SHA-256.

Chaque envoi suit le modèle de la documentation SignPath (versions des actions telles qu'écrites dans cette documentation à la date de lecture, à revérifier) :

```yaml
- name: Envoyer le fichier à signer
  id: a-signer
  uses: actions/upload-artifact@v7
  with:
    name: a-signer
    path: target/release/retro-eq.exe

- name: Demander la signature
  uses: signpath/github-action-submit-signing-request@v3
  with:
    api-token: '${{ secrets.SIGNPATH_API_TOKEN }}'
    organization-id: '<identifiant de l’organisation SignPath>'
    project-slug: '<slug du projet>'
    signing-policy-slug: '<slug de la politique>'
    github-artifact-id: '${{ steps.a-signer.outputs.artifact-id }}'
    wait-for-completion: true
    output-artifact-directory: 'signe'
```

Le fichier signé arrive dans `signe/` ; une étape `Copy-Item` le remet à sa place. Le job aura sans doute besoin de `actions: read` en plus de `contents: write` **[non vérifié : la documentation ne le demande que pour un dépôt privé sans l'application GitHub de SignPath]**.

Comme chaque signature attend une approbation manuelle, le job de publication restera en attente jusqu'à ce qu'Alexis approuve dans SignPath.
