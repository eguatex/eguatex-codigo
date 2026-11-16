#!/usr/bin/env bash
# Baixa os arquivos do motor LaTeX do ÉguaTex Android (busytex, TeX Live 2023 em WebAssembly)
# e confere cada um pelo SHA-256. Origem e lista em DEPENDENCIAS.md.
#   bash scripts/baixar-dependencias.sh            instala o que falta
#   bash scripts/baixar-dependencias.sh --forcar   reinstala tudo; a pasta atual vira <pasta>.antiga-AAAAMMDD
#
# EGUATEX_DEPS   pasta base dos destinos (padrao: a pasta acima do repositorio, onde o
#                scripts/gerar-apk.sh procura eguatex-android-deps/busytex). O atualizar-tudo.sh
#                do repositorio eguatex nao define esta variavel e usa esse caminho fixo; outra
#                pasta serve para teste ou copia de seguranca.
# EGUATEX_CACHE  pasta dos downloads (padrao: ../eguatex-cache, ao lado do repositorio)
#
# Cada pasta instalada ganha um selo ao lado dela (<pasta>.selo) com nome, versao e SHA-256.
# Sem --forcar: pasta com o selo certo e pulada, pasta sem selo e mantida com aviso e selo
# diferente para o script. O selo so prova o download; --forcar sempre renomeia e reinstala.
# A chave de assinatura (eguatex-android-deps/chave) nunca passa por este script.
set -euo pipefail

