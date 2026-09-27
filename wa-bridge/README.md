# wa-bridge

WA Bridge — local-first companion for the official Move to iOS + WhatsApp transfer.

**Scope authority:** `WA_Bridge_Production_Roadmap_v1.2.md`
**Risk gates:** `WA_Bridge_Risk_Register_v1.0.md`
**Implementation plan:** `../WA_Bridge_Phase_Implementation_Plan.md`

## Layout

```
wa-bridge/
├── apps/            # android/ · ios/ (shared iPhone+iPad) · desktop/
├── core/            # Rust transfer core (transport, protocol, checkpoint)
├── docs/            # architecture/ · qa/ (Phase 0 templates live in docs/qa/phase0/)
└── tests/           # protocol/ · migration/ · interruption/ (added from Phase 1)
```

## Status

- Phase 0 (Feasibility Spike) — **in progress, blocked on hardware.** Protocol +
  templates ready in `docs/qa/phase0/`; capability matrix still unfilled and
  ADR-P1…P5 still pending.
- Phase 1 (Transfer Core) — **engine, crypto, resume and chaos harness in place.**
  - `core/`: transport trait + loopback/fault injection, manifest model with
    **per-chunk SHA-256**, atomic checkpoint store with a real resume planner,
    chunked sender/receiver with Level 1 (per-chunk) and Level 2 (staged-object)
    integrity verification.
  - `core/src/crypto.rs`: AES-256-GCM AEAD framing, Argon2id KDF, key versioning,
    purpose-separated session/vault keys.
  - `core/src/session.rs`: guarded session state machine (illegal transitions are
    rejected, not silently allowed).
  - Tests: `cargo test` (unit) + `cargo test --test interruption_chaos` (25/50/90%
    interruption, network drop, corrupted chunk) + `cargo test --release
    --test throughput_bench -- --ignored` (throughput baselines).
  - CI: `.github/workflows/ci.yml` (fmt, clippy `-D warnings`, tests, benches on
    Linux/Windows/macOS).
- Phases 2–13 — scaffold only (Android Gradle project + inventory engine + JNI
  surface drafted; iOS/desktop are README-only).

## Scope locks (do not violate in code or copy)

1. Companion to the official path — no injection, no bypass.
2. Vault = accessible media + inventory + manifests only; never a WhatsApp-restorable backup.
3. Wi-Fi default; USB-C experimental and only if Phase 0 proves it on iPhone AND iPad.
4. "WA Bridge checks passed" + user checklist; never imply WhatsApp-internal verification.
5. Equal first-class iPhone and iPad.
6. Never auto-erase the source; "safe to erase" only after vault + checks.
7. Local-first; no chat content off-device by default; no secrets in logs.
