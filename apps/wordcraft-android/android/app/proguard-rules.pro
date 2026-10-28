# WordCraft Android ProGuard / R8 Rules

# Keep native JNI bindings
-keepclasseswithmembernames class * {
    native <methods>;
}

# Keep JNI Bridge and Data Models
-keep class ai.storyteller.wordcraft.WordcraftBridge { *; }
-keep class ai.storyteller.wordcraft.EditorUiState { *; }
-keep class ai.storyteller.wordcraft.WordcraftCanvas { *; }
-keep class ai.storyteller.wordcraft.WordcraftInputConnection { *; }
