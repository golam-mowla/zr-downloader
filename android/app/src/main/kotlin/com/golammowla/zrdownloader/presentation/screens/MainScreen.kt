package com.golammowla.zrdownloader.presentation.screens

import android.widget.Toast
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.DarkMode
import androidx.compose.material.icons.filled.LightMode
import androidx.compose.material.icons.filled.Search
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedTextFieldDefaults
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.golammowla.zrdownloader.R
import com.golammowla.zrdownloader.presentation.components.AddDownloadDialog
import com.golammowla.zrdownloader.presentation.components.CategoryFilterRow
import com.golammowla.zrdownloader.presentation.components.DownloadDetailsDialog
import com.golammowla.zrdownloader.presentation.components.DownloadItemCard
import com.golammowla.zrdownloader.presentation.components.MediaGrabberDialog
import com.golammowla.zrdownloader.presentation.components.SettingsDialog
import com.golammowla.zrdownloader.presentation.components.StatusFilterChipsRow
import com.golammowla.zrdownloader.presentation.theme.GlassAccentGradientEnd
import com.golammowla.zrdownloader.presentation.theme.GlassAccentGradientStart
import com.golammowla.zrdownloader.presentation.theme.GlassDarkCard
import com.golammowla.zrdownloader.presentation.theme.GlassDarkStroke
import com.golammowla.zrdownloader.presentation.theme.GlassLightCard
import com.golammowla.zrdownloader.presentation.theme.GlassLightStroke
import com.golammowla.zrdownloader.presentation.theme.PrimaryBlue
import com.golammowla.zrdownloader.presentation.viewmodel.DownloadViewModel

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun MainScreen(
    viewModel: DownloadViewModel,
    modifier: Modifier = Modifier
) {
    val uiState by viewModel.uiState.collectAsState()
    val isDark = uiState.isDarkTheme
    val context = LocalContext.current
    var isSearchVisible by remember { mutableStateOf(false) }

    // Glass Background Colors (from ZR Downloader Android - Glass App Preview)
    val backgroundBrush = if (isDark) {
        Brush.verticalGradient(
            colors = listOf(
                Color(0xFF1B1B2B),
                Color(0xFF241A5E),
                Color(0xFF1F1838),
                Color(0xFF0F2A36)
            )
        )
    } else {
        Brush.verticalGradient(
            colors = listOf(
                Color(0xFFF2F4FA),
                Color(0xFFE4E7F5),
                Color(0xFFEAF5F8)
            )
        )
    }

    val cardBg = if (isDark) GlassDarkCard else GlassLightCard
    val strokeColor = if (isDark) GlassDarkStroke else GlassLightStroke

    Scaffold(
        containerColor = Color.Transparent,
        modifier = modifier.background(backgroundBrush)
    ) { paddingValues ->
        Box(
            modifier = Modifier
                .fillMaxSize()
                .padding(paddingValues)
        ) {
            Column(
                modifier = Modifier
                    .fillMaxSize()
                    .padding(bottom = 76.dp) // Room for floating dock
            ) {
                // Top Header Row (.top: Logo + ZR Downloader + Theme Button + Search Button)
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(horizontal = 16.dp, vertical = 10.dp),
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    // Red ZR Logo Tile (.logo: 36x36, radius 12dp)
                    val logoShape = RoundedCornerShape(12.dp)
                    Box(
                        modifier = Modifier
                            .size(38.dp)
                            .clip(logoShape)
                            .background(Color(0xFFD40203))
                            .border(1.dp, strokeColor, logoShape),
                        contentAlignment = Alignment.Center
                    ) {
                        Image(
                            painter = painterResource(id = R.drawable.ic_zr_logo),
                            contentDescription = "ZR Downloader Logo",
                            modifier = Modifier.size(38.dp)
                        )
                    }

                    Spacer(modifier = Modifier.width(12.dp))

                    Column(modifier = Modifier.weight(1f)) {
                        Text(
                            text = "ZR Downloader",
                            fontSize = 19.sp,
                            fontWeight = FontWeight.Bold,
                            color = MaterialTheme.colorScheme.onSurface,
                            letterSpacing = (-0.2).sp
                        )
                        Text(
                            text = "High-Speed Accelerator",
                            fontSize = 11.5.sp,
                            color = MaterialTheme.colorScheme.onSurfaceVariant
                        )
                    }

                    // Circular Glass Theme Toggle (.ib)
                    Box(
                        modifier = Modifier
                            .size(38.dp)
                            .clip(CircleShape)
                            .background(cardBg)
                            .border(1.dp, strokeColor, CircleShape)
                            .clickable { viewModel.toggleDarkTheme(!uiState.isDarkTheme) },
                        contentAlignment = Alignment.Center
                    ) {
                        Icon(
                            imageVector = if (uiState.isDarkTheme) Icons.Default.LightMode else Icons.Default.DarkMode,
                            contentDescription = "Toggle Appearance",
                            tint = MaterialTheme.colorScheme.onSurface,
                            modifier = Modifier.size(18.dp)
                        )
                    }

                    Spacer(modifier = Modifier.width(8.dp))

                    // Circular Glass Search Button (.ib)
                    Box(
                        modifier = Modifier
                            .size(38.dp)
                            .clip(CircleShape)
                            .background(cardBg)
                            .border(1.dp, strokeColor, CircleShape)
                            .clickable { isSearchVisible = !isSearchVisible },
                        contentAlignment = Alignment.Center
                    ) {
                        Icon(
                            imageVector = if (isSearchVisible) Icons.Default.Close else Icons.Default.Search,
                            contentDescription = "Search Downloads",
                            tint = MaterialTheme.colorScheme.onSurface,
                            modifier = Modifier.size(18.dp)
                        )
                    }
                }

                // Glass Summary Card (.sum: Speed & Active)
                val sumCardShape = RoundedCornerShape(22.dp)
                Box(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(horizontal = 16.dp, vertical = 4.dp)
                        .clip(sumCardShape)
                        .background(cardBg)
                        .border(1.dp, strokeColor, sumCardShape)
                        .padding(horizontal = 18.dp, vertical = 12.dp)
                ) {
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.SpaceBetween,
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        Column {
                            Text(
                                text = "Speed",
                                fontSize = 11.5.sp,
                                color = MaterialTheme.colorScheme.onSurfaceVariant
                            )
                            Text(
                                text = uiState.totalSpeedFormatted,
                                fontSize = 23.sp,
                                fontWeight = FontWeight.Bold,
                                color = MaterialTheme.colorScheme.onSurface
                            )
                        }

                        Column(horizontalAlignment = Alignment.End) {
                            Text(
                                text = "Active",
                                fontSize = 11.5.sp,
                                color = MaterialTheme.colorScheme.onSurfaceVariant
                            )
                            Text(
                                text = "${uiState.activeDownloadsCount}",
                                fontSize = 23.sp,
                                fontWeight = FontWeight.Bold,
                                color = MaterialTheme.colorScheme.onSurface
                            )
                        }
                    }
                }

                // Search Input (When visible)
                if (isSearchVisible) {
                    OutlinedTextField(
                        value = uiState.searchQuery,
                        onValueChange = { viewModel.setSearchQuery(it) },
                        placeholder = { Text("Search downloads...") },
                        leadingIcon = {
                            Icon(
                                imageVector = Icons.Default.Search,
                                contentDescription = null,
                                tint = MaterialTheme.colorScheme.onSurfaceVariant
                            )
                        },
                        trailingIcon = {
                            if (uiState.searchQuery.isNotEmpty()) {
                                IconButton(onClick = { viewModel.setSearchQuery("") }) {
                                    Icon(
                                        imageVector = Icons.Default.Close,
                                        contentDescription = "Clear",
                                        tint = MaterialTheme.colorScheme.onSurfaceVariant
                                    )
                                }
                            }
                        },
                        singleLine = true,
                        shape = RoundedCornerShape(14.dp),
                        colors = OutlinedTextFieldDefaults.colors(
                            focusedBorderColor = PrimaryBlue,
                            unfocusedBorderColor = strokeColor,
                            focusedContainerColor = cardBg,
                            unfocusedContainerColor = cardBg
                        ),
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(horizontal = 16.dp, vertical = 4.dp)
                    )
                }

                // Status Filter Chips (.chips: All, Downloading, Finished, Paused, Error)
                StatusFilterChipsRow(
                    selectedStatus = uiState.selectedStatus,
                    onSelectStatus = { viewModel.setStatusFilter(it) },
                    isDark = isDark
                )

                // Category Filter Row
                CategoryFilterRow(
                    selectedCategory = uiState.selectedCategory,
                    onSelectCategory = { viewModel.setCategory(it) },
                    isDark = isDark
                )

                // Downloads List or Empty State (.list / .empty)
                if (uiState.downloads.isEmpty()) {
                    Box(
                        modifier = Modifier
                            .fillMaxWidth()
                            .weight(1f)
                            .padding(24.dp),
                        contentAlignment = Alignment.Center
                    ) {
                        Column(
                            horizontalAlignment = Alignment.CenterHorizontally,
                            verticalArrangement = Arrangement.Center
                        ) {
                            Text(
                                text = "Nothing here",
                                fontSize = 16.sp,
                                fontWeight = FontWeight.Bold,
                                color = MaterialTheme.colorScheme.onSurface
                            )
                            Spacer(modifier = Modifier.height(4.dp))
                            Text(
                                text = "Tap Add URL to start a download.",
                                fontSize = 13.sp,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                                textAlign = TextAlign.Center
                            )
                        }
                    }
                } else {
                    LazyColumn(
                        modifier = Modifier
                            .fillMaxWidth()
                            .weight(1f),
                        contentPadding = PaddingValues(horizontal = 16.dp, vertical = 6.dp),
                        verticalArrangement = Arrangement.spacedBy(10.dp)
                    ) {
                        items(uiState.downloads, key = { it.id }) { item ->
                            DownloadItemCard(
                                item = item,
                                onClick = { viewModel.selectItem(item) },
                                onPause = { viewModel.pauseDownload(item.id) },
                                onResume = { viewModel.resumeDownload(item.id) },
                                onDelete = { viewModel.deleteDownload(item.id, deleteFile = false) },
                                isDark = isDark
                            )
                        }
                    }
                }
            }

            // Floating Action Button (.fab: ＋ Add URL pill with blue gradient)
            val fabShape = RoundedCornerShape(18.dp)
            Box(
                modifier = Modifier
                    .align(Alignment.BottomEnd)
                    .padding(end = 16.dp, bottom = 84.dp)
                    .shadow(elevation = 12.dp, shape = fabShape, ambientColor = PrimaryBlue, spotColor = PrimaryBlue)
                    .clip(fabShape)
                    .background(
                        Brush.verticalGradient(
                            listOf(GlassAccentGradientStart, GlassAccentGradientEnd)
                        )
                    )
                    .clickable { viewModel.openAddDialog() }
                    .padding(horizontal = 18.dp, vertical = 13.dp),
                contentAlignment = Alignment.Center
            ) {
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(6.dp)
                ) {
                    Icon(
                        imageVector = Icons.Default.Add,
                        contentDescription = "Add Download",
                        tint = Color.White,
                        modifier = Modifier.size(18.dp)
                    )
                    Text(
                        text = "Add URL",
                        fontSize = 14.sp,
                        fontWeight = FontWeight.Bold,
                        color = Color.White
                    )
                }
            }

            // Floating Glass Bottom Navigation Dock (.nav)
            val dockShape = RoundedCornerShape(26.dp)
            Box(
                modifier = Modifier
                    .align(Alignment.BottomCenter)
                    .fillMaxWidth()
                    .padding(start = 12.dp, end = 12.dp, bottom = 10.dp)
                    .height(64.dp)
                    .clip(dockShape)
                    .background(cardBg)
                    .border(1.dp, strokeColor, dockShape)
                    .padding(horizontal = 6.dp)
            ) {
                Row(
                    modifier = Modifier.fillMaxSize(),
                    horizontalArrangement = Arrangement.SpaceAround,
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    // Downloads Tab
                    GlassDockTab(
                        icon = "⬇",
                        label = "Downloads",
                        isSelected = uiState.currentTab == "downloads",
                        onClick = { viewModel.setTab("downloads") }
                    )

                    // Grabber Tab
                    GlassDockTab(
                        icon = "✦",
                        label = "Grabber",
                        isSelected = uiState.currentTab == "grabber",
                        onClick = {
                            viewModel.setTab("grabber")
                            viewModel.openGrabberDialog()
                        }
                    )

                    // Browser Tab
                    GlassDockTab(
                        icon = "🌐",
                        label = "Browser",
                        isSelected = uiState.currentTab == "browser",
                        onClick = {
                            viewModel.setTab("browser")
                            Toast.makeText(context, "Browser Bridge active on port 9666", Toast.LENGTH_SHORT).show()
                        }
                    )

                    // Settings Tab
                    GlassDockTab(
                        icon = "⚙",
                        label = "Settings",
                        isSelected = uiState.currentTab == "settings",
                        onClick = {
                            viewModel.setTab("settings")
                            viewModel.openSettingsDialog()
                        }
                    )
                }
            }
        }

        // Add Download Dialog
        if (uiState.isAddDialogOpen) {
            AddDownloadDialog(
                initialUrl = uiState.initialAddUrl,
                onDismiss = { viewModel.closeAddDialog() },
                onStartDownload = { url, filename, connections ->
                    viewModel.startDownload(url, filename, connections)
                }
            )
        }

        // Media Grabber Dialog
        if (uiState.isGrabberDialogOpen) {
            MediaGrabberDialog(
                initialUrl = uiState.initialAddUrl,
                onDismiss = { viewModel.closeGrabberDialog() },
                onStartDownload = { url, filename, connections ->
                    viewModel.startDownload(url, filename, connections)
                }
            )
        }

        // Settings & Engines Dialog
        if (uiState.isSettingsDialogOpen) {
            SettingsDialog(
                isDarkTheme = uiState.isDarkTheme,
                defaultConnections = uiState.defaultConnections,
                ytdlpVersion = uiState.ytdlpVersion,
                ffmpegVersion = uiState.ffmpegVersion,
                isUpdating = uiState.isUpdatingEngine,
                updateMessage = uiState.engineUpdateMessage,
                onToggleDarkTheme = { viewModel.toggleDarkTheme(it) },
                onUpdateConnections = { viewModel.updateDefaultConnections(it) },
                onUpdateYtdlp = { viewModel.updateYtdlpEngine() },
                onUpdateFfmpeg = { viewModel.updateFfmpegEngine() },
                onDismiss = { viewModel.closeSettingsDialog() }
            )
        }

        // Download Details Dialog
        uiState.selectedItem?.let { selected ->
            DownloadDetailsDialog(
                item = selected,
                onDismiss = { viewModel.selectItem(null) },
                onPause = { viewModel.pauseDownload(selected.id) },
                onResume = { viewModel.resumeDownload(selected.id) },
                onDelete = { viewModel.deleteDownload(selected.id, deleteFile = false) }
            )
        }
    }
}

@Composable
private fun GlassDockTab(
    icon: String,
    label: String,
    isSelected: Boolean,
    onClick: () -> Unit
) {
    val pillShape = RoundedCornerShape(15.dp)

    Column(
        modifier = Modifier
            .clickable(onClick = onClick)
            .padding(horizontal = 8.dp, vertical = 4.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.Center
    ) {
        Box(
            modifier = Modifier
                .size(width = 54.dp, height = 28.dp)
                .clip(pillShape)
                .background(if (isSelected) PrimaryBlue else Color.Transparent),
            contentAlignment = Alignment.Center
        ) {
            Text(
                text = icon,
                fontSize = 16.sp,
                color = if (isSelected) Color.White else MaterialTheme.colorScheme.onSurfaceVariant
            )
        }
        Spacer(modifier = Modifier.height(2.dp))
        Text(
            text = label,
            fontSize = 11.sp,
            fontWeight = if (isSelected) FontWeight.SemiBold else FontWeight.Normal,
            color = if (isSelected) MaterialTheme.colorScheme.onSurface else MaterialTheme.colorScheme.onSurfaceVariant
        )
    }
}

