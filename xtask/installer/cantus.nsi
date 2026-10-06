; Cantus — the Windows installer (ADR 0030). Built by `cargo run -p xtask -- package`,
; which writes config.nsh, files.nsh and strings.nsh beside this copy.
;
; Per user, without administrator rights: the program goes to
; %LOCALAPPDATA%\Programs\Cantus, the plug-ins to the per-user CLAP and VST3 folders
; (%LOCALAPPDATA%\Programs\Common\CLAP and \VST3, each in a Cantus folder), and every
; registry key is under HKCU. Silent switches: /S (NSIS's own), /UPDATE (keep the
; components chosen before — what the app's updater passes), /RELAUNCH (open Cantus
; when done).

Unicode true
ManifestDPIAware true
SetCompressor /SOLID lzma
RequestExecutionLevel user

!include "config.nsh"
!include "MUI2.nsh"
!include "FileFunc.nsh"
!include "LogicLib.nsh"
!include "Sections.nsh"

!define REG_APP "Software\Cantus"
!define REG_UNINST "Software\Microsoft\Windows\CurrentVersion\Uninstall\Cantus"
!define CLAP_DIR "$LOCALAPPDATA\Programs\Common\CLAP\Cantus"
!define VST3_DIR "$LOCALAPPDATA\Programs\Common\VST3\Cantus"
!define PROGID "Cantus.Project"
!define HOMEPAGE "https://github.com/luis314159/cantus"
!define SHCNE_ASSOCCHANGED 0x08000000

!include "files.nsh"

Name "Cantus${FLAVOR_LABEL}"
OutFile "${OUTFILE}"
InstallDir "$LOCALAPPDATA\Programs\Cantus"
InstallDirRegKey HKCU "${REG_APP}" "InstallDir"
BrandingText "Cantus ${VERSION}"
ShowInstDetails show
ShowUninstDetails show

VIProductVersion "${VERSION4}"
VIFileVersion "${VERSION4}"
VIAddVersionKey /LANG=0 "ProductName" "Cantus"
VIAddVersionKey /LANG=0 "ProductVersion" "${VERSION}"
VIAddVersionKey /LANG=0 "FileVersion" "${VERSION}"
VIAddVersionKey /LANG=0 "FileDescription" "Cantus ${VERSION} setup"
VIAddVersionKey /LANG=0 "CompanyName" "Cantus"
VIAddVersionKey /LANG=0 "LegalCopyright" "GPL-3.0-or-later"

; ---- the pages ---------------------------------------------------------------------

!define MUI_ABORTWARNING
!ifdef ICON
  !define MUI_ICON "${ICON}"
  !define MUI_UNICON "${ICON}"
!endif
!define MUI_COMPONENTSPAGE_SMALLDESC
; The GPL asks nobody to accept it in order to run the program: the page informs.
!define MUI_LICENSEPAGE_BUTTON "$(^NextBtn)"
!define MUI_LICENSEPAGE_TEXT_BOTTOM "$(CANTUS_LICENSE_BOTTOM)"
!define MUI_FINISHPAGE_RUN "$INSTDIR\cantus.exe"

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_LICENSE "${LICENSE_FILE}"
!insertmacro MUI_PAGE_COMPONENTS
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_COMPONENTS
!insertmacro MUI_UNPAGE_INSTFILES

; English first: the fallback for every other system language.
!insertmacro MUI_LANGUAGE "English"
!insertmacro MUI_LANGUAGE "Spanish"
!include "strings.nsh"

; ---- waiting for Cantus to close (an update starts as the app quits) ---------------

