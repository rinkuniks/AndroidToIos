# WA Bridge — Production Roadmap

**Document version:** 1.1  
**Product type:** Local-first device migration, backup, archive, and verification platform  
**Primary UX goal:** A non-technical user should be able to start a migration in a few taps, connect two devices by USB-C or the same Wi-Fi network, and be guided safely through every required step on **both iPhone and iPad**.

> **Product boundary:** WA Bridge is the intelligent companion to Apple’s official Move to iOS.  
> It does **not** claim to bypass WhatsApp encryption, Apple sandboxing, or inject unsupported databases into WhatsApp.  
> Where WhatsApp and Apple provide an official migration path, WA Bridge prepares the devices, creates a protected recovery checkpoint, guides the supported step, and verifies everything it can.  
> The source Android device is never erased automatically.

---

## 1. Product Vision

WA Bridge makes moving WhatsApp history from Android to an **iPhone or iPad** feel simple, safe, and recoverable:

1. Install/open WA Bridge  
2. Select Android → iPhone or Android → iPad  
3. Put both devices next to each other  
4. Choose USB-C Cable or Wi-Fi  
5. Pair automatically or scan one QR code  
6. WA Bridge checks storage, battery, connectivity, WhatsApp readiness, and permissions  
7. Creates a protected recovery checkpoint (Secure Vault)  
8. Tap **Start Transfer**  
9. Guides the official Move to iOS + WhatsApp step  
10. Validates the result and shows an integrity report  

Target interaction count for the normal case: **5–7 meaningful taps**.  
The experience must feel equally polished and reliable on **both iPhone and iPad**.

---

## 2. Core Product Principles

### 2.1 Zero-loss first
Never modify or delete the source WhatsApp data during migration. Create a recovery checkpoint before risky operations.

### 2.2 Local-first privacy
Chat content, media, documents, voice notes, and backups should remain between the user’s devices by default. Cloud servers are not required for normal transfer.

### 2.3 Two connection modes
**USB-C ↔ USB-C** is the preferred high-speed/stable path where platform capabilities permit direct communication.  
**Same Wi-Fi / local peer-to-peer network** is the simplest cable-free fallback.

### 2.4 Automatic fallback
If cable communication is unavailable or restricted by iPadOS/iOS/Android, WA Bridge should offer Wi-Fi automatically rather than presenting a technical error.

### 2.5 Resume instead of restart
A failed 80 GB transfer must resume from the last verified chunk.

### 2.6 One screen at a time
Do not expose filesystem paths, ports, databases, cryptographic terms, or networking details to normal users.

### 2.7 Verify before declaring success
“Transfer finished” and “migration verified” are separate states. Success is shown only after validation.

### 2.8 Equal first-class support for iPhone and iPad
Every feature, screen, transport, verification step, and recovery path must work equally well on both iPhone and iPad. iPad is never treated as a secondary or “bonus” target.

---

## 3. User Experience

### Screen 1 — Welcome

```
WA Bridge

Move your WhatsApp chats safely
from Android to iPhone or iPad

[ Transfer WhatsApp ]

[ Restore / View Backup ]
```

### Screen 2 — Direction

```
Where are you moving?

[ Android → iPhone ]
[ Android → iPad ]
```

### Screen 2.5 — Destination Status

```
Is your iPhone / iPad new or already set up?

[ New or Factory Reset ]     ← Recommended for full official transfer
[ Already set up ]

The official WhatsApp transfer works best on a new or reset device.
We will create a protected recovery copy either way.
```

**If the user chooses “Already set up”:**

```
Your iPhone / iPad is already set up

Official full transfer only works during initial setup.

[ Guide me through reset + official transfer ]
[ Create Secure Vault only ]
```

### Screen 3 — Connect

```
Connect your devices

Recommended
[ 🔌 USB-C Cable ]

No cable?
[ 📶 Wi-Fi Transfer ]

WA Bridge will choose the fastest available connection automatically.
```

Advanced users can open a small “Connection options” link.

### Screen 4 — Pair

**Cable:**

```
Android  ●────────────●  iPhone / iPad
              USB-C

          Connected ✓
```

**Wi-Fi:**

```
Keep both devices on the same Wi-Fi

Android     ◉  ←→  ◉     iPhone / iPad

          Connected ✓
```

If local discovery fails, display a QR code on one device. Scanning it transfers the temporary pairing information.

### Screen 5 — Ready Check

