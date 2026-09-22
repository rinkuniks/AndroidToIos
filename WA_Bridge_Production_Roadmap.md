# WA Bridge --- Production Roadmap

## Simple Android → iPhone/iPad WhatsApp Migration & Preservation Platform

**Document version:** 1.0\
**Product type:** Local-first device migration, backup, archive, and
verification platform\
**Primary UX goal:** A non-technical user should be able to start a
migration in a few taps, connect two devices by USB-C or the same Wi-Fi
network, and be guided safely through every required step.

> **Product boundary:** WA Bridge must not claim to bypass WhatsApp
> encryption, Apple sandboxing, or inject unsupported databases into
> WhatsApp. Where WhatsApp/Apple provide an official migration/import
> mechanism, WA Bridge orchestrates that supported step and verifies the
> surrounding backup/migration. The source device is never erased
> automatically.

------------------------------------------------------------------------

# 1. Product Vision

WA Bridge should make moving WhatsApp history from Android to an Apple
device feel like moving data between two phones during initial setup:

1.  Install/open WA Bridge.
2.  Select **Android → iPhone/iPad**.
3.  Put both devices next to each other.
4.  Choose **USB-C Cable** or **Wi-Fi**.
5.  Pair automatically or scan one QR code.
6.  WA Bridge checks storage, battery, connectivity, WhatsApp readiness,
    and permissions.
7.  Tap **Start Transfer**.
8.  The application performs backup/preservation, guides the supported
    WhatsApp migration step, and continuously reports progress.
9.  WA Bridge validates the result.
10. User sees **Migration Complete** and an integrity report.

Target interaction count for the normal case: **5--7 meaningful taps**.

------------------------------------------------------------------------

# 2. Core Product Principles

## 2.1 Zero-loss first

Never modify or delete the source WhatsApp data during migration. Create
a recovery checkpoint before risky operations.

## 2.2 Local-first privacy

Chat content, media, documents, voice notes, and backups should remain
between the user's devices by default. Cloud servers are not required
for normal transfer.

## 2.3 Two connection modes

**USB-C ↔ USB-C** is the preferred high-speed/stable path where platform
capabilities permit direct communication.\
**Same Wi-Fi / local peer-to-peer network** is the simplest cable-free
fallback.

## 2.4 Automatic fallback

If cable communication is unavailable or restricted by iPadOS/Android,
WA Bridge should offer Wi-Fi automatically rather than presenting a
technical error.

## 2.5 Resume instead of restart

A failed 80 GB transfer must resume from the last verified chunk.

## 2.6 One screen at a time

Do not expose filesystem paths, ports, databases, cryptographic terms,
or networking details to normal users.

## 2.7 Verify before declaring success

"Transfer finished" and "migration verified" are separate states.
Success is shown only after validation.

------------------------------------------------------------------------

# 3. User Experience

## Screen 1 --- Welcome

``` text
┌──────────────────────────────────┐
│            WA Bridge             │
│                                  │
│   Move your chats safely         │
│                                  │
│   [ Transfer WhatsApp ]          │
│                                  │
│   [ Restore / View Backup ]      │
└──────────────────────────────────┘
```

## Screen 2 --- Direction

``` text
Where are you moving?

[ Android → iPhone / iPad ]
[ iPhone → Android          ]
[ Android → Android         ]
[ iPhone / iPad → Apple     ]
```

V1 focuses on **Android → iPhone/iPad**.

## Screen 3 --- Connect

``` text
Connect your devices

Recommended
[ 🔌 USB-C Cable ]

No cable?
[ 📶 Wi-Fi Transfer ]

WA Bridge will choose the fastest
available connection automatically.
```

Use a **Smart Connect** button in the final UX. Advanced users can
manually select the transport from a small "Connection options" link.

## Screen 4 --- Pair

Cable:

``` text
Android ●────────────● iPad
          USB-C

        Connected ✓
```

Wi-Fi:

``` text
Keep both devices on the same Wi-Fi

Android     ◉  ←→  ◉     iPad

          Connected ✓
```

If local discovery fails, display a QR code on one device. Scanning it
transfers the temporary pairing information.

## Screen 5 --- Ready Check

``` text
Preparing your transfer...

✓ Devices connected
✓ Battery sufficient
✓ 48.7 GB WhatsApp data found
✓ 73.2 GB available on destination
✓ Permissions ready
✓ Recovery checkpoint ready

[ Start Transfer ]
```

