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
  DeleteRegKey HKCU "Software\Classes\*\shell\FileChameleonHang"
!macroend

!macro NSIS_HOOK_POSTINSTALL
  ; The exe keeps its path across updates, so Explorer and the taskbar may keep showing the
  ; previous build's icon from their cache. Tell the shell that icons changed
  ; (SHCNE_ASSOCCHANGED, SHCNF_IDLIST) so it reloads them.
  System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0, p 0, p 0)'
!macroend
