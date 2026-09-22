# apps/desktop

Windows-first desktop recovery bridge — **Phase 8** (implementation plan).

- Tauri + Rust + React/TypeScript; reuses the Rust core via native bindings (`../../core`)
- Functions: Android ingest (USB/Wi-Fi), PC → iPhone/iPad bridge, vault manager + viewer, SSD export, interrupted-migration recovery, diagnostics bundle (metadata only — L7)
- Windows 10/11 first; macOS later

Requires the `core` crate to be feature-complete for Phase 1 gates.
