package com.golammowla.gdm

import android.content.Intent
import android.net.Uri
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.viewModels
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import com.golammowla.gdm.presentation.screens.MainScreen
import com.golammowla.gdm.presentation.theme.GdmTheme
import com.golammowla.gdm.presentation.viewmodel.DownloadViewModel
import dagger.hilt.android.AndroidEntryPoint
import timber.log.Timber

@AndroidEntryPoint
class MainActivity : ComponentActivity() {

    private val viewModel: DownloadViewModel by viewModels()

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        handleIncomingIntent(intent)

        setContent {
            val uiState by viewModel.uiState.collectAsState()
            GdmTheme(darkTheme = uiState.isDarkTheme) {
                Surface(
                    modifier = Modifier.fillMaxSize(),
                    color = MaterialTheme.colorScheme.background
                ) {
                    MainScreen(viewModel = viewModel)
                }
            }
        }
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        setIntent(intent)
        handleIncomingIntent(intent)
    }

    private fun handleIncomingIntent(intent: Intent?) {
        if (intent == null) return

        val action = intent.action
        Timber.d("Handling incoming intent with action: $action")

        var targetUrl: String? = null

        when (action) {
            Intent.ACTION_VIEW -> {
                val data: Uri? = intent.data
                if (data != null) {
                    targetUrl = data.toString()
                }
            }
            Intent.ACTION_SEND -> {
                if (intent.type == "text/plain") {
                    val sharedText = intent.getStringExtra(Intent.EXTRA_TEXT)
                    if (!sharedText.isNullOrBlank()) {
                        // Extract URL if surrounded by extra text
                        val urlPattern = "(https?://[^\\s]+|magnet:[^\\s]+)".toRegex()
                        val match = urlPattern.find(sharedText)
                        targetUrl = match?.value ?: sharedText.trim()
                    }
                }
            }
        }

        if (!targetUrl.isNullOrBlank()) {
            val cleanUrl = targetUrl.trim().removeSurrounding("\"", "\"").removeSurrounding("'", "'")
            Timber.i("Intercepted download URL from external browser/app: $cleanUrl")
            viewModel.openAddDialog(cleanUrl)
        }
    }
}
