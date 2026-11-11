; Instalador de ClipVault (Inno Setup 6).
;   Con administrador (Archivos de programa, todos los usuarios):  ISCC /DADMIN /DMyAppVersion=2.0.0 clipvault.iss
;   Sin administrador (AppData\Local\Programs, solo tu usuario):   ISCC /DMyAppVersion=2.0.0 clipvault.iss
; Normalmente se compila con build_installer.ps1, que genera las dos versiones.

#ifndef MyAppVersion
  #define MyAppVersion "2.0.0"
#endif
#define MyAppName "ClipVault"
#define MyAppExeName "ClipVault.exe"
#define MyAppPublisher "StalFre"
#define MyAppURL "https://github.com/StalFre/ClipVault"

#ifdef ADMIN
  #define Variant "Admin"
#else
  #define Variant "User"
#endif

[Setup]
#ifdef ADMIN
AppId={{8F3D2A71-5C4B-4E1A-9A6D-2B7C1E0F4A11}
PrivilegesRequired=admin
#else
AppId={{8F3D2A71-5C4B-4E1A-9A6D-2B7C1E0F4A22}
PrivilegesRequired=lowest
#endif
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
AppSupportURL={#MyAppURL}/issues
AppUpdatesURL={#MyAppURL}/releases
DefaultDirName={autopf}\{#MyAppName}
DefaultGroupName={#MyAppName}
DisableProgramGroupPage=yes
OutputDir=..\dist
OutputBaseFilename=ClipVault-Setup-{#MyAppVersion}-{#Variant}
SetupIconFile=..\assets\icon.ico
UninstallDisplayIcon={app}\{#MyAppExeName}
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
CloseApplications=yes
RestartApplications=no
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible

[Languages]
Name: "spanish"; MessagesFile: "compiler:Languages\Spanish.isl"
Name: "english"; MessagesFile: "compiler:Default.isl"

[CustomMessages]
spanish.TaskStartup=Iniciar ClipVault con Windows (en la bandeja)
spanish.TaskExt=Instalar la extensión del navegador (te guiamos paso a paso)
spanish.TaskGroupExtra=Opciones adicionales:
spanish.ExtTitle=Extensión del navegador
spanish.ExtDesc=Sigue estos pasos para activar la extensión en Chrome, Brave o Edge.
spanish.ExtSteps=1. Pulsa «Abrir página de extensiones» (o escribe chrome://extensions en tu navegador).%n2. Activa «Modo de desarrollador» (arriba a la derecha).%n3. Pulsa «Cargar descomprimida» y elige esta carpeta:
spanish.ExtSteps2=4. Fija el icono de ClipVault desde el botón del puzzle de la barra.%n5. Con ClipVault abierto, entra a un video de YouTube: aparecerá el aviso para descargarlo.
spanish.ExtOpenFolder=Abrir carpeta de la extensión
spanish.ExtOpenPage=Abrir página de extensiones
spanish.ExtNoBrowser=No encontré Chrome, Brave ni Edge. Abre tu navegador y escribe chrome://extensions en la barra de direcciones.
spanish.UninstallData=¿Quieres borrar también tu configuración e historial de ClipVault?
english.TaskStartup=Start ClipVault with Windows (in the tray)
english.TaskExt=Install the browser extension (we will guide you step by step)
english.TaskGroupExtra=Additional options:
english.ExtTitle=Browser extension
english.ExtDesc=Follow these steps to enable the extension in Chrome, Brave or Edge.
english.ExtSteps=1. Click "Open extensions page" (or type chrome://extensions in your browser).%n2. Turn on "Developer mode" (top right).%n3. Click "Load unpacked" and choose this folder:
english.ExtSteps2=4. Pin the ClipVault icon from the puzzle button in the toolbar.%n5. With ClipVault open, visit a YouTube video: a prompt to download it will appear.
english.ExtOpenFolder=Open extension folder
english.ExtOpenPage=Open extensions page
english.ExtNoBrowser=I could not find Chrome, Brave or Edge. Open your browser and type chrome://extensions in the address bar.
english.UninstallData=Do you also want to delete your ClipVault settings and history?

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked
Name: "startup"; Description: "{cm:TaskStartup}"; GroupDescription: "{cm:TaskGroupExtra}"; Flags: unchecked
Name: "installext"; Description: "{cm:TaskExt}"; GroupDescription: "{cm:TaskGroupExtra}"

[Files]
Source: "..\target\release\clipvault.exe"; DestDir: "{app}"; DestName: "{#MyAppExeName}"; Flags: ignoreversion
Source: "..\assets\icon.ico"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\ClipVault-extension\*"; DestDir: "{app}\ClipVault-extension"; Flags: ignoreversion recursesubdirs createallsubdirs; Tasks: installext

[Icons]
Name: "{autoprograms}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; Tasks: desktopicon
Name: "{autostartup}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; Parameters: "--tray"; Tasks: startup

[Run]
Filename: "{app}\{#MyAppExeName}"; Description: "{cm:LaunchProgram,{#MyAppName}}"; Flags: nowait postinstall

[UninstallRun]
Filename: "{sys}\taskkill.exe"; Parameters: "/IM {#MyAppExeName} /F"; Flags: runhidden; RunOnceId: "KillClipVault"

[Code]
var
  ExtPage: TWizardPage;
  ExtPathEdit: TNewEdit;

function ExtDir: String;
begin
  Result := ExpandConstant('{app}\ClipVault-extension');
end;

procedure OpenExtFolder(Sender: TObject);
var
  R: Integer;
begin
  ShellExec('open', ExtDir, '', '', SW_SHOWNORMAL, ewNoWait, R);
end;

function FindBrowser(const Exe: String): String;
var
  P: String;
begin
  Result := '';
  if RegQueryStringValue(HKLM, 'SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths\' + Exe, '', P) or
     RegQueryStringValue(HKCU, 'SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths\' + Exe, '', P) then
    if FileExists(P) then
      Result := P;
end;

procedure OpenExtPage(Sender: TObject);
var
  Exe, Url: String;
  R: Integer;
begin
  Exe := FindBrowser('chrome.exe');
  Url := 'chrome://extensions';
  if Exe = '' then begin
    Exe := FindBrowser('brave.exe');
    Url := 'brave://extensions';
  end;
  if Exe = '' then begin
    Exe := FindBrowser('msedge.exe');
    Url := 'edge://extensions';
  end;
  if Exe = '' then
    MsgBox(CustomMessage('ExtNoBrowser'), mbInformation, MB_OK)
  else
    Exec(Exe, Url, '', SW_SHOWNORMAL, ewNoWait, R);
end;

procedure InitializeWizard;
var
  Steps1, Steps2: TNewStaticText;
  BtnFolder, BtnPage: TNewButton;
begin
  ExtPage := CreateCustomPage(wpInstalling, CustomMessage('ExtTitle'), CustomMessage('ExtDesc'));

  Steps1 := TNewStaticText.Create(ExtPage);
  Steps1.Parent := ExtPage.Surface;
  Steps1.WordWrap := True;
  Steps1.AutoSize := True;
  Steps1.Width := ExtPage.SurfaceWidth;
  Steps1.Caption := CustomMessage('ExtSteps');

  ExtPathEdit := TNewEdit.Create(ExtPage);
  ExtPathEdit.Parent := ExtPage.Surface;
  ExtPathEdit.Top := Steps1.Top + Steps1.Height + ScaleY(8);
  ExtPathEdit.Width := ExtPage.SurfaceWidth;
  ExtPathEdit.ReadOnly := True;

  Steps2 := TNewStaticText.Create(ExtPage);
  Steps2.Parent := ExtPage.Surface;
  Steps2.WordWrap := True;
  Steps2.AutoSize := True;
  Steps2.Width := ExtPage.SurfaceWidth;
  Steps2.Top := ExtPathEdit.Top + ExtPathEdit.Height + ScaleY(10);
  Steps2.Caption := CustomMessage('ExtSteps2');

  BtnPage := TNewButton.Create(ExtPage);
  BtnPage.Parent := ExtPage.Surface;
  BtnPage.Caption := CustomMessage('ExtOpenPage');
  BtnPage.Width := ScaleX(190);
  BtnPage.Height := ScaleY(26);
  BtnPage.Top := Steps2.Top + Steps2.Height + ScaleY(14);
  BtnPage.OnClick := @OpenExtPage;

  BtnFolder := TNewButton.Create(ExtPage);
  BtnFolder.Parent := ExtPage.Surface;
  BtnFolder.Caption := CustomMessage('ExtOpenFolder');
  BtnFolder.Width := ScaleX(190);
  BtnFolder.Height := ScaleY(26);
  BtnFolder.Top := BtnPage.Top;
  BtnFolder.Left := BtnPage.Left + BtnPage.Width + ScaleX(10);
  BtnFolder.OnClick := @OpenExtFolder;
end;

procedure CurPageChanged(CurPageID: Integer);
begin
  if CurPageID = ExtPage.ID then
    ExtPathEdit.Text := ExtDir;
end;

function ShouldSkipPage(PageID: Integer): Boolean;
begin
  Result := False;
  if PageID = ExtPage.ID then
    Result := not WizardIsTaskSelected('installext');
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usPostUninstall then
    if MsgBox(CustomMessage('UninstallData'), mbConfirmation, MB_YESNO or MB_DEFBUTTON2) = IDYES then
      DelTree(ExpandConstant('{localappdata}\ClipVault'), True, True, True);
end;
