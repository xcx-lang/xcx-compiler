; XCX Compiler Ecosystem - Inno Setup Script
; Version: 3.0

#define MyAppName "XCX Compiler Ecosystem"
#define MyAppVersion "3.0"
#define MyAppPublisher "XCX Team"
#define MyAppExeName "xcx.exe"
#define MyAppURL "https://xcx-team.example.com"
#define MyAppSupportURL "https://xcx-team.example.com/support"

; SHA-256 checksums for binary files
; How to get: certutil -hashfile file.exe SHA256  (Windows)
;              sha256sum file.exe                   (Linux/macOS)
; Fill in values before each build!
#define ChecksumXcxExe  "AABBCCDDEEFF00112233445566778899AABBCCDDEEFF00112233445566778899"
#define ChecksumPaxXcx  "1122334455667788990011223344556677889900112233445566778899001122"
#define ChecksumMathXcx "2233445566778899001122334455667788990011223344556677889900112233"

[Setup]
AppId={{74D8E2B2-3A9B-4F8C-A63D-0F2A6C44D871}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
AppSupportURL={#MyAppSupportURL}
AppUpdatesURL={#MyAppURL}
DefaultDirName={autopf}\XCX
DefaultGroupName={#MyAppName}
AllowNoIcons=yes
; Minimum version: Windows 10 (10.0)
MinVersion=10.0
LicenseFile={#SourcePath}\resources\LICENSE.txt
InfoBeforeFile={#SourcePath}\resources\README.txt
OutputDir={#SourcePath}\Output
OutputBaseFilename=xcx-setup-v3.0
SetupIconFile={#SourcePath}\resources\icons\xcx.ico
; Sidebar image for wizard (164x314 px, BMP or PNG format)
WizardImageFile={#SourcePath}\resources\wizard\wizard-sidebar.bmp
; Small header image for wizard steps (55x58 px)
WizardSmallImageFile={#SourcePath}\resources\wizard\wizard-header.bmp
; Show language selection dialog at startup
ShowLanguageDialog=yes
; Write install log to %TEMP% - useful for debugging user-side errors
SetupLogging=yes
Compression=lzma
SolidCompression=yes
WizardStyle=modern
; 64-bit only; on 32-bit the installer will display an error message
ArchitecturesAllowed=x64
ArchitecturesInstallIn64BitMode=x64
; Close application if running before installation
CloseApplications=yes
CloseApplicationsFilter=xcx.exe
RestartApplications=yes
; Admin privileges required to write to HKLM and {autopf}
PrivilegesRequired=admin
; Proper handling of reinstallation
UsePreviousAppDir=yes
UsePreviousGroup=yes

[Languages]
; English is the default/core language - listed first
Name: "english";             MessagesFile: "compiler:Default.isl"
; All languages bundled with Inno Setup (alphabetical after English)
Name: "armenian";            MessagesFile: "compiler:Languages\Armenian.isl"
Name: "brazilianportuguese"; MessagesFile: "compiler:Languages\BrazilianPortuguese.isl"
Name: "bulgarian";           MessagesFile: "compiler:Languages\Bulgarian.isl"
Name: "catalan";             MessagesFile: "compiler:Languages\Catalan.isl"
Name: "corsican";            MessagesFile: "compiler:Languages\Corsican.isl"
Name: "czech";               MessagesFile: "compiler:Languages\Czech.isl"
Name: "danish";              MessagesFile: "compiler:Languages\Danish.isl"
Name: "dutch";               MessagesFile: "compiler:Languages\Dutch.isl"
Name: "finnish";             MessagesFile: "compiler:Languages\Finnish.isl"
Name: "french";              MessagesFile: "compiler:Languages\French.isl"
Name: "german";              MessagesFile: "compiler:Languages\German.isl"
Name: "hebrew";              MessagesFile: "compiler:Languages\Hebrew.isl"
Name: "hungarian";           MessagesFile: "compiler:Languages\Hungarian.isl"
Name: "italian";             MessagesFile: "compiler:Languages\Italian.isl"
Name: "japanese";            MessagesFile: "compiler:Languages\Japanese.isl"
Name: "korean";              MessagesFile: "compiler:Languages\Korean.isl"
Name: "norwegian";           MessagesFile: "compiler:Languages\Norwegian.isl"
Name: "polish";              MessagesFile: "compiler:Languages\Polish.isl"
Name: "portuguese";          MessagesFile: "compiler:Languages\Portuguese.isl"
Name: "russian";             MessagesFile: "compiler:Languages\Russian.isl"
Name: "slovak";              MessagesFile: "compiler:Languages\Slovak.isl"
Name: "slovenian";           MessagesFile: "compiler:Languages\Slovenian.isl"
Name: "spanish";             MessagesFile: "compiler:Languages\Spanish.isl"
Name: "turkish";             MessagesFile: "compiler:Languages\Turkish.isl"
Name: "ukrainian";           MessagesFile: "compiler:Languages\Ukrainian.isl"

[Types]
Name: "full";    Description: "{cm:FullInstallation}"
Name: "compact"; Description: "{cm:CompactInstallation}"
Name: "custom";  Description: "{cm:CustomInstallation}"; Flags: iscustom

[Components]
Name: "core"; Description: "{cm:CoreComponent}";        Types: full compact custom; Flags: fixed
Name: "pax";  Description: "{cm:PaxComponent}";         Types: full custom
Name: "math"; Description: "{cm:MathComponent}";        Types: full custom

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked; Components: core

[Files]
Source: "{#SourcePath}\bin\xcx.exe";             DestDir: "{app}\bin"; Flags: ignoreversion; Components: core
Source: "{#SourcePath}\lib\pax.xcx";             DestDir: "{app}\lib"; Flags: ignoreversion; Components: pax
Source: "{#SourcePath}\lib\math.xcx";            DestDir: "{app}\lib"; Flags: ignoreversion; Components: math
Source: "{#SourcePath}\resources\icons\xcx.ico"; DestDir: "{app}";     Flags: ignoreversion; Components: core
Source: "{#SourcePath}\resources\icons\pax.ico"; DestDir: "{app}";     Flags: ignoreversion; Components: pax
Source: "{#SourcePath}\resources\LICENSE.txt";   DestDir: "{app}";     Flags: ignoreversion; Components: core
Source: "{#SourcePath}\resources\README.txt";    DestDir: "{app}";     Flags: ignoreversion; Components: core

; Remove old files from previous versions that no longer exist in v3.0
; Add files/folders removed between versions here
[InstallDelete]
Type: files;          Name: "{app}\bin\xcx-old.exe"
Type: files;          Name: "{app}\lib\compat.xcx"
Type: filesandordirs; Name: "{app}\lib\v2"

[Icons]
Name: "{group}\{#MyAppName}";                       Filename: "{app}\bin\{#MyAppExeName}"; IconFilename: "{app}\xcx.ico"
Name: "{group}\{cm:UninstallProgram,{#MyAppName}}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\{#MyAppName}";                 Filename: "{app}\bin\{#MyAppExeName}"; IconFilename: "{app}\xcx.ico"; Tasks: desktopicon

[Registry]
; --- PATH ---
; Using HKLM instead of HKCU because installation requires admin - system PATH is preferable
Root: HKLM; Subkey: "SYSTEM\CurrentControlSet\Control\Session Manager\Environment"; \
  ValueType: expandsz; ValueName: "Path"; \
  ValueData: "{olddata};{app}\bin"; \
  Check: NeedsAddPath; \
  Flags: preservestringtype uninsdeletevalue

; --- .xcx file association ---
Root: HKLM; Subkey: "Software\Classes\.xcx";                          ValueType: string; ValueName: ""; ValueData: "XCX.Script";     Flags: uninsdeletevalue
Root: HKLM; Subkey: "Software\Classes\XCX.Script";                    ValueType: string; ValueName: ""; ValueData: "XCX Script File"; Flags: uninsdeletekey
Root: HKLM; Subkey: "Software\Classes\XCX.Script\DefaultIcon";        ValueType: string; ValueName: ""; ValueData: "{app}\xcx.ico,0"; Flags: uninsdeletekey
Root: HKLM; Subkey: "Software\Classes\XCX.Script\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\bin\xcx.exe"" ""%1"""; Flags: uninsdeletekey

; --- .pax file association ---
Root: HKLM; Subkey: "Software\Classes\.pax";                           ValueType: string; ValueName: ""; ValueData: "PAX.Package";     Flags: uninsdeletevalue;  Components: pax
Root: HKLM; Subkey: "Software\Classes\PAX.Package";                    ValueType: string; ValueName: ""; ValueData: "PAX Package File"; Flags: uninsdeletekey;   Components: pax
Root: HKLM; Subkey: "Software\Classes\PAX.Package\DefaultIcon";        ValueType: string; ValueName: ""; ValueData: "{app}\pax.ico,0"; Flags: uninsdeletekey;    Components: pax
Root: HKLM; Subkey: "Software\Classes\PAX.Package\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\bin\xcx.exe"" ""%1"""; Flags: uninsdeletekey; Components: pax

[UninstallDelete]
; Remove files created by the application after installation (logs, cache, config)
Type: filesandordirs; Name: "{app}\logs"
Type: filesandordirs; Name: "{app}\cache"
Type: filesandordirs; Name: "{app}\config"
; Remove the entire folder if empty after uninstall
Type: dirifempty;     Name: "{app}"

[Run]
Filename: "{app}\bin\xcx.exe"; \
  Description: "{cm:LaunchAfterInstall,{#MyAppName}}"; \
  Flags: nowait postinstall skipifsilent

; ---------------------------------------------------------------------------
; Custom Messages - translatable strings used via {cm:...} above
; Add a block for every language listed in [Languages].
; ---------------------------------------------------------------------------
[CustomMessages]

; --- English (default) ---
english.FullInstallation=Full installation
english.CompactInstallation=Compact installation
english.CustomInstallation=Custom installation
english.CoreComponent=XCX Compiler Core
english.PaxComponent=PAX Package Manager
english.MathComponent=Math Standard Library
english.LaunchAfterInstall=Launch %1

; --- Armenian ---
armenian.FullInstallation=Ամբողջական տեղակայում
armenian.CompactInstallation=Կոմպակտ տեղակայում
armenian.CustomInstallation=Հատուկ տեղակայում
armenian.CoreComponent=XCX կոմպիլյատորի հիմք
armenian.PaxComponent=PAX փաթեթների կառավարիչ
armenian.MathComponent=Մաթեմատիկական ստանդարտ գրադարան
armenian.LaunchAfterInstall=Գործարկել %1

; --- Brazilian Portuguese ---
brazilianportuguese.FullInstallation=Instalação completa
brazilianportuguese.CompactInstallation=Instalação compacta
brazilianportuguese.CustomInstallation=Instalação personalizada
brazilianportuguese.CoreComponent=Núcleo do compilador XCX
brazilianportuguese.PaxComponent=Gerenciador de pacotes PAX
brazilianportuguese.MathComponent=Biblioteca matemática padrão
brazilianportuguese.LaunchAfterInstall=Iniciar %1

; --- Bulgarian ---
bulgarian.FullInstallation=Пълна инсталация
bulgarian.CompactInstallation=Компактна инсталация
bulgarian.CustomInstallation=Персонализирана инсталация
bulgarian.CoreComponent=Ядро на компилатора XCX
bulgarian.PaxComponent=Мениджър на пакети PAX
bulgarian.MathComponent=Стандартна математическа библиотека
bulgarian.LaunchAfterInstall=Стартиране на %1

; --- Catalan ---
catalan.FullInstallation=Instal·lació completa
catalan.CompactInstallation=Instal·lació compacta
catalan.CustomInstallation=Instal·lació personalitzada
catalan.CoreComponent=Nucli del compilador XCX
catalan.PaxComponent=Gestor de paquets PAX
catalan.MathComponent=Biblioteca matemàtica estàndard
catalan.LaunchAfterInstall=Inicia %1

; --- Corsican ---
corsican.FullInstallation=Installazione completa
corsican.CompactInstallation=Installazione compatta
corsican.CustomInstallation=Installazione persunalizata
corsican.CoreComponent=Core di u compilatore XCX
corsican.PaxComponent=Gestore di pacchetti PAX
corsican.MathComponent=Libreria matematica standard
corsican.LaunchAfterInstall=Lancia %1

; --- Czech ---
czech.FullInstallation=Úplná instalace
czech.CompactInstallation=Kompaktní instalace
czech.CustomInstallation=Vlastní instalace
czech.CoreComponent=Jádro kompilátoru XCX
czech.PaxComponent=Správce balíčků PAX
czech.MathComponent=Standardní matematická knihovna
czech.LaunchAfterInstall=Spustit %1

; --- Danish ---
danish.FullInstallation=Fuld installation
danish.CompactInstallation=Kompakt installation
danish.CustomInstallation=Brugerdefineret installation
danish.CoreComponent=XCX Compiler-kerne
danish.PaxComponent=PAX-pakkehåndtering
danish.MathComponent=Standard matematikbibliotek
danish.LaunchAfterInstall=Start %1

; --- Dutch ---
dutch.FullInstallation=Volledige installatie
dutch.CompactInstallation=Compacte installatie
dutch.CustomInstallation=Aangepaste installatie
dutch.CoreComponent=XCX Compiler-kern
dutch.PaxComponent=PAX-pakketbeheer
dutch.MathComponent=Standaard wiskundebibliotheek
dutch.LaunchAfterInstall=%1 starten

; --- Finnish ---
finnish.FullInstallation=Täydellinen asennus
finnish.CompactInstallation=Kompakti asennus
finnish.CustomInstallation=Mukautettu asennus
finnish.CoreComponent=XCX-kääntäjän ydin
finnish.PaxComponent=PAX-paketinhallinta
finnish.MathComponent=Matemaattinen vakiokirjasto
finnish.LaunchAfterInstall=Käynnistä %1

; --- French ---
french.FullInstallation=Installation complète
french.CompactInstallation=Installation compacte
french.CustomInstallation=Installation personnalisée
french.CoreComponent=Noyau du compilateur XCX
french.PaxComponent=Gestionnaire de paquets PAX
french.MathComponent=Bibliothèque standard mathématique
french.LaunchAfterInstall=Lancer %1

; --- German ---
german.FullInstallation=Vollständige Installation
german.CompactInstallation=Kompakte Installation
german.CustomInstallation=Benutzerdefinierte Installation
german.CoreComponent=XCX Compiler-Kern
german.PaxComponent=PAX-Paketverwaltung
german.MathComponent=Mathematische Standardbibliothek
german.LaunchAfterInstall=%1 starten

; --- Hebrew ---
hebrew.FullInstallation=התקנה מלאה
hebrew.CompactInstallation=התקנה קומפקטית
hebrew.CustomInstallation=התקנה מותאמת אישית
hebrew.CoreComponent=ליבת מהדר XCX
hebrew.PaxComponent=מנהל חבילות PAX
hebrew.MathComponent=ספריית מתמטיקה סטנדרטית
hebrew.LaunchAfterInstall=הפעל את %1

; --- Hungarian ---
hungarian.FullInstallation=Teljes telepítés
hungarian.CompactInstallation=Kompakt telepítés
hungarian.CustomInstallation=Egyéni telepítés
hungarian.CoreComponent=XCX fordító mag
hungarian.PaxComponent=PAX csomagkezelő
hungarian.MathComponent=Matematikai szabványos könyvtár
hungarian.LaunchAfterInstall=%1 indítása

; --- Italian ---
italian.FullInstallation=Installazione completa
italian.CompactInstallation=Installazione compatta
italian.CustomInstallation=Installazione personalizzata
italian.CoreComponent=Nucleo del compilatore XCX
italian.PaxComponent=Gestore pacchetti PAX
italian.MathComponent=Libreria matematica standard
italian.LaunchAfterInstall=Avvia %1

; --- Japanese ---
japanese.FullInstallation=フルインストール
japanese.CompactInstallation=コンパクトインストール
japanese.CustomInstallation=カスタムインストール
japanese.CoreComponent=XCX コンパイラコア
japanese.PaxComponent=PAX パッケージマネージャ
japanese.MathComponent=数学標準ライブラリ
japanese.LaunchAfterInstall=%1 を起動

; --- Korean ---
korean.FullInstallation=전체 설치
korean.CompactInstallation=간략 설치
korean.CustomInstallation=사용자 지정 설치
korean.CoreComponent=XCX 컴파일러 코어
korean.PaxComponent=PAX 패키지 관리자
korean.MathComponent=수학 표준 라이브러리
korean.LaunchAfterInstall=%1 실행

; --- Norwegian ---
norwegian.FullInstallation=Full installasjon
norwegian.CompactInstallation=Kompakt installasjon
norwegian.CustomInstallation=Tilpasset installasjon
norwegian.CoreComponent=XCX kompilator-kjerne
norwegian.PaxComponent=PAX pakkehåndterer
norwegian.MathComponent=Standard matematikkbibliotek
norwegian.LaunchAfterInstall=Start %1

; --- Polish ---
polish.FullInstallation=Pełna instalacja
polish.CompactInstallation=Instalacja kompaktowa
polish.CustomInstallation=Instalacja niestandardowa
polish.CoreComponent=Jądro kompilatora XCX
polish.PaxComponent=Menedżer pakietów PAX
polish.MathComponent=Standardowa biblioteka matematyczna
polish.LaunchAfterInstall=Uruchom %1

; --- Portuguese ---
portuguese.FullInstallation=Instalação completa
portuguese.CompactInstallation=Instalação compacta
portuguese.CustomInstallation=Instalação personalizada
portuguese.CoreComponent=Núcleo do compilador XCX
portuguese.PaxComponent=Gestor de pacotes PAX
portuguese.MathComponent=Biblioteca matemática padrão
portuguese.LaunchAfterInstall=Iniciar %1

; --- Russian ---
russian.FullInstallation=Полная установка
russian.CompactInstallation=Компактная установка
russian.CustomInstallation=Выборочная установка
russian.CoreComponent=Ядро компилятора XCX
russian.PaxComponent=Менеджер пакетов PAX
russian.MathComponent=Стандартная математическая библиотека
russian.LaunchAfterInstall=Запустить %1

; --- Slovak ---
slovak.FullInstallation=Úplná inštalácia
slovak.CompactInstallation=Kompaktná inštalácia
slovak.CustomInstallation=Vlastná inštalácia
slovak.CoreComponent=Jadro kompilátora XCX
slovak.PaxComponent=Správca balíkov PAX
slovak.MathComponent=Štandardná matematická knižnica
slovak.LaunchAfterInstall=Spustiť %1

; --- Slovenian ---
slovenian.FullInstallation=Popolna namestitev
slovenian.CompactInstallation=Kompaktna namestitev
slovenian.CustomInstallation=Namestitev po meri
slovenian.CoreComponent=Jedro prevajalnika XCX
slovenian.PaxComponent=Upravitelj paketov PAX
slovenian.MathComponent=Standardna matematična knjižnica
slovenian.LaunchAfterInstall=Zaženi %1

; --- Spanish ---
spanish.FullInstallation=Instalación completa
spanish.CompactInstallation=Instalación compacta
spanish.CustomInstallation=Instalación personalizada
spanish.CoreComponent=Núcleo del compilador XCX
spanish.PaxComponent=Gestor de paquetes PAX
spanish.MathComponent=Biblioteca estándar de matemáticas
spanish.LaunchAfterInstall=Iniciar %1

; --- Turkish ---
turkish.FullInstallation=Tam kurulum
turkish.CompactInstallation=Kompakt kurulum
turkish.CustomInstallation=Özel kurulum
turkish.CoreComponent=XCX Derleyici Çekirdeği
turkish.PaxComponent=PAX Paket Yöneticisi
turkish.MathComponent=Standart Matematik Kütüphanesi
turkish.LaunchAfterInstall=%1 başlat

; --- Ukrainian ---
ukrainian.FullInstallation=Повне встановлення
ukrainian.CompactInstallation=Компактне встановлення
ukrainian.CustomInstallation=Вибіркове встановлення
ukrainian.CoreComponent=Ядро компілятора XCX
ukrainian.PaxComponent=Менеджер пакетів PAX
ukrainian.MathComponent=Стандартна математична бібліотека
ukrainian.LaunchAfterInstall=Запустити %1

[Code]
const
  WM_SETTINGCHANGE = $001A;
  SMTO_ABORTIFHUNG = $0002;
  // System PATH registry key (HKLM) - consistent with [Registry] section
  PATH_REGKEY = 'SYSTEM\CurrentControlSet\Control\Session Manager\Environment';

function SendMessageTimeout(hWnd: HWND; Msg: Cardinal; wParam: LongInt; lParam: String;
  fuFlags: Cardinal; uTimeout: Cardinal; out lpdwResult: Cardinal): LongInt;
  external 'SendMessageTimeoutA@user32.dll stdcall';

// -------------------------------------------------------------------------
// Notify the system about environment variable changes
// -------------------------------------------------------------------------
procedure UpdateEnv();
var
  ResultAddr: Cardinal;
begin
  SendMessageTimeout($FFFF, WM_SETTINGCHANGE, 0, 'Environment', SMTO_ABORTIFHUNG, 5000, ResultAddr);
end;

// -------------------------------------------------------------------------
// Check whether {app}\bin is already in PATH (prevents duplicates on reinstall)
// -------------------------------------------------------------------------
function NeedsAddPath(): Boolean;
var
  Path: String;
  BinPath: String;
begin
  BinPath := Uppercase(ExpandConstant('{app}\bin'));
  if RegQueryStringValue(HKEY_LOCAL_MACHINE, PATH_REGKEY, 'Path', Path) then
    Result := Pos(BinPath, Uppercase(Path)) = 0
  else
    Result := True;
end;

// -------------------------------------------------------------------------
// Remove {app}\bin from the system PATH - handles duplicate entries and
// mixed casing
// -------------------------------------------------------------------------
procedure RemoveFromPath();
var
  Path: String;
  BinPath: String;
  Parts: TStringList;
  i: Integer;
  NewPath: String;
  Changed: Boolean;
begin
  BinPath := Uppercase(ExpandConstant('{app}\bin'));

  if not RegQueryStringValue(HKEY_LOCAL_MACHINE, PATH_REGKEY, 'Path', Path) then
    Exit;

  Parts := TStringList.Create;
  try
    Parts.Delimiter := ';';
    Parts.StrictDelimiter := True;
    Parts.DelimitedText := Path;

    Changed := False;
    // Iterate backwards so we can safely delete elements
    for i := Parts.Count - 1 downto 0 do
    begin
      if Uppercase(Trim(Parts[i])) = BinPath then
      begin
        Parts.Delete(i);
        Changed := True;
      end;
    end;

    if Changed then
    begin
      // Rejoin, skipping empty segments
      NewPath := '';
      for i := 0 to Parts.Count - 1 do
      begin
        if Trim(Parts[i]) <> '' then
        begin
          if NewPath <> '' then
            NewPath := NewPath + ';';
          NewPath := NewPath + Parts[i];
        end;
      end;

      if NewPath = '' then
        RegDeleteValue(HKEY_LOCAL_MACHINE, PATH_REGKEY, 'Path')
      else
        RegWriteExpandStringValue(HKEY_LOCAL_MACHINE, PATH_REGKEY, 'Path', NewPath);
    end;
  finally
    Parts.Free;
  end;
end;

// -------------------------------------------------------------------------
// Recursively delete a folder and all its contents
// -------------------------------------------------------------------------
procedure DeleteDirRecursive(Dir: String);
var
  FindRec: TFindRec;
  FilePath: String;
begin
  if FindFirst(Dir + '\*', FindRec) then
  begin
    try
      repeat
        if (FindRec.Name <> '.') and (FindRec.Name <> '..') then
        begin
          FilePath := Dir + '\' + FindRec.Name;
          if FindRec.Attributes and FILE_ATTRIBUTE_DIRECTORY <> 0 then
            DeleteDirRecursive(FilePath)
          else
            DeleteFile(FilePath);
        end;
      until not FindNext(FindRec);
    finally
      FindClose(FindRec);
    end;
  end;
  RemoveDir(Dir);
end;

// -------------------------------------------------------------------------
// Compute SHA-256 of a file and return it as an uppercase hex string.
// Uses Windows CryptoAPI via certutil (available on every Windows since XP).
// -------------------------------------------------------------------------
function GetFileSHA256(const FilePath: String): String;
var
  ResultCode: Integer;
  TempFile: String;
  Lines: TStringList;
  Line: String;
begin
  Result := '';
  TempFile := ExpandConstant('{tmp}\checksum_out.txt');

  // Use cmd /C to redirect certutil output to a temp file
  Exec(ExpandConstant('{sys}\cmd.exe'),
    '/C certutil -hashfile "' + FilePath + '" SHA256 > "' + TempFile + '"',
    '', SW_HIDE, ewWaitUntilTerminated, ResultCode);

  if not FileExists(TempFile) then
    Exit;

  Lines := TStringList.Create;
  try
    Lines.LoadFromFile(TempFile);
    // certutil outputs 3 lines: header, hash, "CertUtil: -hashfile command completed"
    // The hash is on the second line (index 1)
    if Lines.Count >= 2 then
    begin
      Line := Trim(Lines[1]);
      // Remove any spaces inserted by some certutil versions
      StringChangeEx(Line, ' ', '', True);
      Result := Uppercase(Line);
    end;
  finally
    Lines.Free;
    DeleteFile(TempFile);
  end;
end;

// -------------------------------------------------------------------------
// Verify the checksum of one file. Returns True if OK or if the file does
// not exist yet (a deselected component won't block the install).
// -------------------------------------------------------------------------
function VerifyChecksum(const FilePath, ExpectedHash, FileName: String): Boolean;
var
  ActualHash: String;
begin
  Result := True;

  if not FileExists(FilePath) then
    Exit; // Source file not present yet; a missing file error will surface later

  ActualHash := GetFileSHA256(FilePath);

  if ActualHash = '' then
  begin
    MsgBox('Warning: Could not compute SHA-256 checksum for:' + #13#10 + FileName + #13#10#13#10 +
           'Installation will continue, but file integrity cannot be verified.',
           mbConfirmation, MB_OK);
    Exit; // Treat as a warning, not a fatal error
  end;

  if ActualHash <> Uppercase(ExpectedHash) then
  begin
    MsgBox('Checksum verification FAILED for:' + #13#10 + FileName + #13#10#13#10 +
           'Expected: ' + Uppercase(ExpectedHash) + #13#10 +
           'Got:      ' + ActualHash + #13#10#13#10 +
           'The installation package may be corrupted or tampered with.' + #13#10 +
           'Installation will be aborted.',
           mbError, MB_OK);
    Result := False;
  end;
end;

// -------------------------------------------------------------------------
// Show an error on 32-bit systems and verify file checksums before
// installation begins.
// -------------------------------------------------------------------------
function InitializeSetup(): Boolean;
var
  SourcePath: String;
begin
  Result := True;

  // Architecture check (ArchitecturesAllowed=x64 already blocks, but this
  // gives a friendlier, explicit error message)
  if not Is64BitInstallMode then
  begin
    MsgBox('XCX Compiler Ecosystem requires a 64-bit version of Windows.', mbError, MB_OK);
    Result := False;
    Exit;
  end;

  // SHA-256 verification of binaries before installation begins
  SourcePath := ExpandConstant('{src}');

  if not VerifyChecksum(SourcePath + '\bin\xcx.exe',
    '{#ChecksumXcxExe}', 'xcx.exe') then
  begin
    Result := False;
    Exit;
  end;

  if not VerifyChecksum(SourcePath + '\lib\pax.xcx',
    '{#ChecksumPaxXcx}', 'pax.xcx') then
  begin
    Result := False;
    Exit;
  end;

  if not VerifyChecksum(SourcePath + '\lib\math.xcx',
    '{#ChecksumMathXcx}', 'math.xcx') then
  begin
    Result := False;
    Exit;
  end;
end;

// -------------------------------------------------------------------------
// Request a system restart after installation.
//
// This is the simplest and most reliable way to ensure {app}\bin is in the
// PATH for every terminal and process - no hacks needed. Inno Setup shows
// the standard "Restart now / Restart later" dialog automatically translated
// to the language the user selected at the start of the installer.
//
// Behaviour during silent installs:
//   /SILENT      - dialog is suppressed; no automatic restart occurs
//   /VERYSILENT  - same as above
//   /VERYSILENT /RESTARTEXITCODE=1641 - installer exits with code 1641 so
//                  the caller (e.g. SCCM, PDQ) knows to trigger a restart
// -------------------------------------------------------------------------
function NeedRestart(): Boolean;
begin
  Result := True;
end;

procedure CurStepChanged(CurStep: TSetupStep);
begin
  // Broadcast WM_SETTINGCHANGE right after installation so that any app
  // already open (e.g. Explorer) picks up the new PATH from the registry
  // without waiting for the restart.
  if CurStep = ssPostInstall then
    UpdateEnv();
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
var
  AppDir: String;
begin
  // Remove PATH entry before notifying the system
  if CurUninstallStep = usUninstall then
    RemoveFromPath();

  // Delete the entire application folder after the standard uninstall
  // (catches leftover user-created files, logs, cache, etc.)
  if CurUninstallStep = usPostUninstall then
  begin
    AppDir := ExpandConstant('{app}');
    if DirExists(AppDir) then
      DeleteDirRecursive(AppDir);

    UpdateEnv();
  end;
end;
