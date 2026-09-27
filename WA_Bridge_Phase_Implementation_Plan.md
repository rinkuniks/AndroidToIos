# WA Bridge — Phase-wise Implementation Plan

**Document version:** 1.0
**Implements:** `WA_Bridge_Production_Roadmap_v1.2.md` (scope authority) + `WA_Bridge_Risk_Register_v1.0.md` (risk gates)
**Technical reference:** `WA_Bridge_Production_Roadmap_v1.1-1.md` Technical Appendix (architecture, protocol, stack — still valid wherever v1.2 does not override)
**Status:** Phase 0 in progress (blocked on hardware); Phase 1 core substantially
implemented (crypto, resume, chaos harness, CI); Phases 2–13 scaffold-only.

---

## 1. How this plan works

- **v1.2 wins all conflicts.** Where the older v1.1 roadmap promises more (full chat vault, USB-C as first-class, "Verification Complete"), v1.2's risk-mitigated scope replaces it.
- **Every phase ends in a hard exit gate.** A phase is not done until its gate criteria are written down and verified. Phases 2 and 3 may run in parallel, but gates are never waived.
- **Risk IDs** (C1–C4 critical, H1–H5 high, from the Risk Register) are traced to the phases that mitigate them (Section 6).
- **Durations** assume a small senior team (≈3 engineers: 1 Rust/core, 1 Android, 1 iOS; desktop shared) and must be re-baselined after Phase 0.

## 2. Non-negotiable scope locks (from v1.2 — apply to every phase)

| # | Lock | Source |
|---|------|--------|
| L1 | Companion to the official Move to iOS + WhatsApp path — never injection, never bypass, never a replacement | v1.2 boundary; C3 |
| L2 | Secure Vault = accessible media + inventory + integrity manifests only. Never marketed as a WhatsApp-restorable backup | v1.2 §8; C1 |
| L3 | Same-Wi-Fi is the default transport. USB-C is experimental and ships only after Phase 0 proof on **both** iPhone and iPad | v1.2 §5; C2 |
| L4 | Verification = transport + archive integrity + user checklist. Wording: "WA Bridge checks passed" — never implied full verification of WhatsApp internals | v1.2 §9; C4 |
| L5 | Equal first-class iPhone and iPad. A feature that fails on either is fixed, gated, or cut — never shipped broken | v1.2 §2.8; H4 |
| L6 | Source Android device is never auto-erased; "safe to erase" messaging only after vault + checks complete | v1.2 Screen 9 |
| L7 | Local-first: no chat content leaves the local path by default; normal transfers need no backend; telemetry contains no private content | v1.2 §2.2 |
| L8 | Never modify or delete source WhatsApp data; checkpoint before any risky operation | v1.2 §2.1 |

## 3. Phase overview

| Phase | Name | Primary output | Est. duration | Depends on |
|---|---|---|---|---|
| 0 | Feasibility Spike | Capability matrix + go/no-go scope decisions | 3–4 wks | — |
| 1 | Transfer Core (Rust) | Session/manifest/chunk/resume engine + test harness | 6–8 wks | 0 |
| 2 | Android Source App | Inventory, foreground transfer, vault writer | 6–8 wks | 1 (∥ Phase 3) |
| 3 | iPhone/iPad Receiver | Pairing, receiver, staging, resume, verification | 5–7 wks | 1 (∥ Phase 2) |
| 4 | Smart Connect | Wi-Fi discovery/pairing UX, health, reconnect, USB-C gate | 3–4 wks | 2, 3 |
| 5 | Secure Vault | Encrypted vault v1, recovery key, SSD export, viewer v0 | 3–4 wks | 2, 3 |
| 6 | Official Migration Assistant | Guided official flow + state detection + recovery UX | 4–5 wks | 4, 5 |
| 7 | Verification Engine | Levels 1–3, user checklist, downloadable report | 3–4 wks | 6 |
| 8 | Desktop Recovery Bridge | Windows bridge, vault manager/viewer, diagnostics | 4–6 wks | 5 (∥ Phases 6–7) |
| 9 | UX Polish | 5–7 taps, a11y, localization, iPad-adaptive layouts | 3–4 wks | 7, 8 |
| 10 | Production Hardening | Security/privacy audit + chaos test matrix | 4–6 wks | 9 |
| 11 | Device Compatibility Lab | Multi-OEM/device matrix + remote config | 3–4 wks | 9 (∥ Phase 10) |
| 12 | Beta | 50 → 500 → 5,000 staged rollout + honest metrics | 6–8 wks | 10, 11 |
| 13 | Public Release | Store submissions, compliance, staged rollout | 2–3 wks | 12 |