Warnings should contain a direct fix button whenever possible.

## Screen 6 --- Transfer

``` text
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

Keep both devices connected.
```

The exact speed shown must be measured, never estimated before a
transfer starts.

## Screen 7 --- WhatsApp Import Assistant

If an official Apple/WhatsApp setup/import step is required, WA Bridge
changes into a guided assistant:

``` text
Your protected copy is ready ✓

Now complete the WhatsApp import.

1. Keep both devices nearby
2. Continue Apple setup
3. Select WhatsApp when prompted

[ Show Me What To Do ]
```

The app must detect what it can automatically and clearly distinguish
user-required system steps.

## Screen 8 --- Verification

``` text
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

## Screen 9 --- Complete

``` text
        Migration Complete ✓

Your protected archive is verified.

Source phone: unchanged
Recovery copy: available
Destination: verified

[ View Report ]
[ Finish ]
```

Never display "Safe to erase old phone" unless the product has completed
every verification that it can actually perform. Prefer: **"WA Bridge
checks passed. Review your important chats before erasing the old
device."**

------------------------------------------------------------------------

# 4. Connection Architecture

## 4.1 Smart Transport Layer

``` text
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

The transfer engine must not care which transport is being used. A
common interface lets the application switch transports without
rebuilding the migration.

Suggested abstraction:

``` text
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

------------------------------------------------------------------------

# 5. USB-C ↔ USB-C Mode

## Goal

Fastest, most predictable option for large archives.

## Flow

1.  User connects Android and iPad/iPhone using a data-capable USB-C
    cable.
2.  Apps detect the connection where platform APIs permit.
3.  Devices establish an authenticated session.
4.  Destination and source exchange capabilities.
5.  Transfer engine begins chunked transfer.
6.  Every chunk is authenticated and verified.
7.  Interrupted transfers resume from the last verified chunk.

## Important engineering constraint

A physical USB-C cable does **not** automatically mean arbitrary
app-to-app filesystem access. Android USB host/accessory capabilities
and Apple's supported accessory/network/document mechanisms must be
validated on real hardware. Therefore:

**Phase 0 must prove direct Android ↔ iPad USB-C app communication
before USB-C is advertised as a guaranteed feature.**

If iPadOS restrictions prevent the desired direct channel, WA Bridge can
still use the cable during official OS migration steps while its own
preservation/verification channel falls back to local Wi-Fi or a desktop
bridge.

## Desktop Bridge fallback

``` text
Android ──USB── Windows/Mac ──USB/Wi-Fi── iPad
```

This should be an advanced fallback, not the primary consumer
experience.

------------------------------------------------------------------------

# 6. Wi-Fi Mode

## User experience

Both devices join the same Wi-Fi. WA Bridge discovers the other device
automatically.

## Discovery

Evaluate: - Bonjour/mDNS - Local network service discovery - QR
bootstrap when automatic discovery fails - Platform-supported
peer-to-peer networking where appropriate

## Secure pairing

Discovery does not equal trust.

Recommended flow:

``` text
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

## Wi-Fi transfer requirements

-   TLS or equivalent authenticated encrypted channel
-   No internet dependency
-   Chunked streaming
-   Parallel media streams where beneficial
-   Backpressure
-   Retry per chunk
-   Resume manifest
-   Network-change recovery
-   Screen-lock resilience where OS permits
-   Prevent accidental mobile-data relay
-   Detect weak Wi-Fi and recommend cable when available

------------------------------------------------------------------------

# 7. Transfer Protocol

Do not transfer one giant archive file.

Use chunked objects:

``` text
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

Each object records: - object ID - logical category - size - source
metadata - content hash - chunk count - completion state

Each chunk records: - sequence number - length -
integrity/authentication value - transfer state

This enables pause/resume and selective retry.

------------------------------------------------------------------------

# 8. WA Bridge Secure Vault

The product should create an optional portable preservation archive
independently of the destination WhatsApp app.

``` text
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

Only include data that WA Bridge can lawfully and technically access. Do
not claim the vault is a WhatsApp-restorable backup unless an officially
supported restore path exists.

## Encryption

Recommended: - AES-256-GCM for encrypted vault objects - Platform
Keychain/Keystore for local secrets - Argon2id for password-derived
portable-vault keys - Secure random nonces - Key versioning -
Recovery-key flow

Passwords and raw encryption keys must never be logged.

------------------------------------------------------------------------

# 9. Integrity Engine

