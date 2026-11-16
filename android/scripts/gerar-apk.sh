#!/usr/bin/env bash
# Gera o APK do EguaTex Android assinado.
#   bash scripts/gerar-apk.sh                      celulares e tablets (arm64 e armv7), no Desktop
#   ALVOS=x86_64 SAIDA=arquivo.apk bash ...        APK para o emulador do PC
#
# O `tauri android build` termina criando um link simbolico do .so do Rust, que o
# Windows so permite no Modo de Desenvolvedor. Por isso o script:
#   1. deixa o Tauri compilar o frontend e o Rust de cada alvo (o passo do link falha);
#   2. copia os .so e o motor LaTeX (busytex) para o projeto Gradle;
#   3. roda o Gradle pulando a etapa do Rust.
set -euo pipefail

RAIZ="$(cd "$(dirname "$0")/.." && pwd)"
DEPS="$RAIZ/../eguatex-android-deps/busytex"
# EGUATEX_DEPS troca a pasta base, como no scripts/baixar-dependencias.sh
[ -n "${EGUATEX_DEPS:-}" ] && DEPS="$EGUATEX_DEPS/eguatex-android-deps/busytex"
if [ ! -f "$DEPS/busytex.wasm" ]; then
  echo "Falta o motor LaTeX em $DEPS (busytex.wasm). Rode antes: bash scripts/baixar-dependencias.sh"; exit 1
fi
APP="$RAIZ/src-tauri/gen/android/app"
# Android SDK e NDK 27: vem de ANDROID_HOME e NDK_HOME; sem elas, procura em <pasta do usuario>/Android/Sdk
nativo() { if command -v cygpath >/dev/null 2>&1; then cygpath -w "$1"; else printf '%s' "$1"; fi; }
if [ -z "${ANDROID_HOME:-}" ]; then
  [ -d "$HOME/Android/Sdk" ] || { echo "Defina ANDROID_HOME com a pasta do Android SDK."; exit 1; }
  ANDROID_HOME="$(nativo "$HOME/Android/Sdk")"
fi
if [ -z "${NDK_HOME:-}" ]; then
  NDK="$(ls -d "$ANDROID_HOME"/ndk/27.* 2>/dev/null | sort | tail -1)"
  [ -n "$NDK" ] || { echo "Defina NDK_HOME com a pasta do NDK 27 (falta em $ANDROID_HOME/ndk)."; exit 1; }
  NDK_HOME="$(nativo "$NDK")"
fi
export ANDROID_HOME NDK_HOME

declare -A ABI=( [aarch64]=arm64-v8a [armv7]=armeabi-v7a [x86_64]=x86_64 )
declare -A TRIPLA=( [aarch64]=aarch64-linux-android [armv7]=armv7-linux-androideabi [x86_64]=x86_64-linux-android )
declare -A ARQ=( [aarch64]=arm64 [armv7]=arm [x86_64]=x86_64 )
ALVOS="${ALVOS:-aarch64 armv7}"
SAIDA="${SAIDA:-$HOME/Desktop/EguaTex_android.apk}"

# o app foi pensado para o celular deitado; sem isto o editor e o PDF nao cabem lado a lado
if ! grep -q 'android:screenOrientation="fullSensor"' "$APP/src/main/AndroidManifest.xml"; then
  echo "AndroidManifest.xml sem android:screenOrientation=\"fullSensor\" na activity (o app gira: em pe como o Overleaf, deitado lado a lado)."; exit 1
fi

cd "$RAIZ"
for alvo in $ALVOS; do
  so="$RAIZ/src-tauri/target/${TRIPLA[$alvo]}/release/libeguatex_lib.so"
  echo "== Rust para $alvo"
  # apaga o .so anterior: se a compilacao falhar, o APK nao sai com o .so de uma versao antiga
  rm -f "$so"
  # EXTRA_TAURI="--features teste-devtools" so no APK de teste do emulador (depuracao do WebView para o Playwright)
  npx tauri android build --apk --target "$alvo" ${EXTRA_TAURI:-} > "$RAIZ/scripts/build-$alvo.log" 2>&1 || true
  # o erro do link simbolico e esperado (Windows sem Modo de Desenvolvedor); qualquer outro erro para o script
  if grep -i "^ *error" "$RAIZ/scripts/build-$alvo.log" | grep -qv "symbolic link" || [ ! -f "$so" ]; then
    echo "Falhou a compilacao do Rust para $alvo. Veja scripts/build-$alvo.log"; exit 1
  fi
done

# so os ABIs pedidos entram no APK
rm -rf "$APP/src/main/jniLibs"
for alvo in $ALVOS; do
  mkdir -p "$APP/src/main/jniLibs/${ABI[$alvo]}"
  cp "$RAIZ/src-tauri/target/${TRIPLA[$alvo]}/release/libeguatex_lib.so" "$APP/src/main/jniLibs/${ABI[$alvo]}/"
done

echo "== Motor LaTeX"
DEST="$APP/src/main/assets/busytex"
mkdir -p "$DEST"
for f in busytex.js busytex.wasm \
  texlive-basic.js texlive-basic.data \
  ubuntu-texlive-latex-base.js ubuntu-texlive-latex-base.data \
  ubuntu-texlive-latex-recommended.js ubuntu-texlive-latex-recommended.data \
  ubuntu-texlive-latex-extra.js ubuntu-texlive-latex-extra.data \
  ubuntu-texlive-science.js ubuntu-texlive-science.data \
  ubuntu-texlive-fonts-recommended.js ubuntu-texlive-fonts-recommended.data; do
  [ -f "$DEST/$f" ] && [ "$DEST/$f" -nt "$DEPS/$f" ] || cp "$DEPS/$f" "$DEST/"
done
cp "$RAIZ/motor-tex/busytex_pipeline.js" "$RAIZ/motor-tex/eguatex_worker.js" "$DEST/"
[ -d "$RAIZ/motor-tex/extras" ] && cp -r "$RAIZ/motor-tex/extras" "$DEST/" || true

echo "== Gradle"
abis=""; arqs=""; alvos=""
for a in $ALVOS; do abis="$abis${ABI[$a]},"; arqs="$arqs${ARQ[$a]},"; alvos="$alvos$a,"; done
cd "$RAIZ/src-tauri/gen/android"
./gradlew.bat assembleUniversalRelease -x rustBuildUniversalRelease \
  -PabiList="${abis%,}" -ParchList="${arqs%,}" -PtargetList="${alvos%,}" \
  > "$RAIZ/scripts/build-gradle.log" 2>&1 || { tail -40 "$RAIZ/scripts/build-gradle.log"; exit 1; }

APK=$(ls -t app/build/outputs/apk/universal/release/*.apk | head -1)
cp "$APK" "$SAIDA"
ls -la "$SAIDA"