**Critical path to feature-complete V1:** 0 → 1 → 2+3 → 4 → 5 → 6 → 7 → 9 ≈ **8–10 months**.
**Full Definition of Done (hardening + lab + beta + release):** ≈ **12–16 months**.

## 4. Phase details

### Phase 0 — Feasibility Spike (GATE)

**Objective:** Prove or kill the product's hardest assumptions **before any UI investment**. Every V1 scope item must reach an acceptable status here on both iPhone and iPad.

**Test hardware (minimum):**
- Sources: CMF Phone 2 Pro + one second Android from a different OEM
- Destinations: recent USB-C iPhone + iPad 10th gen or newer (Air/Pro)
- Wi-Fi router supporting reload tests, USB-C data cables, SSD for desktop trials

**Must-prove experiments (Risk Register §5):**
1. **Official path success rate** — real Move to iOS + WhatsApp runs with ~20 GB, ~50 GB, and 80 GB+ libraries to iPhone **and** iPad; record success rate, duration, failure modes. (C3)
2. **What an unrooted Android app can read** — enumerate everything WhatsApp exposes without root; confirm realistic vault contents (media vs. `msgstore.db`). (C1)
3. **USB-C app-to-app reality** — attempt sustained bidirectional third-party transfer over USB-C. Expected: "No" or "Very limited" → confirms experimental gating. (C2)
4. **Background survival** — transfer survival + resume under screen lock, Low Power/Battery Saver, app switching, app kill, per platform. (H1)
5. **Official-transfer observability** — what a third-party app can legitimately detect while Move to iOS runs (feeds Phase 6 heuristics). (C3)

**Capability matrix (fill separately for iPhone and for iPad):**
Same-Wi-Fi secure transfer · QR pairing fallback · Direct USB-C app-to-app data · Official Move to iOS + WhatsApp flow · Accessible data inventory · Resume after interruption (25/50/90%) · Background/screen-lock behaviour · Large transfer (50–100 GB) · Local Network permission handling · Storage calculation accuracy.

**Throwaway prototypes (v1.1 §22):** A — Wi-Fi transfer spike · B — USB-C spike · C — migration run · D — UX paper flow.

**Exit gate (all must be true):**
- [ ] Written capability matrix; every row marked **Supported / Supported-with-official-flow / Fallback-required / Not-supported — separately for iPhone and iPad**
- [ ] Scope decisions logged: USB-C in (experimental) or out; final vault contents; assistant-detection feasibility
- [ ] MVP list re-confirmed or trimmed; H1/H3/H5 re-estimated with measured data
- [ ] 20/50/80 GB test datasets prepared for Phases 1–3

**Risks addressed:** C1, C2, C3 (evidence); H1, H4, H5 (evidence).

---

### Phase 1 — Transfer Core (Rust)

**Objective:** One transport-agnostic engine that chunks, hashes, encrypts, checkpoints, and resumes — shared by Android (JNI), iOS/iPadOS (FFI), and desktop (native bindings).

**Build:**
- `Transport` trait: `discover / pair / authenticate / openSession / sendChunk / receiveChunk / health / reconnect / close` (v1.1 §4.1)
- Session protocol + state machine: `CREATED → PREFLIGHT → CHECKPOINTING → TRANSFERRING ⇄ PAUSED/RECONNECTING → TRANSFER_VERIFIED → … → COMPLETE` (v1.1 §11)
- Manifest model: migration / objects / chunks with id, category, size, source metadata, content hash, chunk count, transfer + verification state (v1.1 §7)
- Chunking + streaming SHA-256; AEAD framing (AES-256-GCM); Argon2id KDF module; secure nonces; key versioning
- Checkpoint store + resume planner (resume from last **verified** chunk); error model; progress events
- Property/unit tests + interruption-injection harness (kill at 25/50/90%, corrupt chunks, network change)

