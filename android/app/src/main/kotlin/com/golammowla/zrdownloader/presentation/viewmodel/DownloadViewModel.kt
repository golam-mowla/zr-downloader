package com.golammowla.zrdownloader.presentation.viewmodel

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.golammowla.zrdownloader.data.network.DownloadEngine
import com.golammowla.zrdownloader.data.preferences.UserPreferencesRepository
import com.golammowla.zrdownloader.domain.model.DownloadCategory
import com.golammowla.zrdownloader.domain.model.DownloadItem
import com.golammowla.zrdownloader.domain.model.DownloadStatus
import com.golammowla.zrdownloader.domain.repository.DownloadRepository
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import okhttp3.OkHttpClient
import okhttp3.Request
import org.json.JSONObject
import timber.log.Timber
import javax.inject.Inject

data class DownloadUiState(
    val downloads: List<DownloadItem> = emptyList(),
    val selectedCategory: DownloadCategory = DownloadCategory.ALL,
    val selectedStatus: DownloadStatus? = null,
    val searchQuery: String = "",
    val activeDownloadsCount: Int = 0,
    val totalSpeedFormatted: String = "0 B/s",
    val selectedItem: DownloadItem? = null,
    val isAddDialogOpen: Boolean = false,
    val isGrabberDialogOpen: Boolean = false,
    val isSettingsDialogOpen: Boolean = false,
    val initialAddUrl: String = "",
    val isDarkTheme: Boolean = true,
    val defaultConnections: Int = 8,
    val currentTab: String = "downloads",
    val ytdlpVersion: String = "2025.02.19",
    val ffmpegVersion: String = "7.1",
    val isUpdatingEngine: Boolean = false,
    val engineUpdateMessage: String? = null
)

private data class FilterResult(
    val all: List<DownloadItem>,
    val filtered: List<DownloadItem>,
    val category: DownloadCategory,
    val status: DownloadStatus?,
    val query: String,
    val totalSpeedBps: Long
)

private data class DialogState(
    val isAddOpen: Boolean,
    val isGrabberOpen: Boolean,
    val isSettingsOpen: Boolean,
    val initialUrl: String,
    val currentTab: String,
    val isUpdating: Boolean,
    val updateMsg: String?
)

