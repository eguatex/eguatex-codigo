# Creditos e atribuicoes

O EguaTex e software livre sob licenca AGPL-3.0-or-later.
Este arquivo registra todo codigo, dados ou logica reutilizados de outros projetos.

## Overleaf Community Edition

- **Licenca:** AGPL-3.0
- **Repositorio:** https://github.com/overleaf/overleaf
- **O que foi estudado:** arquitetura de microsservicos, fluxo de compilacao (CLSI), colaboracao real-time (Socket.IO + OT), layout de tres paineis, galeria de templates.
- **O que foi reutilizado:** conceitos e fluxos de interface. Nenhum codigo copiado diretamente.
- **Estrutura de interface adaptada (Rodada 4, copyright Overleaf Ltd, AGPL-3.0)**, reescrita em Svelte a partir de `services/web/frontend/js/features/`:
  - ordem da barra superior e opcoes de layout: `ide-react/components/toolbar/toolbar.tsx:125-150`, `change-layout-options.tsx`, `project-title.tsx` (usados em `Toolbar.svelte`);
  - ordem dos icones da barra lateral (arquivos, busca, chat; ajuda antes de configuracoes): `ide-react/components/rail/rail.tsx:100-170` e `rail-help-dropdown.tsx` (usados em `BarraLateral.svelte`);
  - painel de busca no projeto: `ide-react/components/rail/full-project-search-panel.tsx` (usado em `Layout.svelte`);
  - janela de atalhos: `hotkeys-modal/` (usado em `AtalhosModal.svelte`);
  - barra de menus e paleta de comandos: `ide-react/components/toolbar/menu-bar.tsx`, `command-dropdown.tsx`, `command-palette/` (usados em `src/lib/comandos.ts`, `MenuBarra.svelte`, `PaletaComandos.svelte`);
  - barra do editor, transbordo, trilha, paleta de simbolos e autocompletar: `source-editor/components/toolbar/*.tsx`, `overflow.tsx`, `codemirror-toolbar.tsx`, `symbol-palette`, `auto-complete` (usados em `EditorToolbar.svelte`, `Trilha.svelte`, `PaletaSimbolos.svelte`, `src/lib/editor/autocompletar.ts`);
  - painel de arquivos e estrutura: `file-tree/components/` e `outline/components/` (usados em `PainelArquivos.svelte`, `FileTree.svelte`, `Estrutura.svelte`);
  - painel do PDF e logs: `pdf-preview/components/` (usados em `PdfViewer.svelte` e `ListaLogs.svelte`);
  - configuracoes e contagem de palavras: `ide-settings/components/` e `word-count-modal/` (usados em `ConfiguracoesModal.svelte` e `ContagemPalavrasModal.svelte`);
  - lista de projetos: `project-list/components/` (usado em `src/routes/+page.svelte`).
- **Fase 0 do plano de 09/11/2026 (copyright Overleaf Ltd, AGPL-3.0)**, so a estrutura e as medidas, reescritas em Svelte. Nenhum texto nem codigo copiado:
  - abas Todos, Erros, Avisos e Info dentro do painel de log, com o "Raw logs" so na aba Todos e os botoes de acao no rodape, a direita: `pdf-preview/components/error-logs.tsx:73-77, 116` (usado em `ListaLogs.svelte`);
  - estilo das abas do log (borda de 3 px embaixo da aba escolhida, contador redondo) e rodape alinhado a direita: `stylesheets/pages/editor/logs.scss:27-74` (usado em `ListaLogs.svelte`);
  - barra unica do PDF com lado esquerdo (compilar, logs, baixar) e lado direito (tema do PDF, pagina, zoom), e a versao estreita com so o tema e o menu de zoom: `pdf-preview/components/pdf-preview-hybrid-toolbar.tsx:47-58` e `pdf-viewer-controls-toolbar.tsx:137-172` (usado em `PdfViewer.svelte`);
  - contador do botao de logs com erros mais avisos, em cor de aviso quando nao ha erro: `pdf-preview/components/pdf-hybrid-logs-button.tsx:24-62` (usado em `PdfViewer.svelte`);
  - menu de modo Editando, Revisando e Visualizando, e o ocultar de desfazer, refazer e formatacao fora do modo de edicao: `review-panel/components/review-mode-switcher.tsx:38-107` e `source-editor/components/toolbar/toolbar-layout.tsx:43-121` (usados em `EditorToolbar.svelte`, `EditorPanel.svelte` e `src/routes/+page.svelte`);
  - medidas da barra do editor (botao de 24 px com margem de 1 px, grupo com padding 0 4px e margem 4px 0, borda esquerda de 1 px a 30%, hover a 10%, ativo a 20%, desabilitado a 0,2, lado direito com espaco de 6 px e divisoria de 1x16 px): `stylesheets/components/editor-toolbar.scss:1-82` (usado em `EditorToolbar.svelte`).
