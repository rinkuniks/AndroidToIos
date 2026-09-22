# Phase 0 — Capability Matrix Template

Fill **one row per capability, separately for iPhone and iPad** (Plan Phase 0 gate).
Status values: `Supported` / `Supported-with-official-flow` / `Fallback-required` / `Not-supported`.

| Capability | iPhone status | iPad status | Notes / evidence |
|---|---|---|---|
| Same-Wi-Fi secure transfer | | | |
| QR pairing fallback | | | |
| Direct USB-C app-to-app data | | | Expected: Not-supported / Very-limited (C2) |
| Official Move to iOS + WhatsApp flow | | | Record success rate per library size |
| Accessible data inventory (unrooted) | | | Media vs. database (C1) |
| Resume after interruption (25%) | | | |
| Resume after interruption (50%) | | | |
| Resume after interruption (90%) | | | |
| Background / screen-lock survival | | | Lock, Low Power, app switch, app kill (H1) |
| Large transfer (50–100 GB) | | | Record thermal/battery (H5) |
| Local Network permission handling | | | First-run + deny-recovery |
| Storage calculation accuracy | | | |

## Must-prove experiments (Risk Register §5)

| # | Experiment | Pass criteria | Result | Date | Tester |
|---|---|---|---|---|---|
| 1 | Official path success @ 20/50/80 GB (iPhone + iPad) (C3) | Written success rate + failure modes per size/platform | | | |
| 2 | Unrooted Android readable data (C1) | Enumerated list: media / metadata / not-accessible | | | |
| 3 | USB-C app-to-app sustained transfer (C2) | Go/no-go verdict for experimental gating | | | |
| 4 | Background survival + resume (H1) | Resume succeeds in all 4 scenarios, both platforms | | | |
| 5 | Official-transfer observability (C3) | List of detectable signals for Phase 6 heuristics | | | |

**Exit rule:** every row filled + scope decisions logged in `docs/architecture/DECISIONS.md` before any UI investment.
