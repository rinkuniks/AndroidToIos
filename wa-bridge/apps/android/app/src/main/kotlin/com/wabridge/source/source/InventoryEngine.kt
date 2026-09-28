package com.wabridge.source.source

import android.content.ContentResolver
import android.content.Context
import android.net.Uri
import android.provider.OpenableColumns
import android.util.Log
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import java.io.InputStream
import java.util.UUID

/**
 * Scans accessible WhatsApp media files (images, videos, audio, voice notes,
 * documents, stickers) via Android's Storage Access Framework (SAF) and
 * MediaStore.
 *
 * Scope lock C1: We NEVER attempt to access msgstore.db or any encrypted
 * database. Only media files accessible via SAF/MediaStore are inventoried.
 * The inventory is a media-only Secure Vault — never a WhatsApp-restorable backup.
 *
 * The scanning runs as a foreground service (H1) and streams SHA-256 hashes
 * for each file, producing a manifest that the Rust core will transfer.
 *
 * (Plan Phase 2; roadmap v1.2 §13, H1, H5, C1)
 */
class InventoryEngine(private val context: Context) {

    private val _scanState = MutableStateFlow<InventoryScanState>(InventoryScanState.Idle)
    val scanState: StateFlow<InventoryScanState> = _scanState.asStateFlow()

    private val _items = mutableListOf<MediaItem>()

    private var _manifestJson: String = ""

    /**
     * Scan for accessible WhatsApp media files using MediaStore + SAF.
     * Scope lock C1: Only accessible media + metadata. No database access.
     */
    suspend fun scanWhatsAppMedia() {
        _scanState.value = InventoryScanState.Scanning(0, 0)
        _items.clear()

        // 1. Collect from MediaStore (accessible WhatsApp cache/thumbnails)
        val mediaStoreItems = scanMediaStore()
        _items.addAll(mediaStoreItems)
        _scanState.value = InventoryScanState.Scanning(_items.size, _items.size)

        // 2. SAF-collected URIs are added separately via addSaffiles()
        //    (called by MainActivity after the SAF picker returns)

        // 3. Compute streaming SHA-256 for each item (required for 100GB+ media, H5)
        var scanned = 0
        for (item in _items) {
            try {
                item.contentHash = computeStreamingHash(item.uri)
                scanned++
                _scanState.value = InventoryScanState.Scanning(scanned, _items.size)
            } catch (e: Exception) {
                Log.w("InventoryEngine", "Failed to hash ${item.uri}: ${e.message}")
            }
        }

        // 4. Build the manifest JSON (sent to Rust core via JNI)
        _manifestJson = buildManifestJson(_items)

        val totalBytes = _items.sumOf { it.size }
        _scanState.value = InventoryScanState.Complete(
            itemCount = _items.size,
            totalBytes = totalBytes,
            manifestJson = _manifestJson,
                )
    }

    /**
     * Scan MediaStore for accessible WhatsApp media.
     * WhatsApp stores media in: Android/media/com.whatsapp/WAImages, WAMedia, etc.
     * These directories are accessible via MediaStore without SAF (C1 scope).
     */
    private fun scanMediaStore(): List<MediaItem> {
        val items = mutableListOf<MediaItem>()
        val contentResolver: ContentResolver = context.contentResolver

        val whatsAppDirs = listOf(
            "WhatsApp/Images",
            "WhatsApp/Video",
            "WhatsApp/Audio",
            "WhatsApp/VoiceNotes",
            "WhatsApp/Documents",
            "WhatsApp/Stickers",
        )

        for (dir in whatsAppDirs) {
            val projection = arrayOf(
                android.provider.MediaStore.Files.FileColumns._ID,
                android.provider.MediaStore.Files.FileColumns.DISPLAY_NAME,
                android.provider.MediaStore.Files.FileColumns.SIZE,
                android.provider.MediaStore.Files.FileColumns.MIME_TYPE,
                android.provider.MediaStore.Files.FileColumns.DATE_MODIFIED,
            )

            val selection = "relative_path LIKE '%" + dir.replace("WhatsApp/", "") + "%'"
            val cursor = contentResolver.query(
                android.provider.MediaStore.Files.getContentUri("external"),
                projection,
                selection,
                null,
                null,
            )

            cursor?.use {
                val idIdx = it.getColumnIndexOrThrow(android.provider.MediaStore.Files.FileColumns._ID)
                val nameIdx = it.getColumnIndexOrThrow(android.provider.MediaStore.Files.FileColumns.DISPLAY_NAME)
                val sizeIdx = it.getColumnIndexOrThrow(android.provider.MediaStore.Files.FileColumns.SIZE)
                val mimeIdx = it.getColumnIndexOrThrow(android.provider.MediaStore.Files.FileColumns.MIME_TYPE)
                val dateIdx = it.getColumnIndexOrThrow(android.provider.MediaStore.Files.FileColumns.DATE_MODIFIED)

                while (it.moveToNext()) {
                    val id = it.getLong(idIdx)
                    val name = it.getString(nameIdx)
                    val size = it.getLong(sizeIdx)
                    val mime = it.getString(mimeIdx) ?: "application/octet-stream"
                    val date = it.getLong(dateIdx)

                    val category = categorizeByMime(mime) ?: continue
                    val uri = android.net.Uri.withAppendedPath(
                        android.provider.MediaStore.Files.getContentUri("external"),
                        id.toString(),
                    )

                    items.add(
                        MediaItem(
                            id = "wa://$id",
                            objectId = "wa-${System.currentTimeMillis()}-${id}",
                            uri = uri,
                            displayName = name,
                            mimeType = mime,
                            size = size,
                            category = category,
                            dateAdded = date,
                            contentHash = "",
                        ),
                    )
                }
            }
        }
                return items
    }

