// @ts-nocheck: usa modulos do Node (fs, zlib); o projeto nao tem @types/node
// Leitor minimo de zip para os testes: le o diretorio central e aceita entradas sem
// compressao (as do pdf-para-word) ou com deflate.
// @ts-expect-error sem @types/node no projeto
import { inflateRawSync } from 'node:zlib';

export function descompactar(zip: Uint8Array): Record<string, Uint8Array> {
  const v = new DataView(zip.buffer, zip.byteOffset, zip.byteLength);
  let fim = zip.length - 22;
  while (fim > 0 && v.getUint32(fim, true) !== 0x06054b50) fim--;
  const n = v.getUint16(fim + 10, true);
  let pos = v.getUint32(fim + 16, true);
  const saida: Record<string, Uint8Array> = {};
  for (let i = 0; i < n; i++) {
    const metodo = v.getUint16(pos + 10, true);
    const tam = v.getUint32(pos + 20, true);
    const nome = v.getUint16(pos + 28, true);
    const extra = v.getUint16(pos + 30, true);
    const coment = v.getUint16(pos + 32, true);
    const local = v.getUint32(pos + 42, true);
    const nomeTxt = new TextDecoder().decode(zip.subarray(pos + 46, pos + 46 + nome));
    const ini = local + 30 + v.getUint16(local + 26, true) + v.getUint16(local + 28, true);
    const bruto = zip.subarray(ini, ini + tam);
    saida[nomeTxt] = metodo === 8 ? new Uint8Array(inflateRawSync(bruto)) : bruto;
    pos += 46 + nome + extra + coment;
  }
  return saida;
}
