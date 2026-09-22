# services

Optional backend — **normal transfers must never require it** (L7).

- `account/` — subscriptions/entitlements (Phase 12+, post-MVP)
- `entitlement/` — license validation
- `compatibility/` — remote device-compatibility configuration (Phase 11): known-issue mitigations shipped without app updates

Rule: never upload chat content, media, or any private payload. Anonymized crash telemetry only with explicit consent.