# ---------------------------------------------------------------------------
# Itens: nome|versao|destino (relativo a EGUATEX_DEPS)|modo|pasta de dentro do arquivo
#   modo copiar: arquivos soltos copiados para o destino (o unico modo neste repositorio;
#   o modo extrair, com 7z, existe so no script do repositorio eguatex)
TAG_BUSYTEX=build_wasm_4499aa69fd3cf77ad86a47287d9a5193cf5ad993_7936974349_1
URL_BUSYTEX=https://github.com/busytex/busytex/releases/download/$TAG_BUSYTEX
ITENS=(
  "busytex|$TAG_BUSYTEX|eguatex-android-deps/busytex|copiar|"
)
# Arquivos: item|arquivo|sha256|origem (https://..., baixado pelo curl)
ARQUIVOS=(
  "busytex|busytex.js|f381d9ba816d9a4389d429f54fe85f13396aa2f822973d9b66e1f527ced3434b|$URL_BUSYTEX/busytex.js"
  "busytex|busytex.wasm|a7785b59bfddb8667e7738443f111430e3ee2bd3ec0c1c26629267849e091415|$URL_BUSYTEX/busytex.wasm"
  "busytex|busytex_pipeline.js|f8741cfd1fb0ac3c11daeee8386bc024f59d8b14c3000ad64b9c00dce7d61265|$URL_BUSYTEX/busytex_pipeline.js"
  "busytex|busytex_worker.js|623136034d61d52be43df97bbe320612f0cca139b955431fb168c529531d21c4|$URL_BUSYTEX/busytex_worker.js"
  "busytex|texlive-basic.data|c955651ca293f09b2e57d6360fa2cb61ad710b4ae51e9fbc04b049477a8aeb71|$URL_BUSYTEX/texlive-basic.data"
  "busytex|texlive-basic.js|23cc4ce427d27d555fe2a050b97ff8c95bb331425018e9159130e74b62373ff1|$URL_BUSYTEX/texlive-basic.js"
  "busytex|texlive-basic.js.providespackage.txt|a248a9dc6a89e69c291a773202e27fa300212b33e6b351aeba5d751e6bc53a54|$URL_BUSYTEX/texlive-basic.js.providespackage.txt"
  "busytex|texlive-basic.txt|3d153bd5444e5cb6b48fe0dad8aa26313a4748a8e0ccacc6f72df94db63d2e32|$URL_BUSYTEX/texlive-basic.txt"
  "busytex|ubuntu-texlive-fonts-recommended.data|3154397a58db2ac62d6bcf99e44786c6ea62cc31c2343adc42d51f2088b6a3f1|$URL_BUSYTEX/ubuntu-texlive-fonts-recommended.data"
  "busytex|ubuntu-texlive-fonts-recommended.js|708f60e5fc46b64204e8361a0d0939b3f8f0713f9b7fae379baa67e5dbe0c37a|$URL_BUSYTEX/ubuntu-texlive-fonts-recommended.js"
  "busytex|ubuntu-texlive-latex-base.data|0dd2dfaaa2e6a9674090cbf8cf3a01419ce37063080f762aee0d82ad7263ead8|$URL_BUSYTEX/ubuntu-texlive-latex-base.data"
  "busytex|ubuntu-texlive-latex-base.js|807f0bfbe66ff919c1799737acd751dfba6c1ca2afd8d71db2bb30c3c5c62144|$URL_BUSYTEX/ubuntu-texlive-latex-base.js"
  "busytex|ubuntu-texlive-latex-extra.data|fb4dc0440f4b2bd2d5138dbb5689e6a33cd20c9704934422436cb97af5d623c4|$URL_BUSYTEX/ubuntu-texlive-latex-extra.data"
  "busytex|ubuntu-texlive-latex-extra.js|a99a68b79e6a93f3ca9d94f46afe8a62e7e06b6d4c5e90a705ec15fc42d8ecda|$URL_BUSYTEX/ubuntu-texlive-latex-extra.js"
  "busytex|ubuntu-texlive-latex-extra.js.providespackage.txt|8ee355729f1ac204b5150a5a33836d166569de4ac70ecf0fafa89c9cac01f8c2|$URL_BUSYTEX/ubuntu-texlive-latex-extra.js.providespackage.txt"
  "busytex|ubuntu-texlive-latex-recommended.data|fa9a03f928a0af0e9c5a24e2ce2956e64db9d580dd2c3a7517176aac16dc9006|$URL_BUSYTEX/ubuntu-texlive-latex-recommended.data"
  "busytex|ubuntu-texlive-latex-recommended.js|bbd975921a74adae9119988ae8a740ca9f4837db772061d313c67729fba1d9ee|$URL_BUSYTEX/ubuntu-texlive-latex-recommended.js"
  "busytex|ubuntu-texlive-latex-recommended.js.providespackage.txt|3bb8cd12392dfac9eb89e50feb79f810d03e70702bea282af7b45b56cfee7fef|$URL_BUSYTEX/ubuntu-texlive-latex-recommended.js.providespackage.txt"
  "busytex|ubuntu-texlive-science.data|23c5decd471c214f1a4d16eabe02f2fe1a0b5aee94a8708713d03c4616cd5d61|$URL_BUSYTEX/ubuntu-texlive-science.data"
  "busytex|ubuntu-texlive-science.js|cf6ddcc894f0b8a5a9493286edc8abba67280041c21cdfa51d7dadffea85c19a|$URL_BUSYTEX/ubuntu-texlive-science.js"
  "busytex|versions.txt|bb4c0babeb41ae2e5bff11b476fb0dacbadd2a616dbde4de4bf3dd0e6859d299|$URL_BUSYTEX/versions.txt"
)
FERRAMENTAS=(curl sha256sum)
# ---------------------------------------------------------------------------

RAIZ="$(cd "$(dirname "$0")/.." && pwd)"
DEPS="${EGUATEX_DEPS:-$(cd "$RAIZ/.." && pwd)}"
CACHE="${EGUATEX_CACHE:-$(cd "$RAIZ/.." && pwd)/eguatex-cache}"
FORCAR=0
for a in "$@"; do
  case "$a" in
    --forcar) FORCAR=1 ;;
    *) echo "Opcao desconhecida: $a (use --forcar ou nada)"; exit 2 ;;
  esac
done

erro() { echo "ERRO: $*" >&2; exit 1; }

for f in "${FERRAMENTAS[@]}"; do
  command -v "$f" >/dev/null 2>&1 || erro "falta a ferramenta $f no PATH. Instale e rode de novo."
done

hash_de() { sha256sum "$1" | cut -d' ' -f1 | tr -d '\\'; }

