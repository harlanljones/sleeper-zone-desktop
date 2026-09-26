; Sleeper Zone Desktop — Inno Setup 6 installer script.
;
; Built on CI (.github/workflows/windows.yml), which passes
; /DMyAppVersion=<cargo package version>. Local build:
;
;   cargo build --release --target x86_64-pc-windows-msvc
;   iscc /DMyAppVersion=0.1.0 packaging\windows\installer.iss
;
; The exe keeps the console subsystem (see src/main.rs), so the
; installer does not need any terminal handling. User config and the
; player cache live in %APPDATA%\sleeper-zone and are created at
; runtime — intentionally left behind on uninstall.

#define MyAppName "Sleeper Zone Desktop"
#define MyAppExe "sleeper-zone-desktop.exe"
#ifndef MyAppVersion
  #define MyAppVersion "0.1.0"
#endif

[Setup]
AppId={{50afa492-6e3a-4fd3-bedf-2ccdc4b14216}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppVerName={#MyAppName} {#MyAppVersion}
DefaultDirName={autopf}\Sleeper Zone
DefaultGroupName=Sleeper Zone
OutputDir=dist
OutputBaseFilename=sleeper-zone-desktop-setup-{#MyAppVersion}
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
UninstallDisplayName={#MyAppName}
VersionInfoVersion={#MyAppVersion}

[Files]
Source: "..\..\target\x86_64-pc-windows-msvc\release\{#MyAppExe}"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\{#MyAppName}"; Filename: "{app}\{#MyAppExe}"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExe}"; Tasks: desktopicon

[Tasks]
Name: "desktopicon"; Description: "Create a &desktop icon"; Flags: unchecked

[Run]
Filename: "{app}\{#MyAppExe}"; Description: "Launch {#MyAppName}"; Flags: nowait postinstall skipifsilent
