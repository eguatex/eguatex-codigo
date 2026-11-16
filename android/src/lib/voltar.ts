// Botao Voltar do Android: fecha o que estiver aberto (menu, popover, gaveta) em vez de sair do app.
//
// Cada item aberto poe uma entrada no historico do WebView. O Voltar tira uma entrada, dispara
// `popstate` e fecha o ultimo item registrado. Nunca se poe entrada nova dentro do `popstate`
// (o WebView nao a conta para o proximo Voltar).
//
// Quando um item fecha por outro caminho (toque fora, escolha de item), a entrada sobra. Ela sai por
// `history.go(-n)` num `setTimeout(0)`, com n = entradas que passam do numero de itens abertos.
// Se outro item abriu nesse meio tempo (um toque que fecha um e abre outro), ele aproveita a
// entrada sobrante e n sai 0: nao ha `history.back()` pendente competindo com o `pushState`.

const pilha: Array<() => void> = [];
let entradas = 0; // entradas extras que este modulo poe no historico
let ignorar = 0; // popstate gerado por history.go(), que nao e um Voltar da pessoa
let ouvindo = false;
let limpezaAgendada = false;

function aoVoltar() {
  if (ignorar > 0) {
    ignorar--;
    return;
  }
  if (entradas > 0) entradas--;
  pilha.pop()?.();
}

function agendarLimpeza() {
  if (limpezaAgendada) return;
  limpezaAgendada = true;
  setTimeout(() => {
    limpezaAgendada = false;
    const sobra = entradas - pilha.length;
    if (sobra > 0) {
      entradas -= sobra;
      ignorar++;
      history.go(-sobra);
    }
  }, 0);
}

/** Chame ao abrir. Devolve a funcao que deve rodar quando o elemento fechar por qualquer motivo. */
export function registrarFechavel(fechar: () => void): () => void {
  if (!ouvindo) {
    window.addEventListener('popstate', aoVoltar);
    ouvindo = true;
  }
  pilha.push(fechar);
  // a entrada de um fechamento ainda pendente serve para este item
  if (entradas < pilha.length) {
    history.pushState({ eguatexVoltar: true }, '');
    entradas++;
  }
  return () => {
    const i = pilha.indexOf(fechar);
    if (i === -1) return; // ja foi fechado pelo Voltar
    pilha.splice(i, 1);
    agendarLimpeza();
  };
}
