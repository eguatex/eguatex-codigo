package br.ufpa.eguatex

import android.content.Intent
import android.content.res.Configuration
import android.graphics.Color
import android.os.Build
import android.os.Bundle
import android.view.Display
import android.view.WindowManager
import android.view.View
import android.webkit.WebView
import androidx.activity.enableEdgeToEdge
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat

class MainActivity : TauriActivity() {
  companion object {
    // EscolherPasta abre o seletor de pastas a partir desta atividade
    @Volatile @JvmStatic var instancia: MainActivity? = null

    /** Largura minima da pagina (px CSS) no modo mesa: um monitor de 1920 px a 125%, como no Windows. */
    const val LARGURA_MESA = 1536

    /** Escala do WebView no modo mesa, em porcento de pixel fisico por px CSS. 0 mantem a escala
     *  padrao: vale quando a densidade da tela ja deixa a pagina com LARGURA_MESA px ou mais. */
    @JvmStatic
    fun porcentoDeMesa(larguraPx: Int, densidade: Float): Int {
      if (larguraPx <= 0 || densidade <= 0f) return 0
      val escala = maxOf(1f, larguraPx.toFloat() / LARGURA_MESA)
      return if (escala >= densidade) 0 else Math.round(escala * 100)
    }
  }

  @Deprecated("startActivityForResult continua o caminho mais simples para o JNI")
  override fun onActivityResult(requestCode: Int, resultCode: Int, data: Intent?) {
    @Suppress("DEPRECATION")
    super.onActivityResult(requestCode, resultCode, data)
    if (requestCode == EscolherPasta.PEDIDO) EscolherPasta.aoVoltar(this, resultCode, data)
  }

  override fun onDestroy() {
    if (instancia === this) instancia = null
    super.onDestroy()
  }

  // Celular (menor lado da tela abaixo de 600 dp): esconde a barra de notificacoes para
  // ganhar altura no modo deitado. Um deslize na borda de cima mostra a barra por instantes.
  // Tablets mantem a barra.
  private var escondeBarraDeStatus = false

  private var webView: WebView? = null

  override fun onWebViewCreate(webView: WebView) {
    this.webView = webView
    ajustarEscalaDeMesa()
  }

  private fun telaAtual(): Display? =
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) display else @Suppress("DEPRECATION") windowManager.defaultDisplay

  // Modo mesa (Samsung DeX, monitor ligado ao celular). Tres sinais, porque cada versao do DeX usa um:
  // janela em outra tela, tipo de interface "mesa" ou o campo semDesktopModeEnabled da Samsung.
  private fun emModoMesa(): Boolean {
    if (telaAtual()?.displayId.let { it != null && it != Display.DEFAULT_DISPLAY }) return true
    val cfg = resources.configuration
    if ((cfg.uiMode and Configuration.UI_MODE_TYPE_MASK) == Configuration.UI_MODE_TYPE_DESK) return true
    return try {
      Configuration::class.java.getField("semDesktopModeEnabled").getInt(cfg) == 1
    } catch (e: Throwable) {
      false
    }
  }

  // Largura real da janela: no DeX ela pode ser menor que o monitor.
  private fun larguraDaJanelaPx(): Int =
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) windowManager.currentWindowMetrics.bounds.width()
    else resources.displayMetrics.widthPixels

  // No DeX o monitor herda a densidade do celular (S25 FE: 2,8). Um monitor de 1920 px ficava com
  // 683 px de pagina e o app entrava no layout de celular deitado, com tudo gigante. Aqui a escala
  // do WebView cai ate a janela valer LARGURA_MESA px de pagina: o layout de computador, com menus,
  // arvore de arquivos, editor e PDF lado a lado. Na tela do aparelho a escala volta ao padrao.
  private fun ajustarEscalaDeMesa() {
    val wv = webView ?: return
    var porcento = 0
    if (emModoMesa()) {
      porcento = porcentoDeMesa(larguraDaJanelaPx(), resources.configuration.densityDpi / 160f)
    }
    if (porcento == escalaAplicada) return
    escalaAplicada = porcento
    wv.setInitialScale(porcento)
  }

  private var escalaAplicada = 0

  private fun esconderBarraDeStatus() {
    // no modo mesa a janela fica num monitor: a barra do sistema continua, como no tablet
    escondeBarraDeStatus = !emModoMesa() && resources.configuration.smallestScreenWidthDp < 600
    val controle0 = WindowInsetsControllerCompat(window, window.decorView)
    // em pe a barra de notificacoes fica, como no Overleaf; deitado ela some para sobrar altura
    if (!escondeBarraDeStatus || resources.configuration.orientation != Configuration.ORIENTATION_LANDSCAPE) {
      controle0.show(WindowInsetsCompat.Type.statusBars())
      return
    }
    val controle = WindowInsetsControllerCompat(window, window.decorView)
    controle.systemBarsBehavior = WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
    controle.hide(WindowInsetsCompat.Type.statusBars())
  }

  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
    instancia = this

    esconderBarraDeStatus()

    // Afasta o app da barra de notificacao, da barra de navegacao e do teclado.
    // Sem isto, no Android 15 o conteudo fica por baixo da barra de notificacao.
    val raiz = findViewById<View>(android.R.id.content)
    // Em paisagem o recorte da camera fica na lateral. A janela passa a ocupar essa faixa e ela
    // recebe a cor das barras do app (superficie-0), em vez do fundo claro do tema. O padding
    // abaixo continua afastando botoes e texto do recorte.
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
      window.attributes = window.attributes.apply {
        layoutInDisplayCutoutMode = WindowManager.LayoutParams.LAYOUT_IN_DISPLAY_CUTOUT_MODE_SHORT_EDGES
      }
    }
    val escuro = (resources.configuration.uiMode and Configuration.UI_MODE_NIGHT_MASK) == Configuration.UI_MODE_NIGHT_YES
    window.decorView.setBackgroundColor(if (escuro) Color.parseColor("#1B222C") else Color.WHITE)
    ViewCompat.setOnApplyWindowInsetsListener(raiz) { view, insets ->
      val barras = insets.getInsets(
        WindowInsetsCompat.Type.systemBars() or
          WindowInsetsCompat.Type.displayCutout() or
          WindowInsetsCompat.Type.ime()
      )
      // Com a barra de status escondida, o topo nao reserva espaco nenhum (nem para ela, nem para
      // o recorte da tela): o celular fica deitado e a camera fica ao lado, nao em cima.
      val deitado = resources.configuration.orientation == Configuration.ORIENTATION_LANDSCAPE
      val topo = if (escondeBarraDeStatus && deitado) 0 else barras.top
      view.setPadding(barras.left, topo, barras.right, barras.bottom)
      WindowInsetsCompat.CONSUMED
    }
  }

  // O sistema mostra a barra de novo depois de teclado, dialogos e troca de app.
  override fun onResume() {
    super.onResume()
    ajustarEscalaDeMesa()
    esconderBarraDeStatus()
  }

  override fun onConfigurationChanged(newConfig: Configuration) {
    super.onConfigurationChanged(newConfig)
    ajustarEscalaDeMesa()
    esconderBarraDeStatus()
    window.decorView.requestApplyInsets()
  }

  override fun onWindowFocusChanged(hasFocus: Boolean) {
    super.onWindowFocusChanged(hasFocus)
    if (hasFocus) esconderBarraDeStatus()
  }
}
