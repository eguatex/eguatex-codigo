// Exportar e importar atalhos: "Salvar como" do Tauri + cmd_gravar_bytes (o mesmo caminho do conversor de Word)
// e leitura do arquivo escolhido pelo seletor do sistema (input type=file).

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

/** Le o arquivo, valida no schema e soma aos atalhos atuais. Avisa o resultado. */
export async function importarAtalhosDeArquivo(arquivo: File): Promise<void> {
  try {
    const r = importarTexto(await arquivo.text());
    if (r.ok) alert(t('atalhos.importar-ok'));
    else {
      const erro = traduzirErroImportacao(r.erros[0]);
      alert(tf('atalhos.importar-erro', { erro }));
    }
  } catch (e) {
    alert(tf('atalhos.importar-erro', { erro: String(e) }));
  }
}
