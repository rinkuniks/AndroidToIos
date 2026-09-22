# WA Bridge — Production Roadmap

**Document version:** 1.2  
**Product type:** Local-first companion for official WhatsApp migration, media preservation, and verification  
**Primary UX goal:** A non-technical user should be able to start a migration in a few taps, connect two devices by USB-C or the same Wi-Fi network, and be guided safely through every required step on **both iPhone and iPad**.

> **Product boundary & positioning:**  
> WA Bridge is the intelligent **companion** to Apple’s official Move to iOS + WhatsApp transfer.  
> It does **not** claim to bypass WhatsApp encryption, Apple sandboxing, or inject unsupported databases into WhatsApp.  
> It prepares devices, creates a protected media + inventory archive **before** any risky step, guides the supported official path, and verifies everything it can legitimately verify.  
> The source Android device is never erased automatically.  
> Full chat history restoration into WhatsApp depends on the official Apple/WhatsApp path succeeding.

---

## Version History / Changelog

### v1.2 (Current – Risk-Mitigated)
- Critical risk mitigations applied after principal engineering review.
- Secure Vault redefined as media + accessible inventory only (no claim of full independent chat restore).
- USB-C demoted to experimental / advanced only until Phase 0 proves reliability on both iPhone and iPad.
- Verification language softened; strong emphasis on user checklist for what only WhatsApp can confirm.
- Positioning locked as “companion to Move to iOS”.
- MVP and Definition of Done tightened to match realistic capabilities.
- Added explicit preflight “What will actually transfer” requirement.

### v1.1
- Equal first-class iPhone + iPad support.
- “New or Already set up” branch.
- Companion positioning introduced.
- Secure Vault mandatory before official transfer.
- Technical Appendix added.

### v1.0
- Original vision and roadmap.

---

## 1. Product Vision

WA Bridge makes the official Android → iPhone/iPad WhatsApp transfer safer, more recoverable, and leaves the user with a permanent protected archive:

1. Install/open WA Bridge  
2. Select Android → iPhone or Android → iPad  
3. Confirm whether the Apple device is new/reset or already set up  
4. Connect devices (Wi-Fi primary; USB-C experimental)  
5. Pair automatically or via QR  
6. WA Bridge runs preflight and shows **what will actually transfer**  
7. Creates Secure Vault (media + accessible inventory) **first**  
8. Guides the official Move to iOS + WhatsApp step  
9. Verifies transport + vault integrity and provides a clear user checklist  
10. Shows Migration Complete only after honest verification  

Target: **5–7 meaningful taps** for the normal path.  
Equal reliability and polish on **both iPhone and iPad**.

---

## 2. Core Product Principles

2.1 **Zero-loss first** — Never modify or delete source WhatsApp data. Always create a recovery checkpoint before risky operations.  

2.2 **Local-first privacy** — Chat content and media stay on the user’s devices by default.  

2.3 **Companion, not replacement** — We enhance and protect the official path; we do not replace it.  

2.4 **Honest capabilities** — Never claim we can access or restore more than the platform and WhatsApp allow.  

2.5 **Resume instead of restart** — Large transfers resume from the last verified chunk.  

2.6 **One screen at a time** — No technical jargon for normal users.  

2.7 **Verify only what we can** — “WA Bridge checks passed” + clear user checklist. Never imply full internal WhatsApp verification.  

2.8 **Equal first-class support for iPhone and iPad**.

---

## 3. User Experience (Key Screens – Updated)

**Screen 1 — Welcome**  
Move your WhatsApp chats safely from Android to iPhone or iPad (companion to official transfer).

**Screen 2 — Direction**  
Android → iPhone / Android → iPad

**Screen 2.5 — Destination Status**  
New or Factory Reset (recommended) vs Already set up (with clear options: guide through reset + official path, or Secure Vault only).

**Screen 3 — Connect**  
Wi-Fi Transfer (recommended)  
USB-C Cable (Experimental – only if Phase 0 validated)

**Screen 5 — Ready Check + “What will actually transfer”**  
Shows estimated size, media inventory, known official limitations, and that full chat history depends on the official path succeeding.

**Screen 7 — Official Import Assistant**  
Guides the real Move to iOS + WhatsApp steps with device-specific instructions.

**Screen 8 — Verification**  
WA Bridge checks passed (transport + vault).  
Clear checklist of items the user should verify inside WhatsApp.

**Screen 9 — Complete**  
Never say “Safe to erase” until both vault and possible checks are done. Prefer: “WA Bridge checks passed. Please open WhatsApp and review your important chats before erasing the old device.”

---

