package com.golammowla.zrdownloader

import android.app.Application
import androidx.hilt.work.HiltWorkerFactory
import androidx.work.Configuration
import androidx.work.WorkManager
import com.yausername.ffmpeg.FFmpeg
import com.yausername.youtubedl_android.YoutubeDL
import dagger.hilt.android.HiltAndroidApp
import timber.log.Timber
import javax.inject.Inject

@HiltAndroidApp
class ZrApplication : Application(), Configuration.Provider {

    @Inject
    lateinit var workerFactory: HiltWorkerFactory

    override val workManagerConfiguration: Configuration
        get() {
            val builder = Configuration.Builder()
            if (::workerFactory.isInitialized) {
                builder.setWorkerFactory(workerFactory)
            }
            return builder.build()
        }

    override fun onCreate() {
        super.onCreate()
        Timber.plant(Timber.DebugTree())
        Timber.i("ZR Downloader Native Android Initialized")
        try {
            WorkManager.initialize(this, workManagerConfiguration)
        } catch (e: Exception) {
            Timber.w(e, "WorkManager already initialized or skipped")
        }

        try {
            YoutubeDL.getInstance().init(this)
            FFmpeg.getInstance().init(this)
            Timber.i("yt-dlp and FFmpeg native engines initialized successfully")
        } catch (e: Throwable) {
            Timber.e(e, "Failed to initialize native yt-dlp/FFmpeg engine")
        }
    }
}
