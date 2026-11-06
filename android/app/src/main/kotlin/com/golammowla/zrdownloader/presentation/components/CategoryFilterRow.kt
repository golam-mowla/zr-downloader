package com.golammowla.zrdownloader.presentation.components

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
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
import com.golammowla.zrdownloader.domain.model.DownloadCategory
import com.golammowla.zrdownloader.domain.model.DownloadStatus
import com.golammowla.zrdownloader.presentation.theme.GlassAccentGradientEnd
import com.golammowla.zrdownloader.presentation.theme.GlassAccentGradientStart
import com.golammowla.zrdownloader.presentation.theme.GlassDarkCard
import com.golammowla.zrdownloader.presentation.theme.GlassDarkStroke
import com.golammowla.zrdownloader.presentation.theme.GlassLightCard
import com.golammowla.zrdownloader.presentation.theme.GlassLightStroke
import com.golammowla.zrdownloader.presentation.theme.PrimaryBlue

@Composable
fun StatusFilterChipsRow(
    selectedStatus: DownloadStatus?,
    onSelectStatus: (DownloadStatus?) -> Unit,
    isDark: Boolean,
    modifier: Modifier = Modifier
) {
    val filters = listOf(
        Pair("All", null),
        Pair("Downloading", DownloadStatus.DOWNLOADING),
        Pair("Finished", DownloadStatus.COMPLETED),
        Pair("Paused", DownloadStatus.PAUSED),
        Pair("Error", DownloadStatus.ERROR)
    )

    val cardBg = if (isDark) GlassDarkCard else GlassLightCard
    val strokeColor = if (isDark) GlassDarkStroke else GlassLightStroke

    Row(
        modifier = modifier
            .fillMaxWidth()
            .horizontalScroll(rememberScrollState())
            .padding(horizontal = 16.dp, vertical = 6.dp),
        horizontalArrangement = Arrangement.spacedBy(8.dp)
    ) {
        filters.forEach { (label, status) ->
            val isSelected = selectedStatus == status
            val pillShape = RoundedCornerShape(999.dp)

            Box(
                modifier = Modifier
                    .clip(pillShape)
                    .background(if (isSelected) PrimaryBlue else cardBg)
                    .then(
                        if (!isSelected) Modifier.border(1.dp, strokeColor, pillShape) else Modifier
                    )
                    .clickable { onSelectStatus(status) }
                    .padding(horizontal = 14.dp, vertical = 7.dp),
                contentAlignment = Alignment.Center
            ) {
                Text(
                    text = label,
                    fontSize = 13.sp,
                    fontWeight = if (isSelected) FontWeight.SemiBold else FontWeight.Normal,
                    color = if (isSelected) Color.White else MaterialTheme.colorScheme.onSurface
                )
            }
        }
    }
}

@Composable
fun CategoryFilterRow(
    selectedCategory: DownloadCategory,
    onSelectCategory: (DownloadCategory) -> Unit,
    isDark: Boolean,
    modifier: Modifier = Modifier
) {
    val cardBg = if (isDark) GlassDarkCard else GlassLightCard
    val strokeColor = if (isDark) GlassDarkStroke else GlassLightStroke

    Row(
        modifier = modifier
            .fillMaxWidth()
            .horizontalScroll(rememberScrollState())
            .padding(horizontal = 16.dp, vertical = 4.dp),
        horizontalArrangement = Arrangement.spacedBy(6.dp)
    ) {
        DownloadCategory.values().forEach { category ->
            val isSelected = category == selectedCategory
            val pillShape = RoundedCornerShape(999.dp)

            Box(
                modifier = Modifier
                    .clip(pillShape)
                    .background(if (isSelected) PrimaryBlue.copy(alpha = 0.85f) else cardBg.copy(alpha = 0.4f))
                    .then(
                        if (!isSelected) Modifier.border(1.dp, strokeColor.copy(alpha = 0.5f), pillShape) else Modifier
                    )
                    .clickable { onSelectCategory(category) }
                    .padding(horizontal = 11.dp, vertical = 5.dp),
                contentAlignment = Alignment.Center
            ) {
                Text(
                    text = category.label,
                    fontSize = 11.5.sp,
                    fontWeight = if (isSelected) FontWeight.SemiBold else FontWeight.Normal,
                    color = if (isSelected) Color.White else MaterialTheme.colorScheme.onSurfaceVariant
                )
            }
        }
    }
}

