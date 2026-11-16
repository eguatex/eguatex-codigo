"""Gera os pacotes LaTeX extras do motor do celular (busytex, TeX Live 2023 reduzido).

Para cada pacote ou classe da lista, compila um documento minimo com o TeX Live completo
do PC (pdflatex -recorder), anota os arquivos usados que o busytex nao tem e descobre de
quais pacotes do TeX Live eles vem. Todos os arquivos de execucao desses pacotes (estilos,
classes, metricas e fontes Type 1) vao para motor-tex/extras/pacotes, com um indice:

  motor-tex/extras/pacotes.json
    gatilhos: nome usado no documento -> pacotes do TeX Live necessarios
    pacotes:  pacote do TeX Live -> arquivos
    arquivos: arquivo -> pacote do TeX Live (para tentar de novo quando faltar um arquivo)

e um mapa de fontes unico (eguatex-fontes.map) com os .map desses pacotes.

Uso: python scripts/gerar-extras.py lista.txt [lista2.txt ...]
Cada linha da lista: "pac nome", "cls nome" ou so "nome" (pacote).
"""

import base64
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

RAIZ = Path(__file__).resolve().parent.parent
TL = Path("c:/texlive/2026")
BUSYTEX = RAIZ.parent / "eguatex-android-deps" / "busytex"
SAIDA = RAIZ / "motor-tex" / "extras"
PGF = SAIDA / "pgf"
EXTENSOES = {
    ".sty", ".cls", ".def", ".fd", ".cfg", ".tex", ".clo", ".ldf", ".bbx", ".cbx", ".lbx",
    ".dbx", ".bst", ".tfm", ".vf", ".pfb", ".enc", ".map", ".ist", ".dict", ".otp",
}
# pacotes do TeX Live maiores que isso ficam de fora (o APK ja tem 240 MB)
BINARIOS = {".tfm", ".vf", ".pfb"}
LIMITE_PACOTE = 25 * 1024 * 1024


def nomes_busytex() -> set[str]:
    nomes = set()
    for js in BUSYTEX.glob("*.js"):
        for m in re.finditer(r'"filename": "([^"]+)"', js.read_text(encoding="utf-8", errors="ignore")):
            nomes.add(m.group(1).rsplit("/", 1)[-1])
    return nomes


def ler_tlpdb():
    """pacote -> arquivos de execucao (caminhos relativos ao texmf-dist) e arquivo -> pacote."""
    pacote_arquivos: dict[str, list[str]] = {}
    arquivo_pacote: dict[str, str] = {}
    atual = None
    em_runfiles = False
    for linha in (TL / "tlpkg" / "texlive.tlpdb").open(encoding="utf-8", errors="ignore"):
        if linha.startswith("name "):
            atual = linha[5:].strip()
            em_runfiles = False
        elif linha.startswith("runfiles"):
            em_runfiles = True
        elif not linha.startswith(" "):
            em_runfiles = False
        elif em_runfiles and atual and "." not in atual:
            caminho = linha.strip().split()[0]
            if caminho.startswith("RELOC/"):
                caminho = "texmf-dist/" + caminho[6:]
            pacote_arquivos.setdefault(atual, []).append(caminho)
            arquivo_pacote.setdefault(caminho.rsplit("/", 1)[-1], atual)
    return pacote_arquivos, arquivo_pacote


def documento(tipo: str, nome: str) -> str:
    if tipo == "cls":
        corpo = {
            "beamer": r"\begin{frame}{T}Teste.\end{frame}",
            "letter": r"\begin{letter}{X}\opening{Oi}Teste.\closing{Tchau}\end{letter}",
            "scrlttr2": r"\begin{letter}{X}\opening{Oi}Teste.\closing{Tchau}\end{letter}",
            "moderncv": r"\makecvtitle Teste.",
        }.get(nome, "Teste.")
        extra = {"moderncv": r"\moderncvstyle{classic}\name{A}{B}", "achemso": r"\title{T}"}.get(nome, "")
        return f"\\documentclass{{{nome}}}\n{extra}\n\\begin{{document}}\n{corpo}\n\\end{{document}}\n"
    extra = r"\usepackage{hyperref}" if nome == "hyperxmp" else ""
    corpo = r"\textbf{Negrito} \textit{italico} $x^2$ Teste."
    return f"\\documentclass{{article}}\n{extra}\n\\usepackage{{{nome}}}\n\\begin{{document}}\n{corpo}\n\\end{{document}}\n"


def gravar(tipo: str, nome: str) -> tuple[str, list[str], str]:
    """Compila e devolve os caminhos (relativos ao texmf-dist) que o documento leu."""
    pasta = Path(tempfile.mkdtemp(prefix="extras-"))
    try:
        (pasta / "t.tex").write_text(documento(tipo, nome), encoding="utf-8")
        ambiente = dict(os.environ, PATH=str(TL / "bin" / "windows") + os.pathsep + os.environ["PATH"])
        subprocess.run(
            ["pdflatex", "-recorder", "-interaction=nonstopmode", "-halt-on-error", "t.tex"],
            cwd=pasta, env=ambiente, capture_output=True, timeout=300,
        )
        ok = (pasta / "t.pdf").exists()
        lidos = []
        fls = pasta / "t.fls"
        if fls.exists():
            for linha in fls.read_text(encoding="utf-8", errors="ignore").splitlines():
                if linha.startswith("INPUT "):
                    c = linha[6:].replace("\\", "/")
                    i = c.lower().find("/texmf-dist/")
                    if i >= 0:
                        lidos.append("texmf-dist/" + c[i + len("/texmf-dist/"):])
        return nome, sorted(set(lidos)), "ok" if ok else "falhou no PC"
    finally:
        shutil.rmtree(pasta, ignore_errors=True)