# baixa um arquivo para o cache (ou usa o que ja esta la) e confere o SHA-256.
# Hash errado: apaga e baixa de novo uma vez. Errou de novo: para com codigo 1.
baixar() {
  local arquivo="$1" esperado="$2" origem="$3" destino="$CACHE/$1" obtido tentativa
  mkdir -p "$CACHE"
  for tentativa in 1 2; do
    if [ ! -f "$destino" ]; then
      echo "  baixando $arquivo"
      rm -rf "$CACHE/.baixando"; mkdir -p "$CACHE/.baixando"
      case "$origem" in
        https://*)
          curl -fsSL --retry 3 -o "$CACHE/.baixando/$arquivo" "$origem" \
            || erro "falhou o download de $origem" ;;
        *) erro "origem desconhecida: $origem" ;;
      esac
      mv "$CACHE/.baixando/$arquivo" "$destino"
      rm -rf "$CACHE/.baixando"
    fi
    obtido="$(hash_de "$destino")"
    [ "$obtido" = "$esperado" ] && return 0
    rm -f "$destino"
    if [ "$tentativa" = 1 ]; then
      echo "  SHA-256 errado em $arquivo, apagado do cache; baixando de novo"
    else
      erro "SHA-256 errado em $arquivo depois de baixar de novo. Esperado: $esperado. Obtido: $obtido. O destino nao foi tocado."
    fi
  done
}

for item in "${ITENS[@]}"; do
  IFS='|' read -r nome versao rel modo _ <<< "$item"
  [ "$modo" = copiar ] || erro "modo $modo nao existe neste script (so copiar)"
  pasta="$DEPS/$rel"
  selo="$pasta.selo"
  parcial="$pasta.parcial"
  echo "== $nome $versao em $pasta"

  # selo esperado: nome, versao e o SHA-256 de cada arquivo do item
  esperado="nome=$nome"$'\n'"versao=$versao"
  lista=()
  for linha in "${ARQUIVOS[@]}"; do
    IFS='|' read -r dono arq sha origem <<< "$linha"
    [ "$dono" = "$nome" ] || continue
    esperado+=$'\n'"sha256=$sha $arq"
    lista+=("$arq|$sha|$origem")
  done

  # sobra de uma execucao interrompida
  [ -e "$parcial" ] && { echo "  apagando $parcial (sobra de execucao anterior)"; rm -rf "$parcial"; }

  # --forcar sempre reinstala: o selo prova o download, nao que os arquivos da pasta estao inteiros
  if [ -e "$pasta" ] && [ "$FORCAR" != 1 ]; then
    if [ -f "$selo" ] && [ "$(cat "$selo")" = "$esperado" ]; then
      echo "  ja esta"; continue
    fi
    if [ -f "$selo" ]; then
      erro "o selo $selo tem outra versao ou outro SHA-256. Para trocar a pasta, rode com --forcar."
    fi
    echo "  AVISO: a pasta existe sem selo; mantida como esta. Para trocar, rode com --forcar."
    continue
  fi

  for l in "${lista[@]}"; do
    IFS='|' read -r arq sha origem <<< "$l"
    baixar "$arq" "$sha" "$origem"
  done

  mkdir -p "$parcial"
  for l in "${lista[@]}"; do
    IFS='|' read -r arq _ _ <<< "$l"
    cp "$CACHE/$arq" "$parcial/"
  done

  if [ -e "$pasta" ]; then
    # nome livre: .antiga-AAAAMMDD, depois -2, -3...; o mv nunca cai dentro de pasta existente
    base_antiga="$pasta.antiga-$(date +%Y%m%d)"
    antiga="$base_antiga"; n=2
    while [ -e "$antiga" ] || [ -e "$antiga.selo" ]; do antiga="$base_antiga-$n"; n=$((n + 1)); done
    echo "  --forcar: a pasta atual vira $antiga"
    mv "$pasta" "$antiga"
    [ -f "$selo" ] && mv "$selo" "$antiga.selo"
  fi
  mkdir -p "$(dirname "$pasta")"
  mv "$parcial" "$pasta"
  printf '%s\n' "$esperado" > "$selo"
  echo "  instalado"
done
echo "Pronto."
