# Coding standards

The code in this repository is written and reviewed against a three-layer
methodology, adapted from published sources. Each layer answers a different
question, applied in order — structure first, then surface, then language idiom.

| Layer | File | Question | Source |
|---|---|---|---|
| Structure | [`ousterhout.md`](ousterhout.md) | Are modules deep? Is complexity hidden? | Ousterhout, *A Philosophy of Software Design* |
| Expression | [`readable-code.md`](readable-code.md) | Can any name be misread? Do comments say *why*? | Boswell & Foucher, *The Art of Readable Code* |
| Language | [`rust-specifics.md`](rust-specifics.md) | Are types, errors, and visibility used well? | Rust idiom |

[`GUIDE.md`](GUIDE.md) is the plain-English overview of how the three fit together.

## How they're applied to Meridian

Meridian is a portfolio project, so the standards are applied as a **manual
review pass** — structure → names → Rust idiom — rather than the full automated
enforcement chain (CI gates, complexity audits) described in `GUIDE.md`; that
tooling lives in a separate system. The goal is the same, and it is visible in
the code:

- The data layer is split into **deep, single-purpose modules** (`company`,
  `compare`, `dashboards`, `search`, `format`) — each describable in one
  sentence without "and".
- Query-string parsing lives in one place (`query.rs`) rather than being
  duplicated across callers.
- Names avoid the ambiguous words the standards call out (`get`, `process`,
  `handle`); comments explain *why*, not *what*.
- Non-public items are `pub(crate)`; startup failures use `.expect()` with a
  message documenting the invariant; pure logic (formatting, FX conversion) has
  co-located unit tests that read as specifications.