- **Fase 1 do plano de 10/11/2026 (commit `e039ad26c5bf5422eb57b89fc7e57c75055e631d`, copyright Overleaf Ltd, AGPL-3.0)**, so medidas e comportamento, reescritos em Svelte. Nenhum texto nem codigo copiado. Caminhos relativos a `services/web/frontend/`:
  - barra do editor sem corte fixo por modo, com o transbordo por largura de tras para frente, o grupo negrito e italico que nunca sai, o grupo elastico do "mais" que so aparece com item escondido, o botao de matematica com icone de calculadora e o Omega em texto serifado: `js/features/source-editor/components/codemirror-toolbar.tsx:79-160`, `components/toolbar/toolbar-layout.tsx`, `toolbar-items.tsx`, `overflow.tsx`, `toolbar-button.tsx`, `section-heading-dropdown.tsx`, `extensions/toolbar/toolbar-panel.ts` (usados em `EditorToolbar.svelte` e `src/lib/transbordo.ts`);
  - barra de 32 px com padding 0 5px, borda de baixo so no tema escuro, icones de 20 px, recuo so dentro de lista (profundidade da lista) e fora de arquivo LaTeX so desfazer e refazer: `stylesheets/pages/editor/toolbar.scss:289-296`, `stylesheets/foundations/variables.scss:4`, `js/features/source-editor/components/toolbar/toolbar-items.tsx:63, 180-207` (usados em `EditorToolbar.svelte`, `Layout.svelte` e `src/lib/editor/profundidade-lista.ts`);
  - zona direita com o alternador Codigo/Visual de 26 px, o botao de modo (Revisando e Visualizando com cores proprias) e "Ir para o PDF" so no layout Somente editor: `js/features/source-editor/components/editor-switch.tsx`, `switch-to-pdf-button.tsx`, `js/features/review-panel/components/review-mode-switcher.tsx`, `stylesheets/pages/editor/toolbar.scss:308-401`, `stylesheets/pages/editor/review-panel.scss:709-802` (usados em `EditorToolbar.svelte`);
  - barra do editor escondida no modo foco: `js/features/source-editor/components/codemirror-toolbar.tsx:164` (usado em `Layout.svelte`);
  - abas de arquivo com as medidas, o caminho cortado a esquerda, o botao de fechar redondo e bloqueado na ultima aba, o indicador de soltura de 5 px ao arrastar para reordenar e o menu do botao direito com Fechar e Fechar as outras: `js/features/source-editor/components/tabs/tab.tsx`, `tabs-container.tsx`, `tabs-context-menu.tsx`, `js/features/ide-react/context/tabs-context.tsx` (moveTab), `stylesheets/pages/editor/tabs.scss` (usados em `BarraAbas.svelte`, `src/lib/abas.svelte.ts` e `src/routes/+page.svelte`);
  - medidas da trilha (28 px, padding 4 px, gap 2 px) e trilha desligada por padrao: `js/features/source-editor/extensions/breadcrumbs-panel.ts:3-41` e `services/web/app/src/models/User.mjs:108` (usados em `Trilha.svelte` e `src/lib/preferencias.svelte.ts`).
- **Biblioteca compartilhada:** CodeMirror 6 (licenca MIT, autor Marijn Haverbeke) e PDF.js (licenca Apache 2.0, Mozilla Foundation) sao usados pelo Overleaf e tambem pelo EguaTex como dependencias independentes.

## TeXstudio

- **Licenca:** GPL-3.0
- **Repositorio:** https://github.com/texstudio-org/texstudio
- **O que foi estudado:** subsistemas de compilacao (BuildManager), editor (LatexDocument/LatexEditorView), SyncTeX, corretor ortografico (Hunspell), motor de scripts (QJSEngine), verificacao de sintaxe, autocompletar via arquivos `.cwl`.
- **O que foi reutilizado:**
  - Formato e conteudo dos arquivos `.cwl` (listas de comandos LaTeX para autocompletar). Os dados originais sao GPL-3.0; o EguaTex os redistribui sob AGPL-3.0-or-later (compativel).
  - Categorias de simbolos e mapeamentos simbolo -> comando LaTeX extraidos dos recursos `qrc_symbols`.
- **Nenhum codigo C++/Qt foi copiado.** A interface e totalmente reimplementada em Svelte/TypeScript.

## SyncTeX

