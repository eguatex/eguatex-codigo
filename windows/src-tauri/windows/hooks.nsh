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
  System::Call 'advapi32::CredDeleteW(w "google-drive.eguatex", i 1, i 0) i'
  System::Call 'advapi32::CredDeleteW(w "openai-compativel.eguatex", i 1, i 0) i'
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

; O fonts.conf do TinyTeX aponta as pastas de fontes da maquina onde ele foi montado
; (C:/Users/runneradmin/AppData/Roaming/TinyTeX/...). Troca cada linha
; <dir>.../texmf-dist/fonts/...</dir> pela pasta real, como o app faz (compilador.rs,
; corrigir_fonts_conf). Corrigido aqui, o app de outra conta do Windows nao precisa gravar nele.
; Entrada: $R1 = %PROGRAMDATA%.
!macro EGUATEX_CORRIGIR_FONTS_CONF
  Push $R2
  Push $R3
  Push $R4
  Push $R5
  Push $R6
  Push $R7
  Push $R8
  StrCpy $R3 "$R1\EguaTex\TinyTeX\texmf-var\fonts\conf\fonts.conf"
  ${If} ${FileExists} "$R3"
    ${WordReplace} "$R1\EguaTex\TinyTeX" "\" "/" "+" $R2
    ClearErrors
    FileOpen $R4 "$R3" r
    ${IfNot} ${Errors}
      FileOpen $R5 "$R3.novo" w
      ${IfNot} ${Errors}
        ${Do}
          ClearErrors
          FileRead $R4 $R6
          ${If} ${Errors}
            ${ExitDo}
          ${EndIf}
          StrCpy $R7 $R6 5
          ${If} $R7 == "<dir>"
            ${StrCase} $R7 $R6 "L"
            ${StrLoc} $R8 $R7 "/texmf-dist/fonts/" ">"
            ${If} $R8 != ""
              ; o resto da linha (pasta de fontes, </dir> e a quebra) fica como estava
              StrCpy $R6 $R6 "" $R8
              StrCpy $R6 "<dir>$R2$R6"
            ${EndIf}
          ${EndIf}
          FileWrite $R5 $R6
        ${Loop}
        FileClose $R5
        FileClose $R4
        ; troca so com o novo pronto; se a troca falhar, o original volta
        Delete "$R3.velho"
        Rename "$R3" "$R3.velho"
        Rename "$R3.novo" "$R3"
        ${If} ${FileExists} "$R3"
          Delete "$R3.velho"
        ${Else}
          Rename "$R3.velho" "$R3"
          Delete "$R3.novo"
        ${EndIf}
      ${Else}
        FileClose $R4
      ${EndIf}
    ${EndIf}
  ${EndIf}
  Pop $R8
  Pop $R7
  Pop $R6
  Pop $R5
  Pop $R4
  Pop $R3
  Pop $R2
!macroend

; Preparo do TinyTeX feito pelo instalador, com a conta que instala: cache de fontes do XeLaTeX
; (fc-cache) e base de nomes do LuaLaTeX (luaotfload-tool --update), no texmf-var. Os formatos
; (.fmt) ja vem prontos no TinyTeX. A marca eguatex-preparo-1 (PREPARO_TINYTEX em compilador.rs)
; faz o app pular o mesmo preparo na primeira abertura.
!macro EGUATEX_PREPARAR_TINYTEX
  Push $R2
  DetailPrint "Preparando as fontes do LaTeX..."
  nsExec::Exec '"$R1\EguaTex\TinyTeX\bin\windows\fc-cache.exe" -v'
  Pop $R2
  nsExec::Exec '"$R1\EguaTex\TinyTeX\bin\windows\luaotfload-tool.exe" --update'
  Pop $R2
  ClearErrors
  FileOpen $R2 "$R1\EguaTex\TinyTeX\texmf-var\eguatex-preparo-1" w
  ${IfNot} ${Errors}
    FileWrite $R2 "fc-cache e luaotfload-tool --update (instalador)$\r$\n"
    FileClose $R2
  ${EndIf}
  Pop $R2
!macroend

; Permissao protegida do TinyTeX, a mesma pelo Rename e pelo CopyFiles. O TinyTeX movido de
; %LOCALAPPDATA% leva as permissoes da conta que instalou (outra conta nao le nada); o copiado
; herda as do %PROGRAMDATA%, onde os Usuarios criam arquivos (outra conta plantaria uma DLL em
; bin\windows). O icacls tira a heranca e deixa so: SISTEMA (S-1-5-18), Administradores
; (S-1-5-32-544) e a conta que instala com controle total, e os Usuarios (S-1-5-32-545) com
; leitura e execucao. SIDs valem em Windows de qualquer idioma ("Users" nao existe em portugues).
; Nenhuma gravacao para os Usuarios: quem trocasse o pdflatex.exe, um .fmt ou o cache Lua do
; luaotfload rodaria codigo na conta de quem compilasse depois. Outra conta grava TEXMFVAR e o
; cache de fontes numa pasta so dela (o app cuida). O icacls sem /T ja propaga para dentro.
!macro EGUATEX_PERMISSOES_TINYTEX
  Push $R2
  Push $R3
  ReadEnvStr $R2 USERNAME
  ReadEnvStr $R3 USERDOMAIN
  ${If} $R3 != ""
    StrCpy $R2 "$R3\$R2"
  ${EndIf}
  nsExec::Exec '"$SYSDIR\icacls.exe" "$R1\EguaTex\TinyTeX" /inheritance:r /grant:r *S-1-5-18:(OI)(CI)F *S-1-5-32-544:(OI)(CI)F "$R2:(OI)(CI)F" *S-1-5-32-545:(OI)(CI)RX /C /Q'
  Pop $R3
  Pop $R3
  Pop $R2