def main():
    itens = []
    for lista in sys.argv[1:]:
        for linha in Path(lista).read_text(encoding="utf-8").splitlines():
            partes = linha.split()
            if not partes:
                continue
            itens.append(("cls", partes[1]) if partes[0] == "cls" else ("pac", partes[-1]))
    busy = nomes_busytex()
    pgf = {p.name for p in PGF.iterdir()} if PGF.exists() else set()
    pacote_arquivos, arquivo_pacote = ler_tlpdb()

    with ThreadPoolExecutor(8) as ex:
        resultados = list(ex.map(lambda it: gravar(*it), itens))

    gatilhos: dict[str, list[str]] = {}
    usa_pgf: set[str] = set()
    for nome, lidos, estado in resultados:
        faltam = [c for c in lidos if c.rsplit("/", 1)[-1] not in busy]
        if any(c.rsplit("/", 1)[-1] in pgf for c in faltam):
            usa_pgf.add(nome)
        # fontname traz so o texfonts.map, que o busytex dispensa
        pacotes = sorted({arquivo_pacote[c.rsplit("/", 1)[-1]] for c in faltam if c.rsplit("/", 1)[-1] in arquivo_pacote} - {"pgf", "fontname"})
        if pacotes:
            gatilhos[nome] = pacotes
        print(f"{estado:13} {nome}: {len(faltam)} arquivos faltam, pacotes {pacotes}")

    destino = SAIDA / "pacotes"
    if destino.exists():
        shutil.rmtree(destino)
    destino.mkdir(parents=True)
    indice_pacotes: dict[str, list[str]] = {}
    indice_arquivos: dict[str, str] = {}
    mapas = []
    tamanhos = {}
    for pac in sorted({p for ps in gatilhos.values() for p in ps}):
        arquivos = []
        total = 0
        for c in pacote_arquivos.get(pac, []):
            base = c.rsplit("/", 1)[-1]
            ext = os.path.splitext(base)[1].lower()
            # tudo o que fica em texmf-dist/tex entra (.rtx, .inc, .clo...); de fonts, so os tipos do pdflatex
            em_tex = c.startswith("texmf-dist/tex/") and ext not in (".lua", ".pl", ".py")
            if not (em_tex or ext in EXTENSOES) or base in busy or base in pgf:
                continue
            if ext == ".map" and "/fonts/map/" not in c:
                continue
            origem = TL / c
            if not origem.exists() or base in indice_arquivos or any(b == base for b, _, _ in arquivos):
                continue
            total += origem.stat().st_size
            arquivos.append((base, origem, ext))
        if total > LIMITE_PACOTE:
            print(f"IGNORADO {pac}: {total/1e6:.1f} MB")
            continue
        # um arquivo por pacote: o celular busca tudo de uma vez
        conteudo = {}
        for base, origem, ext in arquivos:
            if ext == ".map":
                mapas.append(origem.read_text(encoding="utf-8", errors="ignore"))
                continue
            dados = origem.read_bytes()
            if ext in BINARIOS:
                conteudo[base] = {"b": base64.b64encode(dados).decode()}
            else:
                conteudo[base] = {"t": dados.decode("utf-8", errors="replace")}
            indice_arquivos[base] = pac
        (destino / f"{pac}.json").write_text(json.dumps(conteudo, ensure_ascii=False), encoding="utf-8")
        indice_pacotes[pac] = sorted(conteudo)
        tamanhos[pac] = total
    (SAIDA / "pacotes-fontes.map").write_text("\n".join(mapas), encoding="utf-8")
    gatilhos = {n: [p for p in ps if p in indice_pacotes] for n, ps in gatilhos.items()}
    # todo .sty e .cls embutido tambem dispara o proprio pacote; as dependencias vem
    # depois, quando o motor reclamar de um arquivo que falta
    for base, pac in indice_arquivos.items():
        if base.endswith((".sty", ".cls")):
            gatilhos.setdefault(base[:-4], [pac])
    indice = {
        "gatilhos": {n: ps for n, ps in gatilhos.items() if ps},
        "pgf": sorted(usa_pgf),
        "pacotes": sorted(indice_pacotes),
        "arquivos": indice_arquivos,
    }
    (SAIDA / "pacotes.json").write_text(json.dumps(indice, ensure_ascii=False), encoding="utf-8")
    total = sum(tamanhos.values())
    print(f"{len(indice_pacotes)} pacotes, {len(indice_arquivos)} arquivos, {total/1e6:.1f} MB")
    for pac, t in sorted(tamanhos.items(), key=lambda x: -x[1])[:15]:
        print(f"  {pac}: {t/1e6:.1f} MB")


if __name__ == "__main__":
    main()
