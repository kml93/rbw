---
description: Repository layout, naming, and how to work with the user on device-bound steps.
alwaysApply: true
agents:
  - main
  - implementer
trigger: always_on
---

# Work Context

- For file layout and naming rules (handoffs, spikes, ADRs, drafts), read `./.agents/docs/domain.md`.

## User interaction

- Steps needing the user physically (fingerprint, approval in the Bitwarden app, master password, 2FA code) MUST be prepared first, then announced with the `ask` tool before running. The user is not always at the phone.
- Never conclude that a mechanism fails when the user did not interact.
- Nothing durable on the user's machines (config, installed binaries, keystore keys) without explicit approval. Experiments go to a temporary directory; keep what must survive in this repository.
