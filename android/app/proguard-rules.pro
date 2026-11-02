# Proguard rules for GDM Native Android
-keepattributes *Annotation*
-keepclassmembers class * {
    @androidx.room.* <methods>;
}
-keep class * extends androidx.room.RoomDatabase
-dontwarn javax.annotation.**
-keepclassmembers enum * { *; }
