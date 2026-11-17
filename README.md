<p align="center">
  <a href="https://www.eguatex.com.br/">
    <img src="https://eguatex.com.br/assets/img/banner.jpg" alt="ÉguaTeX. Escreva LaTeX de qualquer lugar, mesmo sem internet. Feito na Amazônia. Ao lado, a logo do Curupira de perfil, com cabelo em chamas e colar de folhas." width="100%">
  </a>
</p>

<p align="center">
  <a href="https://www.eguatex.com.br/"><strong>www.eguatex.com.br</strong></a>
</p>

# ÉguaTeX

Editor LaTeX gratuito e offline, feito na Amazônia.

**Escreva LaTeX de qualquer lugar, mesmo sem internet.**

Este é o código-fonte do ÉguaTeX. Para só usar o app, baixe a versão mais recente em [www.eguatex.com.br](https://www.eguatex.com.br/) ou na aba [Releases](../../releases).

## O que tem aqui

| Pasta | Conteúdo |
| --- | --- |
| `windows/` | App para computador (Tauri 2, Svelte 5 e Rust, com o TinyTeX embutido) |
| `android/` | App para celular e tablet (Tauri 2, Svelte 5 e Rust, com o TeX Live em WebAssembly pelo busytex) |

Cada versão publicada aqui corresponde exatamente aos instaladores daquela versão, que ficam na aba Releases.

## Como compilar

Você vai precisar de [Node.js](https://nodejs.org/) 20 ou mais novo e de [Rust](https://rustup.rs/).

**Computador (pasta `windows/`):**

```bash
cd windows
bash scripts/baixar-dependencias.sh   # baixa e confere o TinyTeX e o Pandoc
npm install
npx tauri build                       # gera o instalador
```

**Celular (pasta `android/`):** além do Node.js e do Rust, precisa do Android SDK e do NDK 27.

```bash
cd android
bash scripts/baixar-dependencias.sh   # baixa e confere o busytex
npm install
bash scripts/gerar-apk.sh             # gera o APK
```

O login com Google (usado só para o Google Drive) precisa de um cliente OAuth próprio, no arquivo `src-tauri/google-oauth.json`, que não faz parte deste repositório. Sem ele, o app funciona normalmente, só sem o Google Drive.

## Licenças

- **Código:** GNU Affero General Public License, versão 3 ou posterior (arquivo `LICENSE`). Copyright (C) 2026 José Paulo de Almeida Junior e Jéssica Lopes Melo.
- **Componentes de terceiros:** cada um segue a própria licença, listada em `CREDITOS.md` e `DEPENDENCIAS.md` dentro de cada pasta.
- **Nome, logo e identidade visual:** não fazem parte da licença do código. Veja `MARCA.md`.

## Contribuir

O desenvolvimento acontece num espaço de trabalho interno da equipe, e este repositório recebe uma cópia do código a cada versão lançada. Sugestões e avisos de erro são bem-vindos nas [Issues](../../issues). Veja como em `CONTRIBUTING.md`.

Para avisar sobre uma falha de segurança, siga `SECURITY.md` e não abra uma issue pública.

## Contato

contatoeguatex@gmail.com

<p align="center">
  <sub>Feito na Amazônia por José Paulo de Almeida Junior e Jéssica Lopes Melo</sub>
</p>
