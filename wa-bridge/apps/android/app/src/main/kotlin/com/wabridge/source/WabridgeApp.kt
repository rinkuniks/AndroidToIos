package com.wabridge.source

import android.app.Application
import android.util.Log
import androidx.lifecycle.ProcessLifecycleOwner
import androidx.lifecycle.lifecycleScope
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

class WabridgeApp : Application() {
    val transferState: StateFlow<TransferState> = MutableStateFlow(TransferState.Idle)
        .asStateFlow()

    private val _uiState = MutableStateFlow(UiState())
    val uiState: StateFlow<UiState> = _uiState.asStateFlow()

    override fun onCreate() {
        super.onCreate()
        Log.d("WABridgeApp", "Application created")
    }
}

data class UiState(
    val currentScreen: Screen = Screen.Welcome,
    val connectionStatus: ConnectionStatus = ConnectionStatus.Disconnected,
    val vault: Vault? = null,
    val transfer: TransferProgress? = null,
)

enum class Screen {
    Welcome, Direction, DestinationStatus, Connect, ReadyCheck, Transfer, VaultComplete
}

enum class ConnectionStatus {
    Disconnected, Connecting, Connected, Verified
}

data class TransferState(
    val isActive: Boolean = false,
    val progress: Float = 0f,
    val bytesTransferred: Long = 0L,
    val bytesTotal: Long = 0L,
    val speedMbps: Float = 0f,
    val status: String = "",
    val reconnectCount: Int = 0,
)

object TransferState {
    val Idle = TransferState(isActive = false, status = "Idle")
    val Scanning = TransferState(isActive = true, status = "Scanning for devices")
    val Connecting = TransferState(isActive = true, status = "Connecting")
    val Ready = TransferState(isActive = false, status = "Ready")
    val Transferring = TransferState(isActive = true, status = "Transferring")
    val Verifying = TransferState(isActive = true, status = "Verifying")
    val Complete = TransferState(isActive = false, status = "Complete")
}

data class Vault(
    val itemCount: Int,
    val totalSizeBytes: Long,
    val isScanning: Boolean = false,
    val scannedItemCount: Int = 0,
)

data class TransferProgress(
    val current: Long,
    val total: Long,
    val speedMbps: Float,
    val percent: Float,
    val state: String, // "transferring", "verifying", "complete"
)
