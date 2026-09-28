package com.wabridge.source.transfer

import android.util.Log
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import java.util.UUID

/**
 * Thin Kotlin wrapper around the Rust core library (wa-bridge-core).
 *
 * The Rust side exposes a C-ABI (cbindgen-compatible) interface that maps to:
 * - Transport trait (Wi-Fi peer discovery, connection, chunked streaming)
 * - Engine Sender/Receiver (chunked send/recv, hash verification, resume)
 * - Checkpoint store (atomic persist for resume after kill)
 * - Manifest model (object inventory)
 *
 * We load `libwabridge_jni.so` which is produced by building the Rust crate
 * with the `jni` feature and the `uniffi` or `jni` bindgen tooling.
 *
 * For Phase 2 scaffold, the native methods are declared as external and the
 * actual linking happens at runtime. The build system (cargo-ndk) packages
 * the .so into jniLibs/.
 */
class TransferController {

    private val _state = MutableStateFlow<TransferUiState>(TransferUiState.Idle)
    val state: StateFlow<TransferUiState> = _state.asStateFlow()

    /**
     * Start a new transfer session.
     *
     * @param manifestJson JSON-serialized Manifest from the Rust core
     * @param sessionKey Pre-shared key delivered via Wi-Fi / QR code
     */
    suspend fun startTransfer(manifestJson: String, sessionKey: String): Result<Unit> =
        wrapCall {
            TransferController_nativeStart(
                manifestJson = manifestJson,
                sessionKey = sessionKey,
            )
        }

    /**
     * Send a single chunk. Returns the sequence number that was acknowledged
     * by the receiver as verified (hash-checked).
     */
    suspend fun sendChunk(
        objectId: String,
        sequence: Long,
        data: ByteArray,
    ): Result<Long> = wrapCall {
        TransferController_nativeSendChunk(
            objectId = objectId,
            sequence = sequence,
            data = data,
        )
    }

    /**
     * Receive the next chunk from the remote peer.
     * Returns a ChunkResult or null if the stream is complete.
     */
    suspend fun receiveChunk(): Result<ChunkResult?> = wrapCall {
        TransferController_nativeReceiveChunk()
    }

    /**
     * Attempt a transparent reconnect after a link fault.
     * Returns the last verified sequence per object (for resume).
     */
    suspend fun reconnect(): Result<Map<String, Long>> = wrapCall {
        TransferController_nativeReconnect()
    }

    /**
     * Get the last verified checkpoint per object (for resume across app kills).
     */
    suspend fun checkpointState(): Result<Map<String, Long>> = wrapCall {
        TransferController_nativeCheckpointState()
    }

    /**
     * Cancel an active transfer.
     */
    suspend fun cancelTransfer(): Result<Unit> = wrapCall {
        TransferController_nativeCancel()
    }

    /**
     * Scan for nearby peers (iPhone / iPad devices advertising WA Bridge).
     * Returns a list of discovered PeerInfo JSON strings.
     */
    suspend fun discoverPeers(): Result<List<String>> = wrapCall {
        TransferController_nativeDiscoverPeers()
    }

    /**
     * Create a loopback transfer pair — used only for self-contained tests.
     */
    fun createTestPair(): String = TransferController_nativeCreateTestPair()

    private suspend fun <T> wrapCall(block: suspend () -> T): Result<T> {
        return try {
            Result.success(block())
        } catch (e: Exception) {
            Log.e("TransferController", "Native call failed: ${e.message}", e)
            Result.failure(e)
        }
    }

    // --- Native method declarations ---
    // These map to the C-ABI entry points defined in the Rust core.
    // The Rust crate must be built with the `jni` feature:
    //   cargo ndk -t arm64-v8a -t armeabi-v7a -t x86_64 build --release

    private external fun TransferController_nativeStart(
        manifestJson: String,
        sessionKey: String,
    ): Unit

    private external fun TransferController_nativeSendChunk(
        objectId: String,
        sequence: Long,
        data: ByteArray,
    ): Long

    private external fun TransferController_nativeReceiveChunk(): ChunkResult?

    private external fun TransferController_nativeReconnect(): Map<String, Long>

    private external fun TransferController_nativeCheckpointState(): Map<String, Long>

    private external fun TransferController_nativeCancel(): Unit

    private external fun TransferController_nativeDiscoverPeers(): List<String>

    private external fun TransferController_nativeCreateTestPair(): String

    data class ChunkResult(
        val objectId: String,
        val sequence: Long,
        val data: ByteArray,
        val hash: String,
    ) : java.io.Serializable

    companion object {
        init {
            System.loadLibrary("wabridge_jni")
        }

        private const val TAG = "TransferController"
    }
}

/**
 * UI state for the transfer screen.
 */
sealed class TransferUiState {
    object Idle : TransferUiState()
    data class Scanning(val peers: List<String> = emptyList()) : TransferUiState()
    data class Connecting(val peer: String) : TransferUiState()
    data class Transferring(
        val current: Long,
        val total: Long,
        val speedMbps: Float,
        val percent: Float,
    ) : TransferUiState()

    object Verifying : TransferUiState()
    object Complete : TransferUiState()
    data class Error(val message: String) : TransferUiState()
}
