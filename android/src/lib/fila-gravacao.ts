// Fila única de gravação por arquivo. Salvar, a IA e o apagar passam por aqui: duas gravações do
// mesmo arquivo nunca rodam juntas, e quem vai apagar espera as gravações em voo terminarem.

interface Entrada {
  caminho: string;
  vez: Promise<void>;
}

const fila = new Map<string, Entrada>();

// "/a.tex" e "a.tex" são o mesmo arquivo (a árvore usa a barra, a IA não)
const chave = (caminho: string) => caminho.replace(/\\/g, '/').replace(/^\/+/, '');

/**
 * Roda `tarefa` quando as gravações anteriores do mesmo arquivo terminarem. `esperou` diz se havia
 * uma gravação na frente: quem esperou deve conferir se ainda precisa gravar. Erro da tarefa
 * passa adiante, e a vez do próximo chega do mesmo jeito.
 */
export async function naFila<T>(caminho: string, tarefa: (esperou: boolean) => Promise<T>): Promise<T> {
  const k = chave(caminho);
  const anterior = fila.get(k)?.vez;
  let terminar!: () => void;
  const vez = new Promise<void>((resolver) => (terminar = resolver));
  const entrada: Entrada = { caminho, vez };
  fila.set(k, entrada);
  try {
    if (anterior) await anterior;
    return await tarefa(anterior !== undefined);
  } finally {
    terminar();
    if (fila.get(k) === entrada) fila.delete(k);
  }
}

/** Espera as gravações em voo dos arquivos que `filtro` aceita (recebe o caminho como foi enfileirado). */
export async function esperar(filtro: (caminho: string) => boolean): Promise<void> {
  const pendentes = [...fila.values()].filter((e) => filtro(e.caminho)).map((e) => e.vez);
  await Promise.all(pendentes);
}
