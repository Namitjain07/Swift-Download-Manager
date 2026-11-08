; Swift Download Manager for Windows: the installer (Inno Setup 6.3 or newer).
;
; Build it (from the source folder; see also `make windows-installer` and the release workflow):
;   ISCC /DAppVersion=1.9.0 /DArch=x64   /DExe=dist\SwiftDownloadManager.exe       packaging\windows\installer.iss
;   ISCC /DAppVersion=1.9.0 /DArch=arm64 /DExe=dist\SwiftDownloadManager-arm64.exe packaging\windows\installer.iss
; Signed (the program, the installer and the uninstaller; see sign.ps1):
;   add  /DSign=1 /Ssdmsign="powershell -NoProfile -ExecutionPolicy Bypass -File packaging\windows\sign.ps1 $f"
; The result is dist\SwiftDownloadManager-<version>-setup-<arch>.exe.
;
; It installs for the current user by default (no administrator needed) and for everybody when run as an
; administrator or when the person picks that on the first page. Silent: setup.exe /VERYSILENT /SUPPRESSMSGBOXES
; (add /ALLUSERS or /CURRENTUSER to choose, /DIR="C:\Some\Folder" for the folder, /TASKS="desktopicon" for the icon).

#ifndef AppVersion
  #error Give the version with /DAppVersion=1.2.3
#endif
#ifndef Arch
  #define Arch "x64"
#endif
#ifndef Exe
  #define Exe "..\..\dist\SwiftDownloadManager.exe"
#endif
; Windows wants four numbers for the version resource: 1.9.0-beta.1 becomes 1.9.0.0.
#define BaseVersion Copy(AppVersion, 1, Pos("-", AppVersion + "-") - 1)
#define NumericVersion BaseVersion + ".0"

#define AppName "Swift Download Manager"
#define AppExe "SwiftDownloadManager.exe"
#define Home "https://github.com/Namitjain07/Swift-Download-Manager"

[Setup]
; Never change this identifier: it is how a new version finds the old one to replace it.
AppId={{12792FF1-A5E4-4E93-B841-4A759CA4FED1}
AppName={#AppName}
AppVersion={#AppVersion}
AppVerName={#AppName} {#AppVersion}
AppPublisher=Namitjain07
AppPublisherURL={#Home}
AppSupportURL={#Home}/issues
AppUpdatesURL={#Home}/releases
VersionInfoVersion={#NumericVersion}
VersionInfoProductName={#AppName}
VersionInfoDescription={#AppName} setup
VersionInfoCopyright=Apache License 2.0
DefaultDirName={autopf}\{#AppName}
DefaultGroupName={#AppName}
DisableProgramGroupPage=yes
DisableDirPage=auto
UninstallDisplayName={#AppName}
UninstallDisplayIcon={app}\{#AppExe}
SetupIconFile=app.ico
LicenseFile=..\..\..\LICENSE
OutputDir=..\..\dist
OutputBaseFilename=SwiftDownloadManager-{#AppVersion}-setup-{#Arch}
Compression=lzma2/ultra64
SolidCompression=yes
WizardStyle=modern
; For the current user by default; the first page offers "for all users" (that asks for an administrator).
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
; A running copy (it can sit in the tray) is asked to close, so its program file can be replaced.
CloseApplications=yes
CloseApplicationsFilter={#AppExe}
RestartApplications=no
#if Arch == "arm64"
ArchitecturesAllowed=arm64
ArchitecturesInstallIn64BitMode=arm64
#else
; Runs on 64-bit Intel and AMD, and on Windows on ARM through its x64 emulation.
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
#endif
MinVersion=10.0.17763
#ifdef Sign
SignTool=sdmsign
SignedUninstaller=yes
#endif

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "{#Exe}"; DestDir: "{app}"; DestName: "{#AppExe}"; Flags: ignoreversion
Source: "..\..\browser-extension\*"; DestDir: "{app}\browser-extension"; Flags: ignoreversion recursesubdirs createallsubdirs
Source: "..\..\..\README.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\..\LICENSE"; DestDir: "{app}"; Flags: ignoreversion
; The public part of the code-signing keys, so that a signature can be checked (see SECURITY.md).
Source: "swift-download-manager-codesign-2.cer"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\{#AppName}"; Filename: "{app}\{#AppExe}"
Name: "{group}\Browser add-on (folder)"; Filename: "{app}\browser-extension"
Name: "{group}\Uninstall {#AppName}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\{#AppExe}"; Tasks: desktopicon

[Run]
Filename: "{app}\{#AppExe}"; Description: "{cm:LaunchProgram,{#AppName}}"; Flags: nowait postinstall skipifsilent

[UninstallRun]
; The program can keep running in the tray after its window is closed; stop it so that its files can go.
Filename: "{sys}\taskkill.exe"; Parameters: "/IM {#AppExe}"; Flags: runhidden skipifdoesntexist; RunOnceId: "StopSwiftDownloadManager"

[Messages]
FinishedLabel=Setup has finished installing [name] on your computer.%n%nYour settings and your download list are kept when you uninstall, so that a new installation carries on where you stopped. The browser add-on is in the "browser-extension" folder of the installation; Help > Browser integration in the program shows how to add it to your browser.