**Exit gate:**
- [~] 20–100 GB synthetic datasets over loopback + real Wi-Fi; resume from last verified chunk after app kill, process restart, simulated network drop — **done:** simulated kill / network drop / corruption with mid-object resume in `core/tests/interruption_chaos.rs` (25/50/90%). **Outstanding:** real Wi-Fi transport, 20–100 GB datasets
- [x] Zero silent corruption: every resumed object hash-verifies — per-chunk Level 1 + staged-object Level 2 verification; mid-object resume across processes now works (ADR-006/007)
- [x] Throughput baseline recorded (feeds H5 ETA work) — 112 MB/s at 50 MB / 4 MiB chunks over loopback (gate: >10 MB/s)
- [~] CI green on all dev machines; fuzz seeds prepared for Phase 10 — workflow added (`.github/workflows/ci.yml`: fmt + clippy `-D warnings` + tests + benches, Linux/Windows/macOS); fuzz seeds outstanding

**Risks addressed:** H3 (primary); C2 (consumes verdict); H5.

---

### Phase 2 — Android Source App (Kotlin + Jetpack Compose)

**Objective:** Source-side app: inventory accessible data, transfer over Wi-Fi via the Rust core, write the Secure Vault — never touching source WhatsApp data destructively (L8).

**Build:**
- Compose skeleton: Welcome → Direction → Destination Status → Connect → Ready Check (Screens 1–5)
- Permission/access layer: notifications, media read / Storage Access Framework; explicit "what WA Bridge can access" scan; no root assumptions (C1)
- **Inventory engine:** streamed SHA-256 enumeration of accessible WhatsApp media (images, video, voice notes, documents, stickers where exposed) + metadata/index
- Foreground service + WorkManager strategy, partial wake locks, battery/thermal monitors, keep-screen-on guidance (H1, H5)
- JNI binding to Rust core; Wi-Fi sender role; chunk streaming from SAF
- Preliminary vault writer (structure per v1.1 §8; encryption hardened in Phase 5)
- Preflight: WhatsApp presence/compat, storage math, battery, power-saving restrictions, one-tap **"Fix everything I can"**

**Exit gate:**
- [ ] 50 GB real library inventoried (sizes match filesystem within tolerance) and transferred to a test receiver
- [ ] Background survival matrix (lock / Low Power / app switch / kill) passes on 2 OEMs
- [ ] Source-immutability audit: source checksums identical before vs. after run (L8)
- [ ] Thermal + battery telemetry captured for an 80 GB run (H5)

**Risks addressed:** C1, H1, H3, H5.

---

### Phase 3 — iPhone/iPad Receiver (Swift + SwiftUI)

**Objective:** One codebase that receives, stages, verifies, and resumes **equally well on iPhone and iPad** (L5, H4).

**Build:**
- SwiftUI skeleton; Network.framework listener + Bonjour advertisement; Local Network permission flows validated on both form factors
- QR + numeric-code pairing UI; encrypted staging area (CryptoKit AES-GCM; Keychain for local secrets)
- Storage monitor with honest pre-transfer space math; background task APIs with documented OS limits (H1)
- Resume receiver + Level 1 (per-chunk) and Level 2 (object hash) verification
- iPad-specific validation: Split View, Stage Manager, external storage, background rules (H4)

**Exit gate:**
- [ ] Feature-parity checklist passes on a recent iPhone and an iPad 10th gen+
- [ ] Resume matrix (interruption at 25/50/90%) green on both devices
- [ ] Local Network permission first-run + deny-recovery path validated on both

**Risks addressed:** H1, H3, H4.

---

### Phase 4 — Smart Connect

**Objective:** The connection experience: fast automatic discovery, QR fallback, invisible reconnection — with USB-C shown only per the Phase 0 verdict (L3).

**Build:**
- Transport manager + automatic fallback UX; weak-Wi-Fi detection → recommends desktop bridge
- mDNS discovery + QR bootstrap when discovery fails; pairing-code compare screen
- Health monitoring (link quality/throughput), auto-reconnect, network-change recovery — user sees only "Connection interrupted. Reconnecting…"
- Mobile-data relay prevention (bind to local interface)
- USB-C path **only if** Phase 0 proved it: behind an experimental flag with explicit labelling; otherwise hidden

