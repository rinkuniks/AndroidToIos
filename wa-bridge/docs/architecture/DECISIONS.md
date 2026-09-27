# Architecture decisions (ADR log)

Format: Architecture Decision Record. Statuses: Proposed / Accepted / Superseded.

| ID | Decision | Status | Phase |
|---|---|---|---|
| ADR-001 | Shared transfer core in **Rust**, exposed via JNI (Android) and FFI (iOS/iPadOS/desktop) — per roadmap v1.1 §12 | Accepted | 1 |
| ADR-002 | **Chunked objects** (default 4 MiB) with SHA-256 per object; never one giant archive file (v1.1 §7) | Accepted | 1 |
| ADR-003 | Checkpoints persisted **atomically** (temp file + fsync + rename) after every verified chunk group | Accepted | 1 |
| ADR-004 | Only **verified** chunks advance the resume point (H3 gate) | Accepted | 1 |
| ADR-005 | **AES-256-GCM AEAD framing** with a `key_version \|\| purpose \|\| nonce` header; Argon2id KDF (64 MiB / t=3 / p=1 defaults, recorded with the vault) and purpose-separated keys (session vs. vault). Keys zeroize on drop and never appear in `Debug`/error text (L7) | Accepted | 1 |
| ADR-006 | Manifests carry **per-chunk SHA-256 hashes**. Level 1 verifies each chunk on the wire (sender refuses to ship a chunk whose source bytes changed; receiver rejects a chunk before staging); Level 2 re-hashes the staged object in sequence order at completion. This removes the ADR-002 limitation: object digests no longer depend on one continuous stream, so mid-object resume works across processes | Accepted | 1 |
| ADR-007 | Object completion is driven by the **set of received chunk indexes**, never by bytes seen. A receiver seeds that set from its own checkpoint (`ReceivedIndex`) so a sender resuming mid-object closes the object instead of leaving the receiver waiting | Accepted | 1 |
| ADR-008 | A resumed session restarts from the **lower** of the two sides' checkpoints (the receiver's staged prefix is the authority). Re-sending a chunk is harmless — it is verified and de-duplicated; skipping one is not | Accepted | 1 |

## Pending decisions (blocked on Phase 0 evidence)
## ADR-002 note (superseded in part by ADR-006)

ADR-002 still holds for chunked objects and per-object hashing, but the
"cross-process mid-object hash reconstruction is deferred" limitation is now
**resolved**: per-chunk hashes plus staged-object re-hashing (ADR-006) make a
mid-object resume across app restarts both possible and verifiable.



| ID | Decision | Blocked on |
|---|---|---|
| ADR-P1 | USB-C transport in/out of scope (experimental gating, L3) | Phase 0 experiment 3 (C2) |
| ADR-P2 | Final Secure Vault contents (accessible media vs. database reality, L2) | Phase 0 experiment 2 (C1) |
| ADR-P3 | Official-transfer state-detection signals for the Migration Assistant | Phase 0 experiment 5 (C3) |
| ADR-P4 | Background-execution strategy per platform (H1) | Phase 0 experiment 4 |
| ADR-P5 | Chunk size / parallelism after throughput baseline (H5) | Phase 1 benchmarks |
