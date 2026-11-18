; Crochets de l'installateur NSIS d'Aether.
; - Après l'installation : propose de lancer Aether au démarrage de Windows (jamais en mode silencieux).
; - Avant la désinstallation : retire la tâche de démarrage.
; La tâche est créée par Aether lui-même (même logique que dans Réglages), puis il se ferme aussitôt.

!macro NSIS_HOOK_POSTINSTALL
  ${IfNot} ${Silent}
    StrCpy $R0 "Lancer Aether automatiquement au démarrage de Windows ?$\n$\nVous pourrez changer ce choix à tout moment dans Réglages."
    ${If} $LANGUAGE == 1033
      StrCpy $R0 "Start Aether automatically when Windows starts?$\n$\nYou can change this at any time in Settings."
    ${ElseIf} $LANGUAGE == 3082
      StrCpy $R0 "¿Iniciar Aether automáticamente al arrancar Windows?$\n$\nPuedes cambiarlo cuando quieras en Ajustes."
    ${ElseIf} $LANGUAGE == 1031
      StrCpy $R0 "Aether beim Windows-Start automatisch starten?$\n$\nSie können dies jederzeit in den Einstellungen ändern."
    ${ElseIf} $LANGUAGE == 1040
      StrCpy $R0 "Avviare Aether automaticamente all'avvio di Windows?$\n$\nPuoi cambiarlo in qualsiasi momento nelle Impostazioni."
    ${EndIf}
    MessageBox MB_YESNO|MB_ICONQUESTION "$R0" IDNO aether_no_autostart
      ExecWait '"$INSTDIR\aether.exe" --autostart-on'
    aether_no_autostart:
  ${EndIf}
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  ExecWait '"$INSTDIR\aether.exe" --autostart-off'
!macroend
