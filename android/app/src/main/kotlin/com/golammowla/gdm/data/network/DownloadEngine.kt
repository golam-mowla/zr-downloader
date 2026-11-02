package com.golammowla.gdm.data.network

import com.golammowla.gdm.domain.model.DownloadCategory
import com.golammowla.gdm.domain.model.SegmentInfo
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.async
import kotlinx.coroutines.awaitAll
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.withContext
import okhttp3.OkHttpClient
import okhttp3.Request
import timber.log.Timber
import java.io.File
import java.io.InputStream
import java.io.RandomAccessFile
import java.net.URLDecoder
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.atomic.AtomicBoolean
import java.util.concurrent.atomic.AtomicLong
import javax.inject.Inject
import javax.inject.Singleton

data class ProbeResult(
    val totalBytes: Long,
    val isResumable: Boolean,
    val suggestedFilename: String,
    val category: DownloadCategory
)

data class DownloadProgressUpdate(
    val downloadedBytes: Long,
    val totalBytes: Long,
    val percent: Float,
    val speedBps: Long,
    val speedFormatted: String,
    val etaFormatted: String,
    val segments: List<SegmentInfo>
)

@Singleton
class DownloadEngine @Inject constructor(
    private val okHttpClient: OkHttpClient
) {
    private val activeJobs = ConcurrentHashMap<String, AtomicBoolean>()

    suspend fun probeUrl(url: String): ProbeResult = withContext(Dispatchers.IO) {
        var totalBytes = -1L
        var isResumable = false
        var filename = extractFilenameFromUrl(url)

        try {
            val headRequest = Request.Builder()
                .url(url)
                .header("User-Agent", USER_AGENT)
                .head()
                .build()

            okHttpClient.newCall(headRequest).execute().use { response ->
                if (response.isSuccessful) {
                    val lenHeader = response.header("Content-Length")
                    if (!lenHeader.isNullOrBlank()) {
                        totalBytes = lenHeader.toLongOrNull() ?: -1L
                    }

                    val acceptRanges = response.header("Accept-Ranges")
                    if (acceptRanges != null && acceptRanges.contains("bytes", ignoreCase = true)) {
                        isResumable = true
                    }

                    val disposition = response.header("Content-Disposition")
                    val parsedName = parseContentDispositionFilename(disposition)
                    if (!parsedName.isNullOrBlank()) {
                        filename = parsedName
                    }
                }
            }
        } catch (e: Exception) {
            Timber.w(e, "HEAD request probe failed, attempting Range probe")
        }

        // If HEAD failed or gave no length, try GET with Range 0-0
        if (totalBytes <= 0) {
            try {
                val rangeRequest = Request.Builder()
                    .url(url)
                    .header("User-Agent", USER_AGENT)
                    .header("Range", "bytes=0-0")
                    .get()
                    .build()

                okHttpClient.newCall(rangeRequest).execute().use { response ->
                    val contentRange = response.header("Content-Range")
                    if (!contentRange.isNullOrBlank()) {
                        isResumable = true
                        val totalStr = contentRange.substringAfterLast("/")
                        totalBytes = totalStr.toLongOrNull() ?: -1L
                    }
                    val disposition = response.header("Content-Disposition")
                    val parsedName = parseContentDispositionFilename(disposition)
                    if (!parsedName.isNullOrBlank()) {
                        filename = parsedName
                    }
                }
            } catch (e: Exception) {
                Timber.w(e, "Range probe failed")
            }
        }

        val ext = filename.substringAfterLast('.', "")
        val category = DownloadCategory.fromExtension(ext)

        ProbeResult(
            totalBytes = totalBytes.coerceAtLeast(0L),
            isResumable = isResumable,
            suggestedFilename = filename,
            category = category
        )
    }

    suspend fun download(
        taskId: String,
        url: String,
        targetFile: File,
        totalBytes: Long,
        isResumable: Boolean,
        chunkCount: Int,
        existingSegments: List<SegmentInfo>,
        onProgress: suspend (DownloadProgressUpdate) -> Unit
    ) = withContext(Dispatchers.IO) {
        val isCancelled = AtomicBoolean(false)
        activeJobs[taskId] = isCancelled

        targetFile.parentFile?.mkdirs()

        val actualChunks = if (isResumable && totalBytes > 1024 * 1024) {
            chunkCount.coerceIn(1, 32)
        } else {
            1
        }

        val segments = if (existingSegments.isNotEmpty() && existingSegments.size == actualChunks) {
            existingSegments.toMutableList()
        } else {
            calculateSegments(totalBytes, actualChunks)
        }

        // Pre-allocate target file if total size is known
        if (totalBytes > 0 && (!targetFile.exists() || targetFile.length() < totalBytes)) {
            RandomAccessFile(targetFile, "rw").use { raf ->
                raf.setLength(totalBytes)
            }
        }

        val downloadedBytesCounter = AtomicLong(segments.sumOf { it.downloadedBytes })
        val segmentProgressMap = ConcurrentHashMap<Int, AtomicLong>()
        segments.forEach { seg ->
            segmentProgressMap[seg.index] = AtomicLong(seg.downloadedBytes)
        }

        var lastRecordedBytes = downloadedBytesCounter.get()
        var lastTime = System.currentTimeMillis()

        // Ticker job for real-time speed & ETA calculation
        val tickerScope = this
        val tickerJob = kotlinx.coroutines.launch {
            while (isActive && !isCancelled.get()) {
                delay(800)
                val now = System.currentTimeMillis()
                val currentBytes = downloadedBytesCounter.get()
                val timeDiffSec = (now - lastTime) / 1000.0

                if (timeDiffSec > 0.4) {
                    val bytesDiff = (currentBytes - lastRecordedBytes).coerceAtLeast(0)
                    val speedBps = (bytesDiff / timeDiffSec).toLong()
                    lastRecordedBytes = currentBytes
                    lastTime = now

                    val pct = if (totalBytes > 0) {
                        ((currentBytes.toFloat() / totalBytes.toFloat()) * 100f).coerceIn(0f, 99.9f)
                    } else {
                        0f
                    }

                    val etaSec = if (speedBps > 0 && totalBytes > currentBytes) {
                        (totalBytes - currentBytes) / speedBps
                    } else {
                        -1L
                    }

                    val currentSegmentList = segments.map { seg ->
                        val down = segmentProgressMap[seg.index]?.get() ?: seg.downloadedBytes
                        val segTotal = (seg.endByte - seg.startByte + 1).coerceAtLeast(1)
                        seg.copy(
                            downloadedBytes = down,
                            isComplete = down >= segTotal
                        )
                    }

                    onProgress(
                        DownloadProgressUpdate(
                            downloadedBytes = currentBytes,
                            totalBytes = totalBytes,
                            percent = pct,
                            speedBps = speedBps,
                            speedFormatted = formatSpeed(speedBps),
                            etaFormatted = formatEta(etaSec),
                            segments = currentSegmentList
                        )
                    )
                }
            }
        }

        try {
            if (actualChunks > 1) {
                // Multi-threaded segmented download
                coroutineScope {
                    val deferredList = segments.map { segment ->
                        async(Dispatchers.IO) {
                            downloadSegment(
                                url = url,
                                targetFile = targetFile,
                                segment = segment,
                                isCancelled = isCancelled,
                                onByteChunk = { addedBytes ->
                                    downloadedBytesCounter.addAndGet(addedBytes)
                                    segmentProgressMap[segment.index]?.addAndGet(addedBytes)
                                }
                            )
                        }
                    }
                    deferredList.awaitAll()
                }
            } else {
                // Single-stream download
                downloadSingleStream(
                    url = url,
                    targetFile = targetFile,
                    startOffset = downloadedBytesCounter.get(),
                    isCancelled = isCancelled,
                    onByteChunk = { addedBytes ->
                        downloadedBytesCounter.addAndGet(addedBytes)
                        segmentProgressMap[0]?.addAndGet(addedBytes)
                    }
                )
            }

            tickerJob.cancel()

            if (isCancelled.get()) {
                throw CancellationException("Download cancelled or paused by user")
            }

            // Final 100% update
            val finalBytes = if (totalBytes > 0) totalBytes else downloadedBytesCounter.get()
            val completedSegments = segments.map { seg ->
                val segTotal = (seg.endByte - seg.startByte + 1).coerceAtLeast(1)
                seg.copy(downloadedBytes = segTotal, isComplete = true)
            }
            onProgress(
                DownloadProgressUpdate(
                    downloadedBytes = finalBytes,
                    totalBytes = finalBytes,
                    percent = 100f,
                    speedBps = 0L,
                    speedFormatted = "Done",
                    etaFormatted = "0s",
                    segments = completedSegments
                )
            )
        } finally {
            tickerJob.cancel()
            activeJobs.remove(taskId)
        }
    }

    private suspend fun downloadSegment(
        url: String,
        targetFile: File,
        segment: SegmentInfo,
        isCancelled: AtomicBoolean,
        onByteChunk: (Long) -> Unit
    ) = withContext(Dispatchers.IO) {
        val currentStart = segment.startByte + segment.downloadedBytes
        if (currentStart > segment.endByte) return@withContext

        val request = Request.Builder()
            .url(url)
            .header("User-Agent", USER_AGENT)
            .header("Range", "bytes=$currentStart-${segment.endByte}")
            .build()

        okHttpClient.newCall(request).execute().use { response ->
            if (!response.isSuccessful && response.code != 206) {
                throw IllegalStateException("Segment HTTP request failed with code ${response.code}")
            }

            val body = response.body ?: throw IllegalStateException("Empty body for segment ${segment.index}")
            val input: InputStream = body.byteStream()

            RandomAccessFile(targetFile, "rw").use { raf ->
                raf.seek(currentStart)
                val buffer = ByteArray(BUFFER_SIZE)
                var bytesRead: Int

                while (input.read(buffer).also { bytesRead = it } != -1) {
                    if (isCancelled.get()) {
                        throw CancellationException("Download cancelled")
                    }
                    raf.write(buffer, 0, bytesRead)
                    onByteChunk(bytesRead.toLong())
                }
            }
        }
    }

    private suspend fun downloadSingleStream(
        url: String,
        targetFile: File,
        startOffset: Long,
        isCancelled: AtomicBoolean,
        onByteChunk: (Long) -> Unit
    ) = withContext(Dispatchers.IO) {
        val builder = Request.Builder()
            .url(url)
            .header("User-Agent", USER_AGENT)

        if (startOffset > 0) {
            builder.header("Range", "bytes=$startOffset-")
        }

        okHttpClient.newCall(builder.build()).execute().use { response ->
            if (!response.isSuccessful && response.code != 206) {
                throw IllegalStateException("HTTP error code ${response.code}")
            }

            val body = response.body ?: throw IllegalStateException("Empty response body")
            val input = body.byteStream()

            RandomAccessFile(targetFile, "rw").use { raf ->
                if (startOffset > 0) {
                    raf.seek(startOffset)
                }
                val buffer = ByteArray(BUFFER_SIZE)
                var bytesRead: Int

                while (input.read(buffer).also { bytesRead = it } != -1) {
                    if (isCancelled.get()) {
                        throw CancellationException("Download cancelled")
                    }
                    raf.write(buffer, 0, bytesRead)
                    onByteChunk(bytesRead.toLong())
                }
            }
        }
    }

    fun cancelTask(taskId: String) {
        activeJobs[taskId]?.set(true)
    }

    private fun calculateSegments(totalBytes: Long, chunkCount: Int): MutableList<SegmentInfo> {
        val list = mutableListOf<SegmentInfo>()
        if (totalBytes <= 0 || chunkCount <= 1) {
            list.add(SegmentInfo(index = 0, startByte = 0, endByte = (totalBytes - 1).coerceAtLeast(0), downloadedBytes = 0))
            return list
        }

        val chunkSize = totalBytes / chunkCount
        for (i in 0 until chunkCount) {
            val start = i * chunkSize
            val end = if (i == chunkCount - 1) totalBytes - 1 else (i + 1) * chunkSize - 1
            list.add(SegmentInfo(index = i, startByte = start, endByte = end, downloadedBytes = 0))
        }
        return list
    }

    companion object {
        const val USER_AGENT = "Mozilla/5.0 (Linux; Android 14; Mobile) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Mobile Safari/537.36"
        private const val BUFFER_SIZE = 64 * 1024

        fun extractFilenameFromUrl(url: String): String {
            return try {
                val clean = url.substringBefore('?').substringBefore('#')
                val path = java.net.URI(clean).path
                val name = path.substringAfterLast('/')
                if (name.isNotBlank()) URLDecoder.decode(name, "UTF-8") else "downloaded_file"
            } catch (_: Exception) {
                "downloaded_file"
            }
        }

        fun parseContentDispositionFilename(header: String?): String? {
            if (header.isNullOrBlank()) return null
            val pattern = "filename[*]?=([\"']?)([^\"';]+)\\1".toRegex(RegexOption.IGNORE_CASE)
            val match = pattern.find(header) ?: return null
            var name = match.groupValues[2].trim()
            if (name.startsWith("UTF-8''", ignoreCase = true)) {
                name = name.substring(7)
            }
            return try {
                URLDecoder.decode(name, "UTF-8")
            } catch (_: Exception) {
                name
            }
        }

        fun formatSpeed(speedBps: Long): String {
            if (speedBps <= 0) return "0 B/s"
            val kb = speedBps / 1024.0
            val mb = kb / 1024.0
            val gb = mb / 1024.0
            return when {
                gb >= 1.0 -> String.format("%.2f GB/s", gb)
                mb >= 1.0 -> String.format("%.2f MB/s", mb)
                kb >= 1.0 -> String.format("%.1f KB/s", kb)
                else -> "$speedBps B/s"
            }
        }

        fun formatEta(seconds: Long): String {
            if (seconds <= 0) return "--"
            val h = seconds / 3600
            val m = (seconds % 3600) / 60
            val s = seconds % 60
            return when {
                h > 0 -> String.format("%02d:%02d:%02d", h, m, s)
                m > 0 -> String.format("%02d:%02d", m, s)
                else -> "${s}s"
            }
        }
    }
}
