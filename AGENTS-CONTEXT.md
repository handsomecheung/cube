# Project: `fountain-core`

## For the AI Agents

When interacting with this project, consider the following:

- **Portable Build:** `./script/build.sh` (Builds both binaries in Docker).
- **Pure Rust:** No external libraries like OpenCV are needed. All decoding is done via `rqrr` and `image` crates.
- **directory /local:** The `/local` directory is for local use only and will not be committed to git, so do not use scripts from the `/local` directory in `README.md`.

The project uses `rqrr` for QR code decoding in `fountain-decode` and `fountain-wasm`.

## Recent Features
- **Base45 Encoding:** Migrated from Base64 to Base45. This enables QR codes to use the highly efficient **Alphanumeric mode**, significantly increasing single-frame data capacity and reducing image density (making them easier to scan).
- **Efficiency Benchmarking:** Added an end-to-end efficiency test in `tests/integration_test.rs` to measure the conversion ratio from original files to final GIF transfers.
- **Removed OpenCV:** The project is now 100% Rust. Video file decoding has been removed in favor of simpler image/GIF-based transfers.
- **RaptorQ-only Mode:** The project exclusively uses RaptorQ (Fountain Codes) for encoding and decoding.
- **Incremental Decoding (Early Exit):** `fountain-decode` now supports incremental decoding for both GIFs and image sequences. It stops processing as soon as enough RaptorQ packets are collected to reconstruct the file.
- **Unified Codebase:** Core encoding and decoding logic has been abstracted into shared internal functions (`prepare_chunks`, `decode_core`) to ensure consistency and maintainability.
- **GIF Optimization:** `fountain-encode` generates GIFs with an initial "Anchor Frame" (2s duration) containing file metadata.
- **Configurable Pixel Scale:** `fountain-encode` supports a `--pixel-scale` argument to control the size of generated QR codes.

<!-- gitnexus:start -->
# GitNexus — Code Intelligence

This project is indexed by GitNexus as **fountain-core** (211 symbols, 456 relationships, 20 execution flows). Use the GitNexus MCP tools to understand code, assess impact, and navigate safely.

> Index stale? Run `node .gitnexus/run.cjs analyze` from the project root — it auto-selects an available runner. No `.gitnexus/run.cjs` yet? `npx gitnexus analyze` (npm 11 crash → `npm i -g gitnexus`; #1939).

## Always Do

- **MUST run impact analysis before editing any symbol.** Before modifying a function, class, or method, run `impact({target: "symbolName", direction: "upstream"})` and report the blast radius (direct callers, affected processes, risk level) to the user.
- **MUST run `detect_changes()` before committing** to verify your changes only affect expected symbols and execution flows. For regression review, compare against the default branch: `detect_changes({scope: "compare", base_ref: "master"})`.
- **MUST warn the user** if impact analysis returns HIGH or CRITICAL risk before proceeding with edits.
- When exploring unfamiliar code, use `query({query: "concept"})` to find execution flows instead of grepping. It returns process-grouped results ranked by relevance.
- When you need full context on a specific symbol — callers, callees, which execution flows it participates in — use `context({name: "symbolName"})`.

## Never Do

- NEVER edit a function, class, or method without first running `impact` on it.
- NEVER ignore HIGH or CRITICAL risk warnings from impact analysis.
- NEVER rename symbols with find-and-replace — use `rename` which understands the call graph.
- NEVER commit changes without running `detect_changes()` to check affected scope.

## Resources

| Resource | Use for |
|----------|---------|
| `gitnexus://repo/fountain-core/context` | Codebase overview, check index freshness |
| `gitnexus://repo/fountain-core/clusters` | All functional areas |
| `gitnexus://repo/fountain-core/processes` | All execution flows |
| `gitnexus://repo/fountain-core/process/{name}` | Step-by-step execution trace |

## CLI

| Task | Read this skill file |
|------|---------------------|
| Understand architecture / "How does X work?" | `.claude/skills/gitnexus/gitnexus-exploring/SKILL.md` |
| Blast radius / "What breaks if I change X?" | `.claude/skills/gitnexus/gitnexus-impact-analysis/SKILL.md` |
| Trace bugs / "Why is X failing?" | `.claude/skills/gitnexus/gitnexus-debugging/SKILL.md` |
| Rename / extract / split / refactor | `.claude/skills/gitnexus/gitnexus-refactoring/SKILL.md` |
| Tools, resources, schema reference | `.claude/skills/gitnexus/gitnexus-guide/SKILL.md` |
| Index, status, clean, wiki CLI commands | `.claude/skills/gitnexus/gitnexus-cli/SKILL.md` |

<!-- gitnexus:end -->
