package com.golammowla.gdm.data.database.entity

import androidx.room.Entity
import androidx.room.PrimaryKey
import com.golammowla.gdm.domain.model.DownloadCategory
import com.golammowla.gdm.domain.model.DownloadItem
import com.golammowla.gdm.domain.model.DownloadStatus
import com.golammowla.gdm.domain.model.SegmentInfo
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json

@Entity(tableName = "downloads")
data class DownloadEntity(
    @PrimaryKey
    val id: String,
    val url: String,
    val filename: String,
    val savePath: String,
    val totalBytes: Long = 0L,
    val downloadedBytes: Long = 0L,
    val percent: Float = 0f,
    val speedBps: Long = 0L,
    val speedFormatted: String = "0 B/s",
    val etaFormatted: String = "--",
    val status: String = DownloadStatus.QUEUED.name,
    val category: String = DownloadCategory.ALL.name,
    val chunkCount: Int = 8,
    val isResumable: Boolean = true,
    val errorMessage: String? = null,
    val createdAt: Long = System.currentTimeMillis(),
    val segmentsJson: String = "[]"
) {
    fun toDomain(): DownloadItem {
        val parsedSegments = try {
            val list = Json.decodeFromString<List<SegmentDto>>(segmentsJson)
            list.map {
                SegmentInfo(
                    index = it.index,
                    startByte = it.startByte,
                    endByte = it.endByte,
                    downloadedBytes = it.downloadedBytes,
                    isComplete = it.isComplete
                )
            }
        } catch (_: Exception) {
            emptyList()
        }

        return DownloadItem(
            id = id,
            url = url,
            filename = filename,
            savePath = savePath,
            totalBytes = totalBytes,
            downloadedBytes = downloadedBytes,
            percent = percent,
            speedBps = speedBps,
            speedFormatted = speedFormatted,
            etaFormatted = etaFormatted,
            status = try { DownloadStatus.valueOf(status) } catch (_: Exception) { DownloadStatus.QUEUED },
            category = try { DownloadCategory.valueOf(category) } catch (_: Exception) { DownloadCategory.ALL },
            chunkCount = chunkCount,
            isResumable = isResumable,
            errorMessage = errorMessage,
            createdAt = createdAt,
            segments = parsedSegments
        )
    }

    companion object {
        fun fromDomain(item: DownloadItem): DownloadEntity {
            val dtoList = item.segments.map {
                SegmentDto(
                    index = it.index,
                    startByte = it.startByte,
                    endByte = it.endByte,
                    downloadedBytes = it.downloadedBytes,
                    isComplete = it.isComplete
                )
            }
            val jsonStr = Json.encodeToString(dtoList)

            return DownloadEntity(
                id = item.id,
                url = item.url,
                filename = item.filename,
                savePath = item.savePath,
                totalBytes = item.totalBytes,
                downloadedBytes = item.downloadedBytes,
                percent = item.percent,
                speedBps = item.speedBps,
                speedFormatted = item.speedFormatted,
                etaFormatted = item.etaFormatted,
                status = item.status.name,
                category = item.category.name,
                chunkCount = item.chunkCount,
                isResumable = item.isResumable,
                errorMessage = item.errorMessage,
                createdAt = item.createdAt,
                segmentsJson = jsonStr
            )
        }
    }
}

@kotlinx.serialization.Serializable
data class SegmentDto(
    val index: Int,
    val startByte: Long,
    val endByte: Long,
    val downloadedBytes: Long,
    val isComplete: Boolean = false
)