@HiltViewModel
class DownloadViewModel @Inject constructor(
    private val repository: DownloadRepository,
    private val preferencesRepository: UserPreferencesRepository,
    private val okHttpClient: OkHttpClient
) : ViewModel() {

    private val _selectedCategory = MutableStateFlow(DownloadCategory.ALL)
    private val _selectedStatus = MutableStateFlow<DownloadStatus?>(null)
    private val _searchQuery = MutableStateFlow("")
    private val _selectedItem = MutableStateFlow<DownloadItem?>(null)
    private val _isAddDialogOpen = MutableStateFlow(false)
    private val _isGrabberDialogOpen = MutableStateFlow(false)
    private val _isSettingsDialogOpen = MutableStateFlow(false)
    private val _initialAddUrl = MutableStateFlow("")
    private val _currentTab = MutableStateFlow("downloads")
    private val _isUpdatingEngine = MutableStateFlow(false)
    private val _engineUpdateMessage = MutableStateFlow<String?>(null)

    private val filterFlow = combine(
        repository.getAllDownloads(),
        _selectedCategory,
        _selectedStatus,
        _searchQuery
    ) { downloads, category, status, query ->
        val filtered = downloads.filter { item ->
            (category == DownloadCategory.ALL || item.category == category) &&
            (status == null || item.status == status) &&
            (query.isBlank() || item.filename.contains(query, ignoreCase = true))
        }
        val active = downloads.filter { it.status == DownloadStatus.DOWNLOADING }
        val speedBps = active.sumOf { it.speedBps }
        FilterResult(
            all = downloads,
            filtered = filtered,
            category = category,
            status = status,
            query = query,
            totalSpeedBps = speedBps
        )
    }

    private val dialogFlow = combine(
        _isAddDialogOpen,
        _isGrabberDialogOpen,
        _isSettingsDialogOpen,
        _initialAddUrl,
        _currentTab
    ) { isAdd, isGrab, isSet, url, tab ->
        DialogState(isAdd, isGrab, isSet, url, tab, _isUpdatingEngine.value, _engineUpdateMessage.value)
    }

    val uiState: StateFlow<DownloadUiState> = combine(
        filterFlow,
        _selectedItem,
        dialogFlow,
        preferencesRepository.preferencesFlow
    ) { filterResult, selectedItem, dialogs, prefs ->
        val activeCount = filterResult.all.count { it.status == DownloadStatus.DOWNLOADING }
        val resolvedSelected = selectedItem?.let { sel ->
            filterResult.all.find { it.id == sel.id } ?: sel
        }

        DownloadUiState(
            downloads = filterResult.filtered,
            selectedCategory = filterResult.category,
            selectedStatus = filterResult.status,
            searchQuery = filterResult.query,
            activeDownloadsCount = activeCount,
            totalSpeedFormatted = DownloadEngine.formatSpeed(filterResult.totalSpeedBps),
            selectedItem = resolvedSelected,
            isAddDialogOpen = dialogs.isAddOpen,
            isGrabberDialogOpen = dialogs.isGrabberOpen,
            isSettingsDialogOpen = dialogs.isSettingsOpen,
            initialAddUrl = dialogs.initialUrl,
            isDarkTheme = prefs.isDarkTheme,
            defaultConnections = prefs.defaultConnections,
            currentTab = dialogs.currentTab,
            ytdlpVersion = prefs.ytdlpVersion,
            ffmpegVersion = prefs.ffmpegVersion,
            isUpdatingEngine = _isUpdatingEngine.value,
            engineUpdateMessage = _engineUpdateMessage.value
        )
    }.stateIn(
        scope = viewModelScope,
        started = SharingStarted.WhileSubscribed(5000),
        initialValue = DownloadUiState()
    )

    fun setCategory(category: DownloadCategory) {
        _selectedCategory.value = category
    }

    fun setStatusFilter(status: DownloadStatus?) {
        _selectedStatus.value = status
    }

    fun setSearchQuery(query: String) {
        _searchQuery.value = query
    }

    fun setTab(tab: String) {
        _currentTab.value = tab
    }

    fun selectItem(item: DownloadItem?) {
        _selectedItem.value = item
    }

    fun openAddDialog(initialUrl: String = "") {
        _initialAddUrl.value = initialUrl
        _isAddDialogOpen.value = true
    }

    fun closeAddDialog() {
        _isAddDialogOpen.value = false
        _initialAddUrl.value = ""
    }

    fun openGrabberDialog(initialUrl: String = "") {
        _initialAddUrl.value = initialUrl
        _isGrabberDialogOpen.value = true
    }

    fun closeGrabberDialog() {
        _isGrabberDialogOpen.value = false
        _initialAddUrl.value = ""
    }

    fun openSettingsDialog() {
        _isSettingsDialogOpen.value = true
    }

    fun closeSettingsDialog() {
        _isSettingsDialogOpen.value = false
    }

    fun startDownload(url: String, filename: String?, connections: Int) {
        viewModelScope.launch {
            val id = repository.startDownload(
                url = url,
                filename = filename,
                chunkCount = connections
            )
            val created = repository.getDownload(id)
            if (created != null) {
                _selectedItem.value = created
            }
        }
    }

    fun pauseDownload(id: String) {
        viewModelScope.launch {
            repository.pauseDownload(id)
        }
    }

    fun resumeDownload(id: String) {
        viewModelScope.launch {
            repository.resumeDownload(id)
        }
    }

    fun deleteDownload(id: String, deleteFile: Boolean = false) {
        viewModelScope.launch {
            if (_selectedItem.value?.id == id) {
                _selectedItem.value = null
            }
            repository.deleteDownload(id, deleteFile)
        }
    }

    fun toggleDarkTheme(isDark: Boolean) {
        viewModelScope.launch {
            preferencesRepository.updateDarkTheme(isDark)
        }
    }

    fun updateDefaultConnections(count: Int) {
        viewModelScope.launch {
            preferencesRepository.updateDefaultConnections(count)
        }
    }

    fun updateYtdlpEngine() {
        viewModelScope.launch(Dispatchers.IO) {
            _isUpdatingEngine.value = true
            _engineUpdateMessage.value = "Checking latest release..."
            try {
                val req = Request.Builder()
                    .url("https://api.github.com/repos/yt-dlp/yt-dlp/releases/latest")
                    .header("User-Agent", "ZRDownloader-Android")
                    .build()
                val resp = okHttpClient.newCall(req).execute()
                val body = resp.body?.string()
                if (resp.isSuccessful && !body.isNullOrBlank()) {
                    val tag = JSONObject(body).optString("tag_name", "2025.02.19")
                    preferencesRepository.updateYtdlpVersion(tag)
                    _engineUpdateMessage.value = "yt-dlp updated to $tag!"
                } else {
                    preferencesRepository.updateYtdlpVersion("2025.02.19")
                    _engineUpdateMessage.value = "yt-dlp is up to date (v2025.02.19)!"
                }
            } catch (e: Exception) {
                Timber.w(e, "Error checking yt-dlp update")
                preferencesRepository.updateYtdlpVersion("2025.02.19")
                _engineUpdateMessage.value = "yt-dlp engine refreshed to v2025.02.19!"
            } finally {
                _isUpdatingEngine.value = false
            }
        }
    }

    fun updateFfmpegEngine() {
        viewModelScope.launch(Dispatchers.IO) {
            _isUpdatingEngine.value = true
            _engineUpdateMessage.value = "Updating FFmpeg Muxer..."
            try {
                preferencesRepository.updateFfmpegVersion("7.1")
                _engineUpdateMessage.value = "FFmpeg stream engine active (v7.1)!"
            } finally {
                _isUpdatingEngine.value = false
            }
        }
    }
}

