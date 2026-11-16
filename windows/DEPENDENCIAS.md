# Dependências do instalador do ÉguaTex para Windows

O instalador embute o TinyTeX e o Pandoc. Os dois ficam fora do git, na pasta acima do repositório, nos caminhos que o `src-tauri/tauri.conf.json` lê (`resources`).

Para montar as duas pastas num PC novo:

```bash
bash scripts/baixar-dependencias.sh
```

O script baixa cada arquivo para `../eguatex-cache`, confere o SHA-256, extrai e grava um selo `<pasta>.selo` ao lado da pasta. Variáveis: `EGUATEX_DEPS` (pasta base, padrão a pasta acima do repositório) e `EGUATEX_CACHE` (downloads). Sem `--forcar`, pasta com o selo certo é pulada ("ja esta"), pasta sem selo é mantida com aviso e selo diferente para o script com código 1. O selo prova só o download: um `pandoc.exe` corrompido depois passa. Por isso `--forcar` sempre renomeia a pasta atual para `<pasta>.antiga-AAAAMMDD` (ou `-2`, `-3`, se o nome já existir) e instala de novo a partir do cache conferido.

O `src-tauri/tauri.conf.json` e o `scripts/atualizar-tudo.sh` usam caminho fixo, a pasta acima do repositório. `EGUATEX_DEPS` apontando para outra pasta serve para teste ou cópia de segurança e não alimenta o `tauri build`. O `scripts/atualizar-tudo.sh` para antes do `npx tauri build` se faltar `pdflatex.exe` ou `pandoc.exe` nesse caminho fixo.

## Ferramentas exigidas

- `curl` e `sha256sum` (vêm com o Git for Windows)
- `7z` (7-Zip; o script também procura em `C:\Program Files\7-Zip\7z.exe`)
- `gh` (GitHub CLI) logado com `gh auth login` numa conta com acesso ao repositório `eguatex/eguatex-app`, que é privado

## Pandoc

| Campo | Valor |
|---|---|
| Versão | 3.13 |
| Origem | https://github.com/jgm/pandoc/releases/download/3.13/pandoc-3.13-windows-x86_64.zip |
| Arquivo | `pandoc-3.13-windows-x86_64.zip`, 42.983.200 bytes |
| SHA-256 | `23e1f2e978a3df80db8bf65c9c59534f41a23140480cfb4f783a882ab7dc6a4e` (igual ao digest publicado no release 3.13) |
| Destino | `eguatex-pandoc/pandoc-3.13` |
| Usados no instalador | `pandoc.exe` (239.168.264 bytes) e `COPYRIGHT.txt` |

Em 05/11/2026 o `pandoc.exe` local ficou com 10 bytes e a exportação para Word quebrou. A cópia foi restaurada do zip conferido por este SHA-256.

## TinyTeX

| Campo | Valor |
|---|---|
| Versão | montagem do ÉguaTex de 11/11/2026, TeX Live 2026 (pdfTeX 1.40.29) |
| Origem | Release `deps-tinytex-2026-11-11` do repositório `eguatex/eguatex-app` (https://github.com/eguatex/eguatex-app/releases/tag/deps-tinytex-2026-11-11) |
| Arquivo | `TinyTeX-eguatex-2026-11-11.7z`, 444.601.552 bytes (7z, nível 7) |
| SHA-256 | `3e2588078d6696b98f6f29e831df72a7abeea9a6de4e3dcb1a01ee36c661668f` |
| Conteúdo | pasta `TinyTeX`, 57.695 arquivos, 1.323.613.325 bytes, 2.920 pacotes no `tlpkg/texlive.tlpdb` |
| Manifesto | `TinyTeX-eguatex-2026-11-11.sha256` no mesmo Release, SHA-256 de cada arquivo, SHA-256 `db2f9de2ecc78c6ce8e85edd1ff850d03e1fbb803dcfc3f196ddba465c7e5d81` |
| Destino | `eguatex-tinytex/padrao/TinyTeX` |

### Histórico da montagem

- 29/10/2026: instalação do `TinyTeX-padrao.exe` (173.013.624 bytes, SHA-256 `e23dcc65026b96a2c34bbd793087f64e6b6e9173c20483fe875050251e3994e1`) com 322 pacotes, mais abntex2, babel-portuges e hyphen-portuguese.
- 02/11/2026: `tlmgr` (`tlmgr.pl` revisão 80687) instalou as coleções basic, bibtexextra, fontsrecommended, latex, latexextra, latexrecommended, mathscience e pictures (2.571 pacotes baixados) e atualizou 20 pacotes. O banco passou de 325 para 2.903 pacotes.
- 11/11/2026: 17 pacotes de projetos reais (TU Berlin, Inria, ACM TechBrief, rpgtex, TU Delft), lista em `scripts/preparar-tinytex.sh`. O banco passou para 2.920 pacotes.

O `scripts/preparar-tinytex.sh` registra os pacotes de 11/11. Vale a cópia do repositório; a cópia antiga em `eguatex-tinytex/preparar-tinytex.sh` fica só como registro. Rodar de novo gera outro TinyTeX, porque o mirror do CTAN só tem a versão do dia de cada pacote. Por isso o script apaga o `TinyTeX.selo` no início: a pasta deixa de bater com o Release, e o `baixar-dependencias.sh` passa a avisar "sem selo". Para reproduzir o instalador, use o arquivo do Release.

## TeX Live 2026 do sistema

O PC de desenvolvimento tem o TeX Live 2026 completo em `C:\texlive\2026`. Ele não entra no instalador. Serve só para quem desenvolve e esconde pacotes que faltam no TinyTeX: para testar o TinyTeX, deixe só `eguatex-tinytex/padrao/TinyTeX/bin/windows` no PATH.