    /**
     * Compute streaming SHA-256 over an InputStream without loading the
     * entire File into memory (required for 100GB+ media, H5).
     * Direct port of checkpoint::sha256_streaming in the Rust core.
     */
    private fun computeStreamingHash(uri: Uri): String {
        val contentResolver = context.contentResolver
        val input: InputStream = contentResolver.openInputStream(uri)
            ?: throw java.io.IOException("Cannot open $uri")

        val md = java.security.MessageDigest.getInstance("SHA-256")
        val buf = ByteArray(1024 * 1024) // 1 MiB buffer
        input.use {
            var n: Int
            while (input.read(buf).also { n = it } != -1) {
                md.update(buf, 0, n)
            }
        }
        return md.digest().joinToString("") { "%02x".format(it) }
    }

    /**
     * Build the manifest JSON that will be sent to the Rust core as the
     * first frame of the transfer (vault-before-transfer ordering, L6).
     */
    private fun buildManifestJson(items: List<MediaItem>): String {
        val objects = items.map { item ->
            mapOf(
                "id" to item.objectId,
                "category" to item.category.name.lowercase(),
                "size" to item.size,
                "sha256" to item.contentHash,
                "chunk_count" to calculateChunkCount(item.size),
                "chunk_size" to 4 * 1024 * 1024, // matches DEFAULT_CHUNK_SIZE in Rust
                "transfer_state" to "pending",
            )
        }

        val manifest = mapOf(
            "migration_id" to "wa-migration-${System.currentTimeMillis()}",
            "source_device" to "Android",
            "destination_device" to "iOS/iPadOS",
            "created_at" to System.currentTimeMillis().toString(),
            "state" to "preflight",
            "objects" to objects,
        )
        return com.google.gson.Gson().toJson(manifest)
    }

    private fun calculateChunkCount(size: Long): Long {
        val chunkSize = 4 * 1024 * 1024L
        return (size + chunkSize - 1) / chunkSize
    }

    /**
     * Map MIME types to our Category enum (matches the Rust core's Category).
     */
    private fun categorizeByMime(mime: String): MediaItem.Category? {
        return when {
            mime.startsWith("image/") -> MediaItem.Category.IMAGE
            mime.startsWith("video/") -> MediaItem.Category.VIDEO
            mime.startsWith("audio/") -> MediaItem.Category.AUDIO
            mime.contains("sticker") || mime == "image/webp" -> MediaItem.Category.STICKER
            mime.startsWith("application/") || mime.startsWith("text/") -> MediaItem.Category.DOCUMENT
            else -> null
        }
    }

    /**
     * Add files selected via SAF (user-granted URIs).
     * Called by MainActivity.onMediaSelected() after the SAF picker returns.
     */
    suspend fun addSaffiles(uris: List<Uri>) {
        val newItems = uris.mapNotNull { uri ->
            val name = queryDisplayName(uri) ?: "file-${System.currentTimeMillis()}"
            val size = queryFileSize(uri)
            val mime = context.contentResolver.getType(uri) ?: "application/octet-stream"
            val category = categorizeByMime(mime) ?: return@mapNotNull null

            MediaItem(
                id = "saf://$uri",
                objectId = "saf-${UUID.nameUUIDFromBytes(uri.toString().toByteArray())}",
                uri = uri,
                displayName = name,
                mimeType = mime,
                size = size,
                category = category,
                dateAdded = System.currentTimeMillis(),
                contentHash = "",
            ).also {
                it.contentHash = computeStreamingHash(uri)
            }
        }
        _items.addAll(newItems)
    }

    private fun queryDisplayName(uri: Uri): String? {
        var name: String? = null
        try {
            val cursor = context.contentResolver.query(uri, null, null, null, null)
            cursor?.use {
                val nameIdx = it.getColumnIndex(OpenableColumns.DISPLAY_NAME)
                if (nameIdx >= 0 && it.moveToFirst()) {
                    name = it.getString(nameIdx)
                }
            }
        } catch (e: Exception) {
            Log.w("InventoryEngine", "Failed to query $uri: ${e.message}")
        }
        return name ?: uri.lastPathSegment
    }

    private fun queryFileSize(uri: Uri): Long {
        var size = 0L
        try {
            val cursor = context.contentResolver.query(uri, null, null, null, null)
            cursor?.use {
                val sizeIdx = it.getColumnIndex(OpenableColumns.SIZE)
                if (sizeIdx >= 0 && it.moveToFirst()) {
                    size = it.getLong(sizeIdx)
                }
            }
        } catch (e: Exception) {
            Log.w("InventoryEngine", "Failed to query size for $uri: ${e.message}")
        }
        return size
    }
}

data class MediaItem(
    val id: String,
    val objectId: String,
    val uri: Uri,
    val displayName: String,
    val mimeType: String,
    val size: Long,
    val category: Category,
    val dateAdded: Long,
    var contentHash: String,
) : java.io.Serializable {
    enum class Category {
        IMAGE, VIDEO, AUDIO, VOICE_NOTE, DOCUMENT, STICKER, METADATA
    }
}

sealed class InventoryScanState {
    object Idle : InventoryScanState()
    data class Scanning(val scanned: Int, val total: Int) : InventoryScanState()
    data class Complete(
        val itemCount: Int,
        val totalBytes: Long,
        val manifestJson: String,
    ) : InventoryScanState()
}

