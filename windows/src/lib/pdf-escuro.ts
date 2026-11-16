export type ModoPdfEscuro = "desligado" | "tema" | "sempre";

/** Se as paginas do PDF ficam invertidas: "tema" segue o tema escuro da interface. */
export function pdfInvertido(modo: ModoPdfEscuro, temaEscuro: boolean): boolean {
  if (modo === "sempre") return true;
  if (modo === "tema") return temaEscuro;
  return false;
}

/** O botao da barra alterna entre desligado e sempre; a opcao "tema" vira "desligado" se ja esta invertido. */
export function alternarModo(modo: ModoPdfEscuro, invertidoAgora: boolean): ModoPdfEscuro {
  if (modo === "sempre") return "desligado";
  if (modo === "tema") return invertidoAgora ? "desligado" : "sempre";
  return "sempre";
}
