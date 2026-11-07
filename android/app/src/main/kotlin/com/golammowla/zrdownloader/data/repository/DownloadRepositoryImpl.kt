package com.golammowla.zrdownloader.data.repository

import android.content.Context
import androidx.work.Constraints
import androidx.work.ExistingWorkPolicy
import androidx.work.NetworkType
import androidx.work.OneTimeWorkRequestBuilder
import androidx.work.WorkManager
import androidx.work.workDataOf
import com.golammowla.zrdownloader.data.database.dao.DownloadDao
import com.golammowla.zrdownloader.data.database.entity.DownloadEntity
import com.golammowla.zrdownloader.data.network.DownloadEngine
import com.golammowla.zrdownloader.data.preferences.UserPreferencesRepository
import com.golammowla.zrdownloader.data.worker.DownloadWorker
import com.golammowla.zrdownloader.domain.model.DownloadCategory
import com.golammowla.zrdownloader.domain.model.DownloadItem
import com.golammowla.zrdownloader.domain.model.DownloadStatus
import com.golammowla.zrdownloader.domain.repository.DownloadRepository
import com.yausername.youtubedl_android.YoutubeDL
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.map
import java.io.File
import java.util.UUID
import javax.inject.Inject
import javax.inject.Singleton

@Singleton
class DownloadRepositoryImpl @Inject constructor(
    @ApplicationContext private val context: Context,
    private val downloadDao: DownloadDao,
    private val downloadEngine: DownloadEngine,
    private val preferencesRepository: UserPreferencesRepository
) : DownloadRepository {

    private val workManager by lazy { WorkManager.getInstance(context) }

    private fun isMediaUrl(url: String): Boolean {
        val lower = url.lowercase()
        return lower.contains("youtube.com") || lower.contains("youtu.be") ||
                lower.contains("googlevideo.com") || lower.contains("instagram.com") ||
                lower.contains("facebook.com") || lower.contains("fb.watch") ||
                lower.contains("tiktok.com") || lower.contains("kick.com") ||
                lower.contains("twitch.tv") || lower.contains("twitter.com") ||
                lower.contains("x.com") || lower.contains("vimeo.com")
    }

    override fun getAllDownloads(): Flow<List<DownloadItem>> {
        return downloadDao.getAllFlow().map { list -> list.map { it.toDomain() } }
    }

    override fun getDownloadsByCategory(category: DownloadCategory): Flow<List<DownloadItem>> {
        return if (category == DownloadCategory.ALL) {
            getAllDownloads()
        } else {
            downloadDao.getByCategoryFlow(category.name).map { list -> list.map { it.toDomain() } }
        }
    }

    override fun getDownloadById(id: String): Flow<DownloadItem?> {
        return downloadDao.getByIdFlow(id).map { it?.toDomain() }
    }

    override suspend fun getDownload(id: String): DownloadItem? {
        return downloadDao.getById(id)?.toDomain()
    }

    override suspend fun startDownload(
        url: String,
        filename: String?,
        savePath: String?,
        chunkCount: Int
    ): String {
        val taskId = "zr_${System.currentTimeMillis()}_${UUID.randomUUID().toString().take(6)}"
        val prefs = preferencesRepository.preferencesFlow.first()
        val isMedia = isMediaUrl(url)

        val finalFilename = if (!filename.isNullOrBlank()) {
            filename
        } else if (isMedia) {
            "Media_${System.currentTimeMillis()}.mp4"
        } else {
            downloadEngine.probeUrl(url).suggestedFilename
        }

        val finalSavePath = savePath?.takeIf { it.isNotBlank() } ?: prefs.downloadPath
        val finalChunks = if (chunkCount in 1..32) chunkCount else prefs.defaultConnections

        val isAudio = finalFilename.endsWith(".mp3", ignoreCase = true) || finalFilename.endsWith(".m4a", ignoreCase = true)
        val category = when {
            isAudio -> DownloadCategory.AUDIO
            isMedia -> DownloadCategory.VIDEOS
            else -> downloadEngine.probeUrl(url).category
        }

        val totalBytes = if (isMedia) -1L else downloadEngine.probeUrl(url).totalBytes

        val item = DownloadItem(
            id = taskId,
            url = url,
            filename = finalFilename,
            savePath = finalSavePath,
            totalBytes = totalBytes,
            downloadedBytes = 0L,
            percent = 0f,
            status = DownloadStatus.DOWNLOADING,
            category = category,
            chunkCount = finalChunks,
            isResumable = true,
            createdAt = System.currentTimeMillis()
        )

        downloadDao.insert(DownloadEntity.fromDomain(item))
        enqueueWorker(taskId, prefs.wifiOnly)

        return taskId
    }

    override suspend fun pauseDownload(id: String) {
        try {
            YoutubeDL.getInstance().destroyProcessById(id)
        } catch (_: Exception) {}
        downloadEngine.cancelTask(id)
        workManager.cancelUniqueWork(id)
        val current = downloadDao.getById(id)?.toDomain()
        if (current != null) {
            downloadDao.update(
                DownloadEntity.fromDomain(
                    current.copy(status = DownloadStatus.PAUSED, speedFormatted = "Paused", etaFormatted = "--")
                )
            )
        }
    }

    override suspend fun resumeDownload(id: String) {
        val item = downloadDao.getById(id)?.toDomain() ?: return
        val prefs = preferencesRepository.preferencesFlow.first()

        downloadDao.update(
            DownloadEntity.fromDomain(
                item.copy(status = DownloadStatus.DOWNLOADING, speedFormatted = "Resuming...", etaFormatted = "--")
            )
        )
        enqueueWorker(id, prefs.wifiOnly)
    }

    override suspend fun deleteDownload(id: String, deleteFile: Boolean) {
        pauseDownload(id)
        val item = downloadDao.getById(id)?.toDomain()
        downloadDao.deleteById(id)

        if (deleteFile && item != null) {
            try {
                val file = File(item.savePath, item.filename)
                if (file.exists()) file.delete()
                val partDir = File(item.savePath, ".zr_tmp_$id")
                if (partDir.exists()) partDir.deleteRecursively()
            } catch (e: Exception) {
                // Ignore file deletion errors
            }
        }
    }

    override suspend fun cancelDownload(id: String) {
        pauseDownload(id)
        val current = downloadDao.getById(id)?.toDomain()
        if (current != null) {
            downloadDao.update(
                DownloadEntity.fromDomain(
                    current.copy(status = DownloadStatus.PAUSED, speedFormatted = "Cancelled", etaFormatted = "--")
                )
            )
        }
    }

    override suspend fun retryDownload(id: String) {
        resumeDownload(id)
    }

    override suspend fun updateDownload(item: DownloadItem) {
        downloadDao.update(DownloadEntity.fromDomain(item))
    }

    private fun enqueueWorker(taskId: String, wifiOnly: Boolean) {
        val constraints = Constraints.Builder()
            .setRequiredNetworkType(if (wifiOnly) NetworkType.UNMETERED else NetworkType.CONNECTED)
            .build()

        val request = OneTimeWorkRequestBuilder<DownloadWorker>()
            .setConstraints(constraints)
            .setInputData(workDataOf(DownloadWorker.KEY_TASK_ID to taskId))
            .build()

        workManager.enqueueUniqueWork(taskId, ExistingWorkPolicy.REPLACE, request)
    }
}
