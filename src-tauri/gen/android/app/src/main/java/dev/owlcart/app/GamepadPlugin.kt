package dev.owlcart.app

import android.app.Activity
import android.content.Context
import android.hardware.input.InputManager
import android.os.Build
import android.os.VibrationEffect
import android.os.Vibrator
import android.os.VibratorManager
import android.view.InputDevice
import android.webkit.WebView
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin

@InvokeArg
class RumbleArgs {
  var ms: Long = 20
  var strength: Int = 128
}

/**
 * Whether a controller is connected (built in, like the AYN Thor's, or paired),
 * so the app can show controller buttons where keyboard hints would be.
 * The page asks with `connected`, and hears about changes through
 * window.__owlPadConnected(true/false). `rumble` runs the device's vibrator.
 */
@TauriPlugin
class GamepadPlugin(private val activity: Activity) : Plugin(activity) {
  private var mainWeb: WebView? = null
  private var last: Boolean? = null

  override fun load(webView: WebView) {
    mainWeb = webView
    val im = activity.getSystemService(Context.INPUT_SERVICE) as InputManager
    im.registerInputDeviceListener(
      object : InputManager.InputDeviceListener {
        override fun onInputDeviceAdded(id: Int) = changed()
        override fun onInputDeviceRemoved(id: Int) = changed()
        override fun onInputDeviceChanged(id: Int) = changed()
      },
      null,
    )
  }

  private fun hasGamepad(): Boolean =
    InputDevice.getDeviceIds().any { id ->
      val d = InputDevice.getDevice(id) ?: return@any false
      !d.isVirtual &&
        (d.sources and InputDevice.SOURCE_GAMEPAD == InputDevice.SOURCE_GAMEPAD ||
          d.sources and InputDevice.SOURCE_JOYSTICK == InputDevice.SOURCE_JOYSTICK)
    }

  private fun changed() {
    val now = hasGamepad()
    if (now == last) return
    last = now
    mainWeb?.post {
      mainWeb?.evaluateJavascript("window.__owlPadConnected && window.__owlPadConnected($now)", null)
    }
  }

  @Command
  fun rumble(invoke: Invoke) {
    val args = invoke.parseArgs(RumbleArgs::class.java)
    @Suppress("DEPRECATION")
    val v: Vibrator? =
      if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
        (activity.getSystemService(Context.VIBRATOR_MANAGER_SERVICE) as VibratorManager).defaultVibrator
      } else {
        activity.getSystemService(Context.VIBRATOR_SERVICE) as Vibrator?
      }
    if (v != null && v.hasVibrator()) {
      if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
        val amp = if (v.hasAmplitudeControl()) args.strength.coerceIn(1, 255) else VibrationEffect.DEFAULT_AMPLITUDE
        v.vibrate(VibrationEffect.createOneShot(args.ms, amp))
      } else {
        @Suppress("DEPRECATION")
        v.vibrate(args.ms)
      }
    }
    invoke.resolve()
  }

  @Command
  fun connected(invoke: Invoke) {
    val now = hasGamepad()
    last = now
    val ret = JSObject()
    ret.put("connected", now)
    invoke.resolve(ret)
  }
}
