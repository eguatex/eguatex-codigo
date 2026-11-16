package br.ufpa.eguatex

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.Build
import android.os.IBinder

/**
 * Mantem o EguaTex acordado enquanto a pessoa usa o Google no navegador (login e seletor do Drive).
 *
 * O login devolve o codigo a um servidor local do proprio app (http://127.0.0.1:porta). Com o
 * navegador na frente, o Android congela o processo do app que ficou em segundo plano, e a pagina
 * do Google fica carregando sem fim. Um servico em primeiro plano (com notificacao) tira o app da
 * lista de congelados ate o login terminar. O Rust chama iniciar() antes de abrir o navegador e
 * parar() quando o codigo chega, quando da erro ou quando o tempo acaba.
 */
class LoginGoogleServico : Service() {
  companion object {
    private const val CANAL = "eguatex-login"
    private const val ID = 4107

    @JvmStatic
    fun iniciar(contexto: Context) {
      val intencao = Intent(contexto, LoginGoogleServico::class.java)
      if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) contexto.startForegroundService(intencao)
      else contexto.startService(intencao)
    }

    @JvmStatic
    fun parar(contexto: Context) {
      contexto.stopService(Intent(contexto, LoginGoogleServico::class.java))
    }
  }

  override fun onBind(intent: Intent?): IBinder? = null

  override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
    val gerente = getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
      gerente.createNotificationChannel(
        NotificationChannel(CANAL, "Google no navegador", NotificationManager.IMPORTANCE_LOW)
      )
    }
    val construtor = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) Notification.Builder(this, CANAL)
    else @Suppress("DEPRECATION") Notification.Builder(this)
    val aviso = construtor
      .setContentTitle("Aguardando o Google no navegador")
      .setContentText("Quando terminar no navegador, volte ao ÉguaTex.")
      .setSmallIcon(applicationInfo.icon)
      .setOngoing(true)
      .build()
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
      // dataSync nao tem o teto de 3 minutos do shortService (escolher no Drive e baixar pode passar
      // disso); se o sistema recusar, fica o shortService, que ja funcionava no login
      try {
        startForeground(ID, aviso, ServiceInfo.FOREGROUND_SERVICE_TYPE_DATA_SYNC)
      } catch (e: Exception) {
        startForeground(ID, aviso, ServiceInfo.FOREGROUND_SERVICE_TYPE_SHORT_SERVICE)
      }
    } else {
      startForeground(ID, aviso)
    }
    return START_NOT_STICKY
  }

  // o sistema avisa quando o tempo do servico acaba (3 minutos no shortService, 6 horas no dataSync)
  override fun onTimeout(startId: Int) {
    stopSelf()
  }

  override fun onTimeout(startId: Int, fgsType: Int) {
    stopSelf()
  }
}