**Exit gate:**
- [ ] Typical pairing < 60 s on both destinations; QR fallback works when discovery fails
- [ ] Router restart / Wi-Fi drop mid-transfer resumes without user action
- [ ] Connection UI passes "no jargon" review

**Risks addressed:** C2, H1, H4.

---

### Phase 5 — Secure Vault

**Objective:** The pre-transfer recovery checkpoint: an encrypted, portable, honestly-labelled media + inventory archive (L2).

**Build:**
- Vault format v1: `vault.manifest`, `metadata/`, `indexes/`, `media/` (images · video · audio · voice-notes · documents · stickers), `integrity/`, `recovery/`
- AES-256-GCM object encryption; Argon2id-derived portable vault key; Keychain/Keystore local secrets; key versioning; recovery-key flow; secrets never logged
- Flow enforcement: vault creation **mandatory** before the official-transfer step can start (L1, L6)
- SSD export + desktop vault viewer v0 (Tauri shell)
- Honest labelling everywhere: *"Media & file archive — not a WhatsApp-restorable backup"* (C1, H2)

**Exit gate:**
- [ ] Tamper test: any flipped byte in the vault is detected by the integrity manifest
- [ ] Wrong password / lost device → no access; recovery key restores the vault on another machine
- [ ] 100 GB vault written → re-verified → exported to SSD end-to-end
- [ ] Copy audit: zero restore-into-WhatsApp claims (C1, H2)

**Risks addressed:** C1, H2, H3, H5.

---

### Phase 6 — Official Migration Assistant

**Objective:** Guide the real Move to iOS + WhatsApp step with device-specific instructions and honest state detection — the heart of the companion positioning (L1, C3).

**Build:**
- Dynamic guided flow: detects device/OS/WhatsApp versions; **separate instruction sets for iPhone vs. iPad**; New/Factory-reset vs. Already-set-up branch (reset guidance or Vault-only mode); WhatsApp Business unsupported warning
- Stage tracking using Phase 0 findings: detect started / stuck / failed / complete from observable signals + explicit user confirmations; recovery guidance for every failure state (v1.1 §11 taxonomy)
- Checkpoint-first: WA Bridge state persisted before the user leaves the app for the official flow; WA Bridge verification resumes afterwards
- **Never** simulate or bypass OS/WhatsApp-protected operations (L1)

**Exit gate:**
- [ ] 5+ moderated usability sessions: non-technical users complete the guided flow unaided
- [ ] Every state in the failure taxonomy maps to a recovery screen
- [ ] Copy audit: the assistant never declares official-path success on WA Bridge's authority

**Risks addressed:** C3, H2, H4.

---

### Phase 7 — Verification Engine

**Objective:** Honest post-transfer truth: what WA Bridge verified, and what only the user can verify inside WhatsApp (C4, L4).

**Build:**
- Level 1 report: every transferred chunk verified (transport integrity)
- Level 2 report: vault objects vs. source objects hash comparison (archive integrity)
- Level 3: only legitimately observable checks — **always paired with the mandatory user checklist** (specific chats, recent messages, media visible in WhatsApp)
- Downloadable migration report (PDF + JSON): what transferred, vault contents, checks passed, checklist status
- Completion copy: *"WA Bridge checks passed. Please open WhatsApp and review your important chats before erasing the old device."* — never "Safe to erase" before vault + checks (L6)

**Exit gate:**
- [ ] Report generated for success, interrupted, and failed scenarios alike
- [ ] User checklist is mandatory and cannot be silently skipped
- [ ] Copy audit passes C4 rules; no implication of WhatsApp-internal verification

**Risks addressed:** C4, H2.

---

### Phase 8 — Desktop Recovery Bridge (Windows first)

**Objective:** The reliable path for huge libraries and emergencies — elevated in priority per the Risk Register (H1, H5).

