package com.golammowla.zrdownloader.presentation.components

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.AutoAwesome
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.Download
import androidx.compose.material.icons.filled.Movie
import androidx.compose.material.icons.filled.MusicNote
import androidx.compose.material.icons.filled.Search
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedTextFieldDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import coil.compose.AsyncImage
import com.golammowla.zrdownloader.presentation.theme.GlassAccentGradientEnd
import com.golammowla.zrdownloader.presentation.theme.GlassAccentGradientStart
import com.golammowla.zrdownloader.presentation.theme.PrimaryBlue
import com.yausername.youtubedl_android.YoutubeDL
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import timber.log.Timber

data class GrabberFormat(
    val id: String,
    val label: String,
    val ext: String,
    val isAudio: Boolean,
    val streamUrl: String? = null,
    val sizeText: String? = null
)

@Composable
fun MediaGrabberDialog(
    initialUrl: String = "",
    onDismiss: () -> Unit,
    onStartDownload: (url: String, filename: String?, connections: Int) -> Unit
) {
    val coroutineScope = rememberCoroutineScope()
    var urlText by remember { mutableStateOf(initialUrl) }

    val defaultFormats = remember {
        listOf(
            GrabberFormat("best", "Best Available Quality", "mp4", false),
            GrabberFormat("1080p", "1080p Full HD", "mp4", false),
            GrabberFormat("720p", "720p HD", "mp4", false),
            GrabberFormat("480p", "480p SD", "mp4", false),
            GrabberFormat("bestaudio", "MP3 Audio (High Bitrate)", "mp3", true)
        )
    }

    var formatsList by remember { mutableStateOf(defaultFormats) }
    var selectedFormat by remember { mutableStateOf(defaultFormats[0]) }

    var isInspecting by remember { mutableStateOf(false) }
    var inspectedTitle by remember { mutableStateOf<String?>(null) }
    var inspectedUploader by remember { mutableStateOf<String?>(null) }
    var inspectedThumbnail by remember { mutableStateOf<String?>(null) }
    var inspectedDuration by remember { mutableStateOf<String?>(null) }
    var errorMessage by remember { mutableStateOf<String?>(null) }

    fun inspectCurrentUrl(targetUrl: String) {
        val clean = targetUrl.trim()
        if (clean.isBlank()) return
        coroutineScope.launch {
            isInspecting = true
            errorMessage = null
            try {
                val info = withContext(Dispatchers.IO) {
                    YoutubeDL.getInstance().getInfo(clean)
                }

                inspectedTitle = info.title
                inspectedUploader = info.uploader
                inspectedThumbnail = info.thumbnail
                val durSec = info.duration
                if (durSec > 0) {
                    val m = durSec / 60
                    val s = durSec % 60
                    inspectedDuration = "$m:${s.toString().padStart(2, '0')}"
                }

                val raw = info.formats
                if (!raw.isNullOrEmpty()) {
                    val parsed = raw.mapNotNull { f ->
                        val fid = f.formatId ?: return@mapNotNull null
                        val ext = f.ext ?: "mp4"
                        val h = f.height
                        val fps = f.fps
                        val vcodec = f.vcodec
                        val acodec = f.acodec
                        val isAudioOnly = (vcodec == "none" || vcodec.isNullOrEmpty()) && !acodec.isNullOrEmpty() && acodec != "none"
                        val isVideoOnly = (acodec == "none" || acodec.isNullOrEmpty()) && !vcodec.isNullOrEmpty() && vcodec != "none"

                        val sizeMb = if (f.fileSize > 0) String.format("%.1f MB", f.fileSize / (1024.0 * 1024.0)) else null

                        val label = when {
                            isAudioOnly -> {
                                val abr = f.abr
                                val bitr = if (abr > 0) "${abr}k" else "Audio"
                                "Audio • $bitr • ${ext.uppercase()}"
                            }
                            h > 0 -> {
                                val fpsTag = if (fps > 30) "${fps}fps" else ""
                                val codecTag = vcodec?.substringBefore(".") ?: ""
                                "${h}p $fpsTag • ${ext.uppercase()} ${if (isVideoOnly) "(Video)" else ""}".trim()
                            }
                            else -> f.format ?: fid
                        }

                        GrabberFormat(
                            id = fid,
                            label = label,
                            ext = ext,
                            isAudio = isAudioOnly,
                            streamUrl = f.url,
                            sizeText = sizeMb
                        )
                    }

                    // Sort: video with highest resolution first, then audio
                    val sorted = parsed.distinctBy { it.label }
                    if (sorted.isNotEmpty()) {
                        formatsList = sorted
                        selectedFormat = sorted.first()
                    }
                }
            } catch (e: Throwable) {
                Timber.w(e, "Inspect error: ${e.message}")
                errorMessage = "Inspect note: Using best streams (${e.message?.take(80) ?: "Direct link mode"})"
            } finally {
                isInspecting = false
            }
        }
    }

    LaunchedEffect(Unit) {
        if (initialUrl.isNotBlank()) {
            inspectCurrentUrl(initialUrl)
        }
    }

    Dialog(onDismissRequest = onDismiss) {
        Card(
            modifier = Modifier
                .fillMaxWidth()
                .padding(vertical = 12.dp),
            shape = RoundedCornerShape(24.dp),
            colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surface),
            elevation = CardDefaults.cardElevation(defaultElevation = 8.dp)
        ) {
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(20.dp)
            ) {
                // Header Row
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.SpaceBetween,
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Box(
                            modifier = Modifier
                                .size(36.dp)
                                .clip(RoundedCornerShape(10.dp))
                                .background(
                                    Brush.verticalGradient(
                                        listOf(GlassAccentGradientStart, GlassAccentGradientEnd)
                                    )
                                ),
                            contentAlignment = Alignment.Center
                        ) {
                            Icon(
                                imageVector = Icons.Default.AutoAwesome,
                                contentDescription = null,
                                tint = Color.Yellow,
                                modifier = Modifier.size(20.dp)
                            )
                        }
                        Spacer(modifier = Modifier.width(10.dp))
                        Column {
                            Text(
                                text = "Media Grabber",
                                fontSize = 17.sp,
                                fontWeight = FontWeight.Bold,
                                color = MaterialTheme.colorScheme.onSurface
                            )
                            Text(
                                text = "Dynamic Inspection & yt-dlp 16-Stream",
                                fontSize = 11.5.sp,
                                color = MaterialTheme.colorScheme.onSurfaceVariant
                            )
                        }
                    }

                    IconButton(onClick = onDismiss, modifier = Modifier.size(32.dp)) {
                        Icon(
                            imageVector = Icons.Default.Close,
                            contentDescription = "Close",
                            tint = MaterialTheme.colorScheme.onSurfaceVariant
                        )
                    }
                }

                Spacer(modifier = Modifier.height(14.dp))

                // URL Input Field + Inspect Button
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(8.dp)
                ) {
                    OutlinedTextField(
                        value = urlText,
                        onValueChange = {
                            urlText = it
                            inspectedTitle = null
                            inspectedThumbnail = null
                        },
                        label = { Text("Video or Stream Link") },
                        placeholder = { Text("YouTube, Instagram, TikTok, etc.") },
                        singleLine = true,
                        shape = RoundedCornerShape(14.dp),
                        colors = OutlinedTextFieldDefaults.colors(
                            focusedBorderColor = PrimaryBlue,
                            focusedLabelColor = PrimaryBlue
                        ),
                        modifier = Modifier.weight(1f)
                    )

                    Button(
                        onClick = { inspectCurrentUrl(urlText) },
                        enabled = urlText.isNotBlank() && !isInspecting,
                        shape = RoundedCornerShape(14.dp),
                        colors = ButtonDefaults.buttonColors(
                            containerColor = Color(0xFF7C3AED),
                            contentColor = Color.White
                        ),
                        modifier = Modifier.height(54.dp)
                    ) {
                        if (isInspecting) {
                            CircularProgressIndicator(
                                modifier = Modifier.size(18.dp),
                                color = Color.White,
                                strokeWidth = 2.dp
                            )
                        } else {
                            Icon(
                                imageVector = Icons.Default.Search,
                                contentDescription = "Inspect",
                                modifier = Modifier.size(18.dp)
                            )
                            Spacer(modifier = Modifier.width(4.dp))
                            Text("Inspect", fontSize = 12.5.sp, fontWeight = FontWeight.Bold)
                        }
                    }
                }

                // Inspected Media Card (Thumbnail, Title, Uploader)
                if (inspectedTitle != null || inspectedThumbnail != null) {
                    Spacer(modifier = Modifier.height(12.dp))
                    Row(
                        modifier = Modifier
                            .fillMaxWidth()
                            .clip(RoundedCornerShape(14.dp))
                            .background(MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.45f))
                            .padding(10.dp),
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        if (!inspectedThumbnail.isNullOrBlank()) {
                            AsyncImage(
                                model = inspectedThumbnail,
                                contentDescription = "Thumbnail",
                                contentScale = ContentScale.Crop,
                                modifier = Modifier
                                    .size(width = 72.dp, height = 48.dp)
                                    .clip(RoundedCornerShape(8.dp))
                            )
                            Spacer(modifier = Modifier.width(10.dp))
                        }

                        Column(modifier = Modifier.weight(1f)) {
                            Text(
                                text = inspectedTitle ?: "Online Media",
                                fontSize = 13.sp,
                                fontWeight = FontWeight.SemiBold,
                                color = MaterialTheme.colorScheme.onSurface,
                                maxLines = 2,
                                overflow = TextOverflow.Ellipsis
                            )
                            Row(
                                modifier = Modifier.padding(top = 2.dp),
                                horizontalArrangement = Arrangement.spacedBy(8.dp)
                            ) {
                                if (!inspectedUploader.isNullOrBlank()) {
                                    Text(
                                        text = inspectedUploader ?: "",
                                        fontSize = 11.sp,
                                        color = MaterialTheme.colorScheme.onSurfaceVariant
                                    )
                                }
                                if (!inspectedDuration.isNullOrBlank()) {
                                    Text(
                                        text = "• ${inspectedDuration}",
                                        fontSize = 11.sp,
                                        color = PrimaryBlue,
                                        fontWeight = FontWeight.Medium
                                    )
                                }
                            }
                        }
                    }
                }

                if (errorMessage != null) {
                    Spacer(modifier = Modifier.height(6.dp))
                    Text(
                        text = errorMessage ?: "",
                        fontSize = 11.sp,
                        color = MaterialTheme.colorScheme.error,
                        maxLines = 2
                    )
                }

                Spacer(modifier = Modifier.height(12.dp))

                Text(
                    text = "Select Stream Quality (${formatsList.size} available)",
                    fontSize = 12.5.sp,
                    fontWeight = FontWeight.SemiBold,
                    color = MaterialTheme.colorScheme.onSurface
                )

                Spacer(modifier = Modifier.height(8.dp))

                // Scrollable Format List (capped height)
                LazyColumn(
                    modifier = Modifier
                        .fillMaxWidth()
                        .height(160.dp),
                    verticalArrangement = Arrangement.spacedBy(6.dp)
                ) {
                    items(formatsList) { fmt ->
                        val isSelected = selectedFormat.id == fmt.id
                        val itemShape = RoundedCornerShape(12.dp)
                        Box(
                            modifier = Modifier
                                .fillMaxWidth()
                                .clip(itemShape)
                                .background(
                                    if (isSelected) PrimaryBlue.copy(alpha = 0.12f)
                                    else MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.4f)
                                )
                                .border(
                                    width = if (isSelected) 1.5.dp else 1.dp,
                                    color = if (isSelected) PrimaryBlue else Color.Transparent,
                                    shape = itemShape
                                )
                                .clickable { selectedFormat = fmt }
                                .padding(horizontal = 12.dp, vertical = 8.dp)
                        ) {
                            Row(
                                modifier = Modifier.fillMaxWidth(),
                                verticalAlignment = Alignment.CenterVertically,
                                horizontalArrangement = Arrangement.SpaceBetween
                            ) {
                                Row(
                                    verticalAlignment = Alignment.CenterVertically,
                                    modifier = Modifier.weight(1f)
                                ) {
                                    Icon(
                                        imageVector = if (fmt.isAudio) Icons.Default.MusicNote else Icons.Default.Movie,
                                        contentDescription = null,
                                        tint = if (isSelected) PrimaryBlue else MaterialTheme.colorScheme.onSurfaceVariant,
                                        modifier = Modifier.size(18.dp)
                                    )
                                    Spacer(modifier = Modifier.width(8.dp))
                                    Text(
                                        text = fmt.label,
                                        fontSize = 12.5.sp,
                                        fontWeight = if (isSelected) FontWeight.Bold else FontWeight.Normal,
                                        color = if (isSelected) PrimaryBlue else MaterialTheme.colorScheme.onSurface,
                                        maxLines = 1,
                                        overflow = TextOverflow.Ellipsis
                                    )
                                }

                                Row(verticalAlignment = Alignment.CenterVertically) {
                                    if (fmt.sizeText != null) {
                                        Text(
                                            text = fmt.sizeText,
                                            fontSize = 11.sp,
                                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                                            modifier = Modifier.padding(end = 6.dp)
                                        )
                                    }
                                    if (isSelected) {
                                        Icon(
                                            imageVector = Icons.Default.Check,
                                            contentDescription = null,
                                            tint = PrimaryBlue,
                                            modifier = Modifier.size(16.dp)
                                        )
                                    }
                                }
                            }
                        }
                    }
                }

                Spacer(modifier = Modifier.height(16.dp))

                // Download Button
                Button(
                    onClick = {
                        val clean = urlText.trim()
                        if (clean.isNotEmpty()) {
                            val sanitizedTitle = inspectedTitle
                                ?.replace("[^a-zA-Z0-9._-]".toRegex(), "_")
                                ?.take(80)
                            val qualityTag = if (selectedFormat.isAudio) "[Audio]" else "[${selectedFormat.id}]"
                            val targetFilename = if (!sanitizedTitle.isNullOrBlank()) {
                                "$sanitizedTitle $qualityTag.${selectedFormat.ext}"
                            } else {
                                "Media_${System.currentTimeMillis()} $qualityTag.${selectedFormat.ext}"
                            }

                            // Pass original URL so native embedded yt-dlp & FFmpeg process it
                            onStartDownload(clean, targetFilename, 16)
                            onDismiss()
                        }
                    },
                    enabled = urlText.isNotBlank(),
                    shape = RoundedCornerShape(14.dp),
                    colors = ButtonDefaults.buttonColors(
                        containerColor = PrimaryBlue,
                        contentColor = Color.White
                    ),
                    modifier = Modifier
                        .fillMaxWidth()
                        .height(48.dp)
                ) {
                    Icon(imageVector = Icons.Default.Download, contentDescription = null, modifier = Modifier.size(18.dp))
                    Spacer(modifier = Modifier.width(8.dp))
                    Text(
                        text = "Download with ZR Accelerator (16 Streams)",
                        fontWeight = FontWeight.Bold,
                        fontSize = 13.sp
                    )
                }
            }
        }
    }
}
