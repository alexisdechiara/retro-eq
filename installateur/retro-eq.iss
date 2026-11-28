; Installateur de Retro EQ, pour Inno Setup 6.3 ou plus (https://jrsoftware.org/isinfo.php).
; Voir installateur/README.md pour le construire et pour ce qui n'a jamais été essayé.
;
; Pour un lecteur qui découvre Inno Setup : ce fichier se lit de haut en bas.
; - les sections [Entre crochets] décrivent l'installation (fichiers, raccourcis, tâches...) ;
; - la section [Code] est du Pascal : elle ajoute ce que les sections ne savent pas faire
;   (détecter Equalizer APO, poser des questions à la désinstallation) ;
; - les lignes #define / #if sont lues par le compilateur avant tout le reste.
;
; Compilation (la version vient de Cargo.toml, jamais écrite ici) :
;   ISCC.exe /DAppVersion=0.1.0 installateur\retro-eq.iss

#ifndef AppVersion
  #error "Passer la version : ISCC.exe /DAppVersion=x.y.z retro-eq.iss"
#endif
#if VER < EncodeVer(6,3,0)
  #error "Inno Setup 6.3 ou plus est nécessaire (ArchitecturesAllowed=x64compatible)."
#endif
; L'exécutable à installer. Les chemins sont relatifs au dossier de ce script.
#ifndef ExeSource
  #define ExeSource "..\target\release\retro-eq.exe"
#endif
#ifndef DossierSortie
  #define DossierSortie "sortie"
#endif

[Setup]
; Identifiant stable de l'appli : il fait reconnaître une mise à jour. Ne jamais le changer.
AppId={{7B1E4C52-3A9D-4E86-9F0A-5D2C8E1B6A47}
AppName=Retro EQ
AppVersion={#AppVersion}
AppPublisher=Retro EQ
AppPublisherURL=https://github.com/alexisdechiara/retro-eq
AppSupportURL=https://github.com/alexisdechiara/retro-eq/issues
DefaultDirName={autopf}\Retro EQ
DisableProgramGroupPage=yes
UninstallDisplayIcon={app}\retro-eq.exe
; Page de licence : le texte de la GPLv3 du dépôt.
LicenseFile=..\LICENSE
; x64 seulement, Windows 10 22H2 (build 19045) au minimum.
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0.19045
; Droits d'administrateur, pour trois raisons : écrire dans « Program Files » et dans la base
; de registre de la machine (HKLM) ; lancer l'installateur d'Equalizer APO, qui les exige ;
; et, à la désinstallation, retirer le moteur du dossier de configuration d'Equalizer APO.
PrivilegesRequired=admin
; Ces deux lignes font demander la fermeture de Retro EQ s'il tourne (l'appli crée ce mutex
; au démarrage, voir crates/retro-app/src/instance.rs : les deux noms doivent rester égaux).
AppMutex=RetroEQ.EnCours
SetupMutex=RetroEQ.Installateur
CloseApplications=yes
WizardStyle=modern
Compression=lzma2
SolidCompression=yes
OutputDir={#DossierSortie}
OutputBaseFilename=retro-eq-{#AppVersion}-installateur-x64

[Languages]
Name: "fr"; MessagesFile: "compiler:Languages\French.isl"
Name: "en"; MessagesFile: "compiler:Default.isl"

[Files]
Source: "{#ExeSource}"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\LICENSE"; DestDir: "{app}"; DestName: "LICENSE.txt"; Flags: ignoreversion

[Icons]
; {autoprograms}, {autodesktop} et {autostartup} désignent les dossiers « pour tous les
; utilisateurs », puisque l'installation se fait avec les droits d'administrateur.
Name: "{autoprograms}\Retro EQ"; Filename: "{app}\retro-eq.exe"
Name: "{autodesktop}\Retro EQ"; Filename: "{app}\retro-eq.exe"; Tasks: raccourcibureau
Name: "{autostartup}\Retro EQ"; Filename: "{app}\retro-eq.exe"; Tasks: demarrage

[Tasks]
; Cases à cocher de la page « Tâches supplémentaires ». Aucune n'est cochée au départ.
Name: "raccourcibureau"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked
Name: "demarrage"; Description: "{cm:TacheDemarrage}"; Flags: unchecked

[Run]
Filename: "{app}\retro-eq.exe"; Description: "{cm:LaunchProgram,Retro EQ}"; Flags: nowait postinstall skipifsilent

[CustomMessages]
fr.TacheDemarrage=Lancer Retro EQ au démarrage de Windows
en.TacheDemarrage=Start Retro EQ when Windows starts
fr.ApoTitre=Moteur audio : Equalizer APO
en.ApoTitre=Audio engine: Equalizer APO
fr.ApoDescription=Retro EQ s’appuie sur ce moteur gratuit pour modifier le son.
en.ApoDescription=Retro EQ relies on this free engine to change the sound.
fr.ApoExplication=Equalizer APO n’est pas installé sur ce PC. Retro EQ en a besoin pour modifier le son : sans lui, l’appli s’ouvre mais n’égalise rien.%n%nC’est un logiciel libre et gratuit (licence GPLv2), fait par d’autres personnes que Retro EQ. Son propre installateur s’ouvre dans une fenêtre visible : il te demandera sur quelle sortie audio appliquer l’égaliseur (par exemple ton casque), puis de redémarrer Windows.%n%nSi tu préfères, tu peux l’installer toi-même plus tard : https://sourceforge.net/projects/equalizerapo/
en.ApoExplication=Equalizer APO is not installed on this PC. Retro EQ needs it to change the sound: without it the app opens but equalises nothing.%n%nIt is free, open-source software (GPLv2), made by other people than Retro EQ. Its own installer opens in a visible window: it will ask which audio output to equalise (for example your headset), then ask you to restart Windows.%n%nIf you prefer, you can install it yourself later: https://sourceforge.net/projects/equalizerapo/
fr.ApoCase=Télécharger et lancer l’installateur officiel d’Equalizer APO
en.ApoCase=Download and run the official Equalizer APO installer
fr.ApoTelechargementEchoue=Le téléchargement d’Equalizer APO a échoué.%n%nRetro EQ va s’installer quand même. Veux-tu ouvrir la page officielle d’Equalizer APO pour l’installer toi-même ?
en.ApoTelechargementEchoue=Downloading Equalizer APO failed.%n%nRetro EQ will be installed anyway. Do you want to open the official Equalizer APO page to install it yourself?
fr.ApoPasInstalle=Equalizer APO n’a pas été installé (installation annulée ?). Retro EQ est installé, mais il n’égalisera rien tant qu’Equalizer APO n’est pas là : https://sourceforge.net/projects/equalizerapo/
en.ApoPasInstalle=Equalizer APO was not installed (installation cancelled?). Retro EQ is installed, but it will equalise nothing until Equalizer APO is there: https://sourceforge.net/projects/equalizerapo/
fr.DesinstIntro=Rendre le son d’origine : avant de supprimer Retro EQ, son moteur est retiré de tes sorties audio. Le son peut se couper une seconde.
en.DesinstIntro=Restoring the original sound: before Retro EQ is removed, its engine is taken off your audio outputs. The sound may cut out for a second.
fr.DesinstProfils=Tes profils sont gardés dans :
en.DesinstProfils=Your profiles are kept in:
fr.DesinstContinuer=Continuer la désinstallation ?
en.DesinstContinuer=Continue uninstalling?
fr.DesinstApo=Equalizer APO a été installé par Retro EQ. Le retirer aussi ? (Son propre programme de désinstallation s’ouvrira.)
en.DesinstApo=Equalizer APO was installed by Retro EQ. Remove it too? (Its own uninstaller will open.)
fr.DesinstReglages=Supprimer aussi tes réglages et l’historique des tests ? Sinon ils restent pour une réinstallation.
en.DesinstReglages=Also delete your settings and test history? Otherwise they stay for a reinstall.
fr.DesinstProfilsSuppr=Supprimer aussi tes profils ? Non = ils restent dans le dossier indiqué.
en.DesinstProfilsSuppr=Also delete your profiles? No = they stay in the folder shown.
fr.DesinstFermer=Retro EQ est ouvert. Ferme-le (icône de la barre des tâches, puis Quitter), puis clique sur OK.
en.DesinstFermer=Retro EQ is open. Close it (taskbar icon, then Quit), then click OK.
fr.DesinstEchec=Le son d’origine n’a pas pu être rendu (Retro EQ n’a pas répondu, ou le moteur a refusé). La désinstallation est annulée : ton son reste tel qu’il est. Tu peux aussi le rendre depuis Retro EQ, Réglages.
en.DesinstEchec=The original sound could not be restored (Retro EQ did not answer, or the engine refused). Uninstall is cancelled: your sound stays as it is. You can also restore it from Retro EQ, Settings.
fr.DesinstApoManuel=Equalizer APO n’a pas pu être retiré automatiquement. Tu peux le faire dans Paramètres Windows, Applications.
en.DesinstApoManuel=Equalizer APO could not be removed automatically. You can do it in Windows Settings, Apps.

[Code]
const
  // Equalizer APO 1.4.2, 64 bits, sur SourceForge (projet « equalizerapo »). L'adresse nomme
  // la version : elle ne change pas quand une version plus récente sort. L'empreinte SHA-256
  // est celle du fichier relevé le 25/11/2026 ; Inno Setup refuse le fichier si elle diffère.
  // Pour passer à une autre version : changer ces trois constantes, puis recalculer l'empreinte
  // (sha256sum du fichier téléchargé), et vérifier que la nouvelle version est bien la bonne.
  UrlApo = 'https://downloads.sourceforge.net/project/equalizerapo/1.4.2/EqualizerAPO-x64-1.4.2.exe';
  NomApo = 'EqualizerAPO-x64-1.4.2.exe';
  Sha256Apo = '7403be7427bbe1936a40dded082829b6e217fc4f5990fee5cba501f0ae055afa';
  PageOfficielleApo = 'https://sourceforge.net/projects/equalizerapo/';
  CleApo = 'SOFTWARE\EqualizerAPO';
  // Notre propre clé : elle retient qu'Equalizer APO a été installé PAR NOUS (valeur 1), pour
  // que la désinstallation ne retire jamais un Equalizer APO qui était là avant.
  CleRetro = 'SOFTWARE\Retro EQ';
  ValeurApoParNous = 'EqualizerApoInstalleParRetroEq';
  MutexAppli = 'RetroEQ.EnCours';

var
  PageMoteur: TWizardPage;
  CaseApo: TNewCheckBox;
  PageTelechargement: TDownloadWizardPage;
  ApoTelecharge: Boolean;
  // Désinstallation : les choix posés au début, appliqués à la fin.
  RetirerApo, EffacerReglages, EffacerProfils: Boolean;

// En mode 64 bits (voir [Setup]), HKLM désigne la vue 64 bits du registre, celle
// où Equalizer APO 64 bits écrit.
function ApoInstalle(): Boolean;
var
  Dossier: String;
begin
  Result := False;
  if RegQueryStringValue(HKLM, CleApo, 'InstallPath', Dossier) then
    Result := (Dossier <> '') and DirExists(Dossier);
end;

function ApoInstalleParNous(): Boolean;
var
  Valeur: Cardinal;
begin
  Result := False;
  if RegQueryDWordValue(HKLM, CleRetro, ValeurApoParNous, Valeur) then
    Result := (Valeur = 1);
end;

function DossierDonnees(): String;
begin
  Result := ExpandConstant('{userappdata}\Retro EQ');
end;

procedure OuvrirPageApo();
var
  Code: Integer;
begin
  ShellExec('open', PageOfficielleApo, '', '', SW_SHOWNORMAL, ewNoWait, Code);
end;

// ---------- Installation : la page « Equalizer APO » ----------

procedure InitializeWizard;
var
  Texte: TNewStaticText;
begin
  // Placée après les tâches supplémentaires, juste avant le récapitulatif.
  PageMoteur := CreateCustomPage(wpSelectTasks, CustomMessage('ApoTitre'), CustomMessage('ApoDescription'));

  Texte := TNewStaticText.Create(PageMoteur);
  Texte.Parent := PageMoteur.Surface;
  Texte.AutoSize := False;
  Texte.WordWrap := True;
  Texte.Width := PageMoteur.SurfaceWidth;
  Texte.Caption := CustomMessage('ApoExplication');
  Texte.AdjustHeight;

  // Case décochée au départ : rien n'est téléchargé sans que la personne l'ait cochée.
  CaseApo := TNewCheckBox.Create(PageMoteur);
  CaseApo.Parent := PageMoteur.Surface;
  CaseApo.Top := Texte.Top + Texte.Height + ScaleY(16);
  CaseApo.Width := PageMoteur.SurfaceWidth;
  CaseApo.Caption := CustomMessage('ApoCase');
  CaseApo.Checked := False;

  PageTelechargement := CreateDownloadPage(SetupMessage(msgWizardPreparing), SetupMessage(msgPreparingDesc), nil);
  PageTelechargement.ShowBaseNameInsteadOfUrl := True;
end;

// Pas de page si Equalizer APO est déjà là : on le garde tel quel.
function ShouldSkipPage(PageID: Integer): Boolean;
begin
  Result := (PageID = PageMoteur.ID) and ApoInstalle();
end;

// Le téléchargement se fait au clic sur « Installer » (page « Prêt à installer »), avant
// de copier nos fichiers. Un échec n'empêche pas d'installer Retro EQ.
function NextButtonClick(CurPageID: Integer): Boolean;
begin
  Result := True;
  if (CurPageID = wpReady) and CaseApo.Checked and (not ApoInstalle()) then begin
    PageTelechargement.Clear;
    // Le troisième argument est l'empreinte SHA-256 attendue : un fichier différent est refusé.
    PageTelechargement.Add(UrlApo, NomApo, Sha256Apo);
    PageTelechargement.Show;
    try
      try
        PageTelechargement.Download;
        ApoTelecharge := True;
      except
        ApoTelecharge := False;
        if not PageTelechargement.AbortedByUser then begin
          if MsgBox(CustomMessage('ApoTelechargementEchoue') + #13#10#13#10 + GetExceptionMessage, mbError, MB_YESNO) = IDYES then
            OuvrirPageApo();
        end;
      end;
    finally
      PageTelechargement.Hide;
    end;
  end;
end;

// Après la copie de nos fichiers : on lance l'installateur d'Equalizer APO dans sa propre
// fenêtre (visible, jamais en mode silencieux) et on attend qu'il se termine.
procedure CurStepChanged(CurStep: TSetupStep);
var
  Code: Integer;
begin
  if (CurStep = ssPostInstall) and ApoTelecharge then begin
    Code := -1;
    Exec(ExpandConstant('{tmp}\') + NomApo, '', '', SW_SHOWNORMAL, ewWaitUntilTerminated, Code);
    Log('Installateur d''Equalizer APO terminé, code ' + IntToStr(Code));
    if ApoInstalle() then
      // On note que c'est nous : la désinstallation pourra proposer de le retirer.
      RegWriteDWordValue(HKLM, CleRetro, ValeurApoParNous, 1)
    else
      MsgBox(CustomMessage('ApoPasInstalle'), mbInformation, MB_OK);
  end;
end;

// ---------- Désinstallation : rendre le son d'origine ----------

function AppliFermee(): Boolean;
begin
  Result := True;
  while CheckForMutexes(MutexAppli) do begin
    if UninstallSilent() then begin
      Result := False;
      Exit;
    end;
    if MsgBox(CustomMessage('DesinstFermer'), mbInformation, MB_OKCANCEL) = IDCANCEL then begin
      Result := False;
      Exit;
    end;
  end;
end;

// Lance « retro-eq.exe --rendre-son-origine » : sans fenêtre, code 0 si tout va bien.
function RendreSonOrigine(): Boolean;
var
  Exe: String;
  Code: Integer;
begin
  Result := False;
  Code := -1;
  Exe := ExpandConstant('{app}\retro-eq.exe');
  if FileExists(Exe) then
    if Exec(Exe, '--rendre-son-origine', '', SW_HIDE, ewWaitUntilTerminated, Code) then
      Result := (Code = 0);
  Log('--rendre-son-origine : code ' + IntToStr(Code));
end;

// Appelée au tout début de la désinstallation. Renvoyer False l'annule : rien n'a alors été
// supprimé, et le moteur n'est retiré qu'une fois les questions posées et l'appli fermée.
function InitializeUninstall(): Boolean;
begin
  Result := True;
  RetirerApo := False;
  EffacerReglages := False;
  EffacerProfils := False;

  // En désinstallation silencieuse : le moteur est retiré, Equalizer APO et les données
  // sont gardés (aucune question, et rien de supprimé sans l'avoir demandé).
  if not UninstallSilent() then begin
    if MsgBox(CustomMessage('DesinstIntro') + #13#10#13#10 + CustomMessage('DesinstProfils') + #13#10 + DossierDonnees() + #13#10#13#10 + CustomMessage('DesinstContinuer'), mbConfirmation, MB_YESNO) <> IDYES then begin
      Result := False;
      Exit;
    end;
    if ApoInstalleParNous() and ApoInstalle() then
      RetirerApo := (MsgBox(CustomMessage('DesinstApo'), mbConfirmation, MB_YESNO) = IDYES);
    EffacerReglages := (MsgBox(CustomMessage('DesinstReglages'), mbConfirmation, MB_YESNO or MB_DEFBUTTON2) = IDYES);
    if EffacerReglages then
      EffacerProfils := (MsgBox(CustomMessage('DesinstProfilsSuppr') + #13#10 + DossierDonnees() + '\profils', mbConfirmation, MB_YESNO or MB_DEFBUTTON2) = IDYES);
  end;

  if not AppliFermee() then begin
    Result := False;
    Exit;
  end;

  if not RendreSonOrigine() then begin
    // On n'enlève pas Retro EQ si le son d'origine n'a pas été rendu : l'utilisateur peut
    // réessayer, ou le rendre lui-même depuis Réglages.
    if not UninstallSilent() then
      MsgBox(CustomMessage('DesinstEchec'), mbError, MB_OK);
    Result := False;
  end;
end;

procedure SupprimerDonnees(AvecProfils: Boolean);
var
  Dossier: String;
begin
  Dossier := DossierDonnees();
  DelTree(Dossier + '\tests', True, True, True);
  DeleteFile(Dossier + '\reglages.json');
  DeleteFile(Dossier + '\etat-moteur.json');
  if AvecProfils then
    DelTree(Dossier + '\profils', True, True, True);
  // Ne supprime le dossier que s'il est vide : jamais les profils gardés.
  RemoveDir(Dossier);
end;

// Lance le désinstallateur d'Equalizer APO dans sa propre fenêtre (visible).
procedure DesinstallerApo();
var
  Dossier, Programme: String;
  Code: Integer;
begin
  Code := -1;
  if RegQueryStringValue(HKLM, CleApo, 'InstallPath', Dossier) then begin
    Programme := AddBackslash(Dossier) + 'Uninstall.exe';
    if FileExists(Programme) then
      Exec(Programme, '', Dossier, SW_SHOWNORMAL, ewWaitUntilTerminated, Code);
  end;
  if Code < 0 then
    MsgBox(CustomMessage('DesinstApoManuel'), mbInformation, MB_OK);
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  // usPostUninstall : nos fichiers sont déjà supprimés.
  if CurUninstallStep = usPostUninstall then begin
    if EffacerReglages then
      SupprimerDonnees(EffacerProfils);
    if RetirerApo then
      DesinstallerApo();
    RegDeleteKeyIncludingSubkeys(HKLM, CleRetro);
  end;
end;
