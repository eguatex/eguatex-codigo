export interface Modelo {
  id: string;
  nome: string;
  descricao: string;
  nomePadrao: string;
  /** Texto do main.tex. */
  conteudo: string;
}

const branco = String.raw`\documentclass[12pt,a4paper]{article}
\usepackage[T1]{fontenc}
\usepackage[utf8]{inputenc}
\usepackage[brazilian]{babel}
\usepackage{amsmath}
\usepackage{graphicx}
\usepackage{hyperref}

\title{Meu Documento}
\author{Seu Nome}
\date{\today}

\begin{document}
\maketitle

\section{Introdução}
Escreva aqui o texto do seu documento.

\end{document}
`;

export const modelos: Modelo[] = [
  { id: 'branco', nome: 'Documento em branco', descricao: 'Comece do zero', nomePadrao: 'Meu documento', conteudo: branco },
];

export function nomeDePasta(nome: string): string {
  return nome
    .normalize('NFD')
    .replace(/[̀-ͯ]/g, '')
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '') || 'projeto';
}
