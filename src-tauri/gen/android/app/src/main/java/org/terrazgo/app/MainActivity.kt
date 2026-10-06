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
    // The SOFT KEYBOARD is padded for here too, and it has to be: with
    // targetSdk 35+ the window is edge-to-edge, `adjustResize` is ignored, and
    // this listener is the only thing that can shrink the webview. Without the
    // ime() inset the keyboard simply drew OVER the page and the webview was
    // never told — measured on a phone (2026-09-15, Galaxy A22, Android 13,
    // WebView 151): with the keyboard up, `innerHeight`, `visualViewport.height`
    // and a probe element's `100dvh` all still read 818, unchanged, so nothing
    // page-side could have responded to it. What it covered was a form panel's
    // pinned Save bar.
    //
    // The larger of the two rather than their sum: the keyboard sits ON the
    // navigation bar, so adding both would leave a gap the height of the nav bar
    // above the keys.
    ViewCompat.setOnApplyWindowInsetsListener(content) { view, insets ->
      val bars =
        insets.getInsets(
          WindowInsetsCompat.Type.systemBars() or WindowInsetsCompat.Type.displayCutout()
        )
      val ime = insets.getInsets(WindowInsetsCompat.Type.ime())
      view.setPadding(bars.left, bars.top, bars.right, maxOf(bars.bottom, ime.bottom))
      WindowInsetsCompat.CONSUMED
    }
  }
}
