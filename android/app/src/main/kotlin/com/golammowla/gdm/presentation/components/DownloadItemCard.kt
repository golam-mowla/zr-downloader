package com.golammowla.gdm.presentation.components

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.Description
import androidx.compose.material.icons.filled.FolderZip
import androidx.compose.material.icons.filled.Movie
import androidx.compose.material.icons.filled.MusicNote
import androidx.compose.material.icons.filled.Pause
import androidx.compose.material.icons.filled.PlayArrow
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material.icons.filled.Widgets
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.golammowla.gdm.domain.model.DownloadCategory
import com.golammowla.gdm.domain.model.DownloadItem
import com.golammowla.gdm.domain.model.DownloadStatus
import com.golammowla.gdm.presentation.theme.PrimaryBlue
import com.golammowla.gdm.presentation.theme.StatusError
import com.golammowla.gdm.presentation.theme.StatusQueued
import com.golammowla.gdm.presentation.theme.StatusSuccess
import com.golammowla.gdm.presentation.theme.StatusWarning

@Composable
fun DownloadItemCard(
    item: DownloadItem,
    onClick: () -> Unit,
    onPause: () -> Unit,
    onResume: () -> Unit,
    onDelete: () -> Unit,
    modifier: Modifier = Modifier
) {
    Card(
        modifier = modifier
            .fillMaxWidth()
            .clickable(onClick = onClick),
        shape = RoundedCornerShape(12.dp),
        colors = CardDefaults.cardColors(
            containerColor = MaterialTheme.colorScheme.surface
        ),
        elevation = CardDefaults.cardElevation(defaultElevation = 2.dp)
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(14.dp)
        ) {
            Row(
                modifier = Modifier.fillMaxWidth(),
                verticalAlignment = Alignment.CenterVertically
            ) {
                // Category Icon
                Surface(
                    shape = RoundedCornerShape(8.dp),
                    color = MaterialTheme.colorScheme.surfaceVariant,
                    modifier = Modifier.size(40.dp)
                ) {
                    Icon(
                        imageVector = getCategoryIcon(item.category),
                        contentDescription = item.category.name,
                        tint = PrimaryBlue,
                        modifier = Modifier
                            .padding(8.dp)
                            .size(24.dp)
                    )
                }

                Spacer(modifier = Modifier.width(12.dp))

                Column(modifier = Modifier.weight(1f)) {
                    Text(
                        text = item.filename,
                        style = MaterialTheme.typography.titleMedium,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis
                    )
                    Spacer(modifier = Modifier.height(2.dp))
                    Text(
                        text = buildSizeProgressText(item),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                }

                // Status Badge
                StatusBadge(status = item.status)
            }

            Spacer(modifier = Modifier.height(10.dp))

            // Main Progress Bar
            val progressVal = if (item.totalBytes > 0) {
                (item.downloadedBytes.toFloat() / item.totalBytes.toFloat()).coerceIn(0f, 1f)
            } else if (item.isCompleted) {
                1f
            } else {
                (item.percent / 100f).coerceIn(0f, 1f)
            }

            LinearProgressIndicator(
                progress = progressVal,
                modifier = Modifier
                    .fillMaxWidth()
                    .height(6.dp)
                    .clip(RoundedCornerShape(3.dp)),
                color = when (item.status) {
                    DownloadStatus.COMPLETED -> StatusSuccess
                    DownloadStatus.ERROR -> StatusError
                    DownloadStatus.PAUSED -> StatusWarning
                    else -> PrimaryBlue
                },
                trackColor = MaterialTheme.colorScheme.surfaceVariant
            )

            Spacer(modifier = Modifier.height(8.dp))

            // Segmented Connection Mini Visualizer
            SegmentThreadVisualizer(
                segments = item.segments,
                chunkCount = item.chunkCount,
                status = item.status,
                percent = item.percent
            )

            Spacer(modifier = Modifier.height(8.dp))

            // Footer Row: Speed / ETA + Action Buttons
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically
            ) {
                Text(
                    text = if (item.isDownloading) {
                        "⚡ ${item.speedFormatted} • ETA ${item.etaFormatted}"
                    } else if (item.isCompleted) {
                        "Completed ✔"
                    } else if (item.isError) {
                        "Error: ${item.errorMessage ?: "Failed"}"
                    } else {
                        item.status.name
                    },
                    style = MaterialTheme.typography.bodySmall,
                    color = if (item.isError) StatusError else MaterialTheme.colorScheme.onSurfaceVariant,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier.weight(1f)
                )

                Row(horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                    when {
                        item.isDownloading -> {
                            IconButton(onClick = onPause, modifier = Modifier.size(32.dp)) {
                                Icon(
                                    imageVector = Icons.Default.Pause,
                                    contentDescription = "Pause",
                                    tint = StatusWarning
                                )
                            }
                        }
                        item.isPaused -> {
                            IconButton(onClick = onResume, modifier = Modifier.size(32.dp)) {
                                Icon(
                                    imageVector = Icons.Default.PlayArrow,
                                    contentDescription = "Resume",
                                    tint = PrimaryBlue
                                )
                            }
                        }
                        item.isError -> {
                            IconButton(onClick = onResume, modifier = Modifier.size(32.dp)) {
                                Icon(
                                    imageVector = Icons.Default.Refresh,
                                    contentDescription = "Retry",
                                    tint = PrimaryBlue
                                )
                            }
                        }
                    }

                    IconButton(onClick = onDelete, modifier = Modifier.size(32.dp)) {
                        Icon(
                            imageVector = Icons.Default.Delete,
                            contentDescription = "Delete",
                            tint = StatusError
                        )
                    }
                }
            }
        }
    }
}

