"""Gera motor-tex/extras/sempre/ec-para-lm.map.

O motor do celular (busytex) tem as metricas das fontes EC (ecrm1000.tfm etc.), que o
\\usepackage[T1]{fontenc} usa, mas nao tem os desenhos Type 1 delas (pacote cm-super, 60 MB).
Sem desenho, o pdflatex para com "Font ecrm1000 at 600 not found". O Latin Modern tem o
mesmo desenho e ja vem no busytex: este mapa manda cada fonte EC para o Latin Modern de
tamanho mais proximo, com a codificacao lm-ec.enc (mesmos nomes de glifos). As fontes TC
(simbolos do textcomp, codificacao TS1, como tcrm1000) vao para o mesmo Latin Modern com a
lm-ts1.enc; sem elas o TU Berlin parava com "Font tcrm1000 at 600 not found".

Uso: python scripts/gerar-mapa-ec.py
"""

import re
from pathlib import Path

RAIZ = Path(__file__).resolve().parent.parent
BUSYTEX = RAIZ.parent / "eguatex-android-deps" / "busytex"
TL_LM = Path("c:/texlive/2026/texmf-dist/fonts/type1/public/lm")
SAIDA = RAIZ / "motor-tex" / "extras" / "sempre" / "ec-para-lm.map"

# familia EC -> familia Latin Modern
FAMILIAS = {
    "ecrm": "lmr", "ecti": "lmri", "ecbx": "lmbx", "ecsl": "lmro", "ecbi": "lmbxi",
    "ecbl": "lmbxo", "eccc": "lmcsc", "ecsc": "lmcsco", "ectt": "lmtt", "ecit": "lmtti",
    "ecst": "lmtto", "ecss": "lmss", "ecsi": "lmsso", "ecsx": "lmssbx", "ecso": "lmssbo",
    "ecvt": "lmvtt", "ecvi": "lmvtto", "ecrb": "lmb", "ecdh": "lmdunh",
}


def nomes_busytex() -> set[str]:
    nomes = set()
    for js in BUSYTEX.glob("*.js"):
        for m in re.finditer(r'"filename": "([^"]+)"', js.read_text(encoding="utf-8", errors="ignore")):
            nomes.add(m.group(1).rsplit("/", 1)[-1])
    return nomes


def nome_postscript(pfb: Path) -> str | None:
    m = re.search(rb"/FontName\s*/([^\s]+)", pfb.read_bytes()[:4000])
    return m.group(1).decode() if m else None


def main():
    busy = nomes_busytex()
    linhas = []
    for tfm in sorted(n for n in busy if re.fullmatch(r"(ec|tc)[a-z]{2}\d{4}\.tfm", n)):
        familia, tamanho = tfm[:4], int(tfm[4:8]) / 100
        ts1 = familia.startswith("tc")
        destino = FAMILIAS.get("ec" + familia[2:])
        if not destino:
            continue
        opcoes = [
            (abs(int(m.group(1)) - tamanho), m.group(0))
            for n in busy
            if (m := re.fullmatch(rf"{destino}(\d+)\.pfb", n))
        ]
        if not opcoes:
            continue
        pfb = min(opcoes)[1]
        ps = nome_postscript(TL_LM / pfb) or pfb[:-4]
        if ts1:
            linhas.append(f'{tfm[:-4]} {ps} "enclmts1 ReEncodeFont" <lm-ts1.enc <{pfb}')
        else:
            linhas.append(f'{tfm[:-4]} {ps} "enclmec ReEncodeFont" <lm-ec.enc <{pfb}')
    SAIDA.write_text("\n".join(linhas) + "\n", encoding="utf-8")
    print(len(linhas), "linhas em", SAIDA)


if __name__ == "__main__":
    main()