Every accessible file/object receives a cryptographic digest such as
SHA-256.

Example manifest:

``` text
{
  migrationId,
  sourceDevice,
  destinationDevice,
  createdAt,
  objects: [
    {
      id,
      category,
      size,
      hash,
      transferState,
      verificationState
    }
  ]
}
```

Verification levels:

**Level 1 --- Transport integrity**\
Every transmitted chunk arrived correctly.

**Level 2 --- Archive integrity**\
Destination archive objects match source objects.

**Level 3 --- Migration validation**\
Verify what can legitimately be observed after the supported WhatsApp
migration.

Never imply Level 3 can inspect WhatsApp's private iOS container unless
an official interface actually permits it.

------------------------------------------------------------------------

# 10. Automatic Preflight

Before starting:

### Source

-   WhatsApp detected/compatible
-   permissions
-   accessible-data inventory
-   data size
-   free temporary space
-   battery
-   power-saving restrictions
-   app version compatibility

### Destination

-   supported OS/device
-   free storage
-   WA Bridge version
-   local-network permission
-   required setup state for official migration

### Connection

-   transport available
-   estimated link quality
-   cable data capability
-   Wi-Fi reachability
-   secure pairing completed

One large **Fix Everything I Can** action should automate recoverable
settings and provide one-step instructions for settings the OS requires
the user to change.

------------------------------------------------------------------------

# 11. Recovery System

Possible failures: - cable disconnected - Wi-Fi dropped - router
changed - app killed - phone rebooted - iPad locked - storage
exhausted - destination setup interrupted - user exits WhatsApp
migration - corrupted source media

State machine:

``` text
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

**"Connection interrupted. Reconnecting..."**

rather than an error code.

------------------------------------------------------------------------

# 12. Technology Stack

## Android

-   Kotlin
-   Jetpack Compose
-   Coroutines / Flow
-   WorkManager
-   Room / SQLite
-   Storage Access Framework where applicable
-   Android Keystore
-   Network service discovery
-   supported USB APIs

## iPhone/iPad

-   Swift
-   SwiftUI
-   structured concurrency
-   Network framework
-   Bonjour where appropriate
-   CryptoKit
-   Keychain
-   FileManager
-   supported document/accessory APIs
-   background-task APIs where applicable

## Shared transfer core

Preferred: **Rust**

Modules: - transport abstraction - session protocol - chunking -
hashing - encryption - compression where useful - manifests -
checkpoint/resume - error model

Expose the core to: - Android through JNI - iOS through an FFI
boundary - desktop through native bindings

## Desktop companion

-   Tauri
-   Rust
-   React
-   TypeScript

Desktop is primarily: - emergency bridge - SSD backup destination -
archive manager - diagnostics - support tool

## Backend

Normal transfers should require **no backend**.

Optional backend can handle: - account/subscription - entitlement - app
configuration - anonymized crash telemetry with consent - support
tickets - compatibility metadata

Never upload chat content by default.

------------------------------------------------------------------------

# 13. Repository Structure

``` text
wa-bridge/
│
├── apps/
│   ├── android/
│   ├── ios/
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

------------------------------------------------------------------------

# 14. Development Roadmap

## Phase 0 --- Feasibility Spike

**Goal:** prove the product's hardest assumptions before building UI.

Test on the actual target pair first: - CMF Phone 2 Pro / Android 16 -
iPad 10th generation / current supported iPadOS - USB-C ↔ USB-C -
same-Wi-Fi transfer - official WhatsApp Android→Apple migration path -
what metadata/data each third-party app can legitimately access - iPadOS
background behavior - large-file transfer - destination verification
limitations

**Exit criteria:** written capability matrix marking every requirement
as Supported / Supported with official flow / Fallback required / Not
supported.

## Phase 1 --- Transfer Core

Build: - session manager - manifest - chunks - hashes - retries -
checkpoints - progress events - resumability

Target: arbitrary 20--100 GB test datasets.

## Phase 2 --- Android Source App

Build: - onboarding - permission wizard - inventory - storage
calculation - pairing - migration session - background-safe transfer -
checkpoint creation

## Phase 3 --- iPad/iPhone Receiver

Build: - pairing - local-network permissions - receiver - encrypted
staging - storage monitor - resume - integrity verification

## Phase 4 --- Smart Connect

Implement: 1. detect USB capability 2. test transport 3. use USB when
validated 4. otherwise detect same Wi-Fi 5. mDNS discovery 6. QR
fallback 7. connection-health monitoring 8. seamless reconnect