**Build:**
- Tauri + Rust + React/TypeScript app; reuses the Rust core via native bindings
- Android → PC ingest over USB and Wi-Fi; PC → iPhone/iPad bridge where the direct mobile path is unavailable
- Vault manager + viewer: browse, verify, export to SSD
- Interrupted-migration recovery; diagnostics bundle for support

**Exit gate:**
- [ ] 100 GB vault ingest → verify → SSD export on Windows 10/11
- [ ] End-to-end Android → PC → iPad run completes; resume works on the PC leg
- [ ] Diagnostics bundle verified to contain no private content (L7)

**Risks addressed:** H1, H3, H5.

---

### Phase 9 — UX Polish

**Objective:** 5–7 meaningful taps, zero jargon, equal polish on both form factors (L5).

**Build:**
- Full-flow tap audit against the 5–7 target; copy pass (no paths, ports, crypto terms in default UI)
- Accessibility: Dynamic Type + VoiceOver, TalkBack, large text, contrast; light/dark mode
- Localization (EN + 2 highest-value locales first); meaningful notifications; state-explaining animations; real ETA once enough samples exist (H5)
- iPad-adaptive layouts: Split View, Stage Manager, effective use of space (H4)

**Exit gate:**
- [ ] Moderated usability: ≥ 90% first-time success without help, on both platforms
- [ ] Accessibility audit passes on iPhone; Split View/Stage Manager passes on iPad
- [ ] Copy audit: zero technical terms in the default flow

**Risks addressed:** C4 (copy), H4, H5.

---

### Phase 10 — Production Hardening

**Objective:** Break it before users do.

**Run:**
- Security audit + threat-model refresh (v1.1 §16); fuzz protocol/parsers; dependency audit; privacy review (L7)
- Chaos matrix on **both** platforms: power loss, device reboot, app kill, router restart, cable disconnect, low storage, corrupted source media, slow Wi-Fi, 100+ GB stress, thermal-throttle behaviour (H5)

**Exit gate:**
- [ ] Every chaos scenario recovers to a consistent resumable state — no data loss, no silent corruption
- [ ] Automated log scan proves no secrets/keys in logs
- [ ] Security + privacy findings fixed or formally accepted

**Risks addressed:** H1, H3, H5; protects against H2.

---

### Phase 11 — Device Compatibility Lab

**Objective:** Known-device coverage with remote mitigation.

**Run:**
- Android families: Samsung Galaxy, Google Pixel, Nothing/CMF, OnePlus, Xiaomi/Redmi, Motorola
- Apple: multiple USB-C iPhone models; iPad 10th gen+, Air, Pro across supported OS versions
- Remote compatibility configuration service: ship known-issue mitigations without app updates

**Exit gate:**
- [ ] Matrix pass rate ≥ agreed threshold per family; known issues documented with mitigations
- [ ] Remote config service live and exercised

**Risks addressed:** H4.

---

### Phase 12 — Beta

**Objective:** Prove real-world reliability at scale with honest metrics.

**Ladder:** internal alpha → 50-device closed beta → 500-user beta → 5,000-user staged release.

**Metrics (never containing message text, contacts, media, or keys — L7):** connection success, transfer completion, resume success, verification completion, throughput, failure stage — **reported separately for iPhone and iPad** (L5).

**Exit gate:**
- [ ] Success-rate targets met on both platforms (targets derived from Phase 0 baselines)
- [ ] Support load sustainable; top 10 failure causes fixed or mitigated

**Risks addressed:** C3, H1–H5 (validation at scale).

---

### Phase 13 — Public Release

**Objective:** Ship within store rules and honest positioning (L1).

**Checklist:**
- App Store / Play Store review prep: companion-language audit across screens, marketing, and support docs (H2)
- Privacy policy + data-handling disclosure; trademark/nominative use of "WhatsApp" reviewed; security model reviewed
- Staged rollout with remote-config kill switch

---

## 5. Dependency graph

```
Phase 0 ─► Phase 1 ─► Phase 2 (Android) ─┬─► Phase 4 ─► Phase 5 ─► Phase 6 ─► Phase 7 ─┬─► Phase 9 ─► Phase 10 ─► Phase 12 ─► Phase 13
                   └───► Phase 3 (iOS) ──┘                                            │                              ┌─► Phase 11 ─┘
                                                                                       └─► Phase 8 (desktop, ∥ 6–7) ───┘
```

