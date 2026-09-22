# tests

Integration test harnesses (Phase 1 onward).

- `protocol/` — wire-format + framing tests
- `migration/` — end-to-end manifest → chunks → resume
- `interruption/` — chaos harness: kill at 25/50/90%, network drop, corrupt chunk (H3)
- `performance/` — 20/100 GB throughput baselines (H5, ADR-P5)
- `device-matrix/` — outputs from the Phase 11 compatibility lab

Rust integration tests live in `core/tests/` for now; these directories hold scenario definitions and fixtures as the harness grows.
