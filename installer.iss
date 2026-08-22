; Script de Inno Setup para ClipVault.
; Compilar con Inno Setup Compiler (ISCC.exe installer.iss) despues de correr build.bat

#define MyAppName "ClipVault"
#define MyAppVersion "1.0.0"
#define MyAppExeName "ClipVault.exe"

[Setup]
AppId={{B6C1A8E4-6F2A-4C9E-9B1D-CLIPVAULT001}}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
InfoAfterFile=extension_instrucciones.txt
DefaultDirName={autopf}\{#MyAppName}
DefaultGroupName={#MyAppName}
OutputDir=installer_output
OutputBaseFilename=ClipVault_Setup_{#MyAppVersion}
SetupIconFile=static\icon.ico
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
UninstallDisplayIcon={app}\{#MyAppExeName}
DisableProgramGroupPage=yes

[Languages]
Name: "spanish"; MessagesFile: "compiler:Languages\Spanish.isl"

[Tasks]
Name: "desktopicon"; Description: "Crear un acceso directo en el escritorio"; GroupDescription: "Accesos directos:"

[Files]
; Todo lo que genero PyInstaller (onedir): la carpeta completa dist\ClipVault
Source: "dist\ClipVault\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs
; La extension del navegador, para que quede junto al programa instalado
Source: "ClipVault-extension\*"; DestDir: "{app}\ClipVault-extension"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{group}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"
Name: "{group}\Carpeta de la extension de navegador"; Filename: "{app}\browser_extension"
Name: "{group}\Desinstalar {#MyAppName}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; Tasks: desktopicon

[Run]
Filename: "{app}\{#MyAppExeName}"; Description: "Iniciar {#MyAppName} ahora"; Flags: nowait postinstall skipifsilent

[UninstallDelete]
Type: filesandordirs; Name: "{app}"
