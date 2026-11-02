package com.golammowla.gdm.data.worker

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
import com.golammowla.gdm.MainActivity
import com.golammowla.gdm.data.database.dao.DownloadDao
import com.golammowla.gdm.data.database.entity.DownloadEntity
import com.golammowla.gdm.data.network.DownloadEngine
import com.golammowla.gdm.domain.model.DownloadStatus
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
                "GDM Downloads",
                NotificationManager.IMPORTANCE_LOW
            ).apply {
                description = "Shows real-time progress for active downloads"
            }
            notificationManager.createNotificationChannel(channel)
        }
    }

    companion object {
        const val KEY_TASK_ID = "key_task_id"
        private const val CHANNEL_ID = "gdm_downloads_channel"
    }
}
