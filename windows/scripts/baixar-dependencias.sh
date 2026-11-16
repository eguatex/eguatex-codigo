#!/usr/bin/env bash
# Baixa as dependencias grandes do instalador do ÉguaTex para Windows (Pandoc e TinyTeX)
# e confere cada uma pelo SHA-256. Origem, versao e historico em DEPENDENCIAS.md.
#   bash scripts/baixar-dependencias.sh            instala o que falta
#   bash scripts/baixar-dependencias.sh --forcar   reinstala tudo; a pasta atual vira <pasta>.antiga-AAAAMMDD
#
# EGUATEX_DEPS   pasta base dos destinos (padrao: a pasta acima do repositorio). O
#                src-tauri/tauri.conf.json e o scripts/atualizar-tudo.sh usam so esse caminho
#                fixo (../../eguatex-tinytex e ../../eguatex-pandoc); outra pasta serve para
#                teste ou copia de seguranca e nao alimenta o tauri build.
# EGUATEX_CACHE  pasta dos downloads (padrao: ../eguatex-cache, ao lado do repositorio)
#
# Cada pasta instalada ganha um selo ao lado dela (<pasta>.selo) com nome, versao e SHA-256.
# Sem --forcar: pasta com o selo certo e pulada, pasta sem selo e mantida com aviso e selo
# diferente para o script. O selo so prova o download; --forcar sempre renomeia e reinstala.
set -euo pipefail

# ---------------------------------------------------------------------------
# Itens: nome|versao|destino (relativo a EGUATEX_DEPS)|modo|pasta de dentro do arquivo
#   modo extrair: um arquivo compactado com uma pasta no topo
#   modo copiar:  arquivos soltos copiados para o destino
ITENS=(
  "Pandoc|3.13|eguatex-pandoc/pandoc-3.13|extrair|pandoc-3.13"
  "TinyTeX|2026-11-11|eguatex-tinytex/padrao/TinyTeX|extrair|TinyTeX"
)
# Arquivos: item|arquivo|sha256|origem (https://... pelo curl, gh:dono/repo:tag pelo gh)
ARQUIVOS=(
  "Pandoc|pandoc-3.13-windows-x86_64.zip|23e1f2e978a3df80db8bf65c9c59534f41a23140480cfb4f783a882ab7dc6a4e|https://github.com/jgm/pandoc/releases/download/3.13/pandoc-3.13-windows-x86_64.zip"
  "TinyTeX|TinyTeX-eguatex-2026-11-11.7z|3e2588078d6696b98f6f29e831df72a7abeea9a6de4e3dcb1a01ee36c661668f|gh:eguatex/eguatex-app:deps-tinytex-2026-11-11"
)
# o gh e exigido so quando um arquivo gh:... precisa ser baixado
FERRAMENTAS=(curl sha256sum 7z)
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

# caminho que programas nativos do Windows (7z, gh) entendem
nativo() { if command -v cygpath >/dev/null 2>&1; then cygpath -m "$1"; else printf '%s' "$1"; fi; }

SETEZIP=""
for f in "${FERRAMENTAS[@]}"; do
  if [ "$f" = 7z ]; then
    for c in 7z 7z.exe 7za "/c/Program Files/7-Zip/7z.exe"; do
      if command -v "$c" >/dev/null 2>&1; then SETEZIP="$c"; break; fi
    done
    [ -n "$SETEZIP" ] || erro "falta o 7-Zip (7z). Instale em https://www.7-zip.org e rode de novo."
  else
    command -v "$f" >/dev/null 2>&1 || erro "falta a ferramenta $f no PATH. Instale e rode de novo."
  fi
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
        gh:*)
          command -v gh >/dev/null 2>&1 \
            || erro "falta o GitHub CLI (gh) no PATH para baixar $arquivo. Instale em https://cli.github.com, rode gh auth login e tente de novo."
          local resto="${origem#gh:}"
          gh release download "${resto##*:}" -R "${resto%:*}" --pattern "$arquivo" \
            -D "$(nativo "$CACHE/.baixando")" --clobber \
            || erro "falhou o download de $arquivo da tag ${resto##*:} em ${resto%:*}. Confira gh auth login e o acesso ao repositorio." ;;
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
  IFS='|' read -r nome versao rel modo interna <<< "$item"
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
  if [ "$modo" = extrair ]; then
    IFS='|' read -r arq _ _ <<< "${lista[0]}"
    echo "  extraindo $arq"
    "$SETEZIP" x -y -o"$(nativo "$parcial")" "$(nativo "$CACHE/$arq")" > /dev/null \
      || erro "falhou a extracao de $arq"
    [ -d "$parcial/$interna" ] || erro "$arq nao tem a pasta $interna no topo"
  else
    for l in "${lista[@]}"; do
      IFS='|' read -r arq _ _ <<< "$l"
      cp "$CACHE/$arq" "$parcial/"
    done
  fi

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
  if [ "$modo" = extrair ]; then
    mv "$parcial/$interna" "$pasta"
    rm -rf "$parcial"
  else
    mv "$parcial" "$pasta"
  fi
  printf '%s\n' "$esperado" > "$selo"
  echo "  instalado"
done
echo "Pronto."
