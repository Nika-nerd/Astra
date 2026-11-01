
<div align="center">

**Agent Astra**

<br/>

**An autonomous, fully local AI agent for Rust development — a VS Code extension powered by a high-performance Rust backend.**

<br/>

![Rust](https://img.shields.io/badge/Rust-000000?style=for-the-badge&logo=rust&logoColor=white)
![TypeScript](https://img.shields.io/badge/TypeScript-3178C6?style=for-the-badge&logo=typescript&logoColor=white)
![VS Code](https://img.shields.io/badge/VS%20Code-007ACC?style=for-the-badge&logo=visualstudiocode&logoColor=white)
![Ollama](https://img.shields.io/badge/Ollama-Gemma%204-white?style=for-the-badge&logo=ollama&logoColor=black)
![Local First](https://img.shields.io/badge/100%25-Local-success?style=for-the-badge&logo=lock&logoColor=white)

</div>

---

Unlike typical chat assistants that hand you raw, unverified code, Astra plans first, builds autonomously, and verifies its own work with the Rust toolchain before showing you the result. Everything runs on your machine using **Gemma 4** via [Ollama](https://ollama.com).

---

## Highlights

- **Whole-project awareness.** Astra sees your entire codebase through a compact AST "skeleton" (built with `syn`), so a small local model's context window is never flooded with thousands of lines of source.
- **Plan → Refine → Autonomous Build.** Astra first proposes an architectural plan and iterates on it with you. Once you press **Apply**, it runs `cargo` commands, creates files, and writes modules on its own.
- **Self-verifying and self-repairing.** Every build is checked with the compiler, `cargo clippy`, and `cargo test`. Failures are fed back to the model for up to **10 repair iterations**, with protection against infinite loops, deadlocks, and empty stubs.
- **100% local.** No cloud APIs. Your code never leaves your machine.

---

## How It Works

1. **Ask.** Describe a task in the chat panel, optionally attaching files with `@` mentions.
2. **Review the plan.** Astra returns a *Proposed Architectural Plan*. If it needs changes, just reply in the chat (*Refine Plan* mode) and Astra rewrites it.
3. **Apply.** Click **✓ Apply Plan & Build Project**. Astra executes commands, writes files, and runs the verification loop.
4. **Get verified code.** When all checks pass, you see the executed `$ cargo ...` commands and cards for every saved and verified file. The main file opens automatically in the editor.

---

## Architecture

The project consists of two independent layers that communicate via **JSON-RPC over standard I/O (`stdio`)**:

| Layer | Location | Description |
|---|---|---|
| **Rust Backend** | `astra-backend` (`src/`) | Compute core: AST parser, LLM orchestrator, and code verification sandbox |
| **VS Code Extension** | `vscode-extension/` | TypeScript client: Webview chat UI, `@`-mentions, backend process manager |

### Rust Backend (`src/`)

#### Configuration & entry point — `main.rs`, `config.rs`

`AppConfig::from_env()` loads settings from the environment, with defaults tuned for local use:

| Setting | Default | Purpose |
|---|---|---|
| `llm_api_url` | `http://localhost:11434/api/generate` | Ollama endpoint |
| `llm_model` | `gemma4` | Model name |
| `max_repair_attempts` | `10` | Limit for autonomous self-repair iterations |

Logging (`tracing_subscriber`) is routed strictly to **`stderr`**, keeping **`stdout`** clean for JSON responses to VS Code.

#### Indexer & AST parser — `indexer/`

Gives the agent an "X-ray" of the whole project instead of raw sources.

- **`scanner.rs` (`scan_workspace`)** — walks the workspace with the `ignore` crate, respecting `.gitignore` (skipping `/target`, `.git`, `node_modules`), and collects all `.rs` and `Cargo.toml` files.
- **`ast.rs` (`extract_skeleton`)** — parses each `.rs` file with `syn` and converts it back to tokens via `quote`. It keeps imports (`use`), structs, enums, constants, traits, and `impl` blocks, and **strips function/method bodies**, leaving only visibility, name, arguments, and return types.
- **`mod.rs` (`RepoMap::build`)** — merges `Cargo.toml` manifests and all AST skeletons into a single text map of the project (`RepoMap`).

#### LLM integration — `llm/`

- **`client.rs` (`LlmClient`)** — async HTTP client (`reqwest`) with a 300 s timeout. Uses deterministic generation (`temperature: 0.1`), expands Ollama's context to `num_ctx: 16384`, and raises the output limit to `num_predict: 8192` so large files aren't truncated.
- **`prompt.rs`**
  - *Phase 1 — Architect:* `PLAN_SYSTEM_PROMPT` and `build_plan_prompt` produce a step-by-step plan in the user's language, or rebuild it (`previous_plan`) based on user feedback.
  - *Phase 2 — Autonomous Engineer:* `SYSTEM_PROMPT`, `build_initial_prompt`, and `build_repair_prompt` enforce strict quality rules (no `todo!()`, no infinite loops, no holding a `Mutex` across `.await`). During repairs, the model receives the current on-disk state of files and must rewrite only the problematic ones.
  - *Response parsers:* `parse_explanation`, `parse_commands`, and `parse_file_patches` extract `=== EXPLANATION ===`, `=== COMMANDS ===`, and `=== FILE: path === ... === END FILE ===` blocks, with protection against nested markers and unclosed tags.

#### Sandbox, patcher & verifier — `verifier/`

Ensures no syntax error, logic bug, or infinite loop reaches the user.

- **`patcher.rs` (`PatchTransaction`)**
  - Guards against path traversal (rejects paths containing `..` or `/`).
  - Keeps in-memory backups of original files.
  - `apply()` writes new/changed files to disk; `rollback()` restores the original state on failure.
- **`runner.rs`**
  - **`ensure_cargo_project`** — runs `cargo init --bin` automatically if the folder has no `Cargo.toml`.
  - **`run_workspace_commands`** — safely executes agent-generated `cargo` commands (e.g. `cargo add tokio --features full`) and `mkdir`.
  - **Stub protection** — if `src/main.rs` still contains the default `println!("Hello, world!");` after `cargo init`, verification is rejected and the LLM is forced to write real code.
  - **`run_clippy`** (two-stage filter, no speed penalty):
    1. `cargo clippy --fix --allow-dirty --allow-staged` auto-fixes minor issues (unused imports, etc.) in milliseconds, without spending LLM time.
    2. `cargo clippy --message-format=json` with targeted flags promotes critical warnings to hard errors (`-D clippy::infinite_iter`, `-D while_true`, `-D clippy::never_loop`, `-D clippy::await_holding_lock`, `-D clippy::todo`, `-D unreachable_code`) while ignoring cosmetics (`-A dead_code`, `-A unused_variables`).
  - **`run_tests`** (protection against runtime hangs and infinite loops):
    1. Compiles tests first (`cargo test --no-run`).
    2. Runs them inside `tokio::time::timeout(Duration::from_secs(15))` with `kill_on_drop(true)`. If code or a test loops forever or blocks on a socket, the process is killed after 15 s and Gemma 4 receives a precise diagnosis.
  - On success, code is auto-formatted with **`cargo fmt`**.

#### Agent orchestrator & server — `agent/mod.rs`, `server/mod.rs`

- **`AstraAgent::execute`** routes tasks by `TaskMode`:
  - **`TaskMode::Plan`** (`generate_plan`) — builds the `RepoMap`, reads files referenced via `@`, and returns an architectural plan.
  - **`TaskMode::Execute`** (`execute_plan`) — runs the autonomous build loop (up to 10 attempts). Between attempts it accumulates created files in a `BTreeMap` (`accumulated_patches`), rebuilds the up-to-date `RepoMap`, and feeds compiler/test errors back to the model until all checks pass.
- **`run_stdio_server`** — async loop that reads JSON tasks from `stdin` and writes `RpcResponse` messages to `stdout`.

### VS Code Extension (`vscode-extension/`)

- **`package.json`** — registers the `Agent Astra` icon in the Activity Bar, the `astra.chatView` Webview container, the `astra.runTask` command, and the settings `astra.backendPath` and `astra.llmModel`.
- **`src/client.ts` (`AstraBackendClient`)**
  - Locates the compiled `astra-backend` binary and keeps it running as a background child process (`child_process.spawn`).
  - Correlates requests and responses by `id` using a `Map` of promises (`pendingRequests`).
  - Captures the backend's `stderr` and streams agent steps (AST build, attempt number, `clippy` and `test` runs) to the chat UI and the Output panel in real time.
- **`src/webview.ts` & `src/chatHtml.ts` (`AstraChatViewProvider`)**
  - **Workspace detection (`getActiveWorkspaceRoot`)** — always resolves the root of the project currently open in VS Code and passes it to the backend for AST building.
  - **`@` file mentions** — typing `@` in the chat input fetches all `.rs` and `.toml` files in the project and shows an interactive autocomplete menu.
  - **Two-phase UI** — Proposed Architectural Plan card → optional *Refine Plan* feedback → **✓ Apply Plan & Build Project**, after which the chat shows executed `$ cargo ...` commands and cards for saved, verified files, and opens the main file in the editor.
  - **Persistent history** — `retainContextWhenHidden: true` plus state serialization via `vscode.setState()` / `vscode.getState()` preserve chat history and the active plan when you switch IDE tabs.

---

## Getting Started

> Adjust the steps below to match your repository layout.

**Prerequisites**

- [Rust toolchain](https://rustup.rs) (with `cargo`, `clippy`, and `rustfmt`)
- [Ollama](https://ollama.com) with the Gemma 4 model pulled (`ollama pull gemma4`)
- [VS Code](https://code.visualstudio.com) and Node.js (for building the extension)

**Build the backend**

```bash
cargo build --release
```

**Build the extension**

```bash
cd vscode-extension
npm install
npm run compile
```

**Configure** (VS Code settings)

- `astra.backendPath` — path to the compiled `astra-backend` binary (auto-detected if left empty)
- `astra.llmModel` — Ollama model name (default: `gemma4`)

Then open the **Agent Astra** panel from the Activity Bar and start chatting.

---

## Tech Stack

<p>
  <img src="https://skillicons.dev/icons?i=rust,ts,nodejs,vscode,git,github" alt="Tech stack icons"/>
</p>

**Backend:** Rust, `tokio`, `reqwest`, `syn`, `quote`, `ignore`, `tracing`
**Extension:** TypeScript, VS Code Webview API
**LLM:** Gemma 4 via Ollama (local)
**Protocol:** JSON-RPC over `stdio`

---
