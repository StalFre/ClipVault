; Script de Inno Setup para ClipVault -- VERSION SIN ADMINISTRADOR.
; Instala en AppData\Local del usuario actual, sin pedir privilegios elevados.
; Util para PCs restringidas (empresas, colegios, cuentas sin admin).
; Compilar con: ISCC.exe installer_noadmin.iss

#define MyAppName "ClipVault"
#define MyAppVersion "1.0.0"
#define MyAppExeName "ClipVault.exe"

[Setup]
AppId={{B6C1A8E4-6F2A-4C9E-9B1D-CLIPVAULT002}}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
PrivilegesRequired=lowest
DefaultDirName={localappdata}\{#MyAppName}
DefaultGroupName={#MyAppName}
OutputDir=installer_output
OutputBaseFilename=ClipVault_Setup_{#MyAppVersion}_SinAdmin
SetupIconFile=static\icon.ico
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
UninstallDisplayIcon={app}\{#MyAppExeName}
DisableProgramGroupPage=yes
InfoAfterFile=extension_instrucciones.txt

[Languages]
Name: "spanish"; MessagesFile: "compiler:Languages\Spanish.isl"

[Tasks]
Name: "desktopicon"; Description: "Crear un acceso directo en el escritorio"; GroupDescription: "Accesos directos:"

[Files]
; Todo lo que genero PyInstaller (onedir): la carpeta completa dist\ClipVault
Source: "dist\ClipVault\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs
; Extension de navegador
Source: "ClipVault-extension\*"; DestDir: "{app}\ClipVault-extension"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
; user* en vez de {group}/{autodesktop}: no requieren permisos de administrador,
; quedan solo para la cuenta de Windows que instalo el programa.
Name: "{userprograms}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"
Name: "{userprograms}\Desinstalar {#MyAppName}"; Filename: "{uninstallexe}"
Name: "{userdesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; Tasks: desktopicon

[Run]
Filename: "{app}\{#MyAppExeName}"; Description: "Iniciar {#MyAppName} ahora"; Flags: nowait postinstall skipifsilent

[UninstallDelete]
Type: filesandordirs; Name: "{app}"
