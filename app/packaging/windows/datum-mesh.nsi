; Datum Mesh setup (NSIS): per user, under LOCALAPPDATA, with the Gmsh mesher beside the
; program (a separate program, GPL; its licence goes with it). The release workflow builds it as
; DatumMesh-<version>-setup.exe. The in-app updater runs it with /S /RUN: silent, then starts the
; new version. Installing over Datum Mesh 0.6 (the earlier program) replaces it in the same folder,
; under the same uninstall entry, and keeps its shortcuts working.
Unicode true
!include "FileFunc.nsh"
!include "LogicLib.nsh"

!ifndef VERSION
  !define VERSION "0.0.0"
!endif
!ifndef SRC
  !define SRC "..\..\..\out\stage"
!endif

Name "Datum Mesh"
OutFile "DatumMesh-${VERSION}-setup.exe"
InstallDir "$LOCALAPPDATA\Datum Mesh"
InstallDirRegKey HKCU "Software\Datum Mesh" "InstallDir"
RequestExecutionLevel user
SetCompressor /SOLID lzma
Icon "..\..\assets\icons\app\windows\exports\datum-mesh.ico"
UninstallIcon "..\..\assets\icons\app\windows\exports\datum-mesh.ico"
BrandingText "Datum Mesh ${VERSION}"
VIProductVersion "${VERSION}.0"
VIAddVersionKey "ProductName" "Datum Mesh"
VIAddVersionKey "FileDescription" "Datum Mesh setup"
VIAddVersionKey "FileVersion" "${VERSION}"
VIAddVersionKey "ProductVersion" "${VERSION}"
VIAddVersionKey "CompanyName" "Edgar Mendonca"
VIAddVersionKey "LegalCopyright" "MIT licence; Gmsh: GPL (separate program)"

Page directory
Page instfiles
UninstPage uninstConfirm
UninstPage instfiles

!define UNINST "Software\Microsoft\Windows\CurrentVersion\Uninstall\Datum Mesh"

Section "Datum Mesh"
  ; The program may still be closing (the updater starts this as it exits).
  Sleep 1500
  SetOutPath "$INSTDIR"
  File "${SRC}\datum-mesh.exe"
  File "${SRC}\LICENSE.txt"
  File "${SRC}\NOTICE.txt"
  ; The earlier program's name: its shortcuts and file links still open Datum Mesh.
  ${If} ${FileExists} "$INSTDIR\Datum Mesh.exe"
    CopyFiles /SILENT "$INSTDIR\datum-mesh.exe" "$INSTDIR\Datum Mesh.exe"
  ${EndIf}
  SetOutPath "$INSTDIR\gmsh"
  File /r "${SRC}\gmsh\*.*"
  SetOutPath "$INSTDIR"
  WriteUninstaller "$INSTDIR\uninstall.exe"
  CreateShortcut "$SMPROGRAMS\Datum Mesh.lnk" "$INSTDIR\datum-mesh.exe"
  CreateShortcut "$DESKTOP\Datum Mesh.lnk" "$INSTDIR\datum-mesh.exe"
  WriteRegStr HKCU "Software\Datum Mesh" "InstallDir" "$INSTDIR"
  WriteRegStr HKCU "${UNINST}" "DisplayName" "Datum Mesh"
  WriteRegStr HKCU "${UNINST}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "${UNINST}" "Publisher" "Edgar Mendonca"
  WriteRegStr HKCU "${UNINST}" "DisplayIcon" "$INSTDIR\datum-mesh.exe"
  WriteRegStr HKCU "${UNINST}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "${UNINST}" "UninstallString" '"$INSTDIR\uninstall.exe"'
  WriteRegStr HKCU "${UNINST}" "QuietUninstallString" '"$INSTDIR\uninstall.exe" /S'
  WriteRegDWORD HKCU "${UNINST}" "NoModify" 1
  WriteRegDWORD HKCU "${UNINST}" "NoRepair" 1
  ; Design files open in Datum Mesh.
  WriteRegStr HKCU "Software\Classes\.dmx" "" "DatumMesh.Design"
  WriteRegStr HKCU "Software\Classes\DatumMesh.Design" "" "Datum Mesh design"
  WriteRegStr HKCU "Software\Classes\DatumMesh.Design\DefaultIcon" "" "$INSTDIR\datum-mesh.exe,0"
  WriteRegStr HKCU "Software\Classes\DatumMesh.Design\shell\open\command" "" '"$INSTDIR\datum-mesh.exe" "%1"'
  ; Started by the updater: open the new version once installed.
  ${GetParameters} $R0
  ClearErrors
  ${GetOptions} $R0 "/RUN" $R1
  ${IfNot} ${Errors}
    Exec '"$INSTDIR\datum-mesh.exe"'
  ${EndIf}
SectionEnd

Section "Uninstall"
  Delete "$INSTDIR\datum-mesh.exe"
  Delete "$INSTDIR\Datum Mesh.exe"
  Delete "$INSTDIR\LICENSE.txt"
  Delete "$INSTDIR\NOTICE.txt"
  RMDir /r "$INSTDIR\gmsh"
  Delete "$INSTDIR\uninstall.exe"
  RMDir "$INSTDIR"
  Delete "$SMPROGRAMS\Datum Mesh.lnk"
  Delete "$DESKTOP\Datum Mesh.lnk"
  DeleteRegKey HKCU "${UNINST}"
  DeleteRegKey HKCU "Software\Datum Mesh"
  DeleteRegKey HKCU "Software\Classes\DatumMesh.Design"
  DeleteRegValue HKCU "Software\Classes\.dmx" ""
SectionEnd
