package com.golammowla.zrdownloader.data.preferences

import android.content.Context
import android.os.Environment
import androidx.datastore.core.DataStore
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.booleanPreferencesKey
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.intPreferencesKey
import androidx.datastore.preferences.core.stringPreferencesKey
import androidx.datastore.preferences.preferencesDataStore
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.map
import javax.inject.Inject
import javax.inject.Singleton

val Context.dataStore: DataStore<Preferences> by preferencesDataStore(name = "zr_settings")

data class UserPreferences(
    val downloadPath: String,
    val defaultConnections: Int,
    val isDarkTheme: Boolean,
    val wifiOnly: Boolean,
    val autoStart: Boolean,
    val ytdlpVersion: String = "2025.02.19",
    val ffmpegVersion: String = "7.1"
)

@Singleton
class UserPreferencesRepository @Inject constructor(
    @ApplicationContext private val context: Context
) {
    private object Keys {
        val DOWNLOAD_PATH = stringPreferencesKey("download_path")
        val DEFAULT_CONNECTIONS = intPreferencesKey("default_connections")
        val DARK_THEME = booleanPreferencesKey("dark_theme")
        val WIFI_ONLY = booleanPreferencesKey("wifi_only")
        val AUTO_START = booleanPreferencesKey("auto_start")
        val YTDLP_VERSION = stringPreferencesKey("ytdlp_version")
        val FFMPEG_VERSION = stringPreferencesKey("ffmpeg_version")
    }

    val preferencesFlow: Flow<UserPreferences> = context.dataStore.data.map { prefs ->
        val defaultPath = Environment.getExternalStoragePublicDirectory(Environment.DIRECTORY_DOWNLOADS)?.absolutePath
            ?: context.filesDir.absolutePath

        UserPreferences(
            downloadPath = prefs[Keys.DOWNLOAD_PATH] ?: defaultPath,
            defaultConnections = prefs[Keys.DEFAULT_CONNECTIONS] ?: 8,
            isDarkTheme = prefs[Keys.DARK_THEME] ?: true,
            wifiOnly = prefs[Keys.WIFI_ONLY] ?: false,
            autoStart = prefs[Keys.AUTO_START] ?: true,
            ytdlpVersion = prefs[Keys.YTDLP_VERSION] ?: "2025.02.19",
            ffmpegVersion = prefs[Keys.FFMPEG_VERSION] ?: "7.1"
        )
    }

    suspend fun updateDownloadPath(path: String) {
        context.dataStore.edit { it[Keys.DOWNLOAD_PATH] = path }
    }

    suspend fun updateDefaultConnections(count: Int) {
        context.dataStore.edit { it[Keys.DEFAULT_CONNECTIONS] = count.coerceIn(1, 32) }
    }

    suspend fun updateDarkTheme(enabled: Boolean) {
        context.dataStore.edit { it[Keys.DARK_THEME] = enabled }
    }

    suspend fun updateWifiOnly(enabled: Boolean) {
        context.dataStore.edit { it[Keys.WIFI_ONLY] = enabled }
    }

    suspend fun updateAutoStart(enabled: Boolean) {
        context.dataStore.edit { it[Keys.AUTO_START] = enabled }
    }

    suspend fun updateYtdlpVersion(version: String) {
        context.dataStore.edit { it[Keys.YTDLP_VERSION] = version }
    }

    suspend fun updateFfmpegVersion(version: String) {
        context.dataStore.edit { it[Keys.FFMPEG_VERSION] = version }
    }
}

