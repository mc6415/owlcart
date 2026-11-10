package dev.owlcart.app

import android.app.Activity
import android.content.Intent
import androidx.activity.result.ActivityResult
import app.tauri.annotation.ActivityCallback
import app.tauri.annotation.Command
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin

/**
 * Opens Android's system folder picker and returns the chosen folder's tree URI
 * (e.g. content://com.android.externalstorage.documents/tree/primary%3AROMs),
 * or no URI if it was cancelled. The Rust side (folder_picker.rs) turns it into
 * a normal path, which Owlcart can write to with "All files access".
 */
@TauriPlugin
class FolderPickerPlugin(private val activity: Activity) : Plugin(activity) {
  @Command
  fun pickFolder(invoke: Invoke) {
    val intent = Intent(Intent.ACTION_OPEN_DOCUMENT_TREE)
    startActivityForResult(invoke, intent, "folderPicked")
  }

  @ActivityCallback
  fun folderPicked(invoke: Invoke, result: ActivityResult) {
    val ret = JSObject()
    val uri = result.data?.data
    if (result.resultCode == Activity.RESULT_OK && uri != null) {
      ret.put("uri", uri.toString())
    }
    invoke.resolve(ret)
  }
}