## Phase 5 --- Secure Vault

Implement: - encrypted archive - portable recovery key - integrity
manifest - SSD export - desktop vault viewer - restore/export tooling
within supported boundaries

## Phase 6 --- Official Migration Assistant

Create a dynamic guided workflow for the supported Android→Apple
WhatsApp migration.

It should: - recognize device/OS/app versions - show only relevant
instructions - preserve WA Bridge checkpoint first - track the stage -
resume WA Bridge verification afterward

Do not simulate or bypass protected OS/WhatsApp operations.

## Phase 7 --- Verification Engine

Build: - source inventory - transfer manifest comparison - vault
validation - destination-side checks that APIs legitimately permit -
user verification checklist for inaccessible WhatsApp-private state -
downloadable migration report

## Phase 8 --- Desktop Recovery Bridge

Windows first, then macOS.

Functions: - Android backup to SSD - encrypted vault management -
diagnostics - interrupted-migration recovery - device bridge where
mobile-to-mobile direct transport is unavailable

## Phase 9 --- UX Polish

Target: - normal migration in 5--7 meaningful taps - no technical
terminology in default UI - large text/accessibility - light/dark mode -
localization - clear ETA after sufficient transfer data exists -
meaningful notifications - animations that explain state, not decorate
it - no ads during migration

## Phase 10 --- Production Hardening

-   security audit
-   threat modeling
-   fuzz protocol/parser
-   dependency audit
-   privacy review
-   crash recovery
-   power-loss tests
-   corrupted-file tests
-   low-storage tests
-   100+ GB stress tests
-   slow Wi-Fi tests
-   cable disconnect tests
-   router restart tests
-   app-kill tests
-   device reboot tests

## Phase 11 --- Device Compatibility Lab

Minimum families: - Samsung Galaxy - Google Pixel - Nothing / CMF -
OnePlus - Xiaomi / Redmi - Motorola

Apple: - USB-C iPhones - supported iPads - multiple currently supported
iOS/iPadOS versions

Build a remote compatibility configuration so known device-specific
issues can be communicated without forcing an app update.

## Phase 12 --- Beta

Start with: - internal alpha - 50-device closed beta - 500-user beta -
5,000-user staged release

Track: - connection success - transfer completion - resume success -
verification completion - average throughput - failure stage - support
contacts

Telemetry must not contain message text, contact names, media,
encryption keys, or other private content.

## Phase 13 --- Public Release

Release only after the supported migration boundaries, store policies,
privacy policy, trademark usage, and security model have been reviewed.

------------------------------------------------------------------------

# 15. UX Rules for Production

1.  **One primary button per screen.**
2.  Say **"Connect your devices"**, not "Initialize transport."
3.  Say **"Checking your data"**, not "Computing SHA-256 manifest."
4.  Automatically select the best transport.
5.  Never make users type IP addresses.
6.  QR pairing is the universal fallback.
7.  Explain permission requests before the OS prompt.
8.  Keep source data untouched.
9.  Save progress continuously.
10. Show errors with an immediate action.
11. Do not show unsupported certainty.
12. Never require cloud upload for a local transfer.
13. Let users leave the transfer screen without losing progress.
14. Provide accessibility labels and scalable text.
15. Keep ads and upsells out of an active migration.

------------------------------------------------------------------------

# 16. Security Threat Model

Protect against: - another device joining the migration - local-network
sniffing - replay attacks - altered chunks - malicious/corrupt
archives - path traversal - decompression bombs - compromised temporary
files - sensitive logs - leaked recovery keys - malicious QR pairing
codes

Security controls: - authenticated pairing - ephemeral session keys -
encrypted transport - authenticated encryption - signed/authenticated
manifests where appropriate - strict parser limits - sandboxed file
handling - automatic temporary-file cleanup - secrets redaction -
encrypted vault - explicit user authorization for exports

------------------------------------------------------------------------

# 17. Performance Goals

Initial engineering targets, to validate on hardware:

-   Startup: \<2 seconds on modern supported hardware
-   Pairing: normally \<10 seconds
-   Transfer progress updates: smooth and throttled
-   Memory: streaming architecture; never load large videos into RAM
-   Resume: recover without retransmitting verified chunks
-   Integrity: every transferred object verified
-   100 GB migration: supported without architectural changes
-   Storage: warn before insufficient-space failure

Do not promise a fixed MB/s because cable, device, filesystem,
encryption, Wi-Fi, and OS limitations differ.