```
Preparing your transfer...

✓ Devices connected
✓ Battery sufficient
✓ 48.7 GB WhatsApp data found
✓ 73.2 GB available on your iPhone / iPad
✓ Permissions ready
✓ Recovery checkpoint ready

[ Start Transfer ]
```

Warnings should contain a direct fix button whenever possible.

### Screen 6 — Transfer

```
Moving your WhatsApp data

████████████████░░░░  76%

36.9 GB of 48.7 GB

Chats          ✓
Photos         ✓
Voice Notes    ●
Videos         ○
Documents      ○

Connection: USB-C
Transfer speed: 84 MB/s
Estimated: 7 min

Keep both devices connected and unlocked.
```

The exact speed shown must be measured, never estimated before a transfer starts.

### Screen 7 — Official Import Assistant

```
Your protected copy is ready ✓

Now complete the official WhatsApp transfer.

1. Keep both devices nearby
2. Continue on your iPhone / iPad
3. Select WhatsApp when prompted

[ Show Me What To Do ]
```

The app must detect what it can automatically and clearly distinguish user-required system steps. Instructions must be correct for both iPhone and iPad.

### Screen 8 — Verification

```
Checking your migration...

✓ Account
✓ Chat inventory
✓ Groups
✓ Photos
✓ Videos
✓ Voice notes
✓ Documents
✓ Backup integrity

Verification complete.
```

### Screen 9 — Complete

```
        Migration Complete ✓

Your protected archive is verified.

Source phone: unchanged
Recovery copy: available
Your iPhone / iPad: verified

[ View Report ]
[ Finish ]
```

Never display “Safe to erase old phone” unless the product has completed every verification that it can actually perform. Prefer:

> “WA Bridge checks passed. Please open WhatsApp and review your important chats before erasing the old Android device.”

---

## 4. Connection Architecture

### 4.1 Smart Transport Layer

```
                    Migration Session
                           │
                    Transport Manager
                           │
          ┌────────────────┼────────────────┐
          │                │                │
       USB-C            Local Wi-Fi       Wi-Fi Direct /
       Transport        Transport          P2P where supported
          │                │                │
          └────────────────┼────────────────┘
                           │
                    Secure Data Channel
                           │
                     Transfer Engine
```

The transfer engine must not care which transport is being used. A common interface lets the application switch transports without rebuilding the migration.

Suggested abstraction:

```
Transport
 ├─ discover()
 ├─ pair()
 ├─ authenticate()
 ├─ openSession()
 ├─ sendChunk()
 ├─ receiveChunk()
 ├─ health()
 ├─ reconnect()
 └─ close()
```

---

## 5. USB-C ↔ USB-C Mode

### Goal
Fastest, most predictable option for large archives where platform capabilities allow it.

### Important engineering constraint
A physical USB-C cable does **not** automatically mean arbitrary app-to-app filesystem access. Android USB host/accessory capabilities and Apple’s supported accessory/network/document mechanisms must be validated on real hardware for **both iPhone and iPad**.

**Phase 0 must prove direct Android ↔ iPhone and Android ↔ iPad USB-C app communication before USB-C is advertised as a guaranteed feature.**

If iOS/iPadOS restrictions prevent the desired direct channel, WA Bridge can still use the cable during official OS migration steps while its own preservation/verification channel falls back to local Wi-Fi or a desktop bridge.

### Desktop Bridge fallback

```
Android ──USB── Windows/Mac ──USB/Wi-Fi── iPhone or iPad
```

This should be an advanced fallback, not the primary consumer experience.

---

## 6. Wi-Fi Mode

### User experience
Both devices join the same Wi-Fi. WA Bridge discovers the other device automatically.

### Discovery
Evaluate:
- Bonjour/mDNS
- Local network service discovery
- QR bootstrap when automatic discovery fails
- Platform-supported peer-to-peer networking where appropriate

### Secure pairing
Discovery does not equal trust.

Recommended flow:

```
Device A creates:
Session ID + ephemeral public key + nonce

              ↓ QR / discovery

Device B receives information

              ↓

Ephemeral key agreement

              ↓

Both devices display same
short verification code

              ↓

Encrypted authenticated session
```

### Wi-Fi transfer requirements
- TLS or equivalent authenticated encrypted channel
- No internet dependency
- Chunked streaming
- Parallel media streams where beneficial
- Backpressure
- Retry per chunk
- Resume manifest
- Network-change recovery
- Screen-lock resilience where OS permits
- Prevent accidental mobile-data relay
- Detect weak Wi-Fi and recommend cable when available

