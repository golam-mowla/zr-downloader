package com.golammowla.gdm.presentation.components

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
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
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.golammowla.gdm.domain.model.DownloadCategory
import com.golammowla.gdm.domain.model.DownloadItem
import com.golammowla.gdm.domain.model.DownloadStatus
import com.golammowla.gdm.presentation.theme.GlassDarkCard
import com.golammowla.gdm.presentation.theme.GlassDarkStroke
import com.golammowla.gdm.presentation.theme.GlassLightCard
import com.golammowla.gdm.presentation.theme.GlassLightStroke
import com.golammowla.gdm.presentation.theme.GlassProgressGradientEnd
import com.golammowla.gdm.presentation.theme.GlassProgressGradientStart
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
    isDark: Boolean = isSystemInDarkTheme(),
    modifier: Modifier = Modifier
) {
    val cardBg = if (isDark) GlassDarkCard else GlassLightCard
    val strokeColor = if (isDark) GlassDarkStroke else GlassLightStroke
    val cardShape = RoundedCornerShape(20.dp)

    Box(
        modifier = modifier
            .fillMaxWidth()
            .clip(cardShape)
            .background(cardBg)
            .border(1.dp, strokeColor, cardShape)
            .clickable(onClick = onClick)
            .padding(14.dp)
    ) {
        Column(modifier = Modifier.fillMaxWidth()) {
            Row(
                modifier = Modifier.fillMaxWidth(),
                verticalAlignment = Alignment.CenterVertically
            ) {
                // Category Icon Tile (.fi: 44x44, radius 14dp)
                val tileShape = RoundedCornerShape(14.dp)
                Box(
                    modifier = Modifier
                        .size(44.dp)
                        .clip(tileShape)
                        .background(if (isDark) Color(0x33FFFFFF) else Color(0x4DFFFFFF))
                        .border(1.dp, strokeColor, tileShape),
                    contentAlignment = Alignment.Center
                ) {
                    Text(
                        text = getCategoryEmoji(item.category),
                        fontSize = 20.sp
                    )
                }

                Spacer(modifier = Modifier.width(12.dp))

                Column(modifier = Modifier.weight(1f)) {
                    Text(
                        text = item.filename,
                        fontSize = 14.5.sp,
                        fontWeight = FontWeight.SemiBold,
                        color = MaterialTheme.colorScheme.onSurface,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis
                    )
                    Spacer(modifier = Modifier.height(2.dp))
                    Text(
                        text = buildSizeProgressText(item),
                        fontSize = 12.sp,
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                }

                Spacer(modifier = Modifier.width(8.dp))

                // Status Badge Pill
                GlassStatusBadge(status = item.status)
            }

            Spacer(modifier = Modifier.height(12.dp))

            // Main Progress Track (.trk: height 6dp, radius 9dp)
            val progressVal = if (item.totalBytes > 0) {
                (item.downloadedBytes.toFloat() / item.totalBytes.toFloat()).coerceIn(0f, 1f)
            } else if (item.isCompleted) {
                1f
            } else {
                (item.percent / 100f).coerceIn(0f, 1f)
            }

            val trackShape = RoundedCornerShape(9.dp)
            Box(
                modifier = Modifier
                    .fillMaxWidth()
                    .height(6.dp)
                    .clip(trackShape)
                    .background(Color(0x33787896))
            ) {
                val fillBrush = when (item.status) {
                    DownloadStatus.COMPLETED -> Brush.horizontalGradient(listOf(StatusSuccess, StatusSuccess))
                    DownloadStatus.PAUSED -> Brush.horizontalGradient(listOf(StatusWarning, StatusWarning))
                    DownloadStatus.ERROR -> Brush.horizontalGradient(listOf(StatusError, StatusError))
                    else -> Brush.horizontalGradient(listOf(GlassProgressGradientStart, GlassProgressGradientEnd))
                }

                Box(
                    modifier = Modifier
                        .fillMaxWidth(fraction = progressVal)
                        .fillMaxHeight()
                        .clip(trackShape)
                        .background(fillBrush)
                )
            }

            Spacer(modifier = Modifier.height(8.dp))

            // Segmented Connection Mini Visualizer
            SegmentThreadVisualizer(
                segments = item.segments,
                chunkCount = item.chunkCount,
                status = item.status,
                percent = item.percent
            )

            Spacer(modifier = Modifier.height(6.dp))

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
                    fontSize = 11.5.sp,
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
    GlassStatusBadge(status = status)
}

@Composable
fun GlassStatusBadge(status: DownloadStatus) {
    val (bgColor, textColor, label) = when (status) {
        DownloadStatus.COMPLETED -> Triple(StatusSuccess.copy(alpha = 0.18f), StatusSuccess, "Completed")
        DownloadStatus.DOWNLOADING -> Triple(PrimaryBlue.copy(alpha = 0.18f), PrimaryBlue, "Downloading")
        DownloadStatus.PAUSED -> Triple(StatusWarning.copy(alpha = 0.18f), StatusWarning, "Paused")
        DownloadStatus.ERROR -> Triple(StatusError.copy(alpha = 0.18f), StatusError, "Error")
        DownloadStatus.QUEUED -> Triple(StatusQueued.copy(alpha = 0.18f), StatusQueued, "Queued")
    }

    val pillShape = RoundedCornerShape(999.dp)
    Box(
        modifier = Modifier
            .clip(pillShape)
            .background(bgColor)
            .padding(horizontal = 9.dp, vertical = 3.dp),
        contentAlignment = Alignment.Center
    ) {
        Text(
            text = label,
            fontSize = 11.sp,
            fontWeight = FontWeight.SemiBold,
            color = textColor
        )
    }
}

private fun getCategoryEmoji(cat: DownloadCategory): String {
    return when (cat) {
        DownloadCategory.VIDEOS -> "🎬"
        DownloadCategory.AUDIO -> "🎵"
        DownloadCategory.COMPRESSED -> "🗜️"
        DownloadCategory.PROGRAMS -> "⚙️"
        DownloadCategory.DOCUMENTS -> "📄"
        DownloadCategory.TORRENTS -> "📡"
        else -> "⬇️"
    }
}

private fun buildSizeProgressText(item: DownloadItem): String {
    val downStr = formatByteUnits(item.downloadedBytes)
    val totStr = if (item.totalBytes > 0) formatByteUnits(item.totalBytes) else "Unknown"
    val spdStr = if (item.isDownloading && item.speedBps > 0) " · ${item.speedFormatted}" else ""
    return "$downStr of $totStr$spdStr"
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
