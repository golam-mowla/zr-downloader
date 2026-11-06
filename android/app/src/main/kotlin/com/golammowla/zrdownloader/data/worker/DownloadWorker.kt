package com.golammowla.zrdownloader.data.worker

import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.Build
import androidx.core.app.NotificationCompat
import androidx.hilt.work.HiltWorker
import androidx.work.CoroutineWorker
import androidx.work.ForegroundInfo
import androidx.work.WorkerParameters
import com.yausername.youtubedl_android.YoutubeDL
import com.yausername.youtubedl_android.YoutubeDLRequest
import com.golammowla.zrdownloader.MainActivity
import com.golammowla.zrdownloader.data.database.dao.DownloadDao
import com.golammowla.zrdownloader.data.database.entity.DownloadEntity
import com.golammowla.zrdownloader.data.network.DownloadEngine
import com.golammowla.zrdownloader.domain.model.DownloadStatus
import dagger.assisted.Assisted
import dagger.assisted.AssistedInject
import kotlinx.coroutines.CancellationException
import timber.log.Timber
import java.io.File

@HiltWorker
class DownloadWorker @AssistedInject constructor(
    @Assisted private val context: Context,
    @Assisted private val params: WorkerParameters,
    private val downloadDao: DownloadDao,
    private val downloadEngine: DownloadEngine
) : CoroutineWorker(context, params) {

    private val notificationManager =
        context.getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager

    override suspend fun doWork(): Result {
        val taskId = inputData.getString(KEY_TASK_ID) ?: return Result.failure()
        val entity = downloadDao.getById(taskId) ?: return Result.failure()
        val domainItem = entity.toDomain()

        createNotificationChannel()
        val notifId = taskId.hashCode()
        setForeground(createForegroundInfo(domainItem.filename, 0, "Starting...", notifId))

        return try {
            downloadDao.update(
                entity.copy(status = DownloadStatus.DOWNLOADING.name, errorMessage = null)
            )

            val targetFile = File(domainItem.savePath, domainItem.filename)
            val parentDir = targetFile.parentFile
            if (parentDir != null && !parentDir.exists()) {
                parentDir.mkdirs()
            }

            if (isMediaUrl(domainItem.url)) {
                val isAudio = domainItem.filename.endsWith(".mp3", ignoreCase = true) ||
                        domainItem.filename.endsWith(".m4a", ignoreCase = true) ||
                        domainItem.filename.contains("[Audio]", ignoreCase = true)

                val request = YoutubeDLRequest(domainItem.url)
                request.addOption("-o", targetFile.absolutePath)
                request.addOption("--no-part")
                request.addOption("--no-playlist")
                request.addOption("-c")

                if (isAudio) {
                    request.addOption("-x")
                    request.addOption("--audio-format", "mp3")
                    request.addOption("-f", "ba/b")
                } else {
                    val fn = domainItem.filename
                    val formatMatch = "\\[(\\d+p?)\\]".toRegex().find(fn)?.groupValues?.get(1)
                    if (formatMatch != null) {
                        val res = formatMatch.replace("p", "")
                        request.addOption("-f", "bestvideo[height<=$res]+bestaudio/best[height<=$res]/best")
                    } else {
                        request.addOption("-f", "bestvideo+bestaudio/best")
                    }
                    request.addOption("--merge-output-format", "mp4")
                }

                YoutubeDL.getInstance().execute(request, taskId) { progress, etaInSeconds, line ->
                    val pct = progress.coerceIn(0f, 100f)
                    val etaStr = if (etaInSeconds > 0) "${etaInSeconds}s" else "--"
                    val speedStr = if (!line.isNullOrBlank()) line.trim() else "Downloading..."

                    val updatedItem = domainItem.copy(
                        percent = pct,
                        speedFormatted = speedStr,
                        etaFormatted = etaStr,
                        status = if (pct >= 100f) DownloadStatus.COMPLETED else DownloadStatus.DOWNLOADING
                    )
                    downloadDao.update(DownloadEntity.fromDomain(updatedItem))

                    if (pct < 100f) {
                        notificationManager.notify(
                            notifId,
                            buildNotification(
                                domainItem.filename,
                                pct.toInt(),
                                "$speedStr • ETA $etaStr"
                            )
                        )
                    }
                }
            } else {
                downloadEngine.download(
                    taskId = taskId,
                    url = domainItem.url,
                    targetFile = targetFile,
                    totalBytes = domainItem.totalBytes,
                    isResumable = domainItem.isResumable,
                    chunkCount = domainItem.chunkCount,
                    existingSegments = domainItem.segments,
                    onProgress = { update ->
                        val updatedItem = domainItem.copy(
                            downloadedBytes = update.downloadedBytes,
                            totalBytes = update.totalBytes,
                            percent = update.percent,
                            speedBps = update.speedBps,
                            speedFormatted = update.speedFormatted,
                            etaFormatted = update.etaFormatted,
                            status = if (update.percent >= 100f) DownloadStatus.COMPLETED else DownloadStatus.DOWNLOADING,
                            segments = update.segments
                        )

                        downloadDao.update(DownloadEntity.fromDomain(updatedItem))

                        if (update.percent < 100f) {
                            notificationManager.notify(
                                notifId,
                                buildNotification(
                                    domainItem.filename,
                                    update.percent.toInt(),
                                    "${update.speedFormatted} • ETA ${update.etaFormatted}"
                                )
                            )
                        }
                    }
                )
            }

            // Mark as completed in database
            val latest = downloadDao.getById(taskId)?.toDomain() ?: domainItem
            downloadDao.update(
                DownloadEntity.fromDomain(
                    latest.copy(
                        status = DownloadStatus.COMPLETED,
                        percent = 100f,
                        speedFormatted = "Done",
                        etaFormatted = "0s"
                    )
                )
            )

            showCompletedNotification(domainItem.filename, notifId)
            Result.success()
        } catch (e: CancellationException) {
            Timber.i("DownloadWorker paused or cancelled: $taskId")
            try {
                YoutubeDL.getInstance().destroyProcessById(taskId)
            } catch (_: Exception) {}
            val current = downloadDao.getById(taskId)?.toDomain() ?: domainItem
            downloadDao.update(
                DownloadEntity.fromDomain(
                    current.copy(status = DownloadStatus.PAUSED, speedFormatted = "Paused", etaFormatted = "--")
                )
            )
            notificationManager.cancel(notifId)
            Result.success()
        } catch (e: Exception) {
            Timber.e(e, "DownloadWorker failed for $taskId")
            try {
                YoutubeDL.getInstance().destroyProcessById(taskId)
            } catch (_: Exception) {}
            val current = downloadDao.getById(taskId)?.toDomain() ?: domainItem
            downloadDao.update(
                DownloadEntity.fromDomain(
                    current.copy(
                        status = DownloadStatus.ERROR,
                        errorMessage = e.localizedMessage ?: "Network error",
                        speedFormatted = "Error",
                        etaFormatted = "--"
                    )
                )
            )
            showErrorNotification(domainItem.filename, e.localizedMessage ?: "Download failed", notifId)
            Result.failure()
        }
    }

    private fun isMediaUrl(url: String): Boolean {
        val lower = url.lowercase()
        return lower.contains("youtube.com") || lower.contains("youtu.be") ||
                lower.contains("googlevideo.com") || lower.contains("instagram.com") ||
                lower.contains("facebook.com") || lower.contains("fb.watch") ||
                lower.contains("tiktok.com") || lower.contains("kick.com") ||
                lower.contains("twitch.tv") || lower.contains("twitter.com") ||
                lower.contains("x.com") || lower.contains("vimeo.com")
    }

    private fun createForegroundInfo(filename: String, progress: Int, statusText: String, notifId: Int): ForegroundInfo {
        val notification = buildNotification(filename, progress, statusText)
        return if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            ForegroundInfo(notifId, notification, ServiceInfo.FOREGROUND_SERVICE_TYPE_DATA_SYNC)
        } else {
            ForegroundInfo(notifId, notification)
        }
    }

    private fun buildNotification(filename: String, progress: Int, statusText: String) =
        NotificationCompat.Builder(context, CHANNEL_ID)
            .setContentTitle(filename)
            .setContentText(statusText)
            .setSmallIcon(android.R.drawable.stat_sys_download)
            .setProgress(100, progress, progress <= 0)
            .setOngoing(true)
            .setOnlyAlertOnce(true)
            .setContentIntent(createPendingIntent())
            .build()

    private fun showCompletedNotification(filename: String, notifId: Int) {
        val notif = NotificationCompat.Builder(context, CHANNEL_ID)
            .setContentTitle("Download Complete")
            .setContentText(filename)
            .setSmallIcon(android.R.drawable.stat_sys_download_done)
            .setAutoCancel(true)
            .setContentIntent(createPendingIntent())
            .build()
        notificationManager.notify(notifId, notif)
    }

    private fun showErrorNotification(filename: String, errorText: String, notifId: Int) {
        val notif = NotificationCompat.Builder(context, CHANNEL_ID)
            .setContentTitle("Download Error")
            .setContentText("$filename: $errorText")
            .setSmallIcon(android.R.drawable.stat_notify_error)
            .setAutoCancel(true)
            .setContentIntent(createPendingIntent())
            .build()
        notificationManager.notify(notifId, notif)
    }

    private fun createPendingIntent(): PendingIntent {
        val intent = Intent(context, MainActivity::class.java).apply {
            flags = Intent.FLAG_ACTIVITY_SINGLE_TOP or Intent.FLAG_ACTIVITY_CLEAR_TOP
        }
        return PendingIntent.getActivity(
            context,
            0,
            intent,
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
        )
    }

    private fun createNotificationChannel() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            val channel = NotificationChannel(
                CHANNEL_ID,
                "ZR Downloader Downloads",
                NotificationManager.IMPORTANCE_LOW
            ).apply {
                description = "Shows real-time progress for active downloads"
            }
            notificationManager.createNotificationChannel(channel)
        }
    }

    companion object {
        const val KEY_TASK_ID = "key_task_id"
        private const val CHANNEL_ID = "zr_downloads_channel"
    }
}