- **Licenca:** MIT
- **Autor:** Jerome Laurens
- **Uso:** integracao bidirecional entre editor e visualizador PDF.

## TinyTeX e TeX Live

- **Licenca:** os scripts do TinyTeX sao MIT (Yihui Xie); os pacotes do TeX Live tem licencas proprias (LPPL, GPL, MIT e outras, listadas em cada pacote e em https://www.tug.org/texlive/copying.html)
- **Sites:** https://yihui.org/tinytex/ e https://www.tug.org/texlive/
- **Uso:** no Windows o instalador leva o TinyTeX (distribuicao do TeX Live) para compilar sem outro LaTeX instalado. Os binarios e pacotes nao foram alterados. O ÉguaTex so os executa como programas separados.

## busytex

- **Licenca:** MIT (scripts de compilacao); o conteudo embutido vem do TeX Live e de pdfTeX, XeTeX e LuaTeX, cada um com a licenca propria
- **Repositorio:** https://github.com/busytex/busytex
- **Uso:** motor LaTeX em WebAssembly da versao Android, que compila offline. Nao e usado na versao para Windows.

## Lucide

- **Licenca:** ISC (e MIT para trechos derivados do Feather)
- **Repositorio:** https://github.com/lucide-icons/lucide (pacote npm `@lucide/svelte`)
- **Uso:** icones da interface.

## CodeMirror

- **Licenca:** MIT (Copyright Marijn Haverbeke e outros)
- **Site:** https://codemirror.net/ (pacotes npm `codemirror` e `@codemirror/*`)
- **Uso:** editor de texto do codigo LaTeX.

## Svelte e SvelteKit

- **Licenca:** MIT (Copyright os colaboradores do Svelte)
- **Site:** https://svelte.dev/
- **Uso:** framework da interface.

## Tauri

- **Licenca:** MIT ou Apache-2.0, a escolha do usuario
- **Site:** https://tauri.app/
- **Uso:** estrutura do aplicativo (janela, instalador e ponte entre a interface e o codigo Rust).

## Pandoc

- **Licenca:** GPL-2.0-or-later (Copyright (C) 2006-2026 John MacFarlane e colaboradores)
- **Repositorio:** https://github.com/jgm/pandoc
- **Uso:** pandoc.exe 3.13 embutido sem alteracao, para exportar o documento em Word, Markdown e HTML. O aviso de copyright vai junto em pandoc/COPYRIGHT.txt.

## VERO, dicionario pt-BR

- **Licenca:** LGPL-3.0 ou MPL (Copyright (C) 2006-2013 Raimundo Santos Moura e colaboradores)
- **Origem:** VERO 3.2 do LibreOffice, pelo pacote npm dictionary-pt 4.0.0 (https://github.com/wooorm/dictionaries)
- **Uso:** dicionario do corretor ortografico, sem alteracao, em src-tauri/dicionarios/. A licenca vai junto em dicionarios/LICENCA-VERO.txt.

## spellbook

- **Licenca:** MPL-2.0
- **Repositorio:** https://github.com/helix-editor/spellbook
- **Uso:** le o dicionario no formato Hunspell e confere as palavras, offline.

## TeXstudio (ideias)

- **Licenca:** GPL-3.0
- **Repositorio:** https://github.com/texstudio-org/texstudio
- **Uso:** so a ideia do assistente de tabela, da previa ao passar o mouse e dos avisos de rotulos. Nenhum trecho de codigo foi copiado.

---

Ao contribuir, registre aqui qualquer trecho copiado ou adaptado de outro projeto, mantendo o aviso de copyright original.

## KaTeX

- **Licenca:** MIT (Copyright (c) 2013-2020 Khan Academy e colaboradores)
- **Repositorio:** https://github.com/KaTeX/KaTeX
- **Uso:** desenha as formulas no modo Visual do editor, sem internet.

## Salvar como Word (src/lib/pdf-para-word.ts)

Le o PDF compilado com o PDF.js (Apache 2.0, Mozilla), que o app ja usa para mostrar o PDF.
O .docx leva as fontes Latin Modern (GUST Font License, GUST e-foundry), convertidas de OTF
para TTF (script em eguatex-pdf2word/gerar-fontes.py) e guardadas em static/fontes-word/.

## qrcode

- **Licenca:** MIT (Copyright (c) 2012 Ryan Day)
- **Autor:** Ryan Day
- **Pacote npm:** https://www.npmjs.com/package/qrcode (repositorio: https://github.com/soldair/node-qrcode)
- **Uso:** gera o QR Code do Pix no botao Apoiar, no proprio computador, sem internet.
