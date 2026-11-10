# Add project specific ProGuard rules here.
# You can control the set of applied configuration files using the
# proguardFiles setting in build.gradle.
#
# For more details, see
#   http://developer.android.com/guide/developing/tools/proguard.html

# If your project uses WebView with JS, uncomment the following
# and specify the fully qualified class name to the JavaScript interface
# class:
#-keepclassmembers class fqcn.of.javascript.interface.for.webview {
#   public *;
#}

# Uncomment this to preserve the line number information for
# debugging stack traces.
#-keepattributes SourceFile,LineNumberTable

# If you keep the line number information, uncomment this to
# hide the original source file name.
#-renamesourcefileattribute SourceFile
# Owlcart: Tauri finds the app's plugins (folder picker, second screen) and
# their commands by reflection, and the second screen's page calls the
# @JavascriptInterface bridge, so release shrinking must leave them alone.
-keep class dev.owlcart.app.** { *; }
-keepclassmembers class * {
    @android.webkit.JavascriptInterface <methods>;
}
