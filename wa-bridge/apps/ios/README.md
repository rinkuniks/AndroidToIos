# apps/ios

One shared iPhone + iPad app — **Phase 3** (implementation plan).

- Swift + SwiftUI + Structured Concurrency + Network framework (Bonjour) + CryptoKit/Keychain
- Receiver: pairing UI, encrypted staging, storage monitor, resume, Level 1/2 verification
- Equal first-class treatment of iPhone and iPad (L5): Split View, Stage Manager, external storage validated
- Local Network permission flows validated on both form factors (H4)
- Binds the Rust core via FFI (`../../core`)

Blocked until Phase 0 capability matrix passes.
