# JNA and the generated UniFFI bindings are reached reflectively.
-keep class com.sun.jna.** { *; }
-keep class * implements com.sun.jna.** { *; }
-keep class io.github.danperks.normsdk.** { *; }
-dontwarn java.awt.**
