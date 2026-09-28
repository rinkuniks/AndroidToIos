# WA Bridge — Android to iPhone and iPad

WA Bridge is a local-first companion to the official Move to iOS and WhatsApp migration process. It is intended to prepare devices, preserve accessible media and inventory, guide the official transfer, and check the integrity of files it transfers.

**This project is under development. It is not a replacement for Move to iOS or an independent WhatsApp chat restoration tool.**

## Does resetting an iPad make WA Bridge work?

No. Apple's setup migration and WA Bridge's planned receiver are separate flows:

| Flow | Android side | iPad side |
| --- | --- | --- |
| Apple's Move to iOS | Move to iOS app | Built-in migration flow during device setup |
| WA Bridge receiver | WA Bridge Android app | WA Bridge iPhone/iPad app, installed after device setup |

WA Bridge does not connect using the pairing code displayed by Apple's Move to iOS setup screen. Resetting the iPad does not install or enable a WA Bridge receiver.

The iPhone/iPad receiver currently has only a [design README](wa-bridge/apps/ios/README.md). It must be implemented, built, signed, and installed before WA Bridge's planned receiving features can work. Generating an iPad build alone is not yet sufficient because the app implementation is missing. The plan is one shared iPhone and iPad app.

An iPad reset is not required to test that future receiver on an already configured device. For Apple's setup migration, follow [Apple's Move to iOS instructions](https://support.apple.com/en-us/118670).

## Scope and limitations

- Preserve accessible media, inventory, and manifests in a protected archive.
- Use the Rust transfer core for chunk integrity checks and resumable transfers.
- Guide users through the supported official migration process.
- Never represent the archive as a WhatsApp-restorable chat backup.
- Never bypass WhatsApp encryption or inject databases into WhatsApp.
- Never automatically erase source data.

Full WhatsApp chat restoration depends on the official supported path. Planned iPad support is not evidence that direct Android-to-iPad WhatsApp chat migration has been validated; device capabilities still require hardware verification.

## Repository layout

| Path | Purpose |
| --- | --- |
| [wa-bridge/apps/android](wa-bridge/apps/android) | Android source application |
| [wa-bridge/apps/ios](wa-bridge/apps/ios) | Planned shared iPhone/iPad receiver; README only |
| [wa-bridge/apps/desktop](wa-bridge/apps/desktop) | Planned desktop recovery bridge; README only |
| [wa-bridge/core](wa-bridge/core) | Rust transfer, integrity, encryption, and resume core |
| [wa-bridge/docs](wa-bridge/docs) | Architecture and hardware validation documentation |
| [wa-bridge/tests](wa-bridge/tests) | Cross-component test documentation |

## Development

The Android project is located in `wa-bridge/apps/android`. The iOS and desktop applications are not yet runnable implementations. End-to-end Android-to-iPad transfer is not currently ready to test.

To run the Rust core tests with a Rust toolchain installed, start from the repository root:

```sh
cd wa-bridge/core
cargo test
```

These tests cover the core; they do not establish successful migration into WhatsApp or hardware compatibility with an iPad.

## Project documentation

- [Component overview](wa-bridge/README.md)
- [Current scope and production roadmap](WA_Bridge_Production_Roadmap_v1.2.md)
- [Phase implementation plan](WA_Bridge_Phase_Implementation_Plan.md)
- [Risk register](WA_Bridge_Risk_Register_v1.0.md)

The version 1.2 roadmap defines the product boundaries. Hardware feasibility gates must pass before claiming supported migration behavior.
