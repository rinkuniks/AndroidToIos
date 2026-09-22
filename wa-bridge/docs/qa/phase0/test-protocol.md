# Phase 0 — Feasibility Spike Test Protocol (v1.0)

**Gate:** Plan Phase 0. No UI work starts before this document is fully filled in.
**Devices (minimum):** CMF Phone 2 Pro + one other-OEM Android (source); recent USB-C iPhone + iPad 10th gen/Air/Pro (destination); test router; USB-C data cables; SSD.

## Datasets

| Label | Contents | Size target |
|---|---|---|
| DS-20 | Synthetic + anonymized real WhatsApp media | ~20 GB |
| DS-50 | Mixed media, incl. long videos + voice notes | ~50 GB |
| DS-80 | Stress set, worst-case file mix | 80+ GB |

## Experiments

### E1 — Official Move to iOS + WhatsApp success rate (C3)
For each destination (iPhone, iPad) × each dataset (20/50/80 GB): run the official transfer 3×. Record: success/fail, wall time, failure stage, error text, device temp. **Output:** success-rate table; goes to `capability-matrix-template.md` row "Official Move to iOS + WhatsApp flow".

### E2 — What an unrooted Android app can read (C1)
adb shell ls on WhatsApp dirs + SAF probe app. Classify every path: media / metadata / encrypted-database / not-accessible. **Output:** inventory scope list; feeds Secure Vault contents (ADR-P2).

### E3 — USB-C app-to-app transfer reality (C2)
Attempt sustained bidirectional transfer between two third-party apps over USB-C (Android USB accessory/host mode; iPhone/iPad external-accessory + document APIs). Expect "No" or "Very limited". **Output:** ADR-P1 verdict.

### E4 — Background survival + resume (H1)
During a 50 GB Wi-Fi transfer: screen lock, Battery Saver/Low Power, app switch, app kill — 4 scenarios × 2 platforms. Record whether transfer survives and resumes from checkpoint. **Output:** background-execution strategy (ADR-P4).

### E5 — Official-transfer observability (C3)
While the official transfer runs, log every signal a third-party app can observe (network activity, foreground app, user-visible states). **Output:** heuristic signal list for Phase 6 assistant (ADR-P3).

## Exit criteria (all required)

1. Capability matrix filled — every row, separately for iPhone and iPad, using only `Supported` / `Supported-with-official-flow` / `Fallback-required` / `Not-supported`.
2. ADR-P1…P4 logged in `docs/architecture/DECISIONS.md`.
3. MVP list re-confirmed or trimmed; Phase 1–13 durations re-baselined.
4. Datasets DS-20/50/80 kept for Phase 1–3 test harnesses.

Log each run with `experiment-log-template.md`.