All of the above must work equally on iPhone and iPad.

---

## 7. Transfer Protocol

Do not transfer one giant archive file.

Use chunked objects:

```
Migration
 ├── manifest
 ├── metadata
 ├── object-000001
 │    ├── chunk-0001
 │    ├── chunk-0002
 │    └── ...
 ├── object-000002
 └── ...
```

Each object records:
- object ID
- logical category
- size
- source metadata
- content hash
- chunk count
- completion state

Each chunk records:
- sequence number
- length
- integrity/authentication value
- transfer state

This enables pause/resume and selective retry.

---

## 8. WA Bridge Secure Vault

The product should create an optional portable preservation archive independently of the destination WhatsApp app. The Secure Vault is **always created before** any official transfer begins.

```
WA-Bridge-Vault/
│
├── vault.manifest
├── metadata/
├── indexes/
├── chats/
├── media/
│   ├── images/
│   ├── video/
│   ├── audio/
│   ├── voice-notes/
│   ├── documents/
│   └── stickers/
├── integrity/
└── recovery/
```

Only include data that WA Bridge can lawfully and technically access. Do not claim the vault is a WhatsApp-restorable backup unless an officially supported restore path exists.

### Encryption
Recommended:
- AES-256-GCM for encrypted vault objects
- Platform Keychain/Keystore for local secrets
- Argon2id for password-derived portable-vault keys
- Secure random nonces
- Key versioning
- Recovery-key flow

Passwords and raw encryption keys must never be logged.

---

## 9. Integrity Engine

Every accessible file/object receives a cryptographic digest such as SHA-256.

Example manifest:

```json
{
  "migrationId": "...",
  "sourceDevice": "...",
  "destinationDevice": "...",
  "createdAt": "...",
  "objects": [
    {
      "id": "...",
      "category": "...",
      "size": 12345,
      "hash": "...",
      "transferState": "...",
      "verificationState": "..."
    }
  ]
}
```

Verification levels:

**Level 1 — Transport integrity**  
Every transmitted chunk arrived correctly.

**Level 2 — Archive integrity**  
Destination archive objects match source objects.

**Level 3 — Migration validation**  
Verify what can legitimately be observed after the supported WhatsApp migration.

Never imply Level 3 can inspect WhatsApp’s private iOS/iPadOS container unless an official interface actually permits it. Always supplement with a clear user checklist.

---

## 10. Automatic Preflight

Before starting:

### Source
- WhatsApp detected/compatible
- Permissions
- Accessible-data inventory
- Data size
- Free temporary space
- Battery
- Power-saving restrictions
- App version compatibility

### Destination (iPhone and iPad)
- Supported OS/device
- Free storage
- WA Bridge version
- Local-network permission
- Required setup state for official migration

### Connection
- Transport available
- Estimated link quality
- Cable data capability
- Wi-Fi reachability
- Secure pairing completed

One large **Fix Everything I Can** action should automate recoverable settings and provide one-step instructions for settings the OS requires the user to change.

---

## 11. Recovery System

Possible failures:
- Cable disconnected
- Wi-Fi dropped
- Router changed
- App killed
- Phone/iPad rebooted
- Device locked
- Storage exhausted
- Destination setup interrupted
- User exits WhatsApp migration
- Corrupted source media

State machine:

```
CREATED
   ↓
PREFLIGHT
   ↓
CHECKPOINTING
   ↓
TRANSFERRING
   ↕
PAUSED / RECONNECTING
   ↓
TRANSFER_VERIFIED
   ↓
OFFICIAL_IMPORT_REQUIRED
   ↓
POST_VERIFY
   ↓
COMPLETE
```

Persist state after every meaningful checkpoint.

The user should normally see:

**“Connection interrupted. Reconnecting...”**

rather than an error code.

---

## 12. Technology Stack

### Android
- Kotlin
- Jetpack Compose
- Coroutines / Flow
- WorkManager
- Room / SQLite
- Storage Access Framework where applicable
- Android Keystore
- Network service discovery
- Supported USB APIs

### iPhone / iPad
- Swift
- SwiftUI
- Structured concurrency
- Network framework
- Bonjour where appropriate
- CryptoKit
- Keychain
- FileManager
- Supported document/accessory APIs
- Background-task APIs where applicable

### Shared transfer core
Preferred: **Rust**

