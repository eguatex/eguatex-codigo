// Estado reativo dos atalhos do usuario (categorias, botoes, macros). A logica fica em barra-matematica.ts;
// aqui cada mudanca troca `atalhos.usuario` e grava em localStorage. A categoria ativa mora nas preferencias.
// Nunca reatribua `atalhos`; troque so `atalhos.usuario`.

import { preferencias, definirPreferencia } from './preferencias.svelte';
import { t } from './i18n';
import {
  carregarUsuario,
  gravarUsuario,
  importarAtalhos,
  serializarExportacao,
  type AtalhosUsuario,
  type CategoriaEfetiva,
  type ResultadoImportacao,
} from './barra-matematica';

/** Nome na tela: o que o usuario escreveu ou, nas padrao sem renomear, o do i18n. */
export function nomeDaCategoria(c: CategoriaEfetiva): string {
  return c.nome ?? t(`atalhos.cat.${c.id}` as Parameters<typeof t>[0]);
}

export const atalhos =$state<{ usuario: AtalhosUsuario }>({ usuario: carregarUsuario() });

/** Aplica uma funcao pura ao estado e grava. */
export function alterarAtalhos(f: (atual: AtalhosUsuario) => AtalhosUsuario): void {
  const novo = f($state.snapshot(atalhos.usuario) as AtalhosUsuario);
  atalhos.usuario = novo;
  gravarUsuario(novo);
}

/** Clicar na categoria ativa de novo fecha a barra. */
export function alternarCategoria(id: string): void {
  definirPreferencia('categoriaAtiva', preferencias.categoriaAtiva === id ? '' : id);
}

export function fecharBarraMatematica(): void {
  definirPreferencia('categoriaAtiva', '');
}

export function importarTexto(texto: string): ResultadoImportacao {
  const r = importarAtalhos(texto, $state.snapshot(atalhos.usuario) as AtalhosUsuario);
  if (r.ok) {
    atalhos.usuario = r.usuario;
    gravarUsuario(r.usuario);
  }
  return r;
}

export function textoParaExportar(): string {
  return serializarExportacao($state.snapshot(atalhos.usuario) as AtalhosUsuario);
}
