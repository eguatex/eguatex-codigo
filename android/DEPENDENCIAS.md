# Dependências do ÉguaTex Android

O APK leva o motor LaTeX busytex (TeX Live 2023 em WebAssembly) e é assinado com a chave do projeto. Os dois ficam fora do git, em `eguatex-android-deps`, na pasta acima do repositório.

## busytex

Para montar a pasta num PC novo:

```bash
bash scripts/baixar-dependencias.sh
```

O script baixa os 21 arquivos para `../eguatex-cache`, confere o SHA-256 de cada um, copia para `eguatex-android-deps/busytex` e grava o selo `eguatex-android-deps/busytex.selo` ao lado da pasta. Variáveis: `EGUATEX_DEPS` (pasta base, padrão a pasta acima do repositório) e `EGUATEX_CACHE` (downloads). Sem `--forcar`, pasta com o selo certo é pulada ("ja esta"), pasta sem selo é mantida com aviso e selo diferente para o script com código 1. O selo prova só o download, não que os arquivos da pasta continuam inteiros. Por isso `--forcar` sempre renomeia a pasta atual para `busytex.antiga-AAAAMMDD` (ou `-2`, `-3`, se o nome já existir) e instala de novo a partir do cache conferido. Ferramentas exigidas: `curl` e `sha256sum` (vêm com o Git for Windows).

O `scripts/gerar-apk.sh` lê a mesma `EGUATEX_DEPS` e para com uma mensagem se faltar `busytex.wasm`. O `scripts/atualizar-tudo.sh` do repositório eguatex não define `EGUATEX_DEPS` e usa o caminho fixo, a pasta acima do repositório, como o `tauri.conf.json` do Windows. `EGUATEX_DEPS` apontando para outra pasta serve para teste ou cópia de segurança.

| Campo | Valor |
|---|---|
| Origem | release `build_wasm_4499aa69fd3cf77ad86a47287d9a5193cf5ad993_7936974349_1` do repositório `busytex/busytex` |
| URL base | https://github.com/busytex/busytex/releases/download/build_wasm_4499aa69fd3cf77ad86a47287d9a5193cf5ad993_7936974349_1/ |
| Conteúdo | TeX Live 2023, pacotes Ubuntu lunar, emscripten 3.1.43 (`versions.txt`) |
| Total | 21 arquivos, 224.523.149 bytes |
| Destino | `eguatex-android-deps/busytex` |

Em 13/11/2026 os 21 arquivos baixados dessa tag tiveram o mesmo SHA-256 da cópia local de 30/10/2026. A tag irmã `..._7936232851_1` tem outro `texlive-basic.data` e não serve.

O APK usa 14 desses arquivos (lista no `scripts/gerar-apk.sh`). O `busytex_pipeline.js` e o worker do APK vêm de `motor-tex/`, com as mudanças do ÉguaTex. Os outros arquivos ficam como referência dos pacotes de cada `.data`.

| Arquivo | Bytes | SHA-256 |
|---|---|---|
| `busytex.js` | 295606 | `f381d9ba816d9a4389d429f54fe85f13396aa2f822973d9b66e1f527ced3434b` |
| `busytex.wasm` | 30363063 | `a7785b59bfddb8667e7738443f111430e3ee2bd3ec0c1c26629267849e091415` |
| `busytex_pipeline.js` | 30380 | `f8741cfd1fb0ac3c11daeee8386bc024f59d8b14c3000ad64b9c00dce7d61265` |
| `busytex_worker.js` | 1241 | `623136034d61d52be43df97bbe320612f0cca139b955431fb168c529531d21c4` |
| `texlive-basic.data` | 104576512 | `c955651ca293f09b2e57d6360fa2cb61ad710b4ae51e9fbc04b049477a8aeb71` |
| `texlive-basic.js` | 2068492 | `23cc4ce427d27d555fe2a050b97ff8c95bb331425018e9159130e74b62373ff1` |
| `texlive-basic.js.providespackage.txt` | 21585 | `a248a9dc6a89e69c291a773202e27fa300212b33e6b351aeba5d751e6bc53a54` |
| `texlive-basic.txt` | 508123 | `3d153bd5444e5cb6b48fe0dad8aa26313a4748a8e0ccacc6f72df94db63d2e32` |
| `ubuntu-texlive-fonts-recommended.data` | 10264033 | `3154397a58db2ac62d6bcf99e44786c6ea62cc31c2343adc42d51f2088b6a3f1` |
| `ubuntu-texlive-fonts-recommended.js` | 353109 | `708f60e5fc46b64204e8361a0d0939b3f8f0713f9b7fae379baa67e5dbe0c37a` |
| `ubuntu-texlive-latex-base.data` | 5741806 | `0dd2dfaaa2e6a9674090cbf8cf3a01419ce37063080f762aee0d82ad7263ead8` |
| `ubuntu-texlive-latex-base.js` | 299539 | `807f0bfbe66ff919c1799737acd751dfba6c1ca2afd8d71db2bb30c3c5c62144` |
| `ubuntu-texlive-latex-extra.data` | 49513577 | `fb4dc0440f4b2bd2d5138dbb5689e6a33cd20c9704934422436cb97af5d623c4` |
| `ubuntu-texlive-latex-extra.js` | 1398581 | `a99a68b79e6a93f3ca9d94f46afe8a62e7e06b6d4c5e90a705ec15fc42d8ecda` |
| `ubuntu-texlive-latex-extra.js.providespackage.txt` | 100144 | `8ee355729f1ac204b5150a5a33836d166569de4ac70ecf0fafa89c9cac01f8c2` |
| `ubuntu-texlive-latex-recommended.data` | 9058527 | `fa9a03f928a0af0e9c5a24e2ce2956e64db9d580dd2c3a7517176aac16dc9006` |
| `ubuntu-texlive-latex-recommended.js` | 337892 | `bbd975921a74adae9119988ae8a740ca9f4837db772061d313c67729fba1d9ee` |
| `ubuntu-texlive-latex-recommended.js.providespackage.txt` | 10458 | `3bb8cd12392dfac9eb89e50feb79f810d03e70702bea282af7b45b56cfee7fef` |
| `ubuntu-texlive-science.data` | 9320179 | `23c5decd471c214f1a4d16eabe02f2fe1a0b5aee94a8708713d03c4616cd5d61` |
| `ubuntu-texlive-science.js` | 259829 | `cf6ddcc894f0b8a5a9493286edc8abba67280041c21cdfa51d7dadffea85c19a` |
| `versions.txt` | 473 | `bb4c0babeb41ae2e5bff11b476fb0dacbadd2a616dbde4de4bf3dd0e6859d299` |

## Chave de assinatura

- Local: `eguatex-android-deps/chave`, ao lado do repositório.
- O Gradle lê `keystore.properties` dessa pasta. A variável `EGUATEX_CHAVE` troca o caminho do arquivo (`src-tauri/gen/android/app/build.gradle.kts`, linha 19).
- Guarde duas cópias fora do GitHub: uma num pendrive e outra num cofre de senhas ou no Drive pessoal.
- Nunca em git nem em Release. O `.gitignore` recusa `*.jks` e `keystore.properties`, e o `scripts/baixar-dependencias.sh` não toca nessa pasta.
- Sem essa chave, nenhum APK novo instala por cima dos que já estão instalados nos aparelhos. O Android recusa a atualização assinada com outra chave, e a pessoa teria de desinstalar o app e perder os projetos.
