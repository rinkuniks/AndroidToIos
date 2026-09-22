# apps/android

Android source app — **Phase 2** (implementation plan).

- Kotlin + Jetpack Compose + Coroutines/Flow + WorkManager + Room
- Screens 1–5: Welcome → Direction → Destination Status → Connect → Ready Check
- Inventory engine: streamed SHA-256 over accessible WhatsApp media (SAF); no root assumptions (C1)
- Foreground service + battery/thermal guidance (H1, H5)
- Binds the Rust core via JNI (`../../core`)

Blocked until Phase 0 capability matrix passes.
