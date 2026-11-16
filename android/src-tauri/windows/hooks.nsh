; Ganchos do instalador NSIS do EguaTex (ligados em tauri.conf.json > bundle > windows > nsis).

; Apaga o login do Google e a chave de IA, que ficam no cofre de credenciais do Windows,
; e a configuracao da IA (%APPDATA%\eguatex). Nada disso fica nas pastas do programa,
; entao o desinstalador do Tauri nao apaga sozinho.
!macro EGUATEX_APAGAR_LOGIN
  ; CredDeleteW apaga so as duas entradas do EguaTex (tipo 1 = generica), sem abrir
  ; outro programa. Chamar o cmdkey.exe pelo instalador parece roubo de credenciais
  ; para o antivirus.
  System::Call 'advapi32::CredDeleteW(w "google-oauth.eguatex", i 1, i 0) i'
  System::Call 'advapi32::CredDeleteW(w "gemini.eguatex", i 1, i 0) i'
  RmDir /r "$APPDATA\eguatex"
!macroend

; Instalacao nova comeca do zero: sem conta logada e sem a lista de projetos de uma
; instalacao anterior. Conta como nova quando falta o registro do local de instalacao,
; que o desinstalador so apaga quando a pessoa marca "Remover dados do programa".
; Os projetos (pastas em Documentos\EguaTex) nunca sao apagados.
!macro NSIS_HOOK_PREINSTALL
  ${If} $UpdateMode <> 1
    Push $R0
    ReadRegStr $R0 SHCTX "${MANUPRODUCTKEY}" ""
    ${If} $R0 == ""
      !insertmacro EGUATEX_APAGAR_LOGIN
      RmDir /r "$APPDATA\${BUNDLEID}"
      RmDir /r "$LOCALAPPDATA\${BUNDLEID}"
    ${EndIf}
    Pop $R0
  ${EndIf}
!macroend

; Desinstalar sempre sai da conta Google (fora de uma atualizacao). A lista de projetos e
; as preferencias so saem com "Remover dados do programa", como no Tauri.
!macro NSIS_HOOK_POSTUNINSTALL
  ${If} $UpdateMode <> 1
    !insertmacro EGUATEX_APAGAR_LOGIN
    Push $R1
    ReadEnvStr $R1 PROGRAMDATA
    RmDir /r "$R1\EguaTex"
    Pop $R1
  ${EndIf}
!macroend

; O TinyTeX nao funciona em pasta com acento: a pasta do programa e %LOCALAPPDATA%\ÉguaTex
; e o pdflatex nao acha o pdflatex.fmt. Move o TinyTeX para %PROGRAMDATA%\EguaTex,
; que nunca tem acento (o nome de usuario pode ter). O app procura nesse lugar primeiro.
!macro NSIS_HOOK_POSTINSTALL
  Push $R1
  ReadEnvStr $R1 PROGRAMDATA
  ${If} ${FileExists} "$INSTDIR\tex\TinyTeX\bin\windows\pdflatex.exe"
    RmDir /r "$R1\EguaTex\TinyTeX"
    CreateDirectory "$R1\EguaTex"
    Rename "$INSTDIR\tex\TinyTeX" "$R1\EguaTex\TinyTeX"
    ${IfNot} ${FileExists} "$R1\EguaTex\TinyTeX\bin\windows\pdflatex.exe"
      CopyFiles /SILENT "$INSTDIR\tex\TinyTeX" "$R1\EguaTex"
    ${EndIf}
    ${If} ${FileExists} "$R1\EguaTex\TinyTeX\bin\windows\pdflatex.exe"
      RmDir /r "$INSTDIR\tex"
    ${EndIf}
  ${EndIf}
  Pop $R1
!macroend
