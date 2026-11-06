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

        val probe = downloadEngine.probeUrl(url)
        val finalFilename = filename?.takeIf { it.isNotBlank() } ?: probe.suggestedFilename
        val finalSavePath = savePath?.takeIf { it.isNotBlank() } ?: prefs.downloadPath
        val finalChunks = if (chunkCount in 1..32) chunkCount else prefs.defaultConnections

        val item = DownloadItem(
            id = taskId,
            url = url,
            filename = finalFilename,
            savePath = finalSavePath,
            totalBytes = probe.totalBytes,
            downloadedBytes = 0L,
            percent = 0f,
            status = DownloadStatus.DOWNLOADING,
            category = probe.category,
            chunkCount = finalChunks,
            isResumable = probe.isResumable,
            createdAt = System.currentTimeMillis()
        )

        downloadDao.insert(DownloadEntity.fromDomain(item))
        enqueueWorker(taskId, prefs.wifiOnly)

        return taskId
    }

    override suspend fun pauseDownload(id: String) {
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
        val prefs = preferencesRepository.preferencesFlow.first()
        val current = downloadDao.getById(id)?.toDomain()
        if (current != null) {
            downloadDao.update(
                DownloadEntity.fromDomain(
                    current.copy(status = DownloadStatus.QUEUED, errorMessage = null)
                )
            )
            enqueueWorker(id, prefs.wifiOnly)
        }
    }

    override suspend fun cancelDownload(id: String) {
        pauseDownload(id)
    }

    override suspend fun deleteDownload(id: String, deleteFile: Boolean) {
        pauseDownload(id)
        if (deleteFile) {
            val item = downloadDao.getById(id)?.toDomain()
            if (item != null) {
                val f = File(item.savePath, item.filename)
                if (f.exists()) f.delete()
            }
        }
        downloadDao.deleteById(id)
    }

    override suspend fun retryDownload(id: String) {
        resumeDownload(id)
    }

    override suspend fun updateDownload(item: DownloadItem) {
        downloadDao.update(DownloadEntity.fromDomain(item))
    }

    private fun enqueueWorker(taskId: String, wifiOnly: Boolean) {
        val constraints = Constraints.Builder().apply {
            if (wifiOnly) {
                setRequiredNetworkType(NetworkType.UNMETERED)
            } else {
                setRequiredNetworkType(NetworkType.CONNECTED)
            }
        }.build()

        val request = OneTimeWorkRequestBuilder<DownloadWorker>()
            .setConstraints(constraints)
            .setInputData(workDataOf(DownloadWorker.KEY_TASK_ID to taskId))
            .build()

        workManager.enqueueUniqueWork(taskId, ExistingWorkPolicy.REPLACE, request)
    }
}

