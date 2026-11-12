package dev.owlcart.app

import android.annotation.SuppressLint
import android.app.Activity
import android.app.Presentation
import android.content.Context
import android.hardware.display.DisplayManager
import android.os.Bundle
import android.util.Log
import android.view.Display
import android.webkit.JavascriptInterface
import android.webkit.WebView
import android.webkit.WebViewClient
import androidx.appcompat.app.AppCompatActivity
import app.tauri.annotation.Command
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import org.json.JSONObject

/**
 * Dual-screen devices (like the AYN Thor): shows the highlighted game on the
 * second screen while Owlcart is open on the main one.
 *
 * The page is assets/second-screen.html. The app sends it state through the
 * `update` command; taps on it come back to the app's page by calling
 * window.__owlSecondScreen(message) there.
 */
@TauriPlugin
class SecondScreenPlugin(private val activity: Activity) : Plugin(activity) {
  private var presentation: Presentation? = null
  private var screenWeb: WebView? = null
  private var pageReady = false
  private var lastState: String? = null
  private var mainWeb: WebView? = null
  private var inForeground = true

  override fun load(webView: WebView) {
    mainWeb = webView
  }

  /** A screen to show the game on: never the one the app itself is on. */
  @Suppress("DEPRECATION")
  private fun secondDisplay(): Display? {
    val dm = activity.getSystemService(Context.DISPLAY_SERVICE) as DisplayManager
    val here =
      if (android.os.Build.VERSION.SDK_INT >= android.os.Build.VERSION_CODES.R) activity.display?.displayId
      else activity.windowManager.defaultDisplay.displayId
    return dm.getDisplays(DisplayManager.DISPLAY_CATEGORY_PRESENTATION).firstOrNull { it.displayId != here }
  }

  @Command
  fun available(invoke: Invoke) {
    val ret = JSObject()
    ret.put("available", secondDisplay() != null)
    invoke.resolve(ret)
  }

  @Command
  fun update(invoke: Invoke) {
    val state = invoke.getArgs().toString()
    activity.runOnUiThread {
      lastState = state
      if (inForeground) ensureShown()
      push()
    }
    invoke.resolve()
  }

  private fun push() {
    val state = lastState ?: return
    if (pageReady) screenWeb?.evaluateJavascript("window.render($state)", null)
  }

  @SuppressLint("SetJavaScriptEnabled")
  private fun ensureShown() {
    presentation?.let {
      if (!it.isShowing) it.show()
      return
    }
    val display = secondDisplay() ?: return
    try {
      val p = object : Presentation(activity, display) {
        override fun onCreate(savedInstanceState: Bundle?) {
          super.onCreate(savedInstanceState)
          val web = WebView(context)
          web.settings.javaScriptEnabled = true
          if (android.os.Build.VERSION.SDK_INT >= android.os.Build.VERSION_CODES.O) {
            web.defaultFocusHighlightEnabled = false
          }
          web.addJavascriptInterface(Bridge(), "OwlBridge")
          web.webViewClient = object : WebViewClient() {
            override fun onPageFinished(view: WebView, url: String) {
              pageReady = true
              push()
            }
          }
          web.loadUrl("file:///android_asset/second-screen.html")
          screenWeb = web
          setContentView(web)
        }
      }
      p.setOnDismissListener {
        presentation = null
        screenWeb = null
        pageReady = false
      }
      p.show()
      presentation = p
    } catch (e: Exception) {
      Log.e("OwlcartScreen", "couldn't show the second screen", e)
      presentation = null
    }
  }

  /** Taps on the second screen, passed to the app's page. */
  inner class Bridge {
    @JavascriptInterface
    fun send(message: String) {
      // Re-serialise, so only well-formed JSON ever reaches the app's page.
      val json = try {
        JSONObject(message).toString()
      } catch (e: Exception) {
        return
      }
      mainWeb?.post {
        mainWeb?.evaluateJavascript("window.__owlSecondScreen && window.__owlSecondScreen($json)", null)
      }
    }
  }

  // Only show the second screen while Owlcart is the app in front.
  override fun onStop(activity: AppCompatActivity) {
    inForeground = false
    presentation?.hide()
  }

  override fun onResume(activity: AppCompatActivity) {
    inForeground = true
    if (lastState != null) ensureShown()
  }

  override fun onDestroy(activity: AppCompatActivity) {
    presentation?.dismiss()
  }
}
