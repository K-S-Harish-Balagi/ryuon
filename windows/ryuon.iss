; Ryuon installer for Windows (Inno Setup 6). Built by .github/workflows/windows.yml:
;   iscc /DAppVersion=0.4.0 /DSourceDir=..\bundle\Ryuon windows\ryuon.iss
; Installs for the current user by default (no administrator password); the first page offers
; all users instead. Settings live in the registry and are kept on uninstall.

#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif
#ifndef SourceDir
  #define SourceDir "..\bundle\Ryuon"
#endif

[Setup]
; Never change AppId: Windows uses it to find the installed copy for upgrades and uninstall
AppId={{E0971405-A384-4B85-9D36-8F3D0B8279D4}
AppName=Ryuon
AppVersion={#AppVersion}
AppVerName=Ryuon {#AppVersion}
AppPublisher=K S Harish Balagi
AppPublisherURL=https://github.com/K-S-Harish-Balagi/ryuon
AppSupportURL=https://github.com/K-S-Harish-Balagi/ryuon/issues
AppUpdatesURL=https://github.com/K-S-Harish-Balagi/ryuon/releases
AppComments=Plays your music library folder by folder
VersionInfoVersion={#AppVersion}
VersionInfoProductName=Ryuon
VersionInfoProductVersion={#AppVersion}
VersionInfoProductTextVersion={#AppVersion}
VersionInfoDescription=Ryuon installer
DefaultDirName={autopf}\Ryuon
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0
LicenseFile=..\LICENSES\GPL-3.0-or-later.txt
SetupIconFile=ryuon.ico
UninstallDisplayIcon={app}\bin\ryuon.exe
UninstallDisplayName=Ryuon
OutputDir=..\dist-installer
OutputBaseFilename=Ryuon-{#AppVersion}-Setup
Compression=lzma2/ultra64
SolidCompression=yes
WizardStyle=modern
; Closes a running Ryuon before replacing its files on upgrade
CloseApplications=yes
RestartApplications=no

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[InstallDelete]
; An upgrade may ship a different set of plugins and libraries: start from a clean folder
Type: filesandordirs; Name: "{app}\bin"
Type: filesandordirs; Name: "{app}\lib"
Type: filesandordirs; Name: "{app}\libexec"
Type: filesandordirs; Name: "{app}\share"

[Files]
Source: "{#SourceDir}\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{autoprograms}\Ryuon"; Filename: "{app}\bin\ryuon.exe"; WorkingDir: "{app}\bin"; Comment: "Plays your music library folder by folder"
Name: "{autodesktop}\Ryuon"; Filename: "{app}\bin\ryuon.exe"; WorkingDir: "{app}\bin"; Tasks: desktopicon

[Run]
Filename: "{app}\bin\ryuon.exe"; Description: "{cm:LaunchProgram,Ryuon}"; Flags: nowait postinstall skipifsilent
