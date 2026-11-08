; Recall Clipboard installer additions.
;
; Adds a "What to save" page (Text / Images / Files). The choices are written to
; %APPDATA%\<bundle id>\install-options.json after a successful install; the app
; applies them to its settings on next start and deletes the file (see
; Settings::take_install_options). After that, Settings in the app owns them.

!include nsDialogs.nsh
!include LogicLib.nsh

; Expanded by installer.nsi between the directory and start-menu pages, after
; $PassiveMode and $UpdateMode are declared.
!macro RECALL_OPTIONS_PAGE
  Var RecallPageShown
  Var RecallText
  Var RecallImages
  Var RecallFiles
  Var RecallTextBox
  Var RecallImagesBox
  Var RecallFilesBox

  Page custom RecallOptionsShow RecallOptionsLeave

  Function RecallOptionsShow
    ; Silent, passive and update installs keep whatever the user has already chosen.
    ${If} $PassiveMode = 1
    ${OrIf} $UpdateMode = 1
      Abort
    ${EndIf}

    ; Everything is on until the user changes it (and survives Back/Next).
    ${If} $RecallPageShown != 1
      StrCpy $RecallText ${BST_CHECKED}
      StrCpy $RecallImages ${BST_CHECKED}
      StrCpy $RecallFiles ${BST_CHECKED}
    ${EndIf}

    !insertmacro MUI_HEADER_TEXT "What to save" "Choose what Recall Clipboard keeps in your history."
    nsDialogs::Create 1018
    Pop $0
    ${If} $0 == error
      Abort
    ${EndIf}

    ${NSD_CreateLabel} 0 0 100% 20u "Save these kinds of copies:"
    Pop $0

    ${NSD_CreateCheckbox} 10u 22u 100% 12u "&Text (anything you copy as text)"
    Pop $RecallTextBox
    ${NSD_SetState} $RecallTextBox $RecallText

    ${NSD_CreateCheckbox} 10u 38u 100% 12u "&Images (screenshots and copied pictures)"
    Pop $RecallImagesBox
    ${NSD_SetState} $RecallImagesBox $RecallImages

    ${NSD_CreateCheckbox} 10u 54u 100% 12u "&Files (files and folders copied in File Explorer)"
    Pop $RecallFilesBox
    ${NSD_SetState} $RecallFilesBox $RecallFiles

    ${NSD_CreateLabel} 0 80u 100% 30u "You can change this any time in Settings. Everything stays on this PC, and copies from password managers are always skipped."
    Pop $0

    nsDialogs::Show
  FunctionEnd

  Function RecallOptionsLeave
    ${NSD_GetState} $RecallTextBox $RecallText
    ${NSD_GetState} $RecallImagesBox $RecallImages
    ${NSD_GetState} $RecallFilesBox $RecallFiles
    StrCpy $RecallPageShown 1
  FunctionEnd
!macroend

; Turns a checkbox state into a JSON boolean in $R9.
!macro RECALL_JSON_BOOL STATE
  ${If} ${STATE} == ${BST_CHECKED}
    StrCpy $R9 "true"
  ${Else}
    StrCpy $R9 "false"
  ${EndIf}
!macroend

!macro NSIS_HOOK_POSTINSTALL
  ${If} $RecallPageShown == 1
    ; The app's data folder is per user, even for per-machine installs.
    SetShellVarContext current
    CreateDirectory "$APPDATA\${BUNDLEID}"
    FileOpen $R8 "$APPDATA\${BUNDLEID}\install-options.json" w
    ${If} $R8 != ""
      FileWrite $R8 "{"
      !insertmacro RECALL_JSON_BOOL $RecallText
      FileWrite $R8 '"recordText": $R9, '
      !insertmacro RECALL_JSON_BOOL $RecallImages
      FileWrite $R8 '"recordImages": $R9, '
      !insertmacro RECALL_JSON_BOOL $RecallFiles
      FileWrite $R8 '"recordFiles": $R9}'
      FileClose $R8
    ${EndIf}
    !if "${INSTALLMODE}" == "perMachine"
      SetShellVarContext all
    !endif
  ${EndIf}
!macroend