## 5. USB-C ↔ USB-C Mode (Updated)

**Status:** Experimental / Advanced only.

A physical USB-C cable does **not** guarantee arbitrary app-to-app data transfer on iOS/iPadOS.  

Phase 0 must prove sustained, reliable bidirectional transfer on both iPhone and iPad before this path is offered to normal users.  

Until then:
- Default and recommended path = same Wi-Fi
- USB-C appears only under “Connection options” with clear experimental labelling
- Desktop bridge remains the reliable high-volume alternative

---

## 8. WA Bridge Secure Vault (Updated – Critical)

The Secure Vault is created **before** any official transfer begins.

**What it contains (realistic):**
- All media files the app can lawfully access (images, videos, voice notes, documents, stickers where available)
- Inventory / metadata of accessible content
- Integrity manifests and recovery information

**What it does NOT contain / claim:**
- Full decrypted WhatsApp chat database (not accessible without root or official cooperation)
- Ability to inject or restore chats into WhatsApp on iOS/iPadOS outside the official path

The vault is a protected media + inventory archive and recovery checkpoint.  
It is valuable even if the official transfer is interrupted or incomplete.  
It is **not** a full WhatsApp-restorable backup.

Encryption remains AES-256-GCM + Argon2id + platform keystore/keychain as previously specified.

---

## 9. Integrity Engine (Updated)

**Level 1 — Transport integrity** (required)  
Every transferred chunk arrived correctly.

**Level 2 — Archive integrity** (required)  
Vault objects match source objects.

**Level 3 — Migration validation** (limited)  
Only what can legitimately be observed.  
Always accompanied by an explicit user checklist of items that only the user can confirm inside WhatsApp (specific chats, recent messages, etc.).

Never present a screen that implies full internal verification of WhatsApp’s private data.

---

## 18. MVP Definition (Updated – Risk-Mitigated)

V1 is ready only when all of the following work reliably on **both iPhone and iPad**:

**In scope**
- Complete guided companion flow for the official Move to iOS + WhatsApp path
- Clear New vs Already-set-up branching
- Same-Wi-Fi secure pairing + QR fallback
- USB-C only if Phase 0 proves it (otherwise hidden or experimental)
- Secure Vault (media + accessible inventory) created first
- Chunked, resumable transfer of accessible data
- Honest preflight “What will actually transfer” report
- Guided official assistant with correct iPhone/iPad instructions
- Transport + vault integrity verification + user checklist
- Downloadable report
- Windows desktop emergency / high-volume bridge + vault viewer

**Explicitly out of V1**
- Independent full chat history migration or injection into WhatsApp
- Claiming the vault can restore chats into WhatsApp
- WhatsApp Business support (clear warning only)
- Other messengers
- Default cloud upload of content

---

## 23. Definition of Done (Updated)

WA Bridge V1 is production-ready only when:

- A non-technical user can complete the flow on either iPhone or iPad without developer help
- Source data remains untouched
- Transfers of accessible data resume reliably
- Every WA Bridge-transferred object has integrity verification
- No chat content leaves the local path by default
- Capabilities are never overstated
- USB-C is either proven or clearly experimental
- Large transfers survive interruptions
- Privacy/security review passes
- The product never tells the user it is safe to erase the source until the vault exists and possible checks are complete
- Positioning remains “companion to the official path”

---

## Technical Appendix (Key Extracts – Still Valid)

**Minimum OS targets** (re-validate in Phase 0):  
Android 8.0+ (prefer 10+), iOS 15.5+, iPadOS 15.5+.

**Official path limitations** (must surface to users):  
Does not transfer call history, display name, peer-to-peer payments, or WhatsApp Business data. Best results require new or factory-reset destination. Same phone number required. Cannot merge histories later.

**Secure Vault reality check**:  
Media + inventory archive and recovery checkpoint. Not a full chat restore mechanism.

---

## Final Product Experience (Honest Version)

```
Install WA Bridge
      ↓
Choose Android → iPhone or iPad
      ↓
Confirm new/reset or already set up
      ↓
Connect (Wi-Fi recommended)
      ↓
See “What will actually transfer”
      ↓
Secure Vault created first
      ↓
Guided official Move to iOS + WhatsApp step
      ↓
WA Bridge checks + user checklist
      ↓
Complete (with clear next steps)
```

This version closes the critical over-optimism identified in the architecture review while preserving the strong user experience vision.

---

**Related documents**  
- Risk Register & Recommended Scope Adjustments (v1.0)  
- Original technical sections (connection architecture, protocol, recovery, stack, etc.) remain largely unchanged and are still valid under the tightened scope.
