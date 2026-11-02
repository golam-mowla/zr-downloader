package com.golammowla.gdm.domain.repository

import com.golammowla.gdm.domain.model.DownloadCategory
import com.golammowla.gdm.domain.model.DownloadItem
import kotlinx.coroutines.flow.Flow

interface DownloadRepository {
    fun getAllDownloads(): Flow<List<DownloadItem>>
    fun getDownloadsByCategory(category: DownloadCategory): Flow<List<DownloadItem>>
    fun getDownloadById(id: String): Flow<DownloadItem?>
    suspend fun getDownload(id: String): DownloadItem?

    suspend fun startDownload(
        url: String,
        filename: String? = null,
        savePath: String? = null,
        chunkCount: Int = 8
    ): String

    suspend fun pauseDownload(id: String)
    suspend fun resumeDownload(id: String)
    suspend fun cancelDownload(id: String)
    suspend fun deleteDownload(id: String, deleteFile: Boolean = false)
    suspend fun retryDownload(id: String)
    suspend fun updateDownload(item: DownloadItem)
}