!macro WAIT_FOR_CANTUS
  StrCpy $R0 0
  ${DoWhile} ${FileExists} "$INSTDIR\cantus.exe"
    ClearErrors
    FileOpen $R1 "$INSTDIR\cantus.exe" a
    ${IfNot} ${Errors}
      FileClose $R1
      ${Break}
    ${EndIf}
    IntOp $R0 $R0 + 1
    ${If} $R0 > 60
      MessageBox MB_RETRYCANCEL|MB_ICONEXCLAMATION "$(CANTUS_CLOSE_CANTUS)" /SD IDCANCEL IDRETRY +3
      SetErrorLevel 2
      Quit
      StrCpy $R0 0
    ${EndIf}
    Sleep 500
  ${Loop}
!macroend

Function WaitForCantus
  !insertmacro WAIT_FOR_CANTUS
FunctionEnd

Function un.WaitForCantus
  !insertmacro WAIT_FOR_CANTUS
FunctionEnd

!macro NOTIFY_ASSOCIATIONS
  System::Call 'shell32::SHChangeNotify(i ${SHCNE_ASSOCCHANGED}, i 0, p 0, p 0)'
!macroend

; ---- what is installed ---------------------------------------------------------------

Section "!$(CANTUS_SECTION_APP)" SecApp
  SectionIn RO
  Call WaitForCantus
  !insertmacro CANTUS_APP_FILES
  WriteUninstaller "$INSTDIR\uninstall.exe"
  CreateShortcut "$SMPROGRAMS\Cantus.lnk" "$INSTDIR\cantus.exe" "" "$INSTDIR\cantus.exe" 0

  WriteRegStr HKCU "${REG_APP}" "InstallDir" "$INSTDIR"
  WriteRegStr HKCU "${REG_APP}" "Version" "${VERSION}"
  WriteRegStr HKCU "${REG_APP}" "Flavor" "${FLAVOR}"

  WriteRegStr HKCU "${REG_UNINST}" "DisplayName" "Cantus${FLAVOR_LABEL}"
  WriteRegStr HKCU "${REG_UNINST}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "${REG_UNINST}" "Publisher" "Cantus"
  WriteRegStr HKCU "${REG_UNINST}" "DisplayIcon" "$INSTDIR\cantus.exe,0"
  WriteRegStr HKCU "${REG_UNINST}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "${REG_UNINST}" "UninstallString" '"$INSTDIR\uninstall.exe"'
  WriteRegStr HKCU "${REG_UNINST}" "QuietUninstallString" '"$INSTDIR\uninstall.exe" /S'
  WriteRegStr HKCU "${REG_UNINST}" "URLInfoAbout" "${HOMEPAGE}"
  WriteRegStr HKCU "${REG_UNINST}" "URLUpdateInfo" "${HOMEPAGE}/releases"
  WriteRegDWORD HKCU "${REG_UNINST}" "NoModify" 1
  WriteRegDWORD HKCU "${REG_UNINST}" "NoRepair" 1
  WriteRegDWORD HKCU "${REG_UNINST}" "EstimatedSize" ${EST_SIZE_KB}

  ; "Open with" knows Cantus even when it is not the default for .cantus.
  WriteRegStr HKCU "Software\Classes\Applications\cantus.exe" "FriendlyAppName" "Cantus"
  WriteRegStr HKCU "Software\Classes\Applications\cantus.exe\SupportedTypes" ".cantus" ""
  WriteRegStr HKCU "Software\Classes\Applications\cantus.exe\shell\open\command" "" '"$INSTDIR\cantus.exe" "%1"'
SectionEnd

Section "$(CANTUS_SECTION_ASSOC)" SecAssoc
  WriteRegStr HKCU "Software\Classes\.cantus" "" "${PROGID}"
  WriteRegStr HKCU "Software\Classes\.cantus\OpenWithProgids" "${PROGID}" ""
  WriteRegStr HKCU "Software\Classes\${PROGID}" "" "$(CANTUS_PROJECT_TYPE)"
  WriteRegStr HKCU "Software\Classes\${PROGID}\DefaultIcon" "" '"$INSTDIR\cantus.exe",0'
  WriteRegStr HKCU "Software\Classes\${PROGID}\shell\open\command" "" '"$INSTDIR\cantus.exe" "%1"'
  !insertmacro NOTIFY_ASSOCIATIONS
