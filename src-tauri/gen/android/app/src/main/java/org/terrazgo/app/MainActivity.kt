package org.terrazgo.app

import android.graphics.Color
import android.os.Bundle
import android.view.View
import androidx.activity.enableEdgeToEdge
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat

class MainActivity : TauriActivity() {
  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
    // Android 15+ (targetSdk 35+) forces edge-to-edge: the webview would
    // render under the status bar and behind the gesture area. Pad the
    // content view by the system-bar/cutout insets so the app lives strictly
    // between them. The revealed strips show this background color — keep it
    // matched to the app chrome (--panel in src/styles.css).
    val content = findViewById<View>(android.R.id.content)
    content.setBackgroundColor(Color.parseColor("#EDF3EA"))
    ViewCompat.setOnApplyWindowInsetsListener(content) { view, insets ->
      val bars =
        insets.getInsets(
          WindowInsetsCompat.Type.systemBars() or WindowInsetsCompat.Type.displayCutout()
        )
      view.setPadding(bars.left, bars.top, bars.right, bars.bottom)
      WindowInsetsCompat.CONSUMED
    }
  }
}