Modules:
- Transport abstraction
- Session protocol
- Chunking
- Hashing
- Encryption
- Compression where useful
- Manifests
- Checkpoint/resume
- Error model

Expose the core to:
- Android through JNI
- iOS/iPadOS through an FFI boundary
- Desktop through native bindings

### Desktop companion
- Tauri
- Rust
- React
- TypeScript

Desktop is primarily:
- Emergency bridge
- SSD backup destination
- Archive manager
- Diagnostics
- Support tool

### Backend
Normal transfers should require **no backend**.

Optional backend can handle:
- Account/subscription
- Entitlement
- App configuration
- Anonymized crash telemetry with consent
- Support tickets
- Compatibility metadata

Never upload chat content by default.

---

## 13. Repository Structure

```
wa-bridge/
│
├── apps/
│   ├── android/
│   ├── ios/          # Shared for iPhone and iPad
│   └── desktop/
│
├── core/
│   ├── transport/
│   ├── protocol/
│   ├── crypto/
│   ├── vault/
│   ├── integrity/
│   ├── checkpoint/
│   └── diagnostics/
│
├── services/
│   ├── account/
│   ├── entitlement/
│   └── compatibility/
│
├── docs/
│   ├── architecture/
│   ├── security/
│   ├── ux/
│   ├── qa/
│   └── compliance/
│
└── tests/
    ├── protocol/
    ├── migration/
    ├── interruption/
    ├── performance/
    └── device-matrix/
```

---

## 14. Development Roadmap

### Phase 0 — Feasibility Spike
**Goal:** Prove the product’s hardest assumptions before building UI.

Must test on real hardware pairs:

| Source              | Destination              | Required |
|---------------------|--------------------------|----------|
| CMF Phone 2 Pro     | Recent USB-C iPhone      | Yes      |
| CMF Phone 2 Pro     | iPad 10th gen or newer   | Yes      |
| Second Android      | Recent USB-C iPhone      | Yes      |
| Second Android      | Recent iPad              | Yes      |

**Capability matrix (separate status required for iPhone and iPad):**

| Capability                              | iPhone | iPad | Notes |
|-----------------------------------------|--------|------|-------|
| Same-Wi-Fi secure transfer              |        |      |       |
| QR pairing fallback                     |        |      |       |
| Direct USB-C app-to-app data            |        |      |       |
| Official Move to iOS + WhatsApp flow    |        |      |       |
| Accessible data inventory               |        |      |       |
| Resume after interruption (25/50/90%)   |        |      |       |
| Background / screen-lock behaviour      |        |      |       |
| Large transfer (50–100 GB)              |        |      |       |
| Local Network permission handling       |        |      |       |
| Storage calculation accuracy            |        |      |       |

**Exit criteria:** Written capability matrix with every requirement marked Supported / Supported with official flow / Fallback required / Not supported — **separately for iPhone and for iPad**. No feature is advertised until it has acceptable status on both.

### Phase 1 — Transfer Core
Build: session manager, manifest, chunks, hashes, retries, checkpoints, progress events, resumability.  
Target: arbitrary 20–100 GB test datasets.

### Phase 2 — Android Source App
Build: onboarding, permission wizard, inventory, storage calculation, pairing, migration session, background-safe transfer, checkpoint creation.

### Phase 3 — iPhone / iPad Receiver
Build: pairing, local-network permissions, receiver, encrypted staging, storage monitor, resume, integrity verification.  
Must work equally on both form factors.

### Phase 4 — Smart Connect
Implement: detect USB capability, test transport, use USB when validated, otherwise detect same Wi-Fi, mDNS discovery, QR fallback, connection-health monitoring, seamless reconnect.

### Phase 5 — Secure Vault
Implement: encrypted archive, portable recovery key, integrity manifest, SSD export, desktop vault viewer, restore/export tooling within supported boundaries.

### Phase 6 — Official Migration Assistant
Create a dynamic guided workflow for the supported Android → Apple WhatsApp migration.  
It should recognize device/OS/app versions, show only relevant instructions for iPhone or iPad, preserve WA Bridge checkpoint first, track the stage, and resume WA Bridge verification afterward.  
Do not simulate or bypass protected OS/WhatsApp operations.

### Phase 7 — Verification Engine
Build: source inventory, transfer manifest comparison, vault validation, destination-side checks that APIs legitimately permit, user verification checklist for inaccessible WhatsApp-private state, downloadable migration report.

