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
