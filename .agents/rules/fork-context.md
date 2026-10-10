---
description: Fork identity, mission, and branch invariants.
alwaysApply: true
agents:
  - main
  - implementer
trigger: always_on
---

# Fork Context

## Mission

- **Identity**: Public fork of `doy/rbw` (unofficial Bitwarden CLI in Rust).
- **Goal**: unlock `rbw-agent` through a device approval ("Log in with device" auth requests) from the Bitwarden mobile app, so the master password is never typed nor stored on the client. Fallback to the master-password pinentry stays available.
- **Upstream stance**: the maintainer considers `rbw` feature-complete for himself but reviews and merges feature PRs (`README.md`, "Maintenance"). Aim for an upstreamable change.

## Invariants

- **Working branch**: `kml93` (default branch). It carries `.agents/`.
- **`main`**: clean mirror of `upstream/main`. Never commit to it.
- **Upstream PR branches**: start from `main`, never contain `.agents/`.
- **Remotes**: `origin` = `kml93/rbw`, `upstream` = `doy/rbw`.
