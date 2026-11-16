#!/bin/bash
# Prepara o TinyTeX que vai no instalador do ÉguaTex (pasta padrao/TinyTeX).
# Precisa de internet. Roda no Git Bash: bash scripts/preparar-tinytex.sh
#
# Historico:
# - 29/10/2026: abntex2, babel-portuges, hyphen-portuguese (TCC em portugues).
# - 11/11/2026 (v31): pacotes e fontes que faltavam nos projetos reais testados
#   (TU Berlin, Inria, ACM TechBrief, MUL poster, rpgtex, TU Delft). Sem eles o
#   documento parava (arquivo .sty nao encontrado) ou o fontspec trocava a fonte que
#   nao achava pela nullfont, e o texto saia faltando ou encavalado.
#
# O fonts.conf do TinyTeX aponta as pastas de fontes da maquina onde ele foi montado
# (C:/Users/runneradmin/...). Quem corrige e o proprio ÉguaTex na primeira abertura
# (compilador.rs: corrigir_fonts_conf), porque so ele sabe a pasta onde o TinyTeX ficou.
#
# Rodar de novo gera outro TinyTeX: o mirror do CTAN so tem os pacotes do dia, e o
# resultado nao bate com o TinyTeX do Release deps-tinytex-2026-11-11 (DEPENDENCIAS.md).
set -e
RAIZ="$(cd "$(dirname "$0")/../../eguatex-tinytex/padrao" && pwd)/TinyTeX"
TLMGR="$RAIZ/bin/windows/tlmgr.bat"
export PATH="$RAIZ/bin/windows:$PATH"

# o tlmgr muda o TinyTeX no lugar; o selo do scripts/baixar-dependencias.sh deixa de valer
if [ -f "$RAIZ.selo" ]; then
  rm -f "$RAIZ.selo"
  echo "Selo $RAIZ.selo apagado: a pasta TinyTeX deixa de bater com o Release deps-tinytex-2026-11-11."
fi

# copia do banco de pacotes antes de mexer
cp "$RAIZ/tlpkg/texlive.tlpdb" "$(dirname "$RAIZ")/texlive.tlpdb.antes-$(date +%d-%m)"

PACOTES=(
  # 29/10
  abntex2 babel-portuges hyphen-portuguese
  # 11/11: TU Berlin (baposter com arev, que usa as fontes MDA do mathdesign, e babel ngerman)
  arev mathdesign babel-german hyphen-german
  # 11/11: Inria (beamerposter, MnSymbol, fontes Inria Sans, biblatex-apa, acentos do beamer)
  mnsymbol inriafonts beamerposter biblatex-apa multiobjective fontawesome7 qrcode smartdiagram sansmathaccent
  # 11/11: ACM TechBrief e TU Delft (fontes chamadas pelo nome no fontspec)
  libertinus-fonts libertinus-otf dejavu cm-unicode firamath
  # 11/11: rpgtex (capitulares Royal, fontes Kp e Gillius ADF)
  cfr-initials initials kpfonts-otf gillius
)
"$TLMGR" install "${PACOTES[@]}"

# arquivos novos no indice e formatos refeitos (hifenizacao do alemao no pdflatex e no lualatex)
"$RAIZ/bin/windows/mktexlsr.exe"
"$RAIZ/bin/windows/fmtutil-sys.exe" --all

# o ÉguaTex faz o resto na primeira abertura: fonts.conf, fc-cache, luaotfload-tool --update e biber
