// Celular em pe, como o Overleaf no navegador do celular: a interface do computador inteira
// (menus em cima, editor e PDF lado a lado) desenhada numa largura de LARGURA_RETRATO px e reduzida
// (transform: scale) para caber na tela. Fontes e icones ficam proporcionalmente menores, sem um layout separado.

/** Largura (px CSS) em que a interface e desenhada no celular em pe. */
export const LARGURA_RETRATO = 720;

/** Celular em pe: tela de toque, em pe e estreita (menos de 600 px). Tablet nao entra. */
export const CONSULTA_RETRATO = '(orientation: portrait) and (max-width: 599px) and (pointer: coarse)';

/** Fator de reducao para a largura da tela (nunca aumenta). */
export function fatorRetrato(larguraTela: number): number {
  if (!Number.isFinite(larguraTela) || larguraTela <= 0) return 1;
  return Math.min(1, Math.round((larguraTela / LARGURA_RETRATO) * 1000) / 1000);
}

/** Liga ou desliga o retrato: o <body> e desenhado com 720 px e reduzido; o <html> fica com a largura
 *  do aparelho e corta o que sobra. Devolve se esta ligado. */
export function aplicarRetrato(raiz: HTMLElement, emRetrato: boolean, larguraTela: number, corpo?: HTMLElement): boolean {
  const body = corpo ?? (raiz.ownerDocument?.body as HTMLElement | undefined);
  // transform: scale (e nao zoom): o CodeMirror mede a escala pelo tamanho na tela e acerta o toque.
  // A escala fica no <body>: com o <html> mais largo que a tela, o WebView reduzia a pagina sozinho
  // (innerHeight ia a 1467) e o toque e a altura saiam errados (emulador, 13/11/2026).
  if (emRetrato && body) {
    const fator = fatorRetrato(larguraTela);
    const altura = `${Math.round(window.innerHeight / fator)}px`;
    raiz.style.setProperty('overflow', 'hidden');
    raiz.style.setProperty('--altura-retrato', altura);
    raiz.dataset.retrato = '1';
    body.style.setProperty('width', `${LARGURA_RETRATO}px`);
    body.style.setProperty('height', altura);
    body.style.setProperty('transform', `scale(${fator})`);
    body.style.setProperty('transform-origin', '0 0');
    body.style.setProperty('overflow', 'hidden');
    return true;
  }
  for (const p of ['overflow', '--altura-retrato', 'zoom', 'width', 'height', 'transform', 'transform-origin']) raiz.style.removeProperty(p);
  delete raiz.dataset.retrato;
  if (body) for (const p of ['width', 'height', 'transform', 'transform-origin', 'overflow']) body.style.removeProperty(p);
  return false;
}

/** Acompanha a orientacao e mantem o <html> no modo certo enquanto o editor esta aberto (a lista de
 *  projetos tem layout proprio de celular, como no Overleaf). Devolve a funcao que desliga. */
export function acompanharRetrato(aoMudar?: (emRetrato: boolean) => void): () => void {
  if (typeof window === 'undefined' || !window.matchMedia) return () => {};
  const consulta = window.matchMedia(CONSULTA_RETRATO);
  // aplica antes e so depois avisa: em aoMudar?.(aplicar(...)) o argumento nem roda sem aoMudar
  const atualizar = () => {
    const ligado = aplicarRetrato(document.documentElement, consulta.matches, window.innerWidth, document.body);
    aoMudar?.(ligado);
  };
  atualizar();
  consulta.addEventListener('change', atualizar);
  window.addEventListener('resize', atualizar);
  return () => {
    consulta.removeEventListener('change', atualizar);
    window.removeEventListener('resize', atualizar);
    aplicarRetrato(document.documentElement, false, window.innerWidth, document.body);
  };
}