!macroend

; Pasta das contas cujo %LOCALAPPDATA% tem acento e nao tem nome curto: o app cria nela uma
; subpasta por conta, so daquela conta (compilador.rs, pasta_restrita). Sem heranca: SISTEMA e
; Administradores com controle total; os Usuarios so listam e criam subpasta (AD e RX nesta
; pasta, sem passar para as de dentro), sem apagar nem renomear a de outra conta.
!macro EGUATEX_PASTA_CONTAS
  Push $R2
  CreateDirectory "$R1\EguaTex\contas"
  nsExec::Exec '"$SYSDIR\icacls.exe" "$R1\EguaTex\contas" /inheritance:r /grant:r *S-1-5-18:(OI)(CI)F *S-1-5-32-544:(OI)(CI)F *S-1-5-32-545:(AD,RX) /C /Q'
  Pop $R2
  Pop $R2
!macroend

; O Windows guarda os icones num cache e, numa atualizacao, o atalho da Area de Trabalho, o menu
; Iniciar e a barra de tarefas continuavam com a logo da versao anterior. SHChangeNotify com
; SHCNE_ASSOCCHANGED (0x08000000) e SHCNF_FLUSH (0x1000) manda o Explorer reler os icones, e o
; ie4uinit -show (do proprio Windows) refaz o cache de icones do Windows 10 e 11.
!macro EGUATEX_ATUALIZAR_ICONES
  Push $R2
  System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0x1000, p 0, p 0)'
  nsExec::Exec '"$SYSDIR\ie4uinit.exe" -show'
  Pop $R2
  Pop $R2
!macroend

; O TinyTeX nao funciona em pasta com acento: a pasta do programa e %LOCALAPPDATA%\ÉguaTex
; e o pdflatex nao acha o pdflatex.fmt. Move o TinyTeX para %PROGRAMDATA%\EguaTex,
; que nunca tem acento (o nome de usuario pode ter). O app procura nesse lugar primeiro.
; O nome passou de EguaTex para EguaTeX (so muda maiuscula). O Windows mantem o nome antigo
; quando o atalho e regravado por cima, entao renomeia passando por um nome temporario.
!macro EGUATEX_RENOMEAR_ATALHO ARQ
  ${If} ${FileExists} "${ARQ}"
    Delete "${ARQ}.tmp-nome"
    Rename "${ARQ}" "${ARQ}.tmp-nome"
    Rename "${ARQ}.tmp-nome" "${ARQ}"
  ${EndIf}
!macroend
!macro EGUATEX_NOME_ATALHOS
  !insertmacro EGUATEX_RENOMEAR_ATALHO "$SMPROGRAMS\${PRODUCTNAME}.lnk"
  !insertmacro EGUATEX_RENOMEAR_ATALHO "$DESKTOP\${PRODUCTNAME}.lnk"
!macroend

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
  ${If} ${FileExists} "$R1\EguaTex\TinyTeX\bin\windows\pdflatex.exe"
    !insertmacro EGUATEX_CORRIGIR_FONTS_CONF
    !insertmacro EGUATEX_PREPARAR_TINYTEX
    !insertmacro EGUATEX_PERMISSOES_TINYTEX
    !insertmacro EGUATEX_PASTA_CONTAS
  ${EndIf}
  ${If} $EguaAvisoEstado == 1
    FileOpen $R1 "$INSTDIR\avisar-versao-nova.txt" w
    FileWrite $R1 "1"
    FileClose $R1
  ${EndIf}
  !insertmacro EGUATEX_NOME_ATALHOS
  !insertmacro EGUATEX_ATUALIZAR_ICONES
  Pop $R1
!macroend

; Caixa "Avisar quando houver versao nova" na tela de boas-vindas, DESMARCADA de fabrica.
; Marcada, o pos-instalacao grava avisar-versao-nova.txt ao lado do programa; o app le,
; liga o aviso e apaga o arquivo na primeira abertura. A instalacao silenciosa (/S) nao grava.
Var EguaAvisoCaixa
Var EguaAvisoEstado
!define MUI_PAGE_CUSTOMFUNCTION_SHOW EguaAvisoMostrar
!define MUI_PAGE_CUSTOMFUNCTION_LEAVE EguaAvisoSair
Function EguaAvisoMostrar
  ${NSD_CreateCheckbox} 120u 172u 195u 12u "Avisar quando houver vers${U+00E3}o nova"
  Pop $EguaAvisoCaixa
  SetCtlColors $EguaAvisoCaixa "" "FFFFFF"
  ${If} $EguaAvisoEstado == 1
    ${NSD_Check} $EguaAvisoCaixa
  ${EndIf}
FunctionEnd
Function EguaAvisoSair
  ${NSD_GetState} $EguaAvisoCaixa $EguaAvisoEstado
FunctionEnd
