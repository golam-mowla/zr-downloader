package com.golammowla.gdm.presentation.viewmodel

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.golammowla.gdm.data.network.DownloadEngine
import com.golammowla.gdm.data.preferences.UserPreferencesRepository
import com.golammowla.gdm.domain.model.DownloadCategory
import com.golammowla.gdm.domain.model.DownloadItem
import com.golammowla.gdm.domain.model.DownloadStatus
import com.golammowla.gdm.domain.repository.DownloadRepository
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import javax.inject.Inject

data class DownloadUiState(
    val downloads: List<DownloadItem> = emptyList(),
    val selectedCategory: DownloadCategory = DownloadCategory.ALL,
    val searchQuery: String = "",
    val activeDownloadsCount: Int = 0,
    val totalSpeedFormatted: String = "0 B/s",
    val selectedItem: DownloadItem? = null,
    val isAddDialogOpen: Boolean = false,
    val initialAddUrl: String = "",
    val isDarkTheme: Boolean = true
)

private data class FilterResult(
    val all: List<DownloadItem>,
    val filtered: List<DownloadItem>,
    val category: DownloadCategory,
    val query: String,
    val totalSpeedBps: Long
)

@HiltViewModel
class DownloadViewModel @Inject constructor(
    private val repository: DownloadRepository,
    private val preferencesRepository: UserPreferencesRepository
) : ViewModel() {

    private val _selectedCategory = MutableStateFlow(DownloadCategory.ALL)
    private val _searchQuery = MutableStateFlow("")
    private val _selectedItem = MutableStateFlow<DownloadItem?>(null)
    private val _isAddDialogOpen = MutableStateFlow(false)
    private val _initialAddUrl = MutableStateFlow("")

    private val filterFlow = combine(
        repository.getAllDownloads(),
        _selectedCategory,
        _searchQuery
    ) { downloads, category, query ->
        val filtered = downloads.filter { item ->
            (category == DownloadCategory.ALL || item.category == category) &&
            (query.isBlank() || item.filename.contains(query, ignoreCase = true))
        }
        val active = downloads.filter { it.status == DownloadStatus.DOWNLOADING }
        val speedBps = active.sumOf { it.speedBps }
        FilterResult(
            all = downloads,
            filtered = filtered,
            category = category,
            query = query,
            totalSpeedBps = speedBps
        )
    }

    private val dialogFlow = combine(
        _isAddDialogOpen,
        _initialAddUrl
    ) { isOpen, url ->
        Pair(isOpen, url)
    }

    val uiState: StateFlow<DownloadUiState> = combine(
        filterFlow,
        _selectedItem,
        dialogFlow,
        preferencesRepository.preferencesFlow
    ) { filterResult, selectedItem, (isOpen, initialUrl), prefs ->
        val activeCount = filterResult.all.count { it.status == DownloadStatus.DOWNLOADING }
        val resolvedSelected = selectedItem?.let { sel ->
            filterResult.all.find { it.id == sel.id } ?: sel
        }

        DownloadUiState(
            downloads = filterResult.filtered,
            selectedCategory = filterResult.category,
            searchQuery = filterResult.query,
            activeDownloadsCount = activeCount,
            totalSpeedFormatted = DownloadEngine.formatSpeed(filterResult.totalSpeedBps),
            selectedItem = resolvedSelected,
            isAddDialogOpen = isOpen,
            initialAddUrl = initialUrl,
            isDarkTheme = prefs.isDarkTheme
        )
    }.stateIn(
        scope = viewModelScope,
        started = SharingStarted.WhileSubscribed(5000),
        initialValue = DownloadUiState()
    )

    fun setCategory(category: DownloadCategory) {
        _selectedCategory.value = category
    }

    fun setSearchQuery(query: String) {
        _searchQuery.value = query
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
}
