---
title: "ADR-005: Store API keys via fnox with an age provider instead of a plaintext .env"
status: accepted
date: 2026-08-23
context:
  - path: .context/findings/2026-08-23-adr-candidates-from-python-to-rust-port-git-history.md
---

**Status:** Accepted
**Date:** 2026-08-23

## Context

`STREETVIEW_API_KEY` and `DIRECTIONS_API_KEY` were originally read from a plaintext, untracked `.env` file (itself
replacing the Python version's untracked `API_KEYS.py`). A plaintext local file has no protection if accidentally
staged, and there was no committed record of which keys the project needs or how to provision them.

## Decision

Adopt [fnox](https://fnox.jdx.dev/) with an `age` provider (commits `29d59a7`, `d525b55`): pin `fnox`/`age` via
`mise.toml`, commit a `fnox.toml` containing only the public recipient key and age-encrypted ciphertext (no secret
material in git — decryption requires the private age key at `~/.config/fnox/age.txt`, which lives outside the
repo), and add `.env` to `.gitignore` as defense in depth regardless of which secrets tool is in use.

## Consequences

Gains: the two required API keys are documented by the committed `fnox.toml` itself, encrypted at rest in git
history, and decryptable only with a key that never enters the repo. Cost: anyone who needs to run the CLI (a new
contributor, a CI job) now depends on provisioning an `age` key out-of-band before secrets decrypt — there is no
plaintext fallback path, by design.