- Phases 2 and 3 proceed in parallel after Phase 1.
- Phase 8 starts once the vault format stabilizes (after Phase 5 core) and runs alongside 6–7.
- Phase 11 runs parallel with Phase 10.
- **No phase's exit gate may be waived** to recover schedule; cut scope instead.

## 6. Risk traceability (Risk Register → phases)

| Risk | Phases that mitigate it |
|---|---|
| C1 — Vault over-claim | 0 (evidence), 2 (inventory reality), 5 (labelling + enforcement), 6, 7 (copy) |
| C2 — USB-C unreliability | 0 (verdict), 4 (experimental gating) |
| C3 — Official-path fragility | 0 (baselines), 6 (assistant + heuristics), 12 (validation) |
| C4 — False verification confidence | 7, 9 (mandatory checklist + copy rules) |
| H1 — Background limits | 2, 3, 4, 8 (desktop elevation) |
| H2 — Store rejection | 5, 6, 7 copy audits; 13 review prep |
| H3 — Resume complexity | 1, 2, 3, 8, 10 |
| H4 — iPad parity | 3, 4, 9, 10, 11 |
| H5 — Thermal/battery | 2, 8, 9 (ETA), 10 |

## 7. MVP (v1.2 §18) → phase mapping

| MVP item | Delivered by |
|---|---|
| Guided companion flow for official path | 6 |
| New vs. Already-set-up branching | 2 (screens), 6 (logic) |
| Same-Wi-Fi secure pairing + QR fallback | 1, 3, 4 |
| USB-C only if Phase 0 proves it | 0 (verdict), 4 (gating) |
| Secure Vault (media + accessible inventory) first | 2, 5 |
| Chunked, resumable transfer of accessible data | 1, 2, 3 |
| Honest preflight "What will actually transfer" | 2, 7 |
| Guided official assistant (iPhone/iPad instructions) | 6 |
| Transport + vault verification + user checklist | 3, 7 |
| Downloadable report | 7 |
| Windows desktop bridge + vault viewer | 8 |

**Explicitly out of V1 (restated):** independent full chat-history migration/injection into WhatsApp; vault-restore-into-WhatsApp claims; WhatsApp Business support (warning only); other messengers; default cloud upload.

## 8. Repository scaffold (create at Phase 1 kickoff, per v1.1 §13)

```
wa-bridge/
├── apps/            # android/ · ios/ (shared iPhone+iPad) · desktop/
├── core/            # transport/ · protocol/ · crypto/ · vault/ · integrity/ · checkpoint/ · diagnostics/  (Rust)
├── services/        # account/ · entitlement/ · compatibility/
├── docs/            # architecture/ · security/ · ux/ · qa/ · compliance/
└── tests/           # protocol/ · migration/ · interruption/ · performance/ · device-matrix/
```

Minimum OS targets to re-validate in Phase 0: Android 8.0+ (prefer 10+), iOS/iPadOS 15.5+.

## 9. Immediate next actions (Phase 0 kickoff)

1. **Procure/allocate hardware:** CMF Phone 2 Pro, second Android (different OEM), USB-C iPhone, iPad 10th gen+/Air/Pro, test router, USB-C data cables, SSD.
2. **Write the Phase 0 test protocol** + result templates (capability-matrix CSV, per-experiment logs) into `docs/qa/`.
3. **Prepare datasets:** 20/50/80 GB synthetic libraries + anonymized real libraries.
4. **Run prototypes A–D** (Wi-Fi spike, USB-C spike, official-migration run, UX paper flow).
5. **Log decisions** in `docs/architecture/DECISIONS.md`: USB-C in/out, final vault contents, detection feasibility — then re-baseline this plan's durations and scope.

---

**Next step after sign-off:** scaffold the repository (Section 8) and start the Phase 1 Rust core skeleton (`Transport` trait, manifest model, chunk store) while Phase 0 hardware tests run in parallel.

**Document control**

| Version | Date | Notes |
|---|---|---|
| 1.0 | 22 Sep 2026 | Initial phase-wise implementation plan derived from Roadmap v1.2 + Risk Register v1.0 |
