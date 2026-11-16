// Conta Google unica do ÉguaTex: estado e acoes compartilhados por todos os lugares que mostram
// ou pedem o login (icone da conta, Drive, compartilhamento, seletor de arquivos).
// O login libera o Drive, o compartilhamento e a abertura de arquivos do Google. O assistente de
// IA nao usa o login: usa a chave de API da propria pessoa (ConfigIA.svelte).
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { t } from "$lib/i18n";

export type InfoConta = {
  conectado: boolean;
  configurado: boolean;
  email: string;
  nome: string;
  foto: string;
  /** Aviso do Rust quando a conta antiga foi desfeita e a pessoa precisa entrar de novo. */
  aviso?: string;
};

export const conta = $state<{ info: InfoConta; ocupado: boolean; progresso: string; erro: string }>({
  info: { conectado: false, configurado: true, email: "", nome: "", foto: "", aviso: "" },
  ocupado: false,
  progresso: "",
  erro: "",
});

function avisarMudanca() {
  window.dispatchEvent(new Event("conta-google-mudou"));
}

export async function atualizarConta(): Promise<void> {
  try {
    conta.info = await invoke<InfoConta>("cmd_ia_google_info");
  } catch (e) {
    console.error("Erro ao ler a conta Google:", e);
  }
}

async function comProgresso(inicio: string, acao: () => Promise<unknown>): Promise<void> {
  conta.ocupado = true;
  conta.erro = "";
  conta.progresso = inicio;
  const parar = await listen<string>("ia-google-progresso", (e) => (conta.progresso = e.payload));
  try {
    await acao();
  } catch (e) {
    conta.erro = String(e);
    throw e;
  } finally {
    parar();
    conta.ocupado = false;
    conta.progresso = "";
    await atualizarConta();
    avisarMudanca();
  }
}

/** Login no navegador. Todos os botoes "Entrar com Google" chamam esta funcao. Repassa o erro. */
export function entrarComGoogle(): Promise<void> {
  return comProgresso(t("conta.abrindo-navegador"), () => invoke("cmd_conta_google_entrar"));
}

export async function sairDaConta(): Promise<void> {
  conta.erro = "";
  try {
    await invoke("cmd_ia_google_sair");
  } catch (e) {
    conta.erro = String(e);
  }
  await atualizarConta();
  avisarMudanca();
}
