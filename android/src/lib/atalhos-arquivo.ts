// Exportar e importar atalhos no celular. Exportar: "Salvar como" do plugin-dialog e cmd_gravar_bytes (o mesmo
// caminho do conversor de Word; o destino e um endereco content://). Importar: "Abrir" do plugin-dialog e
// cmd_ler_texto_importado, que copia o endereco para uma pasta temporaria com limite de 1 MB e devolve o texto.

import { invoke } from '@tauri-apps/api/core';
import { t, tf } from './i18n';
import { importarTexto, textoParaExportar } from './atalhos.svelte';
import { ERRO_JSON } from './barra-matematica';

function paraBase64(texto: string): string {
  const bytes = new TextEncoder().encode(texto);
  let binario = '';
  for (let i = 0; i < bytes.length; i += 0x8000) binario += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  return btoa(binario);
}

/** Texto da tela para o primeiro erro da importacao. O texto interno do validador nunca chega ao usuario. */
export function traduzirErroImportacao(primeiro: string | undefined): string {
  if (primeiro === ERRO_JSON) return t('atalhos.importar-json-invalido');
  if (primeiro?.includes('reservada')) return t('atalhos.importar-chave-reservada');
  if (primeiro?.includes('id reservado')) return t('atalhos.importar-id-reservado');
  if (primeiro?.includes(': comando ')) return t('atalhos.importar-comando-invalido');
  return t('atalhos.importar-formato');
}

export async function exportarAtalhos(): Promise<void> {
  try {
    const { save } = await import('@tauri-apps/plugin-dialog');
    const destino = await save({
      title: t('atalhos.exportar-titulo'),
      defaultPath: 'atalhos-eguatex.json',
      filters: [{ name: t('atalhos.exportar-filtro'), extensions: ['json'] }],
    });
    if (!destino) return;
    await invoke('cmd_gravar_bytes', { destino, dados: paraBase64(textoParaExportar()) });
  } catch (e) {
    alert(tf('atalhos.exportar-erro', { erro: String(e) }));
  }
}

/** Valida o texto no schema, soma aos atalhos atuais e avisa o resultado. */
export function importarAtalhosDeTexto(texto: string): void {
  const r = importarTexto(texto);
  if (r.ok) alert(t('atalhos.importar-ok'));
  else alert(tf('atalhos.importar-erro', { erro: traduzirErroImportacao(r.erros[0]) }));
}

/** Escolhe o arquivo no seletor do aparelho, le o texto e soma aos atalhos atuais. */
export async function importarAtalhosDoAparelho(): Promise<void> {
  try {
    const { open } = await import('@tauri-apps/plugin-dialog');
    const escolhido = await open({
      multiple: false,
      title: t('atalhos.importar'),
      filters: [{ name: t('atalhos.exportar-filtro'), extensions: ['json'] }],
    });
    if (!escolhido) return;
    const origem = Array.isArray(escolhido) ? escolhido[0] : escolhido;
    const texto = await invoke<string>('cmd_ler_texto_importado', { origem });
    importarAtalhosDeTexto(texto);
  } catch (e) {
    alert(tf('atalhos.importar-erro', { erro: String(e) }));
  }
}
