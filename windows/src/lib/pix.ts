// Gerador de payload Pix BR Code estatico (EMV QRCPS) para apoio ao EguaTex.

export interface DadosPix {
  chave: string;
  nome: string;
  cidade: string;
  valor?: string;
  txid?: string;
}

function pad(id: string, valor: string): string {
  const len = String(valor.length).padStart(2, '0');
  return `${id}${len}${valor}`;
}

// Calculo CRC16 CCITT-FALSE (polinomio 0x1021, inicio 0xFFFF)
export function calcularCrc16(payload: string): string {
  let crc = 0xFFFF;
  for (let i = 0; i < payload.length; i++) {
    crc ^= payload.charCodeAt(i) << 8;
    for (let j = 0; j < 8; j++) {
      if ((crc & 0x8000) !== 0) {
        crc = (crc << 1) ^ 0x1021;
      } else {
        crc = crc << 1;
      }
      crc &= 0xFFFF;
    }
  }
  return crc.toString(16).toUpperCase().padStart(4, '0');
}

export function gerarPayloadPix(dados: DadosPix): string {
  const gui = pad('00', 'br.gov.bcb.pix');
  const chavePix = pad('01', dados.chave);
  const merchantAccount = pad('26', gui + chavePix);

  const mcc = pad('52', '0000');
  const moeda = pad('53', '986'); // BRL
  const valorStr = dados.valor ? pad('54', dados.valor) : '';
  const pais = pad('58', 'BR');
  const nome = pad('59', dados.nome.substring(0, 25));
  const cidade = pad('60', dados.cidade.substring(0, 15));

  const txidVal = dados.txid || '***';
  const addData = pad('62', pad('03', txidVal));

  const payloadSemCrc =
    pad('00', '01') +
    merchantAccount +
    mcc +
    moeda +
    valorStr +
    pais +
    nome +
    cidade +
    addData +
    '6304';

  const crc = calcularCrc16(payloadSemCrc);
  return payloadSemCrc + crc;
}

/**
 * Pix de apoio ao ÉguaTeX. O payload e o texto exato do QR Code gerado pelo banco
 * (lido da imagem em 16/11/2026). Ele NAO e montado de novo pelo app: qualquer mudanca
 * de campo muda o CRC e corrompe o QR. Para trocar, cole o texto novo inteiro do banco;
 * o teste `pix.test.ts` confere o CRC e a chave.
 */
export const PIX_APOIO = {
  chave: 'contatoeguatex@gmail.com',
  nome: 'JOSE PAULO DE ALMEIDA JUNIOR',
  payload: '00020126460014br.gov.bcb.pix0124contatoeguatex@gmail.com5204000053039865802BR5924JOSEPAULODEALMEIDAJUNIOR6009Sao Paulo610901227-20062230519daqr7406303692472506304D3FB',
} as const;

/** O payload termina com o CRC16 certo (campo 63, 4 digitos hexadecimais)? */
export function payloadPixValido(payload: string): boolean {
  const m = /^(.*6304)([0-9A-F]{4})$/.exec(payload);
  return !!m && calcularCrc16(m[1]) === m[2];
}
