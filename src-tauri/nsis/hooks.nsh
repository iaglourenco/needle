; Hooks customizados do instalador NSIS do Needle.
; Documentação: https://tauri.app/distribute/windows-installer/#installer-hooks

!macro NSIS_HOOK_PREUNINSTALL
  ; Encerra o Needle se estiver rodando — senão o Windows não consegue
  ; apagar needle.exe (arquivo em uso).
  nsExec::Exec 'taskkill /F /IM needle.exe'
  Pop $0
  Sleep 500

  ; Remove as entradas do Needle de ~/.claude/settings.json antes do
  ; executável ser apagado. Não mexe em hooks de outras ferramentas.
  IfFileExists "$INSTDIR\needle.exe" 0 +3
    nsExec::Exec '"$INSTDIR\needle.exe" remove-hooks'
    Pop $0
!macroend
