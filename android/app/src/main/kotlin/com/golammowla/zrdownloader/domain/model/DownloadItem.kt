package com.golammowla.zrdownloader.domain.model

enum class DownloadStatus {
    QUEUED,
    DOWNLOADING,
    PAUSED,
    COMPLETED,
    ERROR;

    val isTerminal: Boolean
        get() = this == COMPLETED || this == ERROR
}

enum class DownloadCategory(val label: String) {
    ALL("All"),
    VIDEOS("Videos"),
    AUDIO("Music & Audio"),
    COMPRESSED("Compressed"),
    PROGRAMS("Programs"),
    DOCUMENTS("Documents"),
    TORRENTS("Torrents");

    companion object {
        fun fromExtension(ext: String): DownloadCategory {
            val lower = ext.lowercase()
            return when (lower) {
                "mp4", "mkv", "webm", "avi", "mov", "flv", "wmv", "m3u8", "ts" -> VIDEOS
                "mp3", "m4a", "flac", "wav", "aac", "ogg", "opus" -> AUDIO
                "zip", "rar", "7z", "tar", "gz", "bz2", "xz", "iso", "tgz" -> COMPRESSED
                "apk", "exe", "msi", "dmg", "pkg", "deb", "rpm", "appx" -> PROGRAMS
                "pdf", "doc", "docx", "xls", "xlsx", "ppt", "pptx", "txt", "epub" -> DOCUMENTS
                "torrent" -> TORRENTS
                else -> ALL
            }
        }
    }
}

data class SegmentInfo(
    val index: Int,
    val startByte: Long,
    val endByte: Long,
    val downloadedBytes: Long,
    val isComplete: Boolean = false
) {
    val progress: Float
        get() {
            val total = endByte - startByte + 1
            if (total <= 0) return 0f
            return (downloadedBytes.toFloat() / total.toFloat()).coerceIn(0f, 1f)
        }
}

data class DownloadItem(
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
    val status: DownloadStatus = DownloadStatus.QUEUED,
    val category: DownloadCategory = DownloadCategory.ALL,
    val chunkCount: Int = 8,
    val isResumable: Boolean = true,
    val errorMessage: String? = null,
    val createdAt: Long = System.currentTimeMillis(),
    val segments: List<SegmentInfo> = emptyList()
) {
    val isCompleted: Boolean
        get() = status == DownloadStatus.COMPLETED

    val isDownloading: Boolean
        get() = status == DownloadStatus.DOWNLOADING

    val isPaused: Boolean
        get() = status == DownloadStatus.PAUSED

    val isError: Boolean
        get() = status == DownloadStatus.ERROR
}

