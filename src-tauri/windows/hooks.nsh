; NSIS installer hooks for File Chameleon (wired up in tauri.conf.json > bundle.windows.nsis.installerHooks).

!macro NSIS_HOOK_PREINSTALL
  ; An older, unrelated app also called "File Chameleon" installs itself (with Inno Setup) into
  ; the same default folder. Never mix files with it: its uninstaller could remove ours.
  ; Only the untouched default is redirected; a folder the user picked is kept as is.
  ${If} $INSTDIR == "$LOCALAPPDATA\${PRODUCTNAME}"
  ${AndIf} ${FileExists} "$INSTDIR\unins000.exe"
    StrCpy $INSTDIR "$LOCALAPPDATA\Programs\${PRODUCTNAME}"
    SetOutPath $INSTDIR
  ${EndIf}
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  ; Explorer right-click verb "Convert with File Chameleon" (opt-in, written by the app).
  DeleteRegKey HKCU "Software\Classes\*\shell\FileChameleon"
!macroend
