package dev.owlcart.app

import android.content.Intent
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.os.Environment
import android.provider.Settings
import android.util.Log
import android.view.KeyEvent
import android.view.View
import android.view.ViewGroup
import android.webkit.WebView
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat

class MainActivity : TauriActivity() {
  override fun onCreate(savedInstanceState: Bundle?) {
    super.onCreate(savedInstanceState)
    hideSystemBars()
    askForAllFilesAccess()
  }

  // Bars can come back after the folder picker or a settings screen, so hide
  // them again whenever the app returns to the front.
  override fun onWindowFocusChanged(hasFocus: Boolean) {
    super.onWindowFocusChanged(hasFocus)
    if (hasFocus) {
      hideSystemBars()
      webView()
    }
  }

  /**
   * Controller buttons beyond the D-pad, A and B (which Android already turns
   * into arrows, Enter and Back) go to the page as window.__owlPad("x") etc.
   */
  override fun dispatchKeyEvent(event: KeyEvent): Boolean {
    val name = when (event.keyCode) {
      KeyEvent.KEYCODE_BUTTON_X -> "x"
      KeyEvent.KEYCODE_BUTTON_Y -> "y"
      KeyEvent.KEYCODE_BUTTON_L1 -> "l1"
      KeyEvent.KEYCODE_BUTTON_R1 -> "r1"
      KeyEvent.KEYCODE_BUTTON_L2 -> "l2"
      KeyEvent.KEYCODE_BUTTON_R2 -> "r2"
      KeyEvent.KEYCODE_BUTTON_START -> "start"
      KeyEvent.KEYCODE_BUTTON_SELECT -> "select"
      KeyEvent.KEYCODE_BUTTON_THUMBL -> "l3"
      KeyEvent.KEYCODE_BUTTON_THUMBR -> "r3"
      else -> null
    }
    if (name == null) {
      if (KeyEvent.isGamepadButton(event.keyCode) && event.action == KeyEvent.ACTION_DOWN) {
        Log.d("OwlcartPad", "unmapped button ${KeyEvent.keyCodeToString(event.keyCode)}")
      }
      return super.dispatchKeyEvent(event)
    }
    // Held L1/R1 keep paging; the others act once per press.
    val repeats = name == "l1" || name == "r1"
    if (event.action == KeyEvent.ACTION_DOWN && (event.repeatCount == 0 || repeats)) {
      webView()?.evaluateJavascript("window.__owlPad && window.__owlPad('$name')", null)
    }
    return true
  }

  private var web: WebView? = null
  private fun webView(): WebView? {
    web?.let { return it }
    fun find(v: View): WebView? {
      if (v is WebView) return v
      if (v is ViewGroup) for (i in 0 until v.childCount) find(v.getChildAt(i))?.let { return it }
      return null
    }
    web = find(window.decorView)
    // Using the D-pad gives the web view focus, and Android then tints the
    // whole view to show it. The page shows its own highlight, so turn that off.
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) web?.defaultFocusHighlightEnabled = false
    return web
  }

  /**
   * Full screen: no status or navigation bar. A swipe from the edge shows them
   * briefly over the app, and the back gesture still works.
   */
  private fun hideSystemBars() {
    val controller = WindowCompat.getInsetsController(window, window.decorView)
    controller.systemBarsBehavior = WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
    controller.hide(WindowInsetsCompat.Type.systemBars())
  }

  /**
   * Downloads go to a folder on the main storage (ROMs by default), which on
   * Android 11+ needs "All files access". If it isn't granted yet, open the
   * system screen where it can be switched on.
   */
  private fun askForAllFilesAccess() {
    if (Build.VERSION.SDK_INT < Build.VERSION_CODES.R) return
    if (Environment.isExternalStorageManager()) return
    try {
      startActivity(
        Intent(Settings.ACTION_MANAGE_APP_ALL_FILES_ACCESS_PERMISSION, Uri.parse("package:$packageName"))
      )
    } catch (e: Exception) {
      startActivity(Intent(Settings.ACTION_MANAGE_ALL_FILES_ACCESS_PERMISSION))
    }
  }
}