### Phase 8 — Desktop Recovery Bridge
Windows first, then macOS.  
Functions: Android backup to SSD, encrypted vault management, diagnostics, interrupted-migration recovery, device bridge where mobile-to-mobile direct transport is unavailable.

### Phase 9 — UX Polish
Target:
- Normal migration in 5–7 meaningful taps
- No technical terminology in default UI
- Large text / accessibility
- Light/dark mode
- Localization
- Clear ETA after sufficient transfer data exists
- Meaningful notifications
- Animations that explain state, not decorate it
- No ads during migration
- Equal polish on iPhone and iPad (larger screens used effectively)

### Phase 10 — Production Hardening
- Security audit
- Threat modeling
- Fuzz protocol/parser
- Dependency audit
- Privacy review
- Crash recovery
- Power-loss tests
- Corrupted-file tests
- Low-storage tests
- 100+ GB stress tests
- Slow Wi-Fi tests
- Cable disconnect tests
- Router restart tests
- App-kill tests
- Device reboot tests
- Tests on both iPhone and iPad

### Phase 11 — Device Compatibility Lab
Minimum families:
- Samsung Galaxy
- Google Pixel
- Nothing / CMF
- OnePlus
- Xiaomi / Redmi
- Motorola

Apple:
- USB-C iPhones (multiple models and currently supported iOS versions)
- Supported iPads (10th gen and newer, Air, Pro) across currently supported iPadOS versions

Build a remote compatibility configuration so known device-specific issues can be communicated without forcing an app update.

### Phase 12 — Beta
Start with:
- Internal alpha
- 50-device closed beta
- 500-user beta
- 5,000-user staged release

Track:
- Connection success
- Transfer completion
- Resume success
- Verification completion
- Average throughput
- Failure stage
- Support contacts
- Separate metrics for iPhone vs iPad

Telemetry must not contain message text, contact names, media, encryption keys, or other private content.

### Phase 13 — Public Release
Release only after the supported migration boundaries, store policies, privacy policy, trademark usage, and security model have been reviewed.

---

## 15. UX Rules for Production

1. One primary button per screen.  
2. Say “Connect your devices”, not “Initialize transport.”  
3. Say “Checking your data”, not “Computing SHA-256 manifest.”  
4. Automatically select the best transport.  
5. Never make users type IP addresses.  
6. QR pairing is the universal fallback.  
7. Explain permission requests before the OS prompt.  
8. Keep source data untouched.  
9. Save progress continuously.  
10. Show errors with an immediate action.  
11. Do not show unsupported certainty.  
12. Never require cloud upload for a local transfer.  
13. Let users leave the transfer screen without losing progress.  
14. Provide accessibility labels and scalable text.  
15. Keep ads and upsells out of an active migration.  
16. Always use the correct device name (“iPhone” or “iPad”) based on the actual paired device.  
17. Always tell the user the truth about what the official path can and cannot transfer.  
18. Never say “Safe to erase old phone” until both the Secure Vault and all possible verification steps are complete.  
19. iPhone and iPad must feel equally polished. Larger iPad screens should be used for clearer progress and verification.

---

## 16. Security Threat Model

Protect against:
- Another device joining the migration
- Local-network sniffing
- Replay attacks
- Altered chunks
- Malicious/corrupt archives
- Path traversal
- Decompression bombs
- Compromised temporary files
- Sensitive logs
- Leaked recovery keys
- Malicious QR pairing codes

Security controls:
- Authenticated pairing
- Ephemeral session keys
- Encrypted transport
- Authenticated encryption
- Signed/authenticated manifests where appropriate
- Strict parser limits
- Sandboxed file handling
- Automatic temporary-file cleanup
- Secrets redaction
- Encrypted vault
- Explicit user authorization for exports

---

## 17. Performance Goals

Initial engineering targets, to validate on hardware:

- Startup: < 2 seconds on modern supported hardware
- Pairing: normally < 10 seconds
- Transfer progress updates: smooth and throttled
- Memory: streaming architecture; never load large videos into RAM
- Resume: recover without retransmitting verified chunks
- Integrity: every transferred object verified
- 100 GB migration: supported without architectural changes
- Storage: warn before insufficient-space failure

Do not promise a fixed MB/s because cable, device, filesystem, encryption, Wi-Fi, and OS limitations differ.

---

## 18. MVP Definition

V1 should do these things extremely well on **both iPhone and iPad**:

- Android → iPhone workflow
- Android → iPad workflow
- Clear “New or Already set up” branching
- Same-Wi-Fi secure pairing and transfer
- QR fallback
- USB-C path if Phase 0 proves direct support on both platforms
- Encrypted preservation vault (always created first)
- Chunked/resumable transfer
- Preflight
- Official migration assistant (correct for both devices)
- Integrity verification
- Migration report + user checklist
- Windows/SSD emergency backup option

Do **not** put Telegram, Signal, cloud sync, AI chat search, or dozens of device directions into V1.  
Do **not** claim the ability to inject full chats into an already-activated WhatsApp account.

---

## 19. V2

After V1 stability:
- Android → Android
- Apple → Android
- Apple → Apple archive tooling
- Optional NAS destination
- Optional user-controlled cloud vault
- Scheduled archive health checks
- Family migration mode
- Advanced searchable archive
- Duplicate-media analysis

Support for other messaging platforms should be separate adapters and only use officially/legitimately accessible data.

---

## 20. Business Model

A clean model:

**Free**
- Compatibility scan
- Migration readiness report
- Small test transfer
- Connection test

**One-Time Migration**
- Full migration assistant
- Backup checkpoint
- Verification report

**Pro**
- Encrypted archive management
- Multiple migrations/devices
- Desktop vault tools
- Advanced recovery
- Long-term archive features

Avoid monetizing user chat content or media.

---

## 21. Success Metrics

Primary:
- Completed verified migrations / started migrations

Supporting:
- First-attempt pairing rate
- Connection recovery rate
- Resume success rate
- Transfer integrity failure rate
- Migration-support contact rate
- Time from launch to paired
- Percentage of users completing without support
- Separate metrics for iPhone vs iPad

The most important qualitative metric:

> Could a family member who does not understand backups, USB modes, Wi-Fi networking, encryption, or file systems complete the process on **either an iPhone or an iPad** without help?

If not, the UX is not finished.

---

## 22. Immediate Prototype Plan — Reference Devices

Use current devices as the first reference implementation.

### Prototype A — Wi-Fi
- Android sender app
- iPhone receiver app
- iPad receiver app
- Local discovery
- QR pairing
- Secure session
- 10 GB synthetic transfer
- Interrupt at 25%, 50%, 90%
- Verify resume and hashes on both Apple devices

### Prototype B — USB-C
- Connect both devices with known data-capable USB-C cable
- Enumerate platform-supported communication options on iPhone and iPad
- Prove sustained bidirectional application data transfer
- Test disconnect/reconnect
- Document iOS and iPadOS limitations separately

### Prototype C — Migration
- Create protected source inventory/checkpoint
- Run the current officially supported WhatsApp/Apple migration flow
- Document every user interaction for both iPhone and iPad
- Determine which steps WA Bridge can detect/automate and which must remain system-controlled
- Validate what can actually be verified afterward

### Prototype D — UX
Put A–C behind the clean consumer flow shown in Section 3.  
No engineering terminology should leak into this consumer flow.

---

## 23. Definition of Done

WA Bridge V1 is production-ready only when:

- A non-technical tester can use it without developer help on **both iPhone and iPad**
- Source data remains unchanged after failures
- Transfers resume reliably on both platforms
- Every WA Bridge-transferred object has integrity verification
- No chat content leaves the local migration path by default
- Unsupported WhatsApp/iOS/iPadOS operations are never represented as supported
- Direct USB capability has been proven on supported device combinations or is clearly handled through a fallback
- Large migrations survive connection interruptions
- Privacy/security review passes
- Compatibility and failure states have clear UX on both device types
- The application never instructs the user to erase the source until preservation and verification steps are complete

---

## Final Product Experience

The user should perceive only this:

```
Install WA Bridge
      ↓
Choose Android → iPhone or Android → iPad
      ↓
Confirm new or already set up
      ↓
Connect with USB-C or same Wi-Fi
      ↓
Devices pair automatically
      ↓
WA Bridge checks everything + creates Secure Vault
      ↓
Tap “Start Transfer”
      ↓
Data moves securely (resumable)
      ↓
WA Bridge guides the official WhatsApp / Apple step
      ↓
Verification
      ↓
Migration Complete
```

Everything complicated — network discovery, cryptography, chunks, retries, manifests, checkpoints, compatibility, transport selection, integrity checks, and recovery — belongs behind that experience, and works equally well on **iPhone and iPad**.