@Composable
fun StatusBadge(status: DownloadStatus) {
    val (bgColor, textColor) = when (status) {
        DownloadStatus.COMPLETED -> StatusSuccess.copy(alpha = 0.15f) to StatusSuccess
        DownloadStatus.DOWNLOADING -> PrimaryBlue.copy(alpha = 0.15f) to PrimaryBlue
        DownloadStatus.PAUSED -> StatusWarning.copy(alpha = 0.15f) to StatusWarning
        DownloadStatus.ERROR -> StatusError.copy(alpha = 0.15f) to StatusError
        DownloadStatus.QUEUED -> StatusQueued.copy(alpha = 0.15f) to StatusQueued
    }

    Surface(
        shape = RoundedCornerShape(6.dp),
        color = bgColor,
        modifier = Modifier.padding(start = 8.dp)
    ) {
        Text(
            text = status.name,
            style = MaterialTheme.typography.labelMedium.copy(fontSize = 10.sp),
            fontWeight = FontWeight.Bold,
            color = textColor,
            modifier = Modifier.padding(horizontal = 8.dp, vertical = 3.dp)
        )
    }
}

private fun getCategoryIcon(cat: DownloadCategory): ImageVector {
    return when (cat) {
        DownloadCategory.VIDEOS -> Icons.Default.Movie
        DownloadCategory.AUDIO -> Icons.Default.MusicNote
        DownloadCategory.COMPRESSED -> Icons.Default.FolderZip
        DownloadCategory.PROGRAMS -> Icons.Default.Widgets
        DownloadCategory.DOCUMENTS -> Icons.Default.Description
        else -> Icons.Default.Description
    }
}

private fun buildSizeProgressText(item: DownloadItem): String {
    val downStr = formatByteUnits(item.downloadedBytes)
    val totStr = if (item.totalBytes > 0) formatByteUnits(item.totalBytes) else "Unknown"
    return "$downStr / $totStr (${String.format("%.1f", item.percent)}%)"
}

fun formatByteUnits(bytes: Long): String {
    if (bytes <= 0) return "0 B"
    val kb = bytes / 1024.0
    val mb = kb / 1024.0
    val gb = mb / 1024.0
    return when {
        gb >= 1.0 -> String.format("%.2f GB", gb)
        mb >= 1.0 -> String.format("%.1f MB", mb)
        kb >= 1.0 -> String.format("%.1f KB", kb)
        else -> "$bytes B"
    }
}