------------------------------------------------------------------------

# 18. MVP Definition

V1 should do these things extremely well:

-   Android → iPhone/iPad workflow
-   same-Wi-Fi secure pairing and transfer
-   QR fallback
-   USB-C path if Phase 0 proves direct support
-   encrypted preservation vault
-   chunked/resumable transfer
-   preflight
-   official migration assistant
-   integrity verification
-   migration report
-   Windows/SSD emergency backup option

Do **not** put Telegram, Signal, cloud sync, AI chat search, or dozens
of device directions into V1.

------------------------------------------------------------------------

# 19. V2

After V1 stability: - Android → Android - Apple → Android - Apple →
Apple archive tooling - optional NAS destination - optional
user-controlled cloud vault - scheduled archive health checks - family
migration mode - advanced searchable archive - duplicate-media analysis

Support for other messaging platforms should be separate adapters and
only use officially/legitimately accessible data.

------------------------------------------------------------------------

# 20. Business Model

A clean model:

**Free** - compatibility scan - migration readiness report - small test
transfer - connection test

**One-Time Migration** - full migration assistant - backup checkpoint -
verification report

**Pro** - encrypted archive management - multiple migrations/devices -
desktop vault tools - advanced recovery - long-term archive features

Avoid monetizing user chat content or media.

------------------------------------------------------------------------

# 21. Success Metrics

Primary: - completed verified migrations / started migrations

Supporting: - first-attempt pairing rate - connection recovery rate -
resume success rate - transfer integrity failure rate -
migration-support contact rate - time from launch to paired - percentage
of users completing without support

The most important qualitative metric:

> **Could a family member who does not understand backups, USB modes,
> Wi-Fi networking, encryption, or file systems complete the process
> without help?**

If not, the UX is not finished.

------------------------------------------------------------------------

# 22. Immediate Prototype Plan --- CMF Phone 2 Pro → iPad 10th Gen

Use the current devices as the first reference implementation.

### Prototype A --- Wi-Fi

-   Android sender app
-   iPad receiver app
-   local discovery
-   QR pairing
-   secure session
-   10 GB synthetic transfer
-   interrupt at 25%, 50%, 90%
-   verify resume and hashes

### Prototype B --- USB-C

-   connect both devices with known data-capable USB-C cable
-   enumerate platform-supported communication options
-   prove sustained bidirectional application data transfer
-   test disconnect/reconnect
-   document iPadOS limitations

### Prototype C --- Migration

-   create protected source inventory/checkpoint
-   run the current officially supported WhatsApp/Apple migration flow
-   document every user interaction
-   determine which steps WA Bridge can detect/automate and which must
    remain system-controlled
-   validate what can actually be verified afterward

### Prototype D --- UX

Put A--C behind:

``` text
Transfer WhatsApp
       ↓
Android → iPad
       ↓
Connect Devices
       ↓
Ready ✓
       ↓
Start Transfer
       ↓
Complete WhatsApp Setup
       ↓
Verify
       ↓
Done
```

No engineering terminology should leak into this consumer flow.

------------------------------------------------------------------------

# 23. Definition of Done

WA Bridge V1 is production-ready only when:

-   a non-technical tester can use it without developer help
-   source data remains unchanged after failures
-   transfers resume reliably
-   every WA Bridge-transferred object has integrity verification
-   no chat content leaves the local migration path by default
-   unsupported WhatsApp/iOS operations are never represented as
    supported
-   direct USB capability has been proven on supported device
    combinations or is clearly handled through a fallback
-   large migrations survive connection interruptions
-   privacy/security review passes
-   compatibility and failure states have clear UX
-   the application never instructs the user to erase the source until
    preservation and verification steps are complete

------------------------------------------------------------------------

# Final Product Experience

The user should perceive only this:

``` text
Install WA Bridge
      ↓
Choose old and new device
      ↓
Connect with USB-C
       OR
Use same Wi-Fi
      ↓
Devices pair automatically
      ↓
WA Bridge checks everything
      ↓
Tap "Start Transfer"
      ↓
Protected checkpoint created
      ↓
Data moves securely
      ↓
WA Bridge guides required
WhatsApp/Apple system step
      ↓
Verification
      ↓
Migration Complete
```

Everything complicated---network discovery, cryptography, chunks,
retries, manifests, checkpoints, compatibility, transport selection,
integrity checks, and recovery---belongs behind that experience.
