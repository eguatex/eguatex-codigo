// Estrutura dos menus adaptada de services/web/frontend/js/features/ide-react/components/toolbar/menu-bar.tsx (Overleaf, AGPL-3.0)

import { t } from '$lib/i18n';

export type Acao = (arg?: string) => void;
export type Acoes = Record<string, Acao>;

export type ItemMenu =
  | { tipo: 'comando'; id: string; nome: string; atalho?: string; marcavel?: boolean }
  | { tipo: 'sub'; nome: string; itens: ItemMenu[] }
  | { tipo: 'titulo'; nome: string }
  | { tipo: 'sep' };

export interface Menu {
  nome: string;
  itens: ItemMenu[];
}

const cmd = (id: string, nome: string, atalho?: string): ItemMenu => ({ tipo: 'comando', id, nome, atalho });
const marca = (id: string, nome: string): ItemMenu => ({ tipo: 'comando', id, nome, marcavel: true });
const sub = (nome: string, itens: ItemMenu[]): ItemMenu => ({ tipo: 'sub', nome, itens });
const titulo = (nome: string): ItemMenu => ({ tipo: 'titulo', nome });
const sep: ItemMenu = { tipo: 'sep' };

export const MENUS: Menu[] = [
  {
    nome: t('menu.nome-arquivo'),
    itens: [
      cmd('novo-arquivo', t('menu.novo-arquivo')),
      cmd('nova-pasta', t('menu.nova-pasta')),
      cmd('enviar-arquivo', t('menu.enviar-arquivo')),
      cmd('drive-adicionar-arquivos', t('drive.adicionar-arquivos')),
      cmd('copiar-projeto', t('menu.copiar-projeto')),
      cmd('salvar-no-drive', t('drive.salvar-no-drive')),
      sep,
      cmd('compilar', t('menu.compilar'), 'Ctrl+Enter'),
      cmd('limpar-auxiliares', t('menu.limpar-auxiliares')),
      sep,
      cmd('contar-palavras', t('menu.contar-palavras')),
      cmd('versoes-anteriores', t('menu.versoes-anteriores')),
      sep,
      sub(t('menu.sub-salvar-como'), [
        cmd('baixar-zip', t('menu.baixar-zip')),
        cmd('baixar-pdf', t('menu.baixar-pdf')),
        cmd('exportar-docx', t('menu.exportar-docx')),
        cmd('exportar-md', t('menu.exportar-md')),
        cmd('exportar-html', t('menu.exportar-html')),
      ]),
      sep,
      cmd('configuracoes', t('menu.configuracoes')),
    ],
  },
  {
    nome: t('menu.nome-editar'),
    itens: [
      cmd('desfazer', t('menu.desfazer'), 'Ctrl+Z'),
      cmd('refazer', t('menu.refazer'), 'Ctrl+Y'),
      sep,
      cmd('localizar', t('menu.localizar'), 'Ctrl+F'),
      cmd('buscar-substituir', t('menu.buscar-substituir'), 'Ctrl+H'),
      cmd('selecionar-tudo', t('menu.selecionar-tudo'), 'Ctrl+A'),
      sep,
      cmd('traduzir', t('menu.traduzir'), 'Ctrl+Shift+T'),
    ],
  },
  {
    nome: t('menu.nome-inserir'),
    itens: [
      sub(t('menu.sub-matematica'), [
        cmd('matematica-linha', t('menu.matematica-linha')),
        cmd('matematica-destaque', t('menu.matematica-destaque')),
        sep,
        cmd('ambiente-equation', t('matematica.equation')),
        cmd('ambiente-align', t('matematica.align')),
        cmd('ambiente-gather', t('matematica.gather')),
        cmd('ambiente-cases', t('matematica.cases')),
        cmd('ambiente-pmatrix-2x2', t('matematica.pmatrix-2x2')),
        cmd('ambiente-pmatrix-3x3', t('matematica.pmatrix-3x3')),
        cmd('ambiente-integral', t('matematica.integral')),
        cmd('ambiente-fracao', t('matematica.fracao')),
      ]),
      cmd('simbolo', t('menu.simbolo')),
      sub(t('menu.sub-figura'), [
        cmd('figura-computador', t('menu.figura-computador')),
        cmd('figura-projeto', t('menu.figura-projeto')),
        cmd('figura-drive', t('figura.do-drive')),
      ]),
      sub(t('menu.sub-tabela'), [
        cmd('tabela', t('menu.tabela')),
        cmd('tabela-assistente', t('menu.tabela-assistente')),
        cmd('tabela-planilha', t('menu.tabela-planilha')),
      ]),
      cmd('citacao', t('menu.citacao')),
      cmd('link', t('menu.link')),
      cmd('referencia', t('menu.referencia')),
      sep,
      cmd('comentario', t('menu.comentario'), 'Ctrl+/'),
    ],
  },
  {
    nome: t('menu.nome-ver'),
    itens: [
      marca('layout-ambos', t('menu.layout-ambos')),
      marca('layout-editor', t('menu.layout-editor')),
      marca('layout-pdf', t('menu.layout-pdf')),
      sep,
      titulo(t('menu.titulo-configuracoes-do-editor')),
      marca('ver-trilha', t('menu.ver-trilha')),
      marca('ver-abas', t('menu.ver-abas')),
      marca('ver-previa-equacao', t('menu.ver-previa-equacao')),
      sep,
      cmd('proxima-aba', t('abas.proxima'), 'Ctrl+Tab'),
      cmd('aba-anterior', t('abas.anterior'), 'Ctrl+Shift+Tab'),
      cmd('fechar-aba', t('abas.fechar-atual'), 'Ctrl+W'),
      sep,
      cmd('paleta-comandos', t('menu.paleta-comandos'), 'Ctrl+P'),
      sep,
      titulo(t('menu.titulo-visualizacao-do-pdf')),
      cmd('pdf-apresentacao', t('menu.pdf-apresentacao')),
      sub(t('menu.sub-zoom-do-pdf'), [
        cmd('pdf-mais-zoom', t('menu.pdf-mais-zoom')),
        cmd('pdf-menos-zoom', t('menu.pdf-menos-zoom')),
        cmd('pdf-ajustar-largura', t('menu.pdf-ajustar-largura')),
        cmd('pdf-ajustar-altura', t('menu.pdf-ajustar-altura')),
      ]),
    ],
  },
  {
    nome: t('menu.nome-formatar'),
    itens: [
      cmd('negrito', t('menu.negrito'), 'Ctrl+B'),
      cmd('italico', t('menu.italico'), 'Ctrl+I'),
      sep,
      cmd('lista-marcadores', t('menu.lista-marcadores')),
      cmd('lista-numerada', t('menu.lista-numerada')),
      cmd('aumentar-recuo', t('menu.aumentar-recuo'), 'Tab'),
      cmd('diminuir-recuo', t('menu.diminuir-recuo'), 'Shift+Tab'),
      sep,
      titulo(t('menu.titulo-estilos-de-paragrafo')),
      cmd('estilo-normal', t('menu.estilo-normal')),
      cmd('estilo-secao', t('menu.estilo-secao')),
      cmd('estilo-subsecao', t('menu.estilo-subsecao')),
      cmd('estilo-subsubsecao', t('menu.estilo-subsubsecao')),
      cmd('estilo-paragrafo', t('menu.estilo-paragrafo')),
      cmd('estilo-subparagrafo', t('menu.estilo-subparagrafo')),
    ],
  },
  {
    nome: t('menu.nome-ajuda'),
    itens: [
      cmd('como-usar', t('menu.como-usar')),
      cmd('atalhos', t('menu.atalhos')),
      cmd('documentacao', t('menu.documentacao')),
      cmd('site', t('menu.site')),
      cmd('contato', t('menu.contato')),
      sep,
      cmd('apoiar', t('menu.apoiar')),
      cmd('feedback', t('menu.feedback')),
      cmd('procurar-atualizacao', t('menu.atualizacao')),
      sep,
      cmd('sobre', t('menu.sobre')),
    ],
  },
];

export interface ComandoListado {
  id: string;
  nome: string;
  caminho: string;
  atalho?: string;
}

export function listarComandos(): ComandoListado[] {
  const saida: ComandoListado[] = [];
  const visitar = (itens: ItemMenu[], caminho: string) => {
    for (const item of itens) {
      if (item.tipo === 'comando') saida.push({ id: item.id, nome: item.nome, caminho, atalho: item.atalho });
      else if (item.tipo === 'sub') visitar(item.itens, `${caminho} > ${item.nome}`);
    }
  };
  for (const menu of MENUS) visitar(menu.itens, menu.nome);
  return saida;
}

// "tabela:3x4" vira executar('tabela', '3x4')
export function executar(acoes: Acoes, id: string) {
  const [nome, arg] = id.split(':', 2);
  acoes[nome]?.(arg);
}