SectionEnd

Section /o "$(CANTUS_SECTION_DESKTOP)" SecDesktop
  CreateShortcut "$DESKTOP\Cantus.lnk" "$INSTDIR\cantus.exe" "" "$INSTDIR\cantus.exe" 0
SectionEnd

Section "$(CANTUS_SECTION_CLAP)" SecClap
  !insertmacro CANTUS_CLAP_FILES
SectionEnd

Section "$(CANTUS_SECTION_VST3)" SecVst3
  !insertmacro CANTUS_VST3_FILES
SectionEnd

!if "${FLAVOR}" == "asio"
; FlexASIO (MIT): fetched at install time, the latest release, never bundled; checked
; (a program, the size GitHub reports) and run elevated by flexasio.ps1. Off by default,
; never in a silent install or update, and not remembered. ASIO4ALL is not offered: its
; licence forbids redistribution (the app links to it).
Section /o "$(CANTUS_SECTION_FLEXASIO)" SecFlexAsio
  ${If} ${Silent}
    Return
  ${EndIf}
  InitPluginsDir
  SetOutPath "$PLUGINSDIR"
  File "flexasio.ps1"
  DetailPrint "$(CANTUS_FLEXASIO_FETCHING)"
  nsExec::ExecToLog 'powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "$PLUGINSDIRlexasio.ps1" -Dest "$PLUGINSDIR"'
  Pop $0
  ${If} $0 == 0
    DetailPrint "$(CANTUS_FLEXASIO_DONE)"
  ${ElseIf} $0 == 11
    DetailPrint "$(CANTUS_FLEXASIO_HELD)"
  ${Else}
    DetailPrint "$(CANTUS_FLEXASIO_FAILED)"
  ${EndIf}
  SetOutPath "$INSTDIR"
SectionEnd
!endif

!macro REMEMBER SEC NAME
  ${If} ${SectionIsSelected} ${SEC}
    WriteRegDWORD HKCU "${REG_APP}" "${NAME}" 1
  ${Else}
    WriteRegDWORD HKCU "${REG_APP}" "${NAME}" 0
  ${EndIf}
!macroend

; Hidden: remember the choices (for /UPDATE), and take away what was unticked this time.
Section "-Remember"
  !insertmacro REMEMBER ${SecAssoc} "Assoc"
  !insertmacro REMEMBER ${SecDesktop} "Desktop"
  !insertmacro REMEMBER ${SecClap} "Clap"
  !insertmacro REMEMBER ${SecVst3} "Vst3"
  ${IfNot} ${SectionIsSelected} ${SecDesktop}
    Delete "$DESKTOP\Cantus.lnk"
  ${EndIf}
  ${IfNot} ${SectionIsSelected} ${SecClap}
    !insertmacro CANTUS_CLAP_REMOVE
  ${EndIf}
  ${IfNot} ${SectionIsSelected} ${SecVst3}
    !insertmacro CANTUS_VST3_REMOVE
  ${EndIf}
  ${IfNot} ${SectionIsSelected} ${SecAssoc}
    ReadRegStr $R0 HKCU "Software\Classes\.cantus" ""
    ${If} $R0 == "${PROGID}"
      DeleteRegValue HKCU "Software\Classes\.cantus" ""
      !insertmacro NOTIFY_ASSOCIATIONS
    ${EndIf}
  ${EndIf}
SectionEnd

