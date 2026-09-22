# WA Bridge – Risk Register & Recommended Scope Adjustments

**Principal Engineer / Solution Architect Review**  
**Document Version:** 1.0  
**Date:** 21 September 2026  
**Based on:** WA Bridge Production Roadmap v1.1 → mitigated in v1.2

---

## 1. Critical Risks (Must Address Before Significant Investment)

| ID | Risk | Likelihood | Impact | Root Cause | Recommended Mitigation | Priority | Status in v1.2 |
|----|------|------------|--------|------------|------------------------|----------|----------------|
| C1 | Third-party app cannot access full WhatsApp chat database | Very High | Critical | `msgstore.db` is encrypted with device-bound key; scoped storage + no root | Redefine Secure Vault as **media + accessible metadata only**. Never claim full chat history restoration outside official path. | P0 | Mitigated |
| C2 | Direct USB-C app-to-app transfer is not reliably available | High | High | iOS/iPadOS sandbox + USB Restricted Mode + lack of general-purpose entitlements | Demote USB-C to “Experimental / Advanced only”. Default to Wi-Fi. Only promote after Phase 0 proves sustained performance on both iPhone and iPad. | P0 | Mitigated |
| C3 | Official Move to iOS path is fragile and opaque | High | Critical | Apple controls the transfer; limited observability for third-party apps; requires factory reset for best results | Position product strictly as **companion**. Invest heavily in heuristics for detecting start/stuck/complete/fail states. Provide excellent recovery guidance. | P0 | Mitigated |
| C4 | “Verification Complete” creates false confidence | High | High | Cannot inspect WhatsApp private container on iOS/iPadOS | Change language to “WA Bridge checks passed”. Always show a clear user checklist of what only the user can verify inside WhatsApp. | P0 | Mitigated |

---

## 2. High Risks

| ID | Risk | Likelihood | Impact | Recommended Mitigation | Priority | Status in v1.2 |
|----|------|------------|--------|------------------------|----------|----------------|
| H1 | Long transfers killed by OS background limits | High | High | Aggressive foreground service / background modes + clear user guidance to keep screen on + desktop bridge as reliable alternative | P1 | Partially addressed (desktop elevated) |
| H2 | App Store / Play Store rejection for migration claims | Medium-High | High | Strict “companion to Move to iOS” positioning. Avoid any language that implies independent full migration. | P1 | Mitigated |
| H3 | Resume of large media transfers is extremely complex | High | High | Invest early in robust chunk + manifest + partial-file recovery. Extensive interruption testing required. | P1 | Still requires strong engineering focus |
| H4 | iPad behaviour differs significantly from iPhone | Medium | Medium-High | Explicit iPad test matrix (Stage Manager, Split View, external storage, background rules). Do not assume parity. | P1 | Acknowledged in dual-device requirements |
| H5 | Large media libraries (50–100 GB+) cause thermal/battery throttling | High | Medium | Streaming architecture + thermal monitoring + clear ETA + option to pause/resume overnight via desktop | P1 | Partially addressed |

---

## 3. Recommended Scope Adjustments for V1

**Keep (Core Value)**
- Guided companion experience for the official Move to iOS + WhatsApp path
- Pre-transfer Secure Vault (media + accessible inventory)
- Resumable Wi-Fi transfer of accessible data
- Clear “New vs Already set up” branching
- Honest preflight report of what will actually move
- Post-transfer integrity report + user checklist
- Desktop emergency bridge + vault viewer (elevated importance)

**Demote or Gate**
- USB-C direct transfer → Experimental only (behind Phase 0 proof)
- Any claim of full independent chat history migration → Removed
- Level 3 “full migration verification” → Softened to transport + vault + user checklist

**Add (High Value)**
- Explicit “What will actually transfer” report before user starts
- Strong detection + recovery UX around official path failures
- Desktop path elevated from “emergency” to “recommended for large libraries”
- Clear WhatsApp Business warning (unsupported)
- Thermal / battery guidance during long transfers

**Explicitly Out of V1**
- Injecting chats into an already-activated WhatsApp account
- WhatsApp Business support
- Other messaging apps
- Cloud upload of chat content by default

---

## 4. Revised Positioning Statement

> **WA Bridge is the intelligent companion to Apple’s Move to iOS.**  
> We prepare your devices, create a protected media archive before anything risky happens, guide you through the official WhatsApp transfer, and leave you with a recoverable backup even if the official process is interrupted or incomplete.  
> We do not bypass WhatsApp or Apple security. We make the supported path safer and more recoverable.

This positioning is honest, defensible in app review, and still delivers clear user value.

---

## 5. Phase 0 Must-Prove Items (Updated)

Before any significant UI or marketing work:

1. Measured success rate of official Move to iOS + WhatsApp path with 20 GB, 50 GB, and 80 GB+ libraries on both iPhone and iPad.
2. What data a third-party Android app can actually read and archive without root (media vs database).
3. Whether sustained bidirectional USB-C data transfer between two third-party apps is possible on current iOS/iPadOS (expect “No” or “Very limited”).
4. Background survival and resume behaviour under screen lock, low power mode, and app switching on both platforms.
5. Realistic detection signals available to a third-party app during the official transfer.

Only features that pass these tests should remain in the committed V1 scope.

---

## 6. Summary Recommendation

The original roadmap was ambitious and user-centric, but contained **critical over-optimism** about:

- How much WhatsApp data a third-party app can actually access and preserve independently
- The feasibility of direct USB-C app-to-app transfer
- How much “verification” is realistically possible after the official transfer
- How robust the official Move to iOS path is under real-world conditions

**Actions taken in Roadmap v1.2:**
- Critical risks C1–C4 mitigated through scope and language changes
- Positioning locked as companion
- Secure Vault and Verification sections rewritten honestly
- USB-C demoted
- MVP and Definition of Done tightened

**Remaining focus areas:**
- Strong engineering investment in resume reliability (H3)
- Background execution strategy (H1)
- Explicit iPad behavioural testing (H4)
- Thermal/battery handling for large transfers (H5)

If these remaining items are treated seriously in Phase 0 and early development, WA Bridge becomes a viable, honest, and supportable product.

---

## Document Control

| Version | Date       | Author                        | Notes |
|---------|------------|-------------------------------|-------|
| 1.0     | 21 Sep 2026 | Principal Engineer Review    | Initial risk register after architecture review of Roadmap v1.1. Mitigations applied in Roadmap v1.2. |
