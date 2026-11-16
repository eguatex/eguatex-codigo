package br.ufpa.eguatex

import android.app.Activity
import android.content.Intent
import android.net.Uri
import android.provider.DocumentsContract
import java.io.File

/**
 * "Abrir pasta existente" no Android: o seletor de pastas do sistema (ACTION_OPEN_DOCUMENT_TREE)
 * e a copia da pasta escolhida, com as subpastas, para a pasta de projetos do app.
 *
 * O Rust chama escolher(pai) pelo JNI e depois pergunta resultado() ate ele deixar de ser null:
 * "" quando a pessoa cancela, "erro:..." quando a copia falha, ou o caminho da pasta copiada.
 */
object EscolherPasta {
  const val PEDIDO = 7301
  private const val LIMITE_BYTES = 300L * 1024 * 1024
  private const val LIMITE_ARQUIVOS = 5000

  @Volatile private var resultado: String? = null
  @Volatile private var pai: String = ""

  @JvmStatic
  fun escolher(pastaPai: String): Boolean {
    val atividade = MainActivity.instancia ?: return false
    resultado = null
    pai = pastaPai
    atividade.runOnUiThread {
      @Suppress("DEPRECATION")
      atividade.startActivityForResult(Intent(Intent.ACTION_OPEN_DOCUMENT_TREE), PEDIDO)
    }
    return true
  }

  @JvmStatic
  fun resultado(): String? = resultado

  /** Chamado pela MainActivity quando o seletor fecha. A copia roda fora da thread da tela. */
  fun aoVoltar(atividade: Activity, codigo: Int, dados: Intent?) {
    val arvore = dados?.data
    if (codigo != Activity.RESULT_OK || arvore == null) {
      resultado = ""
      return
    }
    Thread {
      resultado = try {
        copiar(atividade, arvore, File(pai)).absolutePath
      } catch (e: Exception) {
        "erro:" + (e.message ?: e.toString())
      }
    }.start()
  }

  private class Conta { var bytes = 0L; var arquivos = 0 }

  private fun nomeSeguro(nome: String): Boolean =
    nome.isNotBlank() && nome != "." && nome != ".." && !nome.contains('/') && !nome.contains('\\') && !nome.startsWith(".")

  private fun copiar(atividade: Activity, arvore: Uri, pastaPai: File): File {
    val raizId = DocumentsContract.getTreeDocumentId(arvore)
    var nome = nomeDoDocumento(atividade, DocumentsContract.buildDocumentUriUsingTree(arvore, raizId)) ?: "pasta"
    if (!nomeSeguro(nome)) nome = "pasta"
    // nome livre: "tese", "tese (2)", "tese (3)"...
    var destino = File(pastaPai, nome)
    var n = 2
    while (destino.exists()) { destino = File(pastaPai, "$nome ($n)"); n++ }
    destino.mkdirs()
    try {
      copiarFilhos(atividade, arvore, raizId, destino, Conta())
    } catch (e: Exception) {
      destino.deleteRecursively()
      throw e
    }
    return destino
  }

  private fun nomeDoDocumento(atividade: Activity, uri: Uri): String? =
    atividade.contentResolver.query(uri, arrayOf(DocumentsContract.Document.COLUMN_DISPLAY_NAME), null, null, null)?.use {
      if (it.moveToFirst()) it.getString(0) else null
    }

  private fun copiarFilhos(atividade: Activity, arvore: Uri, paiId: String, destino: File, conta: Conta) {
    val filhos = DocumentsContract.buildChildDocumentsUriUsingTree(arvore, paiId)
    val colunas = arrayOf(
      DocumentsContract.Document.COLUMN_DOCUMENT_ID,
      DocumentsContract.Document.COLUMN_DISPLAY_NAME,
      DocumentsContract.Document.COLUMN_MIME_TYPE,
    )
    atividade.contentResolver.query(filhos, colunas, null, null, null)?.use { c ->
      while (c.moveToNext()) {
        val id = c.getString(0)
        val nome = c.getString(1) ?: continue
        // ocultos (.git, .DS_Store) e nomes estranhos ficam de fora
        if (!nomeSeguro(nome)) continue
        val alvo = File(destino, nome)
        if (c.getString(2) == DocumentsContract.Document.MIME_TYPE_DIR) {
          alvo.mkdirs()
          copiarFilhos(atividade, arvore, id, alvo, conta)
          continue
        }
        conta.arquivos++
        if (conta.arquivos > LIMITE_ARQUIVOS) throw IllegalStateException("A pasta tem mais de $LIMITE_ARQUIVOS arquivos.")
        val origem = DocumentsContract.buildDocumentUriUsingTree(arvore, id)
        atividade.contentResolver.openInputStream(origem)?.use { entrada ->
          alvo.outputStream().use { saida ->
            val buffer = ByteArray(64 * 1024)
            while (true) {
              val lidos = entrada.read(buffer)
              if (lidos < 0) break
              conta.bytes += lidos
              if (conta.bytes > LIMITE_BYTES) throw IllegalStateException("A pasta passa de 300 MB.")
              saida.write(buffer, 0, lidos)
            }
          }
        }
      }
    }
  }
}