!insertmacro MUI_FUNCTION_DESCRIPTION_BEGIN
  !insertmacro MUI_DESCRIPTION_TEXT ${SecApp} "$(CANTUS_DESC_APP)"
  !insertmacro MUI_DESCRIPTION_TEXT ${SecAssoc} "$(CANTUS_DESC_ASSOC)"
  !insertmacro MUI_DESCRIPTION_TEXT ${SecDesktop} "$(CANTUS_DESC_DESKTOP)"
  !insertmacro MUI_DESCRIPTION_TEXT ${SecClap} "$(CANTUS_DESC_CLAP)"
  !insertmacro MUI_DESCRIPTION_TEXT ${SecVst3} "$(CANTUS_DESC_VST3)"
  !if "${FLAVOR}" == "asio"
    !insertmacro MUI_DESCRIPTION_TEXT ${SecFlexAsio} "$(CANTUS_DESC_FLEXASIO)"
  !endif
!insertmacro MUI_FUNCTION_DESCRIPTION_END

; A choice stored by an earlier install (1 or 0) wins over the section's default.
!macro RESTORE SEC NAME
  ClearErrors
  ReadRegDWORD $R2 HKCU "${REG_APP}" "${NAME}"
  ${IfNot} ${Errors}
    ${If} $R2 == 1
      !insertmacro SelectSection ${SEC}
    ${Else}
      !insertmacro UnselectSection ${SEC}
    ${EndIf}
  ${EndIf}
!macroend

Function .onInit
  !insertmacro RESTORE ${SecAssoc} "Assoc"
  !insertmacro RESTORE ${SecDesktop} "Desktop"
  !insertmacro RESTORE ${SecClap} "Clap"
  !insertmacro RESTORE ${SecVst3} "Vst3"
FunctionEnd

Function .onInstSuccess
  ${GetParameters} $R0
  ClearErrors
  ${GetOptions} $R0 "/RELAUNCH" $R1
  ${IfNot} ${Errors}
    Exec '"$INSTDIR\cantus.exe"'
  ${EndIf}
FunctionEnd

; ---- the uninstaller -----------------------------------------------------------------

Section "!un.$(CANTUS_UN_SECTION_APP)" UnSecApp
  SectionIn RO
  Call un.WaitForCantus
  !insertmacro CANTUS_APP_REMOVE
  !insertmacro CANTUS_CLAP_REMOVE
  !insertmacro CANTUS_VST3_REMOVE
  Delete "$SMPROGRAMS\Cantus.lnk"
  Delete "$DESKTOP\Cantus.lnk"

  ; The association, only where it is still ours.
  ReadRegStr $R0 HKCU "Software\Classes\.cantus" ""
  ${If} $R0 == "${PROGID}"
    DeleteRegValue HKCU "Software\Classes\.cantus" ""
  ${EndIf}
  DeleteRegValue HKCU "Software\Classes\.cantus\OpenWithProgids" "${PROGID}"
  DeleteRegKey /ifempty HKCU "Software\Classes\.cantus\OpenWithProgids"
  DeleteRegKey /ifempty HKCU "Software\Classes\.cantus"
  DeleteRegKey HKCU "Software\Classes\${PROGID}"
  DeleteRegKey HKCU "Software\Classes\Applications\cantus.exe"
  !insertmacro NOTIFY_ASSOCIATIONS

  DeleteRegKey HKCU "${REG_UNINST}"
  DeleteRegKey HKCU "${REG_APP}"
  Delete "$INSTDIR\uninstall.exe"
  RMDir "$INSTDIR"
SectionEnd

; Off unless ticked: settings, presets, recordings without a project and autosaves are
; the user's, not the program's.
Section /o "un.$(CANTUS_UN_SECTION_DATA)" UnSecData
  RMDir /r "$LOCALAPPDATA\Cantus"
SectionEnd

!insertmacro MUI_UNFUNCTION_DESCRIPTION_BEGIN
  !insertmacro MUI_DESCRIPTION_TEXT ${UnSecApp} "$(CANTUS_UN_DESC_APP)"
  !insertmacro MUI_DESCRIPTION_TEXT ${UnSecData} "$(CANTUS_UN_DESC_DATA)"
!insertmacro MUI_UNFUNCTION_DESCRIPTION_END
