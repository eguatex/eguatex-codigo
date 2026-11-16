"""Acrescenta pacotes do TeX Live aos extras do motor do celular, sem refazer os outros.

O gerar-extras.py descobre os pacotes pelo que o pdflatex le e nao leva fontes OpenType nem
TrueType. Os projetos do XeLaTeX (fontspec) precisam delas: este script copia do TeX Live do PC
todos os arquivos de execucao do pacote pedido (estilos, classes, metricas, Type 1, .otf e .ttf)
para motor-tex/extras/pacotes/<pacote>.json e atualiza o indice motor-tex/extras/pacotes.json:

  gatilhos: nome do .sty ou .cls -> pacotes
  arquivos: arquivo -> pacote (o motor tenta de novo quando o log diz que faltou)
  fontes:   nome da familia sem espacos, em minusculas ("carlito", "inriasans") -> pacote,
            para o \\setmainfont{Carlito} do fontspec, que procura a fonte pelo nome

Um pacote que ja esta no indice e refeito com os arquivos novos. Os .map vao para o
pacotes-fontes.map (linhas repetidas ficam de fora).

Uso: python scripts/adicionar-extras.py pacote [pacote ...]
"""

import base64
import json
import os
import re
import sys
from pathlib import Path

RAIZ = Path(__file__).resolve().parent.parent
TL = Path("c:/texlive/2026")
BUSYTEX = RAIZ.parent / "eguatex-android-deps" / "busytex"
SAIDA = RAIZ / "motor-tex" / "extras"
TEXTO = {".sty", ".cls", ".def", ".fd", ".cfg", ".tex", ".clo", ".ldf", ".bbx", ".cbx", ".lbx",
         ".dbx", ".bst", ".enc", ".map", ".ist", ".dict", ".code.tex", ".fontspec"}
BINARIO = {".tfm", ".vf", ".pfb", ".otf", ".ttf"}
LIMITE_PACOTE = 25 * 1024 * 1024


def nomes_busytex() -> set[str]:
    nomes = set()
    for js in BUSYTEX.glob("*.js"):
        for m in re.finditer(r'"filename": "([^"]+)"', js.read_text(encoding="utf-8", errors="ignore")):
            nomes.add(m.group(1).rsplit("/", 1)[-1])
    return nomes


def runfiles() -> dict[str, list[str]]:
    saida: dict[str, list[str]] = {}
    atual, em = None, False
    for linha in (TL / "tlpkg" / "texlive.tlpdb").open(encoding="utf-8", errors="ignore"):
        if linha.startswith("name "):
            atual, em = linha[5:].strip(), False
        elif linha.startswith("runfiles"):
            em = True
        elif not linha.startswith(" "):
            em = False
        elif em and atual and "." not in atual:
            saida.setdefault(atual, []).append(linha.strip().split()[0].replace("RELOC/", "texmf-dist/"))
    return saida


def chaves_da_fonte(arquivo: str) -> list[str]:
    """Chaves do nome inteiro e da familia (antes do primeiro "-"): latinmodern-math.otf ->
    latinmodernmath e latinmodern; Carlito-Regular.ttf -> carlitoregular e carlito. O motor
    procura do mesmo jeito (chavesDaFonte em eguatex_worker.js)."""
    base = os.path.splitext(arquivo)[0]
    limpa = lambda t: re.sub(r"[^a-z0-9]", "", t.lower())
    return list(dict.fromkeys(c for c in (limpa(base), limpa(base.split("-")[0])) if c))


def main():
    pedidos = sys.argv[1:]
    if not pedidos:
        print(__doc__)
        return
    busy = nomes_busytex()
    todos = runfiles()
    indice = json.loads((SAIDA / "pacotes.json").read_text(encoding="utf-8"))
    indice.setdefault("fontes", {})
    mapa_caminho = SAIDA / "pacotes-fontes.map"
    mapa = mapa_caminho.read_text(encoding="utf-8", errors="ignore") if mapa_caminho.exists() else ""
    linhas_mapa = set(mapa.splitlines())
    pgf = {p.name for p in (SAIDA / "pgf").iterdir()}
    for pac in pedidos:
        if pac not in todos:
            print(f"SEM PACOTE {pac}")
            continue
        conteudo, total, novos_mapas = {}, 0, []
        for c in todos[pac]:
            base = c.rsplit("/", 1)[-1]
            ext = os.path.splitext(base)[1].lower()
            if ext not in TEXTO and ext not in BINARIO:
                continue
            if base in busy or base in pgf or base in conteudo:
                continue
            dono = indice["arquivos"].get(base)
            if dono and dono != pac:
                continue
            origem = TL / c
            if not origem.exists():
                continue
            if ext == ".map":
                if "/fonts/map/" in c:
                    novos_mapas.extend(l for l in origem.read_text(encoding="utf-8", errors="ignore").splitlines() if l not in linhas_mapa)
                continue
            dados = origem.read_bytes()
            total += len(dados)
            conteudo[base] = {"b": base64.b64encode(dados).decode()} if ext in BINARIO else {"t": dados.decode("utf-8", errors="replace")}
        if total > LIMITE_PACOTE:
            print(f"IGNORADO {pac}: {total / 1e6:.1f} MB")
            continue
        if not conteudo:
            print(f"NADA NOVO {pac}")
            continue
        (SAIDA / "pacotes" / f"{pac}.json").write_text(json.dumps(conteudo, ensure_ascii=False), encoding="utf-8")
        if pac not in indice["pacotes"]:
            indice["pacotes"].append(pac)
            indice["pacotes"].sort()
        for base in conteudo:
            indice["arquivos"][base] = pac
            if base.endswith((".sty", ".cls")):
                lista = indice["gatilhos"].setdefault(base[:-4], [])
                if pac not in lista:
                    lista.append(pac)
            if base.lower().endswith((".otf", ".ttf")):
                for chave in chaves_da_fonte(base):
                    indice["fontes"].setdefault(chave, pac)
        for l in novos_mapas:
            linhas_mapa.add(l)
        mapa = mapa.rstrip("\n") + ("\n" + "\n".join(novos_mapas) if novos_mapas else "") + "\n"
        print(f"{pac}: {len(conteudo)} arquivos, {total / 1e6:.2f} MB")
    mapa_caminho.write_text(mapa, encoding="utf-8")
    (SAIDA / "pacotes.json").write_text(json.dumps(indice, ensure_ascii=False), encoding="utf-8")


if __name__ == "__main__":
    main()
