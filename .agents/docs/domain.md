# Domain Docs

Single repository context.

## Rules

- Lazy creation: create glossary (`.agents/CONTEXT.md`) and ADRs only when terms or decisions are resolved, never upfront.
- Conflict guard: flag any conflict with an existing ADR instead of silently overriding it.

## Layout

- `.agents/rules/`: always-on agent rules.
- `.agents/docs/`: governance (`domain.md`) & `adr/` (decisions).
- `.agents/scratch/`: working material.
  - `handoffs/`: session handoffs, deleted once superseded by a published spec/issue/PR.
  - `spikes/`: prototypes. **Tracked in git** here (unlike other forks): they are the proven reference for the implementation. Each spike states how to run it in its header.
  - `local/`: git-ignored notes about the user's own infrastructure. Never commit, never quote in public artifacts.

## Naming

- Grammar: `@` field separator, `_` in slug, `.` locale (`en`/`fr`). 4-digit zero-padded numbers (`0001`).
- Handoffs: `<slug_snake>.<locale>.md` — content only, no process narration; rules/skills stay out.
- Spikes: `<slug_snake>/` directory.
- ADRs: `<number>@<slug_snake>.<locale>.md`.
- Slugs: from H1 before punctuation (`:,-()—`), slugified (lowercase ASCII, strip diacritics, `_`).
