package com.golammowla.zrdownloader.presentation.components

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.golammowla.zrdownloader.domain.model.DownloadStatus
import com.golammowla.zrdownloader.domain.model.SegmentInfo
import com.golammowla.zrdownloader.presentation.theme.PrimaryBlue
import com.golammowla.zrdownloader.presentation.theme.StatusError
import com.golammowla.zrdownloader.presentation.theme.StatusSuccess
import com.golammowla.zrdownloader.presentation.theme.StatusWarning

@Composable
fun SegmentThreadVisualizer(
    segments: List<SegmentInfo>,
    chunkCount: Int,
    status: DownloadStatus,
    percent: Float,
    modifier: Modifier = Modifier
) {
    val totalChunks = if (segments.isNotEmpty()) segments.size else chunkCount.coerceIn(1, 32)

    val activeColor = when (status) {
        DownloadStatus.COMPLETED -> StatusSuccess
        DownloadStatus.ERROR -> StatusError
        DownloadStatus.PAUSED -> StatusWarning
        else -> PrimaryBlue
    }

    Column(modifier = modifier.fillMaxWidth()) {
        Row(
            modifier = Modifier.fillMaxWidth().padding(bottom = 4.dp),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically
        ) {
            Text(
                text = "Segmented Connection:",
                style = MaterialTheme.typography.labelMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
            Text(
                text = "$totalChunks Parallel Streams",
                style = MaterialTheme.typography.labelMedium,
                fontWeight = FontWeight.Bold,
                color = PrimaryBlue
            )
        }

        Row(
            modifier = Modifier
                .fillMaxWidth()
                .height(8.dp),
            horizontalArrangement = Arrangement.spacedBy(2.dp)
        ) {
            for (i in 0 until totalChunks) {
                val seg = segments.getOrNull(i)
                val chunkProg = if (status == DownloadStatus.COMPLETED) {
                    1f
                } else if (seg != null) {
                    seg.progress
                } else {
                    ((percent / 100f) + (i % 3) * 0.04f).coerceIn(0f, 1f)
                }

                Box(
                    modifier = Modifier
                        .weight(1f)
                        .fillMaxHeight()
                        .clip(RoundedCornerShape(1.dp))
                        .background(MaterialTheme.colorScheme.surfaceVariant)
                ) {
                    Box(
                        modifier = Modifier
                            .fillMaxHeight()
                            .fillMaxWidth(chunkProg)
                            .background(activeColor)
                    )
                }
            }
        }
    }
}

