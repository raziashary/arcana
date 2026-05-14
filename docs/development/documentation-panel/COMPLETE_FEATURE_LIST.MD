# AutoDev — Complete Feature List

> **What this document is** — A complete catalog of every feature in AutoDev, written so a developer, a manager, or a designer can all read it and understand what the product does. Each feature is described in plain language; where a technical term genuinely matters, it is introduced and explained.
>
> **Source** — Derived from the AutoDev master plan, the development plan, and the project README. This is the documentation pass; a later pass will verify each item against the actual codebase.
>
> **How to read it** — Sections follow the panels visible inside the AutoDev app. Within each section, features are numbered hierarchically so any item can be cited as "feature 3.9.7" or similar. Every leaf item answers three questions: *what is it*, *how does it work*, and *why does it matter*.

---

## Table of Contents

1. [The Big Picture — What AutoDev Is](#1-the-big-picture--what-autodev-is)
2. [Foundational Architecture](#2-foundational-architecture)
3. [Personal Assistant (Chat Panel)](#3-personal-assistant-chat-panel)
4. [The AutoDev Fleet (AutoDev Panel)](#4-the-autodev-fleet-autodev-panel)
5. [Tools Exposed by AutoDev](#5-tools-exposed-by-autodev)
6. [IDE Panel](#6-ide-panel)
7. [Recipes Panel](#7-recipes-panel)
8. [Visibility of AutoDev (Logs Panel)](#8-visibility-of-autodev-logs-panel)
9. [Settings Panel](#9-settings-panel)
10. [Graph Panel](#10-graph-panel)
11. [Dashboard Panel](#11-dashboard-panel)
12. [Cross-Cutting Subsystems](#12-cross-cutting-subsystems)
13. [Distribution, CLI, Installers](#13-distribution-cli-installers)
14. [Design System & UI v2.0](#14-design-system--ui-v20)
15. [Operational Hardening (from End-to-End Runs)](#15-operational-hardening-from-end-to-end-runs)
16. [Ruflo-Inspired Features](#16-ruflo-inspired-features)
17. [Deferred & Future Items](#17-deferred--future-items)

---

## 1. The Big Picture — What AutoDev Is

### 1.1 Identity & Tagline
1.1.1 **What AutoDev is** — A single downloadable desktop application that lets one person, or a team, run an AI coding agent of their choice on their own machine. Instead of being locked to one AI vendor's app and pricing, AutoDev is a neutral container that talks to any major AI model and adds AIRS Foundry's own agent orchestration on top.
1.1.2 **The promise in one sentence** — *One app. Any model. Full autonomy.* That means a single binary you install, the freedom to plug in any AI provider you want, and the ability to let agents run continuously on tasks without constant human babysitting.
1.1.3 **Who it is for** — Programmers (using the app interactively as their pair-programmer), worker agents (running headless overnight or 24/7 to chew through tasks), and IT staff (who can hand a single installer to a teammate and have them productive in minutes).
1.1.4 **What it replaces** — The combination of Anthropic's Claude Code app, Block's Goose app, plus a pile of custom scripts the AIRS team would otherwise glue together to coordinate multiple AI workers. AutoDev folds all of that into one branded, controllable product.

### 1.2 Why It Exists (the Problem Statement)
1.2.1 **Claude Code is fragile for a 24/7 fleet** — Claude Code only works against Anthropic's subscription plans, which carry rate limits that strangle agents that need to run continuously. A March 2026 source-code leak also exposed the inner workings of Claude Code, turning it into a known attack surface that AIRS cannot patch on its own.
1.2.2 **Claude Code locks you to one vendor** — You cannot route cheaper tasks to a cheaper model, and you cannot run anything locally. That makes long-running, low-stakes maintenance work expensive and dependent on an external network connection.
1.2.3 **Goose is too generic** — The competitor product, Block's Goose, has a healthy model-agnostic foundation but does not understand AIRS Foundry's specific workflow (a Supervisor agent that breaks down work, Workers that execute it, and a QC agent that reviews it). It also lacks any built-in awareness of an AIRS-style task board.
1.2.4 **Why a custom desktop app at all** — Building a desktop app rather than another command-line tool means non-technical AIRS team members can use it, IT can install it the same way they install any other corporate app, and the entire harness is auditable code that AIRS owns rather than someone else's proprietary binary.

### 1.3 Headline Capability Themes
1.3.1 **Model-agnostic by design** — AutoDev can talk to Anthropic, OpenAI, Google Gemini, Microsoft Azure, Amazon Bedrock, or a model running locally on the user's own machine via Ollama. The user picks a provider, enters an API key once, and AutoDev does the rest.
1.3.2 **Advanced three-tier memory** — Agents inside AutoDev remember three distinct kinds of things: what happened in past sessions, durable facts about the user and the projects, and accumulated tricks-of-the-trade from past mistakes. This avoids the "every conversation starts from zero" problem typical of basic chat tools.
1.3.3 **Real agent orchestration** — AutoDev contains a small civilization of agents — a Supervisor that assigns work, Workers that do it, a QC agent that reviews it — and a built-in protocol that lets them coordinate with each other. There are twenty-one named lifecycle state machines tracking everything from agent boot to memory recall.
1.3.4 **AUTODEV.md, edited from the UI** — AutoDev has its own personal-instructions document called AUTODEV.md that lives inside the app (in a database) rather than in a file you have to remember to edit. You change it from a sidebar editor and the change takes effect immediately.
1.3.5 **MCP-native** — MCP (the Model Context Protocol) is an open standard for letting AI agents talk to external services like GitHub, Slack, or Adobe. AutoDev includes a native MCP client built into its Rust core so connecting agents to outside tools requires no extra glue.
1.3.6 **Reusable workflows via Skills and Recipes** — Repeatable agent tasks can be packaged either as a *Skill* (a markdown template the user invokes like a slash command) or a *Recipe* (a scheduled YAML workflow that fires on a timer). Both are first-class concepts the user can author and share.
1.3.7 **Single installer** — End users do not have to install Python, Redis, or Docker. The complex sub-components ship pre-bundled inside the installer and start themselves automatically.
1.3.8 **Two execution modes under one app** — A fast "Personal Assistant" mode that runs natively for direct interactive use, and a heavier "Worker" mode for autonomous fleet work. The user just types; the app picks the right mode internally.
1.3.9 **Real git integration** — AutoDev uses a built-in library to talk to git directly (instead of pretending to be a human typing `git` in a terminal), so it can show diffs, manage branches, and isolate each task in its own working copy reliably.
1.3.10 **Live cost visibility** — Every token an agent spends is tracked. The user sees the current session's cost and a daily total at all times, and can drill into per-agent and per-task spend.

---

## 2. Foundational Architecture

### 2.1 The Application Shell
2.1.1 **A native desktop app, not a website** — AutoDev is built as a true desktop application that runs locally on the user's computer. It uses a technology called Tauri (a Rust-based shell that wraps a web frontend) so the installed app is roughly 15 megabytes instead of the 150+ MB typical of comparable Electron apps.
2.1.2 **Window behavior** — The window opens at 1200 by 800 pixels by default and refuses to shrink below 1000 by 700, so the layout never gets squashed. The theme is dark and the title bar reads "AutoDev"; on Windows it ships with a Content Security Policy that locks down what the embedded web view is allowed to load.
2.1.3 **Six top-level tabs plus a sidebar** — The whole app navigates through six tabs: Chat, AutoDev (the fleet panel), IDE (a built-in code viewer), Graph (a code map), Recipes (workflow library), and Settings. There is also a Dashboard view accessible from a sidebar. Most users will live in the Chat tab.
2.1.4 **The persistent right sidebar** — When you are in the Chat tab, a 176-pixel sidebar on the right always shows the current state of AUTODEV.md (your personal instructions), which MCP services are connected, which skills are loaded, which hooks are active, your current plan if you have one, any subagents running, and a running tally of token use and cost for the session.
2.1.5 **Title bar with notifications** — A small bell icon in the title bar shows unread notifications and counts them. Clicking it opens a list of recent events such as a worker finishing a task or hitting a cost threshold. Custom window controls use a three-dot scheme (close red, minimize amber, maximize green) without text labels, and the entire non-button area of the title bar is a drag region so the user can move the window from anywhere.
2.1.6 **Bottom status bar** — A thin strip at the bottom of the window displays which Personal Assistant mode is active (Fast Dev, Review, or Delegate), the model currently in use, today's running cost, and the health of the background services. All sections are separated by `·` dividers in dim color so the bar stays low-contrast.
2.1.7 **TabBar awareness badges** — Each tab can carry a small colored badge to surface "something is happening on this tab." The AutoDev tab shows an amber dot with the count of active tasks while workers are running; the Logs tab shows a red dot with an error count when errors appear in the last 100 log lines. This lets the user notice issues without polling the tab.
2.1.8 **TabBar live pills (right side)** — A pill shows the currently-active model name; a second pill shows the running session cost. Both pills update live as the session advances. The cost pill resets when the user runs `/clear context`.
2.1.9 **System tray with state-aware icon** — When AutoDev is minimized it sits in the OS system tray. The tray icon has four states: dim logo (idle), animated pulse (agents working), warning overlay (a tool call is waiting for the user's Allow/Deny decision), error overlay (fleet error). Hovering the tray icon shows a matching tooltip. This is critical because the user may have minimized the app and forgotten that a worker needs approval — the tray icon is what brings them back.
2.1.10 **Tray right-click menu** — Open AutoDev, Start Fleet / Stop Fleet, a PA-mode submenu (Fast Dev / Review / Delegate), and Quit. Lets the user control basic fleet state without restoring the window.
2.1.11 **OS-level desktop notifications** — In addition to in-app toasts, native OS notifications fire for high-priority events when the AutoDev window is not focused: task completed, fleet error requiring attention, approval pending, QC rejection. Clicking a notification brings AutoDev to focus and navigates to the relevant tab.

### 2.2 Dual-Mode Execution
2.2.1 **Why there are two modes at all** — AutoDev has to serve two very different jobs. One is interactive — a developer typing into the chat and wanting a snappy reply. The other is autonomous — a worker agent grinding through tasks overnight with rich memory and coordination. Trying to do both with the same code path would make each one worse, so AutoDev runs two cooperating engines.
2.2.2 **Mode 1: Personal Assistant (lightweight, native)** — When you type in the Chat tab, your message goes directly from the desktop app to your chosen AI provider with no detours. Tools (like reading files or running shell commands) execute right inside the desktop app's Rust core. This is fastest, works offline if you use a local model, and keeps working even if some optional background services have crashed.
2.2.3 **What Mode 1 deliberately leaves out** — To stay simple, Mode 1 does not use the heavy machinery like agent-to-agent coordination, vector-based long-term memory, or the twenty-one lifecycle state machines. Those things are valuable but unnecessary when a single human is sitting there guiding the agent in real time.
2.2.4 **Mode 2: Worker Agent (orchestrated, heavyweight)** — When a recipe fires (for example, "Worker Backend checks the task board every two minutes"), the request is routed through a background Python service called the Core Orchestrator. There it gets the full treatment: durable memory, lifecycle tracking, the ability to message other agents, and detailed conversation evolution. Results stream back into the desktop app's UI in real time.
2.2.5 **Automatic fallback** — If the heavy Mode 2 services become unreachable (for example, three consecutive health checks fail), Workers transparently fall back to the lightweight Mode 1 engine so the fleet keeps running in a degraded but functional state. The user does not have to intervene.
2.2.6 **No duplicated work between modes** — Each mode does only what fits its purpose. The Rust shell handles the fast path; the Python orchestrator handles the deep path. They share configuration, the task board, and the user's data, but they do not reinvent each other's logic.

### 2.3 Background Services ("Sidecars")
2.3.1 **What a sidecar is** — A "sidecar" is an extra program that runs in the background alongside the main app and exposes useful features to it. AutoDev manages four such sidecars automatically; the user never has to start or stop them manually.
2.3.2 **The Memory Service** — A Python program that stores and retrieves the agent's long-term memories. It can store memories in several different backends (the default is mem0; alternatives include Redis, MongoDB, ChromaDB, or Google Vertex AI). It has a built-in safety system that, after five failures, will switch automatically to a backup backend so memory keeps working.
2.3.3 **The Core Orchestrator** — The brain that runs Worker-mode agents. This is where the twenty-one state machines, the agent-to-agent protocol, and the streaming engine live. Built on FastAPI (a Python web framework), it listens on a local port and accepts requests from the desktop app.
2.3.4 **Redis (an optional fast cache)** — A small standalone program used as a "warm" memory cache so frequently-used memories return instantly. On Windows the equivalent is called Memurai. AutoDev only checks whether it is alive — it never tries to manage or upgrade it — and if Redis is absent the fleet still works, it just runs slightly slower because it has to read from slower storage.
2.3.5 **The in-app MCP tool server** — AutoDev includes a tiny built-in MCP server (listening on port 8767 on the user's own machine) that exposes the same canonical tools (read file, write file, run shell, etc.) to fleet workers. This is how Workers, Supervisor, and QC all discover and call the standard AutoDev tools through one consistent gateway.
2.3.6 **The sidecar manager** — A coordination layer inside the desktop app that starts the sidecars in the right order, watches their health, restarts them if they crash, and cleanly stops them when the user quits. The order matters: Memory comes up first, then the Orchestrator (which depends on Memory), then the in-app MCP server registration.
2.3.7 **Auto-start on launch** — When the user opens AutoDev, the sidecars boot automatically. If a sidecar is already running externally (for example a developer started it manually), the app detects that and uses it instead of spawning a duplicate.
2.3.8 **Log routing** — Everything the sidecars print is captured and routed into AutoDev's central logging system, with a dedicated log file per sidecar in a centralized logs folder. This means a user can see what the Python services are doing without leaving the AutoDev UI.
2.3.9 **Quietening noisy logs** — Two common pieces of noise are filtered automatically: Windows connection-reset messages (a normal byproduct of certain Python network code), and very long lines coming from system prompts being echoed back. Filtering keeps the log readable without losing genuine errors.
2.3.10 **Log rotation** — Each sidecar log is automatically rolled over to a `.1` backup file once it grows past 20 megabytes, and individual lines longer than 2000 characters are truncated. This stops logs from quietly filling up the disk.
2.3.11 **Health checks with debounce** — The desktop app checks every few seconds whether the Orchestrator is healthy. Crucially, a single missed check does not panic; only three consecutive failures in a row trigger the "Orchestrator is down" state. This avoids unnecessary fallbacks when the Orchestrator is just busy serving a long-running call.
2.3.12 **Auto-restart with backoff** — If a sidecar crashes, AutoDev tries to restart it, waiting longer each time it fails (starting at five seconds, doubling, capped at five minutes). This prevents a stuck program from being restarted in a tight loop.
2.3.13 **Graceful shutdown** — When the user quits, AutoDev politely asks each sidecar to stop, waits five seconds, and only then forcibly kills it if necessary. This ensures pending writes complete cleanly.
2.3.14 **Windows-specific safeguards** — Several environment variables (`PYTHONUTF8`, `PYTHONIOENCODING`) are set when spawning Python sidecars so that unusual characters in log lines do not crash the program. Other variables suppress noisy chatter from a library called LiteLLM that would otherwise flood the logs.
2.3.15 **MCP server registration on startup** — Right after the Orchestrator is healthy, the desktop app tells it "by the way, there is also an MCP server at this address; please discover its tools." This is idempotent — if the Orchestrator already knew about the server from a previous run, the second attempt is treated as a success rather than an error.
2.3.16 **Selective MCP configuration** — The Core Orchestrator can in principle connect to Adobe Creative Cloud tools, but AutoDev ships with a configuration that deliberately omits them. This keeps the AutoDev deployment focused on developer tooling.

### 2.4 Startup Progress System
2.4.1 **Why startup needs a UI** — Booting all the sidecars takes around fifteen seconds. Without feedback, that delay feels like the app has frozen. AutoDev shows a full-screen overlay during this period so the user understands what is happening.
2.4.2 **Twelve named phases** — The startup is broken into twelve human-readable phases: initialize, load skills, start memory, vector database connecting, memory ready, start orchestrator, routers loading, agents loading, living agents loading, orchestrator ready, workers loading, fully ready. Each phase advances monotonically — once you are at "Workers Loading" you cannot go back to an earlier phase.
2.4.3 **The overlay itself** — A full-screen modal showing all twelve phases with icons (a checkmark for done, a spinner for in-progress, a dot for pending), a progress bar, the percentage complete, and a one-line hint about what is happening right now. It auto-dismisses six tenths of a second after reaching "ready."
2.4.4 **How phases are detected** — A "log line scanner" reads each line a sidecar prints and matches it against known patterns. If a line says "memory ready" the FSM advances; if a line says "agents loaded" it advances again. This means the UI reflects the real internal state of the sidecars rather than guessing.
2.4.5 **Reconnect after startup** — When startup finishes, the desktop app re-runs its connectivity probes so that any badges that were drawn during startup (and might have said "offline") refresh to the correct "live" state.
2.4.6 **Already-running detection skips the overlay** — If both core ports (4000 for the Orchestrator and 8000 for the Memory MCP) are already healthy when AutoDev boots (because the user started them externally for development), the loading screen is skipped entirely and a brief one-line toast appears in the StatusBar saying "Fleet already running" with the port assignments. No reason to show a 15-second startup ceremony when the fleet is already up.
2.4.7 **Press Esc to enter PA-Only Mode** — The startup overlay can be dismissed early with the Escape key. The app then continues into "PA-Only Mode" — the Chat tab works normally with the lightweight Rust engine, but the AutoDev tab shows a grayed-out `FLEET OFFLINE` badge and a persistent warning strip at its top offering Start Fleet / Dismiss buttons. The user can keep working interactively without waiting for the heavy sidecars.
2.4.8 **Service rows with timing** — During startup, each service row shows a status icon (○ waiting / ◌ starting / ● done / ✗ error), the port being probed, and a live elapsed time while the service is starting that freezes to its final value once the service is healthy. So the user sees "memory-mcp-server :8000 ✓ 1.2s" right after that service comes up.

### 2.5 Communication Between Frontend and Backend
2.5.1 **Two ways the UI talks to the engine** — The React frontend communicates with the Rust backend through two channels. One channel is request-response (the UI asks the backend to do something and waits for the result). The other channel is a continuous event stream (the backend pushes updates to the UI as things happen, like an agent typing a response one word at a time).
2.5.2 **Server-Sent Events bridge** — When a Worker is running, the Python Orchestrator streams events (thinking text, tool calls, results) over a long-lived connection using a protocol called SSE. The Rust shell receives these and re-emits them as Tauri events so the React UI does not have to know about the Orchestrator directly.
2.5.3 **Categories of events** — Agent events (streaming text, tool started, tool finished, status changed, error, retry, thought snippets), session events (created, cost updated), sidecar events (status changes), provider events (connection test results), worker events (status, tick start/end), task events (created, assigned, status changed), plan events (created, updated, step completed), graph events (rebuilt), hook events (custom), analytics events (cost update, API failure), update events (new version available).
2.5.4 **Why this matters to the user** — Because of these event channels, the UI feels alive: the task board updates the moment a task changes, agent status badges pulse in real time, and there is no "click refresh" button anywhere.

### 2.6 Where Things Live on Disk
2.6.1 **A dedicated AutoDev folder in your home directory** — AutoDev stores all its data in a hidden folder inside your home directory. Nothing is scattered across the operating system; if you want to back up your AutoDev install, copy this folder.
2.6.2 **App settings database** — All preferences, the workspace folder you've chosen, GitHub MCP configuration, model-specific tuning, and your AUTODEV.md content live in one small database file.
2.6.3 **Task board database** — A separate database holds the internal Kanban board: every task ever created, its current status, its assigned worker, who created it, and so on.
2.6.4 **Worker registry database** — The list of every worker agent you've created (or that came pre-defined) lives in its own database, including their model, their schedule, and their bound recipe.
2.6.5 **Plans database** — Plans (multi-step task breakdowns) and their dependencies are stored separately so they survive across sessions and across app restarts.
2.6.6 **Analytics database** — Token use, daily cost roll-ups, per-worker statistics, API failures, and system events all flow into an analytics database that powers the Dashboard.
2.6.7 **Procedural memory folder** — Your AI assistant's accumulated learnings ("the trick to fixing X is Y") live as plain text files inside a memory folder, so you can read or edit them in any text editor outside the app.
2.6.8 **Sessions** — Conversation histories are stored on disk so reopening AutoDev brings back yesterday's chat exactly where you left it.
2.6.9 **Code knowledge graphs** — Each repository you analyze gets its own knowledge-graph database file, named after a hash of the repo path so different repos cannot accidentally share a graph.
2.6.10 **Multi-repo registry** — A small JSON file lists which repositories you have registered with the graph engine so that AutoDev can offer cross-repo analysis.
2.6.11 **Skills folder** — Your slash-command skills are markdown files organized by category (coding, debugging, devops, fleet) inside a skills folder.
2.6.12 **Hooks configuration** — Your custom event hooks (e.g. "auto-format every Rust file after the agent writes it") live in a single configuration file.
2.6.13 **Recipes folder** — Every recipe — both the system-shipped ones and any you author — is a YAML file inside a recipes folder. A backups subfolder stores previous versions when you edit a recipe.
2.6.14 **File checkpoints** — Every time the AI is about to overwrite a file, AutoDev quietly snapshots the old version. These snapshots live in a checkpoints folder, organized by session, with the most recent 50 versions kept per file.
2.6.15 **Per-agent sandboxes** — Each automated agent (Supervisor, each Worker, QC) gets its own little folder where it can dump scratch files. This stops agents from polluting your real project directory.
2.6.16 **Worktrees** — When a worker claims a task, AutoDev creates an isolated copy of the repository (a "worktree" in git parlance) inside a worktrees folder so the worker can change files without disturbing your main branch.
2.6.17 **Logs** — A daily log file is created automatically. Older logs are kept for seven days and then auto-deleted. Sidecar-specific logs sit alongside the main log.
2.6.18 **Plugins** — Installed AIRS plugins live in their own folder so they can be inspected or uninstalled independently.
2.6.19 **Headless CLI configuration** — A separate folder (named `.airs`) holds configuration for the optional command-line tool, including a TOML file with provider keys for headless use.

### 2.7 State Machines (Behavior Models)
2.7.1 **What a state machine is, in plain terms** — A state machine is a formal model that says "this thing can be in one of these named states, and it can only move between them in these specific ways." Using state machines for things like agent lifecycle prevents bugs where, for example, an agent would try to "stop" before it had actually "started."
2.7.2 **State machines in the desktop shell** — Six state machines live in the desktop app: one for the overall app lifecycle, one per sidecar process, one per LLM provider's connection state, one for the permission-confirmation dialog, one for each recipe run, and one for each chat session.
2.7.3 **State machines in the Orchestrator** — Twenty-one separate state machines live in the Python brain, covering agent lifecycle, session lifecycle, task execution, tool execution, memory store, memory recall, MCP server connection, MCP tool discovery, A2A server, A2A request, workflow execution, cache tier (cold/warm/hot/stale/evicted), system startup, system shutdown, agent subprocess, agent config, session runtime, prompt reload, file operations, and authentication.
2.7.4 **Why so many state machines** — Each one is small and focused. Together they give AutoDev unusually rich self-knowledge: at any moment, the system can report exactly which agents are alive, which tools are executing, which memories are loading, and so on. This is the foundation that makes things like the Live Monitor and the system health Dashboard possible.

---

## 3. Personal Assistant (Chat Panel)

### 3.1 What the Personal Assistant Is
3.1.1 **The PA in one sentence** — The "Personal Assistant" or PA is the AI agent you talk to directly in the Chat tab. It is the part of AutoDev a user interacts with by hand, in real time. The PA can read your files, run commands, edit code, and create tasks for the rest of the fleet to do.
3.1.2 **One PA, three behaviors** — The PA has a single conversation and a single memory, but it has a switch (called the PA Mode Toggle) that changes *what it is allowed to do* in the current session. The three behaviors are Fast Development, Review Mode, and Delegate Mode.

### 3.2 The Three PA Modes
3.2.1 **Fast Development — the all-access pass** — This is the default mode for when you are coding side-by-side with the PA. Every tool is available: reading files, writing files, editing files, running shell commands, searching code, calling external services. The PA also gets a system prompt encouraging it to take direct action. There is no "are you sure?" confirmation before each step, because confirmation would interrupt the flow.
3.2.2 **Review Mode — cautious editing** — The PA still has every tool, but each potentially destructive action triggers a confirmation dialog. This is the mode you use when you are not 100% sure of the change about to happen, or when working in a sensitive repository where you want a human-in-the-loop on every file modification.
3.2.3 **Delegate Mode — hands off, delegate to the fleet** — In this mode the PA cannot edit any files or run any shell commands. The only tool it has is "create a task." Anything you ask it gets converted into a structured task on the AutoDev internal task board, where the Supervisor agent will pick it up and assign it to an appropriate Worker. This is the mode you use when the fleet is live and you do not want the PA to step on what Workers are doing.
3.2.4 **How you switch modes** — A three-segment button in the Chat tab header lets you click between Fast Dev, Review, and Delegate. The selection is remembered against the current chat session, so if you reload AutoDev tomorrow and reopen that session, it comes back in the same mode you left it.
3.2.5 **A visible reminder** — The current PA mode is always visible as a colored badge in the bottom status bar (green for Fast Dev, yellow for Review, blue for Delegate). This prevents accidents like "I thought I was in Delegate but I was in Fast Dev."
3.2.6 **Three system prompts, one toggle** — Internally, each mode corresponds to a different system prompt being sent to the AI. Fast Dev encourages direct execution; Review Mode adds caution; Delegate Mode rewrites the agent's understanding of itself to focus on task decomposition rather than file editing. Your AUTODEV.md and your three layers of memory are added on top of whichever base prompt is in use.

### 3.3 LLM Provider Choice (Bring Your Own AI)
3.3.1 **The provider abstraction** — Under the hood there is a single, uniform way to talk to any AI provider. This means you can swap providers without re-learning the app — the chat experience is the same whether you are using Claude, GPT, Gemini, or a local model.
3.3.2 **Anthropic (Claude)** — Talks to Anthropic's API directly using your own API key. Supports the latest Claude reasoning modes including "interleaved thinking," in which the model thinks in between tool calls. Old model names from earlier ages of the API are quietly translated to the new names so old configs keep working.
3.3.3 **OpenAI (GPT family)** — Talks to the OpenAI API. Supports the special "reasoning effort" parameter that o-series models (o1, o3, etc.) accept, which trades latency for more thorough thinking.
3.3.4 **Google Gemini** — Full Gemini support including streaming, tool use, and cost tracking. Useful when you want a model with very large context windows or when you have Google credits.
3.3.5 **Ollama (local models)** — Lets you point AutoDev at a model running on your own laptop or server through Ollama, a free local model runner. No API key is required because the model is yours. AutoDev queries the running Ollama instance to see what models you've downloaded and lets you pick from a dropdown. Context-window sizes are guessed from the model name (Llama, Mistral, CodeLlama, DeepSeek Coder, Qwen 2.5, LLaVA, Phi-3/4, Gemma, Command-R).
3.3.6 **Microsoft Azure OpenAI** — For organizations that use Azure-hosted OpenAI models. Behaves like the OpenAI provider but routes through your Azure endpoint.
3.3.7 **Provider testing** — Whenever you add or change an API key, you can click "Test Connection" and AutoDev makes a tiny test call to confirm the credentials work. This catches typos before they bite during a real task.
3.3.8 **Per-model metadata** — Each provider's models come with their cost-per-million-tokens, whether they support extended thinking, whether they support image input, and how large their context window is. This metadata feeds cost tracking and the context-window meter in the Chat tab.
3.3.9 **Unified streaming** — Regardless of provider, the streaming output is normalized into a common format so the UI does not have to special-case Anthropic versus OpenAI versus Gemini. The same little "..." spinner and the same word-by-word reveal works for every model.

### 3.4 Per-Model Configuration
3.4.1 **Temperature and top-p sliders** — Two well-known knobs in AI: temperature controls how creative or deterministic the model is, top-p controls how broad its word choice is. You can set both per-model in Settings and AutoDev will send those values on every request.
3.4.2 **Extended thinking budget** — For models that support extended thinking (a mode where the model reasons silently before answering), you can set a token budget for that silent reasoning. A slider runs from 1024 up to 32000 tokens.
3.4.3 **Reasoning effort** — For OpenAI's o-series models, a dropdown lets you set low, medium, or high reasoning effort.
3.4.4 **Reset to default** — Every config value has a reset button next to it so you can experiment freely and undo with one click.

### 3.5 AUTODEV.md — Your Personal Instructions for the PA
3.5.1 **What AUTODEV.md is, conceptually** — A personal instruction document for your PA. Think of it as the "memo" that the AI reads at the start of every conversation telling it how *you* like to work — your stack preferences, your conventions, your project quirks.
3.5.2 **Two scopes** — There is one *global* AUTODEV.md that applies everywhere ("I am a senior engineer, be terse, prefer Python with type hints") and one *per-repo* AUTODEV.md that applies only when you are working in a specific project ("This codebase uses FastAPI and pytest; the test command is `pnpm test`"). The repo version overrides the global on overlaps.
3.5.3 **Edited inside the app, not on disk** — Unlike CLAUDE.md (the equivalent in Claude Code) which is just a text file you have to remember to write, AUTODEV.md is stored in AutoDev's internal database and edited from a panel in the sidebar. There is no "where do I put this file?" question.
3.5.4 **A 40,000-character limit** — A live counter in the editor shows how much of the limit you have used. A color warning appears as you approach it. This matches the proven limit used by other agents and is large enough for very detailed instructions.
3.5.5 **The sidebar card** — In the right sidebar a small card shows two dots — one for "global" content, one for "this repo" content. Green means content exists; gray means empty. Buttons let you jump to the editor for either scope.
3.5.6 **The full-screen editor** — Clicking edit opens a full-screen modal with tabs for global and per-repo. There is a save shortcut (Ctrl+S), a reset button, and a "Insert Template" button that drops a starter template into an empty file. You can hit Escape to close or click outside the modal.
3.5.7 **Automatic injection into every conversation** — At the start of every PA turn, AutoDev silently adds both your global and your repo AUTODEV.md content to the model's instructions. You never have to remember to do this.
3.5.8 **No competing format** — AutoDev does not read CLAUDE.md or AGENTS.md files automatically. If you have content in those formats from another tool, you can paste it in. A migration helper that does this with one click is on the roadmap.
3.5.9 **What a typical AUTODEV.md looks like** — Sections usually include the project stack (language, frameworks, package manager), command shortcuts (how to run tests, how to lint), an architecture summary (where different parts of the code live), and coding patterns specific to the project. A template button drops in this skeleton if you want a starting point.

### 3.6 Skills (PA Slash Commands)
3.6.1 **What a skill is** — A reusable workflow you can invoke with a slash command like `/feature-implementation`. Each skill is a markdown file with a small header that names the skill, describes what it does, and lists the parameters it expects. The body of the file is a prompt template with placeholders that get filled in when you run the skill.
3.6.2 **The Skills Palette** — Press Cmd+Shift+S (or Ctrl+Shift+S on Windows) to open a command-palette-style search across all installed skills. You can type to filter, see a preview of what each skill does, and click "Execute" to run one.
3.6.3 **Categories** — Skills are organized into categories: coding (feature implementation, API endpoint design, database migrations, refactoring, code review, adding tests), debugging (error investigation, performance optimization, race-condition hunting), DevOps (Docker setup, CI pipeline fixes, dependency updates), and fleet-specific skills used by automated agents.
3.6.4 **Parameter forms** — When you pick a skill that requires parameters (for example, "feature name" and "target module"), AutoDev shows a form generated automatically from the skill's metadata. Required fields are marked, defaults are pre-filled, and tooltips explain what each field is for.
3.6.5 **Slash-command autocomplete** — Typing `/` in the chat shows matching skills inline so you can pick one without opening the full palette.
3.6.6 **Recent skills** — A short list of recently-run skills appears at the top of the palette so common workflows are one click away.
3.6.7 **Import / share** — A skill is just a markdown file, so you can share one with a teammate by sending them the file. AutoDev has an import button that adds a skill from a URL or a local file.
3.6.8 **Mode-aware skill registration** — Some skills only make sense in certain PA modes (for example, a "delegate this work" skill only makes sense in Delegate mode). The skill metadata can specify which mode it is valid for.

### 3.7 Chat Sessions
3.7.1 **Multiple separate conversations** — You can have many parallel conversations with the PA. Each one is a session with its own history, its own model configuration, and its own PA mode. A 200-pixel sidebar on the left of the Chat tab lists every session.
3.7.2 **Session metadata** — Each session card shows a title (auto-generated from the first message), a relative timestamp ("2 hours ago"), and the number of messages exchanged. Active session is highlighted with a blue border.
3.7.3 **New, switch, delete** — A "+" button creates a new session. Clicking any session card switches to that conversation; history is loaded from disk on demand. A delete button (with confirmation dialog) removes a session permanently.
3.7.4 **Persisted across restarts** — Closing and re-opening AutoDev preserves all sessions; you do not lose work to a restart. The currently active session is also remembered.
3.7.5 **Keyboard shortcuts** — Cmd/Ctrl+T opens a new session; Cmd/Ctrl+W closes the active one. These match common browser conventions so they feel natural.
3.7.6 **Tool calls stored as their own messages** — Inside the database, the AI's tool calls are stored as separate message rows linked to the original assistant message. This keeps the data clean and makes it possible to render tool calls as their own visual blocks in the UI rather than embedded inside the assistant's bubble.
3.7.7 **Three message roles** — Every message in the conversation has a role: user, assistant, or tool. The UI renders each role differently so the conversation flow is clear at a glance.

### 3.8 The Chat Experience
3.8.1 **The chat header** — A bar at the top of the conversation shows the PA mode toggle, the active model name, the number of MCP services connected, the number of skills loaded, and a status indicator for your AUTODEV.md.
3.8.2 **The message list** — A scrollable area. User messages are right-aligned with a green-tinted background. Agent messages are left-aligned with a dark surface. Tool calls show up as their own blocks with a still-darker background and color-coded labels (tool name in blue, arguments in gray, result in green).
3.8.3 **The input bar** — A multiline text input with a green Send button. Hitting Enter sends; Shift+Enter inserts a newline.
3.8.4 **The input meta row** — Just below the input, a small row reminds you of shortcuts ("Attach · /skill · @agent · #recipe") on the left and shows the running token and cost count on the right.
3.8.5 **@-mention file picker** — Typing `@` in the chat input opens a fuzzy-search picker for files and folders inside your workspace. You can type a partial name and AutoDev shows matching paths. Folders are shown with a count of files inside them. Hitting Enter inserts the absolute path of the chosen file into your message.
3.8.6 **Streaming responses** — The PA's reply renders one word at a time as it streams from the AI provider. If the model has "thinking" enabled, the silent reasoning text shows up first in a collapsible "Thinking..." section, then the actual answer follows.
3.8.7 **Tool call cards** — Every tool invocation renders as its own structured card with a colored icon, the tool name, a one-line summary of what happened, and a click-to-expand area with the full input and output. There are sixteen specialized card layouts (one per tool family) so reading file contents looks different from reading a directory listing or running a shell command.
3.8.8 **Permission dialog** — In Review Mode, before any write or shell action, a modal pops up asking you to approve or deny. For file edits it shows a diff of the proposed change so you know exactly what will happen.
3.8.9 **Inline plan display** — When the PA decides a task is complex enough to warrant a plan, the plan renders as a card inline in the chat, with one row per step and a status indicator per step. As steps complete, the card updates.
3.8.10 **Subagent sidebar** — When you spawn subagents (helper AIs that do work in parallel), a 300-pixel sidebar appears on the right edge of the Chat tab showing each subagent, its current activity, and its events in real time.
3.8.11 **Context window meter** — A small progress bar under the input shows how much of your model's context window you have used. It changes color as you approach the limit and includes a tooltip with the precise count. This is the "you are running out of room" warning before you actually run out.
3.8.12 **Running cost** — The bottom-of-chat cost bar shows the current session's spend in dollars and the number of input/output tokens. This number is updated on every turn.
3.8.13 **Thought blocks** — Between tool calls, when the agent emits internal reasoning, that reasoning renders as a collapsible "Thoughts" section inside the assistant bubble. Collapsed by default after the turn completes; expanded only while streaming so the user can optionally watch the thinking happen in real time. The expanded view uses a dimmer monospace text against a code-block background so it reads as "internal" rather than "spoken."
3.8.14 **Streaming cursor and abort** — While the agent is generating, a blinking `▌` cursor in accent green is appended to the last word at a 700 ms interval, and the Send button is replaced by a red `✗ Stop` button that sends a cancellation signal to the backend. On a successful completion the cursor disappears and a timestamp is appended. On a mid-stream error or abort the cursor is replaced with `⚠ Generation interrupted` in warning color.
3.8.15 **Multi-tool turns flow as one bubble** — When the agent uses several tools in one turn, the agent bubble interleaves narrative text, thought blocks, and tool call cards in their execution order — narrative → thoughts → card → narrative → card → final result — rather than fragmenting them into multiple bubbles. The currently-pending card is always the last item in the turn and stays expanded; everything resolved collapses to a one-line summary.
3.8.16 **Inline plan progress mutation** — A plan card rendered in chat mutates in place as steps complete. Each step icon flips (○ pending → ◌ running → ✓ done), the progress bar fills, the step counter increments, all without re-rendering the bubble. So the user can scroll back to an earlier turn and see "this is when the plan was created" while watching the current turn's progress live.

### 3.9 The PA's Three-Tier Memory System
3.9.1 **Why memory matters** — Without memory, every conversation starts from zero and the PA does not remember what you discussed yesterday, last week, or last month. AutoDev solves this with three distinct memory tiers that complement each other.
3.9.2 **Tier 1: Episodic memory (raw experience)** — Every user message and every PA response is recorded with a turn number and timestamp. This is the "autobiography" of the assistant. You can ask the PA "what did we work on yesterday?" and it can answer because it remembers literally.
3.9.3 **Tier 2: Semantic memory (facts about you)** — Durable facts extracted automatically from your conversations, organized by category: project conventions, user preferences, architecture decisions, reference locations. These are graded by confidence (0.8 and above are auto-stored; explicitly-stated "remember this" goes in at 1.0).
3.9.4 **Tier 3: Procedural memory (accumulated tricks)** — Hard-won wisdom that builds up across many sessions, like "Firebase rate limits trigger when batch size exceeds 100" or "the way to fix that flaky test is to mock the clock." Stored as editable markdown files plus a search index. You can read or edit them in any text editor outside the app.
3.9.5 **The Memory Sidebar** — A panel inside the Settings area lets you browse, search, and edit memories across all three tiers. There are tabs for episodic, semantic, procedural, and a cross-tier search.
3.9.6 **Memory injection at the start of every turn** — When you send a message, AutoDev silently fetches a handful of relevant memories from each tier (five episodic, ten semantic, three procedural by default) and adds them to the system prompt. The PA never starts from zero.
3.9.7 **Auto-summarization** — Every twenty turns, AutoDev silently generates a summary of the session so far and stores it as an episodic memory of type "session summary." This means even very long conversations remain navigable in memory.
3.9.8 **Two memory tools for the PA** — A `search_memory` tool lets the PA query its own memory in the middle of a conversation if it needs to. A `remember_this` tool lets you explicitly ask the PA to store a fact ("remember that I prefer early returns over nested ifs").
3.9.9 **Migration from old conversations** — When you upgrade to a version of AutoDev that includes memory, an idempotent migration backfills your existing session history into episodic memory. Nothing is lost.
3.9.10 **Memory Health Panel** — In Settings, a panel reports the health of the memory system: vector store entry count, last recall latency in milliseconds, whether the warm cache is up. There are buttons to purge a worker's memories or inspect the last twenty memories stored for any agent.
3.9.10.a **Sidebar Memory Health mini-view** — When the heavy Mode 2 stack is running, the persistent sidebar shows a compact memory-health section: one mini bar per tier (episodic / semantic / procedural) sized to its freshness score, color-coded by which cache tier currently holds it (hot = accent green, warm = info blue, cold = dim gray), plus a single staleness dot (green nominal / amber degraded / red critical). Clicking the section opens the full Memory Inspector overlay. The section is hidden entirely in PA-Only Mode so it doesn't suggest broken state when the sidecars are off.
3.9.11 **Project-aware memory** — A documented enhancement scopes memory to the current project so that switching projects does not contaminate the PA with stale paths from a different repo. An immediate mitigation is already in place that always injects the current working directory at the top of every Worker turn.
3.9.12 **Fact extraction policy: user-only, confidence-gated** — Semantic facts are extracted only from *user* messages, never from agent responses (extracting from the agent's own words would create a self-referential loop that drifts away from what the user actually said). An LLM extraction step assigns each candidate fact a confidence score from 0 to 1; only facts above 0.7 are auto-stored. The exception is the `remember_this` tool which the user invokes explicitly — those facts are stored at confidence 1.0.
3.9.13 **Fact categories** — Each semantic fact carries one of four category tags: `preference` ("Razi prefers early returns over nesting"), `convention` ("tests live in `__tests__/`"), `relationship` ("the API depends on PostgreSQL"), or `setup` (e.g., build commands, environment variables). Categories drive both the UI filter on the Memory sidebar and the relevance ranking when injecting context.
3.9.14 **Contradiction handling with fact history** — When the user corrects a previously-stored fact ("actually, use late returns here"), the new fact does not overwrite the old one in place. Instead the old fact is marked superseded (with a pointer to the new fact and the reason for the change), and a separate `fact_history` table records every value the fact has held over time. This preserves an audit trail and lets the PA explain "I used to think X but you corrected me on date Y."
3.9.15 **MEMORY.md content rules** — The procedural-memory index is kept short by convention so it fits in a single system-prompt injection: under 200 lines total, with each entry ~1 line and ~150 characters maximum. Details that don't fit go into separate *topic files* (e.g. `debugging.md`, `workflow-patterns.md`, `tool-preferences.md`) which the PA loads on demand only when a query touches that topic.
3.9.16 **Topic files** — Each topic file is a markdown document focused on one theme (debugging, refactoring, deployment, library quirks). The MEMORY.md index references them so the PA knows to fetch the right one. Users can create new topic files from the sidebar with a "+ New Topic" action; the PA can also create them automatically when it accumulates enough learnings on a recurring theme to warrant its own file.
3.9.17 **Session summaries table** — A dedicated `session_summaries` table stores one row per past session with a concise summary, the list of files touched, and the key decisions made. This is what powers the "what did we work on last Tuesday?" query without needing to scan thousands of episodic message rows.
3.9.18 **Memory access analytics** — A `memory_access_log` table records every memory query: the query text, which tier it hit, how many results returned, and the latency in milliseconds. This is what makes the "slow recall" warnings in the Memory Health Panel possible — operators can see if a particular type of query is consistently slow and tune it.
3.9.19 **Memory Sidebar UI panel** — A dedicated sidebar area for procedural memory with a Monaco-style editor for MEMORY.md, save/reset/export buttons, a list of topic files (each clickable to open in the editor), a "+ New Topic" creator, a search box that runs across all three memory tiers, and a stats footer showing fact count, skill count, and "last updated N minutes ago." This is where the user reviews and edits what the PA has learned.

### 3.10 The PA's Planning System
3.10.1 **One unified planner tool** — When a task is complex (the rule of thumb is "five or more steps, or ten or more tool calls"), the PA reaches for a single tool called `planner`. It accepts a small YAML snippet describing what to do (create a plan, add a step, update a step, etc.), so the PA does not have to juggle ten different planning tools.
3.10.2 **Fourteen planner actions** — The actions cover everything from `listActions` (discover what is available) through `create`, `addStep`, `updateStep`, `getPlan`, `listPlans`, `checkDependencies`, `executeTemplate`, `deletePlan`, `abandonPlan`, plus fleet-specific actions for breaking down an epic, claiming a plan item, creating a subplan, and submitting work for QC review.
3.10.3 **Plan scopes** — A plan can be personal (only the PA touches it), fleet (Workers execute against it), or epic (a Supervisor breakdown into many child plans). The scope is shown as a colored badge on the plan card.
3.10.4 **The Plan Card in the sidebar** — When you have an active plan, a card appears in the right sidebar showing its title, a progress bar, the currently-active step, and badges for ownership (PA, Supervisor, Worker, or QC) and scope.
3.10.5 **The Plan Detail view (slide-in panel)** — Clicking the card opens a 400-pixel panel sliding in from the right edge over the persistent sidebar — *not* a blocking modal — so the user can keep watching the chat or the task board while reviewing the plan. The panel shows the whole plan: a collapsible step list (drag-reorderable for draft PA plans), a dependency graph visualization showing which steps are blocked by which, and a row of context-aware action buttons that change depending on which agent owns the plan.
3.10.5.a **Per-step inline notes** — Each in-progress step can carry a "step note" rendered as dim italic text directly below it, where the agent records its current sub-thought ("Working on HealthResponse Pydantic model"). The note updates as the agent progresses and disappears once the step transitions to done.
3.10.6 **Action buttons per role** — PAs see Activate / Add Step / Mark Done / Abandon. Supervisors see Break Down Epic / Assign to Worker / Add Step / Auto-Assign. Workers see Claim Step / Mark Done / Create Subplan / Release Item. QC agents see Start Review / Approve / Reject / Request Changes.
3.10.7 **Inline plan display in chat** — When the PA creates a plan during a conversation, a compact version of the plan card renders right there in the chat. Step completions animate the progress bar so you can watch progress in real time.
3.10.8 **Nested plans (subplans)** — A Worker handling one step of a Supervisor-created plan can create a subplan of its own. The parent and child plans are visualized as a tree.
3.10.9 **Plan templates** — A Skill can include a plan template so that running, say, the `/feature-implementation` skill produces a five-step plan with standard structure rather than starting from scratch.

---

## 4. The AutoDev Fleet (AutoDev Panel)

### 4.1 The Cast of Characters
4.1.1 **The Personal Assistant (PA)** — You. Or rather, the AI you talk to in the Chat tab. The PA is one of a kind — there is exactly one — and it is the entry point into the rest of the fleet.
4.1.2 **The Supervisor (one, system-defined, cannot be deleted)** — A system agent that polls the task board every two minutes, looks at every task with status "ready," and decides what to do with it. If the task is too large, it breaks it down into smaller tasks. If it is small enough, it assigns it to the most appropriate Worker. The Supervisor is a manager; it does not write code itself.
4.1.3 **The QC Agent (one, system-defined, cannot be deleted, ephemeral per review)** — A reviewer agent that wakes up whenever a Worker submits a pull request for review. It inspects the diff, checks test coverage, runs through a standardized review checklist, and either approves the work or rejects it with feedback. It is "ephemeral" in that a fresh QC agent instance is created for each review and destroyed when the review is done.
4.1.4 **The five pre-defined Workers** — Out of the box, AutoDev creates five Workers when you first launch: Worker Backend (claude-sonnet-tier model, for API and agent logic), Worker Frontend (sonnet-tier, for React and UI), Worker Platform (sonnet-tier, for infrastructure), Worker MCP (opus-tier, for protocol-level work that needs the strongest reasoning), and Worker Cleanup (haiku-tier, the cheapest model, for dead-code removal and small refactors).
4.1.5 **Custom Workers** — You can create as many additional Workers as you want from the Settings → Workers panel. Each custom Worker has a name, a type (which controls what kinds of tasks the Supervisor will route to it), a list of primary repositories it is responsible for, a model choice, and a polling interval.
4.1.6 **Worker statuses** — A worker is always in one of: `created` (recipe set up but not yet started), `active` (polling for work), `working` (executing a task right now), `queued` (more tasks waiting), `error` (something blew up), `paused` (manually stopped by user), or `deleted` (soft-deleted, kept for history).
4.1.6.a **Extended status vocabulary** — Recent fixes added intermediate states the UI can distinguish: `polling` (a tick fired and is querying the orchestrator — not yet doing real work), `pausing` (a transient sidecar connection blip, retrying), `cooldown` (backoff window after an error, with a countdown to the next attempt), and `offline` (recipe disabled or sidecar permanently down). These richer states stop "Error" from being shown when a worker is merely mid-poll.
4.1.6.b **Authoritative push-based status (not derived)** — The Agent Roster used to compute each worker's status by inspecting the task board and timestamp staleness, which produced phantom `idle → error → idle` flicker between ticks. The status is now pushed by the backend via a `worker:status` event stream with structured payload (slug, current status, previous status, optional reason, optional cooldown seconds). The UI never infers status from anything else.
4.1.7 **Auto-launch on app open** — When you start AutoDev, every Worker that is not paused or deleted and has a recipe bound to it starts polling immediately. You do not have to click "start fleet" — the fleet wakes up with the app.
4.1.8 **Silent failure on missing config** — If a Worker has no API key configured, it does not crash the app — it just goes into the `error` state and tries again on its next tick, so as soon as you add the missing key the worker comes back to life on its own.

### 4.2 How a Task Flows Through the Fleet
4.2.1 **The full happy-path lifecycle** — You speak to the PA in Delegate Mode and ask for something. The PA writes a structured task to the internal board with status `ready`. The Supervisor sees it on its next poll (within ~2 seconds, because the act of creating a task wakes the Supervisor immediately). The Supervisor decides: small enough to assign directly, or large enough that it should be broken down first. It assigns to the best-fit Worker. The Worker wakes (also pushed, sub-second), claims the task, creates an isolated working copy of the repo, does the work, runs tests, opens a pull request on GitHub, and changes the task status to "in review." QC wakes up, reviews the PR using a graph-aware code-impact analysis, and either approves or rejects. On approval, the Supervisor instructs the Worker to merge and the task closes.
4.2.2 **The internal task board, not GitHub Issues** — Unlike older multi-PC fleet designs that used GitHub Issues as the shared blackboard, AutoDev keeps the task list inside a local database file on the user's own machine. This is dramatically faster (no API latency), is not rate-limited, works offline, and gives the user full control over custom statuses and priorities. GitHub is still where pull requests live — AutoDev just does not use Issues for assignment.
4.2.3 **Task fields** — Every task has a title, a description, a priority (p0-critical through p3-low), a type (epic, feature, bug, refactor, docs, infra), a status, acceptance criteria, the worker it is assigned to, the GitHub PR URL once one exists, the parent task ID if it is part of an epic, and estimated/actual token costs. Extra columns exist for diff risk, budget overrides, phase/milestone/epic hierarchy, and lock fields for orphan detection.
4.2.4 **Task statuses** — `ready` (created but unassigned), `assigned` (Supervisor has picked a Worker), `in_progress` (Worker is executing now), `qc_review` (work submitted, awaiting QC), `done` (passed QC and merged), `qc_rejected` (failed review), `failed` (worker hit an unrecoverable error), `build_failed` (work was produced but it doesn't compile or pass basic validation — a terminal state distinct from generic `failed`), `blocked` (waiting on a dependency), `cancelled`.
4.2.4.a **Why `build_failed` is its own status** — Generic `failed` suggests the agent crashed. `build_failed` distinguishes "the worker did its job, but the resulting code doesn't build" — a different diagnosis with different remediation (re-run the worker on the same task vs. throw the work away). Both the database, the UI, and the `update_task_status` tool schema include this status.

### 4.3 Push-Based Dispatch (No Waiting on Cron)
4.3.1 **Why push beats poll** — In an early fleet design, a Worker would wake up only on its cron schedule. If the schedule was "every 2 minutes" and a task arrived two seconds after the last poll, the task would sit there for nearly 2 minutes before being noticed. AutoDev fixes this by *pushing* — when something interesting happens, the relevant agent is poked awake immediately.
4.3.2 **Creating a task kicks the Supervisor** — The moment a task is created (by you in Delegate Mode, or by a Worker requesting a decomposition), the Supervisor's recipe fires immediately on top of its normal schedule. This collapses an observed 21-second dead window down to under 2 seconds.
4.3.3 **Assigning a task kicks the Worker** — The moment the Supervisor assigns a task, the assigned Worker is kicked. The Worker picks up the task in roughly a second instead of waiting up to its full polling interval.
4.3.4 **Submitting for review kicks QC** — When a Worker changes a task's status to "qc_review," QC is kicked. The result is an end-to-end flow that feels continuous instead of drip-fed.
4.3.5 **Real-time UI refresh** — Every write to the task board emits a `task:changed` event. The AutoDev Panel listens for these and updates instantly. There is no "refresh" button anywhere because there does not need to be one.

### 4.4 The Supervisor in Detail
4.4.1 **What the Supervisor does** — It is the dispatcher. It does not write code; it routes work. Every two minutes (or sooner, if kicked) it scans the task board, picks every `ready` task, decides whether each one is atomic or needs breakdown, and assigns or breaks down accordingly.
4.4.2 **Worker selection** — When the Supervisor needs to pick a Worker for a task, it looks at three signals. First, the task's `repo` field is matched against each Worker's list of primary repositories. Second, the task's `type` is matched against each Worker's specialization (a `bug` in a backend repo goes to Worker Backend). Third, Workers that are currently `working` are skipped so the system does not pile up on a busy Worker.
4.4.3 **Semantic affinity routing** — Beyond the simple match above, AutoDev keeps a history of which Workers have succeeded on which kinds of tasks before. A scorer ranks Workers by their historical success rate on tasks with similar patterns (similar language, similar domain, similar keywords). Over time, the Supervisor learns which Worker is genuinely best for which kinds of work.
4.4.4 **The routing history table** — A small table records every task-to-worker routing decision, the outcome (success, failure, timeout), and how long it took. This feeds the affinity scorer.
4.4.5 **Worker Affinity Panel** — A panel inside the AutoDev Panel visualizes this history as a matrix: Workers along one axis, task patterns along the other, with cells colored by affinity score. You can see at a glance which Worker is your best frontend specialist.
4.4.6 **Mode 2 by default** — The Supervisor runs in the heavyweight Worker mode through the Python orchestrator, so it has access to memory, lifecycle state machines, and the coordination protocol.

### 4.5 The QC Agent in Detail
4.5.1 **Ephemeral by design** — A new QC instance is spun up just for each review and torn down when finished. There is no long-lived QC process holding state between reviews — each PR gets a fresh agent that is not contaminated by the previous review's context.
4.5.2 **The QC review cycle** — Fetch the PR diff, run through a standardized review checklist, post comments where appropriate, and update the task status to either `qc-approved` or `qc-rejected`.
4.5.3 **Graph-aware reviews** — Instead of reading every file in the PR, QC uses the AutoDev code knowledge graph to ask "what other parts of the code does this change affect?" and "what tests cover those areas?" This lets QC focus on the high-risk pieces and skim the rest, which dramatically lowers token cost (under 400 tokens of context for many PRs).
4.5.4 **The QC review skill** — A standardized skill packages the read → analyze → comment → summarize workflow into a single repeatable pattern so every QC review follows the same shape.
4.5.5 **Browser-based verification** — A specialized QC skill drives a real browser (via Playwright) to verify a frontend change actually works in the browser. It launches a browser, navigates to the local dev server, takes screenshots of key states, performs key user interactions, and checks the browser console for errors. The agent decides pass/fail from the screenshots plus the console log.
4.5.6 **Diff risk scoring** — Before reviewing, QC asks "how risky is this diff?" A composite score considers number of lines changed, number of files changed, the code-graph impact radius of the change, and whether security-relevant keywords appear in the diff. The result is a low / medium / high rating that focuses QC's attention.
4.5.7 **The Diff Analysis Card** — A small card surfaces the risk score, the per-file breakdown, and the dominant risk factors. This appears in the task detail expand inside the AutoDev Panel.
4.5.8 **Pre-flight DB gate for QC** — Before spending a token on a QC invocation, the runner checks whether any tasks are actually in `qc_review` status. If the queue is empty, the QC tick is skipped entirely and the worker is set to `idle`. If the queue is non-empty, the actual task list (IDs and titles) is prepended to the trigger message so the QC agent sees them inline rather than relying on its memory of "no tasks last time."
4.5.9 **`get_qc_queue` observability command** — A dedicated command exposes the contents of the QC queue along with each task's age in seconds, so the UI can surface a "QC backlog has been waiting 4 minutes" alert. This lets the user notice if QC is stuck without watching log files.
4.5.10 **Strong recipe imperative** — The QC recipe is written so that calling `list_tasks` is an absolute first step that cannot be bypassed by the agent's memory. Earlier versions had a conditional "if no tasks, stop" clause near the top of the prompt, which an LLM with stale episodic memories could shortcut around. The new wording removes that shortcut, guaranteeing the tool is invoked on every tick.

### 4.6 The Workers in Detail
4.6.1 **Each Worker is a recipe** — A Worker is essentially "an AI agent + a cron schedule + a system prompt." That bundle is called a recipe. When the recipe ticks, the agent wakes up, polls the board, and acts.
4.6.2 **The standard worker pattern** — Claim a task (mark it as yours so no other Worker grabs it), analyze it (read the relevant code, understand acceptance criteria), execute it (write or edit files, run tests), and report it (open a PR, update the task status). This pattern is encoded as a Skill called `worker-task-execution` and is referenced by every standard Worker recipe.
4.6.3 **Worktree isolation per task** — When a Worker claims a task, AutoDev creates a fresh copy of the repository in a special directory and branches off from `main`. The Worker works in that isolated copy. This means several Workers can be working on several different tasks in the same repository at the same time without stepping on each other.
4.6.4 **Orphan detection by process ID** — If a Worker crashes or the app is force-killed, its task can be left in `in_progress` with a stale lock. AutoDev's orphan detector checks whether the process that claimed each in-progress task is actually still alive. If it is not, the task is automatically reset to `ready` so another Worker can pick it up.
4.6.5 **Orphan alert UI** — When orphaned tasks are detected, a red badge appears in the AutoDev Panel listing the affected task IDs and the dead process they were claimed by. This makes it impossible for tasks to silently get stuck.
4.6.6 **Per-Worker memory namespace** — Each Worker uses a stable identifier ("worker-backend," for instance) for its memory so its accumulated learnings persist across all ticks rather than being thrown away each time.
4.6.7 **Headless silence** — Workers do not have a chat UI, so when they emit streaming events (which normally feed a chat window), those events are silently dropped instead of generating warnings.
4.6.8 **Recipe agent sandbox** — Each Worker's "scratch directory" is its own dedicated folder, not the user's project root. So if an agent writes a temporary helper script (which they sometimes do), it lands somewhere out of the way instead of polluting your real project.
4.6.9 **Stranded task reconciliation** — After every Worker tick, the runner checks for tasks that were marked `in_progress` but whose worker has gone quiet (no further status updates). Stranded tasks are auto-promoted to `qc_review` so the QC agent can decide what to do, instead of being orphaned in limbo. This runs before the post-tick sentinel guard so legitimately-finished work isn't accidentally marked as a build failure.
4.6.10 **Post-tick `guard_and_validate`** — After a Worker finishes writing files, the runner inspects the workspace for the expected ecosystem manifest (package.json, Cargo.toml, pyproject.toml, go.mod, etc.) and checks for the corresponding sentinel that proves dependencies were installed (a `node_modules` folder, a `.venv/pyvenv.cfg` file, a Cargo build artifact, a `go.sum` file). If the manifest is present but the sentinel is missing, the task is automatically marked `build_failed` — the worker wrote code but never installed the dependencies needed to run it.
4.6.11 **`[AUTODEV WORKSPACE]` header on every invoke** — Every Worker tick that goes through the heavyweight Mode 2 path has a workspace header prepended to its trigger message, listing the current `working_dir`. This prevents the Worker from accidentally writing to a stale path that might still be remembered by mem0 from an earlier session in a different project.

### 4.7 Worker Management UI (Settings → Workers)
4.7.1 **The worker table** — A grid showing every worker: name, type, status, model, the recipe bound to it, and an action column. System agents (Supervisor, QC) are tagged with a SYSTEM badge so you cannot accidentally delete them.
4.7.2 **Per-row actions** — Start, Stop, and Delete buttons. Delete is a soft delete (the row stays in the database for history) so accidentally clicking delete is recoverable.
4.7.3 **Color-coded status** — Status text uses colors plus a glow effect for "working" so a quick scan of the table tells you what every Worker is up to.
4.7.4 **Inline recipe binding** — A small dropdown in the recipe column lets you bind a different recipe to a Worker without leaving the table.
4.7.5 **The Create Worker modal** — A form for new Workers: name (auto-slugified), worker type (picker), primary repositories (comma-separated), model, polling interval in seconds, and a description. Name and type are required; everything else has sensible defaults.

### 4.8 The Agent Roster
4.8.1 **A live table at the top of the AutoDev Panel** — One row per agent. Each row shows a status dot, the agent's name in monospace, what the agent is currently doing (either the active task or an idle message), and a live elapsed-time timer.
4.8.2 **Color-coded status dots** — Green for active (polling), yellow for working (executing right now), blue for idle, red for error.
4.8.3 **Live elapsed timer** — The number ticks every second. If an agent has been stuck on the same task for more than two minutes, the timer turns a warning color so you notice.
4.8.4 **Inline tool + file** — The row's "current task" column shows the actual tool name and file the agent is working with right now, not a generic "polling..." message. So you might see "worker-backend · edit_file · src/api/health.py +12 -3."
4.8.5 **Driven by the workers database** — The roster is built from the real list of workers in the database, including any custom workers you created, not from a hardcoded list.

### 4.9 The Fleet Live Monitor
4.9.1 **A live mission-control view** — Below the Agent Roster sits the Live Monitor: a grid of cards, one per agent, showing what each is doing in real time. Designed so anyone can glance at the screen and immediately understand the state of the fleet.
4.9.2 **The Live Monitor header** — Shows a pulsing LIVE badge if at least one agent is active, a STALE warning if the last event was more than 30 seconds ago, an OFFLINE warning if the sidecars are down, a collapse toggle (persisted across restarts), and a "last event N seconds ago" counter.
4.9.3 **Card grid** — Auto-fitting columns with at least 200 pixels per card. At a wide display, seven agents render as four columns and two rows. Idle agents are dimmed to half opacity so they fade into the background without disappearing.
4.9.4 **Per-card layout** — Header (status dot, agent name, type icon), a stage pipeline (the agent's progress through its workflow stages), the current tool and file, a token sparkline showing recent token usage, and a tail of recent log lines.
4.9.5 **Five card states** — Active (full color, amber dot, log live), idle (dimmed, blue dot), error (red border, red dot, error message), working-blocked (waiting on something), stopped (gray dot with a Start button).
4.9.6 **Expand on click** — Clicking a card expands it to span two columns and grow the log tail from 3 lines to 12. Click again or hit Escape to collapse.
4.9.7 **Stage pipeline per role** — Workers progress through claim → read → write → test → push. The Supervisor progresses through scan → think → dispatch → monitor. QC progresses through claim PR → read diff → review → verdict. Each stage is one of done (green), current (pulsing amber), pending (gray), error (red).
4.9.8 **Smart stage inference** — The stage doesn't have to be manually set; AutoDev infers it from which tool the agent just used. Reading a file means "read" stage. Editing a file means "write" stage. Running a test command means "test." And so on.
4.9.9 **The mini log tail** — Shows the last several tool calls with a timestamp, the tool name (in blue), a truncated argument (with a full-text tooltip on hover), and a ✓ or ✗ at the end. Errors get a red left border on the line. In-progress operations show a spinning indicator instead of a timestamp.
4.9.10 **Token sparkline** — A small horizontal bar shows session token usage against budget (green → yellow at 70% → red at 90%) paired with a 10-bar sparkline showing token activity in the last five seconds. The visual quickly tells you "this Worker is burning tokens" or "this Worker is mostly idle."
4.9.11 **Event throttling** — Token-update events are batched every half-second instead of being processed on every chunk, which keeps the rendering smooth even when an agent is streaming hundreds of tokens per second.
4.9.12 **Thought snippets** — If the agent is doing chain-of-thought reasoning, short snippets of its thinking can be surfaced as dim italic lines prefixed with `···` in the log tail, so you can see at a glance what it is reasoning about.

### 4.10 The Task Board (Kanban)
4.10.1 **Four columns** — Ready, In Progress, QC Review, Done Today. The first three are open-ended; "Done Today" only shows tasks completed during the current day, so the board doesn't grow forever.
4.10.2 **"Ready" merges `ready` + `assigned`** — Tasks that are merely created versus tasks that have been assigned but not started both appear in the "Ready" column. The distinction lives on the card but doesn't fragment the columns.
4.10.3 **Failed tasks show with ✗** — If a task ended in failure it still goes into the Done Today column for visibility, marked with a clear ✗ so you can spot what went wrong.
4.10.4 **Card layout** — Each task card shows `#id · priority`, the title, and the assigned worker. Clicking the card expands it to show the acceptance criteria and description in full.
4.10.5 **Live progress bar on in-progress tasks** — A small bar inside each in-progress card gives a rough sense of progress.
4.10.6 **Real-time refresh** — Cards move between columns automatically when the underlying status changes. You never have to refresh.
4.10.7 **Drag-and-drop with safe-column rules** — Tasks can be dragged between *Ready*, *Blocked*, and *Done Today* columns by the user. The *In Progress* and *QC Review* columns are read-only — only agents can place tasks there, because moving a task into those columns has real side effects (worker claim, QC kick) that should only happen through tool calls.
4.10.8 **Task Detail Modal** — Click any task card to open a full-width modal showing the title, description, acceptance criteria (rendered markdown), status history timeline (who changed what when), assigned worker, GitHub PR link if a PR has been created, accumulated token cost, and a `[View in IDE]` button that opens the IDE tab to this task's worktree.
4.10.9 **Per-column count badges** — Each column header shows `(N)` indicating how many tasks live in that column. Updates live as tasks move.
4.10.10 **Card priority colors** — Each card carries a small priority badge: `p0` red, `p1` orange, `p2` yellow, `p3` gray. Lets the user spot critical work at a glance even without reading the title.

### 4.11 Metrics Footer
4.11.1 **Cost Today** — Sum of all token costs for the day in dollars (green if under the daily budget).
4.11.2 **Tokens** — Total tokens used today across all agents.
4.11.3 **Closed** — Tasks completed today (passing QC).
4.11.4 **Build Failed** — Count of build-failure system events for the day.
4.11.5 **QC Pass Rate** — Percentage of QC reviews today that resulted in approval; turns yellow below 100%.

### 4.12 Subagents (Workers' Helpers)
4.12.1 **What a subagent is** — When a Worker is handed a complex task, it can spawn smaller helper agents to do parts of the work in parallel. Each subagent is a real, isolated AI agent with its own context, its own tool set, and its own conversation, but its result feeds back to the parent.
4.12.2 **Spawning a single subagent** — A tool called `spawn_subagent` lets the parent dispatch one helper. The parent waits for the helper to finish and receives a summary: what the subagent did, which files it touched, the cost, the number of tokens, and the duration.
4.12.3 **Spawning many in parallel** — A second tool, `spawn_parallel_subagents`, lets the parent fire off between two and ten helpers at the same time. Useful when, for example, the Worker needs to review five files in parallel, or try three different implementation approaches.
4.12.4 **Concurrency safety** — There are caps to prevent a runaway: globally, no more than sixteen subagents at once; per-parent, no more than ten; and nesting depth no more than three (so a subagent's subagent's subagent is the deepest you can go).
4.12.5 **Isolation per subagent** — Each subagent has its own working directory (typically a git worktree), so file changes in one subagent do not collide with another.
4.12.6 **Custom system prompt and tool subset** — When spawning, the parent can specify a custom system prompt and a narrower set of tools the subagent is allowed to use. Subagents can be more constrained than their parents.
4.12.7 **Per-subagent timeout** — Each subagent has a maximum lifetime (default five minutes). If it does not complete in time it is terminated and the parent receives an error.
4.12.8 **Three-layer orphan cleanup** — Subagents are aggressively cleaned up so they cannot leak. First, a happy-path deletion when the subagent finishes. Second, an automatic deletion if the permit is dropped (a Rust pattern that guarantees cleanup on panic). Third, a startup sweep on app launch that removes any subagent registrations left over from a previous crashed run.
4.12.9 **Subagent Monitor UI** — A side panel in the Chat tab shows every active subagent grouped by parent, with depth badges, status indicators, and abort buttons. You can see the entire family tree at a glance.
4.12.10 **Per-subagent event stream** — Click into a subagent and you see its real-time event log (tokens streaming, tools being called, errors happening) the same way you see the main agent's events.
4.12.11 **Subagent result card** — When a subagent finishes, a summary card shows its final output, the files it modified, the cost in dollars, and the tokens used.
4.12.12 **Where subagents are available** — Subagent tools are registered in Fast Development and Review PA modes. They are intentionally not registered in Delegate mode (in Delegate mode the PA delegates work to the fleet, not to subagents).
4.12.13 **Ephemerality via create-invoke-delete lifecycle** — Rather than the Orchestrator having a special "ephemeral agent" API, AutoDev reuses the same three endpoints used by the persistent fleet (Create agent, Invoke-with-stream, Delete agent). A subagent is "ephemeral" because its life is bounded by one subtask: the bridge creates it, streams the result back, and immediately deletes it. So every subagent is a genuine full-fidelity LivingAgent (with memory, lifecycle FSMs, and the A2A protocol) — not a stripped-down stub — for the duration of its task.
4.12.14 **Slug convention** — Each subagent's slug follows the pattern `subagent-{parent_slug}-{epoch_millis}-{seq}` (e.g. `subagent-worker-backend-1745740800123-1`). The parent slug preserves traceability in logs and the UI lineage tree; the millisecond timestamp guarantees uniqueness across restarts; the sequence counter handles same-millisecond parallel spawns. The reserved `subagent-*` prefix is what lets the startup orphan-sweep distinguish ephemeral subagents from persistent fleet members.
4.12.15 **Tiny per-spawn YAML cost** — Each subagent spawn writes a small YAML config file and the delete removes it. The combined I/O cost is roughly 10 to 100 milliseconds, which is negligible against the typical 5-to-60-second LLM inference that follows. The trade-off is accepted because it lets subagents reuse every piece of fleet infrastructure with zero new endpoints.
4.12.16 **Memory records persist after deletion (as a feature)** — Episodic and semantic memories written during a subagent's task remain in the memory store after the subagent is deleted. This is intentional: the parent can later query "what did my subagent learn?" by recalling memories tagged with the subagent's session ID. It also means that re-spawning a similarly-named subagent later could benefit from prior learnings if the cache is queried by parent slug.
4.12.17 **Three-layer cleanup in detail** — Layer 1 (happy path): after the SSE stream completes, the bridge calls DELETE. Layer 2 (RAII): the subagent handle implements a drop trait that fires a best-effort delete on panic, early return, or any `?`-operator bailout. Layer 3 (startup sweep): when the sidecar manager boots core-orchestrator, it first scans the agents directory for any leftover `subagent-*.yaml` files and removes them before the orchestrator starts up. So even a hard crash mid-subagent cannot leave a permanent ghost agent.
4.12.18 **Bridge as the migration boundary** — The lifecycle approach is documented as an architectural deviation from the original Phase 4.9 spec (which envisaged dedicated `/create`, `/process`, `/terminate`, and `/events` endpoints that don't exist in the Orchestrator today). If the Orchestrator ever adds true in-memory transient agents, the bridge layer can be swapped out with zero changes to the manager, the tools, the UI, or any of the type definitions. The bridge is the only thing that knows the underlying contract.

### 4.13 Worker → Supervisor Communication
4.13.1 **What it solves** — Sometimes a Worker hits something it cannot handle: the task is bigger than expected, the work would be better done by a different specialist, it needs information only a human can provide, or it has hit a blocker. Rather than failing silently, the Worker can request specific actions from the Supervisor.
4.13.2 **decompose_audit** — The Worker proposes a breakdown ("this should really be three sub-tasks") and the Supervisor creates those sub-tasks atomically.
4.13.3 **reassign** — The Worker explains why it is not the right specialist and the Supervisor moves the task to a different Worker.
4.13.4 **needs_info** — The Worker has a question only a human can answer. The request escalates to the PA's view so the user can answer.
4.13.5 **report_issue** — The Worker reports a blocking issue or an environmental problem (e.g. "the test runner is broken").
4.13.6 **request_review** — The Worker thinks the work is done and wants QC to start reviewing now, rather than waiting on the normal cadence.
4.13.7 **A visible queue** — Worker requests surface in the AutoDev Panel so the user can see them and act on the `needs_info` ones.

### 4.14 Cost & Token Tracking (Fleet-Wide)
4.14.1 **Per-session cost** — Every chat session has a live cost number, computed from token use and the model's published prices.
4.14.2 **Per-worker cost** — A daily roll-up table aggregates token use and cost per Worker so you can see "Worker Cleanup cost me $0.30 today; Worker MCP cost me $4.20."
4.14.3 **Per-task cost** — Each task records its final token count and cost when it completes, so expensive tasks stand out in the Dashboard.
4.14.4 **Live cost bar in chat** — As the Worker streams output, the cost number updates in real time.
4.14.5 **Budget Circuit Breaker** — Every task has a budget (default: 100,000 tokens, $0.50, 20 turns). Before each new turn, the budget is checked; if any limit is exceeded the task is marked as `error` with a clear reason and the agent stops. Defaults can be overridden per task.
4.14.6 **Budget Bar UI** — A horizontal progress bar in each task card shows token / cost / turn use against the budget. It turns red past 80%.

### 4.15 Epic / Phase / Milestone Hierarchy
4.15.1 **Tasks can be grouped into larger structures** — Beyond just having a parent task, AutoDev supports the notion of a `phase`, a `milestone`, an `epic`, and a list of other tasks that block this one.
4.15.2 **Phase order** — Tasks within a phase are ordered, so the Supervisor knows that phase 2 tasks should not start until phase 1 tasks finish.
4.15.3 **Blocked_by list** — A task can list other tasks it cannot start until those finish. The scheduler refuses to assign a blocked task until its dependencies are done.
4.15.4 **Epic Phase View** — A Gantt-like visualization groups tasks by epic, then by phase, drawing arrows for dependencies. This is the high-level view of long-running work.

### 4.16 Demo Mode
4.16.1 **What it does** — A toggle in Settings that forces every agent to use the lightweight Mode 1 (Rust-native) execution path, bypassing the Python sidecars entirely.
4.16.2 **Why it exists** — Useful for demos, for environments where the sidecars cannot run, for benchmarking the bare Rust engine, and for users who want a leaner footprint at the cost of the deep memory/coordination features.

---

## 5. Tools Exposed by AutoDev

### 5.1 How Tools Work
5.1.1 **What a "tool" means here** — A tool is a thing the AI can choose to use during a conversation. Reading a file is a tool. Running a shell command is a tool. Creating a task is a tool. The AI sees a list of available tools, picks one, fills in the parameters, and AutoDev executes the call and returns the result.
5.1.2 **Common shape across all tools** — Every tool has a name, a description, a JSON schema describing its parameters, a flag for whether it is read-only, and an execute function. This uniform shape means the agent does not have to learn each tool's quirks individually.
5.1.3 **Three sources of tools, one unified registry** — The agent fleet has three different places tools can come from: AutoDev's built-in Rust tools (file I/O, shell, planner, graph, etc.), the Orchestrator's *internal* tools (added Python helpers like web search), and tools discovered from external MCP servers (GitHub, Slack, etc.). They all show up to the LLM as one flat list with no visible distinction; the agent just picks the right name.
5.1.4 **Name-based routing** — Once the LLM picks a tool, a single "unified tool handler" routes the call to the right backend by looking at the name. Names ending in `__internal` go to the internal handler, names ending in `__autodev` go to the AutoDev handler, names with known MCP prefixes (`github_`, `slack_`, etc.) go to the corresponding MCP server. This lets the three sources coexist without naming collisions.
5.1.5 **Tool results carry a `hint` channel** — Every tool result has the main output plus an optional "hint" string. The hint is woven into the LLM's view as a `[HINT]` suffix on the result text but is NOT shown in the raw output stored in the database or surfaced in the UI's tool card. So an agent that hit a result cap sees "narrow your pattern or path" in its own next turn, while the user just sees the actual result. Hints are auto-attached when an output was truncated, when validation found errors, when a token cap was hit, etc.
5.1.6 **Per-mode registration** — Which tools are available depends on the current PA mode. Fast Development gets the full suite. Review gets the same suite with confirmation wrappers. Delegate gets a tiny subset (only create-task and the planner).
5.1.7 **Concurrent vs serial execution** — Read-only tools can run in parallel (up to 10 at a time); write tools run serially. So if the agent decides to read five files at once, all five reads happen simultaneously. But if the agent decides to edit three files, the edits happen one after another to keep things safe.
5.1.8 **Per-tool output budget** — Each tool has a maximum output size to prevent one giant output from swamping the context window. The defaults vary by tool: 8 KB for grep, 20 KB for shell, 64 KB for reading a file, 4 KB for directory listings.
5.1.9 **Inline validation after writes** — After every file write or edit, AutoDev quickly runs a syntax check on the file and surfaces the result in the tool's hint. So an agent writing broken Rust sees "0 syntax errors" or "2 syntax errors" in the same response, without needing a separate verification step.
5.1.10 **Memory-aware execution tracking** — Every tool call is wrapped in a small state machine (pending → executing → completed/failed) and registered in a global execution registry for observability. The wrapper also has a memory-aware twist: before running, it checks whether the same tool failed in similar circumstances before; after running, it stores the outcome so future calls can adapt. This is how the orchestrator learns "this tool tends to fail in this pattern" over time.

### 5.2 File-Reading and Editing Tools
5.2.1 **`read_file`** — Reads a file's contents. Accepts optional line offset and limit so the AI can paginate through very large files. Has a 256 KB size guard: if the file is larger and no line range is given, the tool refuses with an instructive error telling the agent to use offset/limit or grep instead. Output is prefixed with line numbers (e.g. `   1 | use serde::Deserialize;`) so the agent can refer back to specific lines when calling `edit_file`. Detects encoding (UTF-8, UTF-16, with byte-order marks) automatically and refuses binary files.
5.2.2 **`write_file`** — Writes content to a file. Before overwriting, automatically snapshots the existing file so undo is possible (see file checkpointing). Creates parent directories if needed. After writing, runs a quick Tier 1 syntax check and surfaces any errors inline as a small bullet list appended to the success message.
5.2.3 **`edit_file`** — Find-and-replace edits inside an existing file. Requires the find string to be unique in the file (so the agent can't accidentally replace two places); a `replace_all: true` flag exists for the deliberate multi-match case. On a "not found" error, the response gives the file's total line count plus a hint to use `read_file` with start_line/end_line to inspect the target area — the older error format that injected 500 chars of the file back into the prompt has been removed because those previews piled up across retries.
5.2.4 **`append_file`** — Appends content to the end of a file without having to read or rewrite the file. Uses an append-mode file handle that only reads the last byte to check whether the existing file ends in a newline; if not, a newline is inserted before the new content (controlled by an opt-out `newline: false` flag). Creates parent directories if needed.
5.2.5 **`check_file`** — Runs the chosen tiers of validation on a file (syntax, imports, types) and returns consolidated diagnostics with line numbers and severity. The agent uses this when it wants to know whether its work compiles before claiming a task done.
5.2.6 **`list_directory`** — Lists files and subfolders in a directory. Caps the response at a configurable `max_entries` (default 500) so accidentally listing `node_modules` does not blow up the context, and emits a `truncated: true` flag plus a "narrow your scope" hint when the cap is hit.
5.2.7 **`glob`** — Find files matching a glob pattern (like `src/**/*.rs`). Returns paths only by default; size and modification timestamp metadata are opt-in via an `include_metadata: true` flag — agents rarely need them and they add significant overhead.
5.2.8 **`grep`** — Search file contents for a regex pattern. Returns matches with line numbers and highlighted positions. Capped at 30 results and 1 context line by default to keep output bounded; both are tunable per call. Uses a streaming search engine so a hit deep in a giant file is found without loading the whole file into memory. Captures *all* matches on a line (not just the first), so multi-match lines are not silently truncated.

### 5.3 The Shell Tool
5.3.1 **`bash`** — Execute a shell command and capture the output. The default timeout, the exit code, and the standard error are all reported back to the agent.
5.3.2 **Output truncation (tail, not head)** — If a command produces more than the per-tool budget, AutoDev keeps the *tail* of the output (where error messages and stack traces typically live) and drops the head (where progress noise typically lives). The truncation indicator is placed at the *start* of the output rather than the end, so an agent that focuses on the first few lines still sees that truncation happened — earlier versions put the indicator at the bottom where the agent might miss it.
5.3.3 **Windows compatibility hints** — Because shell tools written for Linux often fail on Windows, the system prompt injects compatibility hints (PowerShell equivalents) when running on Windows.
5.3.4 **ANSI color rendering in UI** — The chat panel renders the colored output from commands like cargo and npm correctly, instead of showing the raw escape codes.

### 5.4 Task Board Tools
5.4.1 **`create_task`** — Creates a new task on the internal board with status `ready`. Accepts title, priority, type, acceptance criteria, and an optional description. Returns the new task ID. As a side effect, immediately kicks the Supervisor's recipe so dispatch happens within seconds of creation rather than waiting for the next cron tick. Available in all three PA modes (intentionally safe in Delegate mode because it does no file I/O).
5.4.2 **`list_tasks`** — Returns tasks from the board, optionally filtered by status. By default, runs in a *compact* mode that returns only the essentials (id, title, priority, type, status, assigned_worker, rejection reason, retry count) per task — about 40 tokens each instead of the 200+ a full record consumes. The compact response includes a hint pointing the agent at `get_task(id)` for the full record. A `compact: false` flag exists for the rare case where every field is needed.
5.4.3 **`get_task`** — Returns the full record for one specific task (description, acceptance criteria, timestamps, etc.). The intended workflow is "list compact, then `get_task` on the one or two that matter."
5.4.4 **`update_task_status`** — Updates a task's status and optional assigned worker. As a side effect, kicks the scheduler so the assigned Worker (or QC if transitioning to `qc_review`) wakes up immediately rather than waiting for its next tick. For `rejected` transitions where the caller forgets to pass `assigned_worker`, the tool reads the worker from the existing database row so the kick is never silently skipped.
5.4.5 **`assign_task`** — Atomic assignment-to-worker operation. Validates the chosen worker against the known fleet slugs (rejects a typo or an invented name), updates status to `assigned`, and kicks the assignee so it picks up the task in roughly one second.

### 5.5 Planner Tool
5.5.1 **`planner`** — A single tool with 14 actions (create plan, add step, update step, get plan, list plans, check dependencies, delete plan, abandon plan, plus fleet-specific actions for breaking down epics, claiming items, creating subplans, and submitting for QC review). The input is a small YAML snippet. Registered in all three PA modes including Delegate.
5.5.2 **Discoverable schema via `listActions`** — The planner's tool description is intentionally short (around 200 characters) — it doesn't try to document every action up front because that would balloon the tool description on every LLM call. Instead it tells the agent to call `action: listActions` to fetch the full schema on demand. This keeps every tool-discovery context lean.
5.5.3 **Stable `step_id` references** — Each step gets a UUID at creation time. The `updateStep` action accepts either a 0-based `step_index` (fragile — shifts if steps are inserted out of order) or a stable `step_id` (preferred). Agents that record the `step_id` from each `addStep` response can update that exact step reliably across any number of subsequent additions.
5.5.4 **No more `executeTemplate` action** — An earlier draft of the planner listed an `executeTemplate` action in its description but never implemented it. Agents would try to call it, get an "unknown action" error, retry, and loop. The action has been removed from both the description and the action list so the loop is impossible.

### 5.5b Tool-Call Meta-Arguments (MCP Path)
5.5b.1 **What meta-arguments are** — When a fleet worker (running in the Python orchestrator) calls an AutoDev tool through the in-process MCP server, the LLM only knows about the tool's documented parameters. But the Rust side needs to know who is calling, from where, in what session — context the LLM does not have. The solution is a small set of "meta" arguments prefixed with double underscores that the MCP adapter strips off and threads into the tool's execution context invisibly.
5.5b.2 **`__working_dir`** — The current workspace path. Lets the Rust tool resolve file paths correctly for the calling worker's project rather than some unrelated location.
5.5b.3 **`__session_id`** — The agent's stable session identifier (e.g. `worker-backend-...`). Used for memory scoping and event correlation so events emitted by a tool can be tied back to the right worker.
5.5b.4 **`__agent_role`** — The worker's slug (e.g. `worker-backend`). Used by the supervisor-action tool to know who is requesting what, and to kick the right recipe when the supervisor responds. Without this meta-arg, every fleet request used to be tagged "unknown" and the routing broke.
5.5b.5 **Safe default working dir** — If a worker forgets to pass `__working_dir`, the MCP server falls back to the process's current directory and logs a warning, rather than silently writing to an unrelated `~/.autodev` config folder.

### 5.6 Subagent Tools
5.6.1 **`spawn_subagent`** — Fire-and-wait. The parent dispatches a single helper and blocks until it returns. The helper's system prompt always includes an `[AUTODEV WORKSPACE]` header naming the current working directory — including the case where the caller passes no custom prompt at all — so the helper never has to guess where to write files.
5.6.2 **`spawn_parallel_subagents`** — Fire-many-and-wait. The parent dispatches two to ten helpers at once and waits for all of them. Same workspace-header injection applies to every parallel helper regardless of whether each subtask brought its own prompt.

### 5.7 Memory Tools
5.7.1 **`search_memory`** — Lets the agent query its own memory in the middle of a conversation. Supports tier filter, time range, and result limit.
5.7.2 **`remember_this`** — Lets the user (via the agent) explicitly store a fact in semantic memory with high confidence.

### 5.8 Graph Tools (22 of them)
5.8.1 **Discovery tools (5)** — `get_graph_minimal_context` (returns ~100 tokens of "what is relevant to this task"), `get_graph_architecture_overview` (high-level view of communities and how they connect), `get_graph_stats` (node and edge counts, languages, last update), `list_graph_communities` (named clusters of related code), `get_graph_report` (the full markdown report on the codebase).
5.8.2 **Navigation tools (9)** — `query_graph` (callers of, callees of, imports of, tests for, dependencies of), `keyword_search_nodes` (full-text search across symbol names), `get_node_details` (everything we know about one symbol), `find_graph_path` (shortest dependency path between two symbols), `explain_node` (fuzzy lookup plus top 20 neighbors), `get_file_summary` (a summary of one file's contents), `list_callers` / `list_callees` / `list_tests` (immediate-neighbor queries).
5.8.3 **Impact analysis tools (5)** — `get_impact_radius` (what code is affected if I change this symbol), `detect_graph_changes` (what changed between the previous commit and the current state, plus risk score), `get_review_context` (token-efficient PR review context, capped at 800 tokens), `get_affected_flows` (which execution flows touch this file), `get_flow_criticality` (ranked list of business-critical flows).
5.8.4 **Analysis tools (4)** — `get_god_nodes` (highly-connected symbols that suggest architectural choke points), `get_surprising_connections` (cross-module edges that defy expectations), `get_suggested_questions` (auto-generated investigation prompts), `get_graph_diff` (compare two graph snapshots).
5.8.5 **AutoDev-native tools (4)** — `get_worker_graph` (the subset of the graph created or modified by one Worker), `get_task_graph` (the subset of the graph touched during one task), `get_locked_nodes` (symbols currently being modified by an active Worker), `get_graph_status` (info on the graph engine itself).

### 5.9 Browser Automation Tools
5.9.1 **`launch_browser`** — Starts a real browser session (headless or visible) using Playwright. Returns a session ID for follow-up actions.
5.9.2 **`browser_navigate`** — Navigates the open browser to a given URL.
5.9.3 **`browser_screenshot`** — Captures a screenshot of the current page and returns it as a base64-encoded PNG.
5.9.4 **`browser_click`** — Clicks an element matching a CSS selector.
5.9.5 **`browser_type`** — Types text into an input field matching a CSS selector.
5.9.6 **`browser_eval`** — Evaluates a JavaScript snippet in the page context and returns the result. Useful for asserting "window.errors should be empty."
5.9.7 **`close_browser`** — Tears down the browser session cleanly.

### 5.10 MCP Tools (Connected to External Services)
5.10.1 **GitHub MCP** — Once configured, exposes tools for creating issues and pull requests, managing labels, posting comments, etc. This is how Workers create PRs.
5.10.2 **Slack MCP** — Lets agents send Slack messages.
5.10.3 **Memory MCP** — Exposes six core memory operations: store, search, get, update, delete, list. Used by the Orchestrator under the hood.
5.10.4 **Custom MCP servers** — You can configure additional MCP servers (anything that speaks the protocol). Tools are discovered automatically and become available to agents.
5.10.5 **Adobe MCP (explicitly excluded)** — The Adobe creative-tools MCP server exists upstream but is intentionally not part of the AutoDev deployment, to keep the surface focused on developer work.

### 5.11 The AutoDev MCP Server (Self-Hosted)
5.11.1 **Why this exists** — Workers, Supervisor, and QC all need to call the same tools the PA can call. Rather than reimplementing those tools twice (once in Rust for the PA, once in Python for Workers), AutoDev exposes the Rust tool registry through a tiny self-hosted MCP server that the Python Orchestrator can talk to. One source of truth, two access paths.
5.11.2 **In-process and lightweight** — The server runs inside the desktop app's process on a local-only port. There is no separate binary and no extra startup cost.
5.11.3 **Schema discovery is dynamic** — When a Worker asks "what tools are available?", the answer is built fresh from the current tool registry, so adding a new tool in Rust automatically makes it available to Workers without any extra wiring.
5.11.4 **The MCP Tool Browser UI** — A searchable browser inside Settings lets you see exactly which tools are exposed and inspect each tool's parameter schema. This is great for debugging and for understanding what the fleet has access to.
5.11.5 **Idempotent registration** — When the desktop app tells the Orchestrator about the MCP server, the Orchestrator might respond "I already know about that one" with various HTTP error codes. AutoDev treats any "already exists" response as success.
5.11.6 **Five MCP prompts** — The server also exposes five named workflow templates (review changes, map architecture, debug an issue, onboard a developer, pre-merge check) that other agents can discover and use as guided multi-step processes.
5.11.7 **Localhost-only, no auth** — The MCP server binds to the local loopback address only and intentionally has no authentication. It is designed to be reachable only by other processes on the same machine — security comes from the network boundary, not from a credential layer.
5.11.8 **Meta-argument extraction (see §5.5b)** — On every tool call coming through MCP, the server adapter pulls the `__working_dir`, `__session_id`, and `__agent_role` meta-arguments out of the JSON arguments before forwarding the call to the Rust tool, so the tool's execution context is correctly populated even though the LLM never sees those fields.
5.11.9 **Spawned correctly into Tauri's async runtime** — The server is started using Tauri's own async-runtime spawner rather than a raw tokio spawn, so it works when called from the synchronous startup hook without a "no runtime" panic.

### 5.12 Hooks (Tool-Adjacent Extensibility)
5.12.1 **What hooks add** — Hooks fire at lifecycle points around tools and other events: before a tool runs, after a tool runs, on tool error, on permission request. Hooks let you bolt on custom behavior without modifying AutoDev itself.

---

## 6. IDE Panel

### 6.1 Layout
6.1.1 **A compact built-in code viewer** — The IDE tab is not trying to replace VS Code. It is a viewer integrated into AutoDev that shows you what the agents are looking at and changing. Read-only by default.
6.1.2 **Repo selector and toolbar** — At the top: a dropdown to pick which locally-cloned repository to view, buttons to clone a new repo, refresh the tree, and open the file search.
6.1.3 **Tab bar** — A row of open file tabs with close X's, middle-click-to-close, and drag-to-reorder, just like a normal editor.
6.1.4 **File tree on the left** — A 180-pixel collapsible tree of folders and files, with status badges (M for modified, U for untracked, S for staged, D for deleted, ! for conflict).
6.1.5 **Code viewer on the right** — A Monaco-based editor (the same engine VS Code uses) with syntax highlighting for 50+ languages. Read-only by default to keep the agent firmly in charge of writes.
6.1.6 **Symbol Outline** — A collapsible panel on the right that lists every function, class, struct, enum, trait, module, and constant in the current file. Click any symbol to jump.
6.1.7 **Git sub-panel** — Toggle-on Git panel with five tabs: Changes, History, Branches, Stash, and Undo. This is the equivalent of the "Source Control" pane in VS Code.
6.1.8 **Status bar at the bottom** — Shows test results, type-check status, current git branch, the active worktree, and the file's language.

### 6.2 File Tree Behavior
6.2.1 **Recursive scan with depth control** — The tree loads lazily; deep folders show a "load more" placeholder until you expand them. This keeps initial render fast on large repos.
6.2.2 **Live git status badges** — Each file's badge reflects its current git status. Modify a file and the badge appears the moment you save.
6.2.3 **Skip lists for noisy folders** — Folders like `node_modules`, `target`, and `.git` are skipped automatically. You can pass additional ignore patterns.
6.2.4 **File icons** — Lucide-react icons differentiate folders from files. Extension-specific icons are on the roadmap.
6.2.5 **Persisted expansion state** — Which folders you have expanded is saved between sessions, so reopening the tree restores your view.
6.2.6 **Context menu** — Right-click a file or folder for Open, Copy Path, Delete, Rename, New File, and New Folder.
6.2.7 **Drag and drop** — Drag a file inside the tree to move it (within the same repo).

### 6.3 The Code Viewer
6.3.1 **Monaco editor** — Industry-standard editor component. Syntax highlighting for over 50 languages.
6.3.2 **Read-only mode** — The viewer is read-only by default because writes should happen through the AI's tools, not by direct typing. This avoids the AI and the human accidentally fighting over the same file.
6.3.3 **Line numbers and current-line highlight** — Standard editor affordances.
6.3.4 **Code folding** — Click the fold marker in the gutter to collapse a function or block.
6.3.5 **Minimap** — A small overview of the whole file on the right edge.
6.3.6 **Go-to-line API** — Tool result cards in the chat link to specific lines in the IDE. Clicking the link jumps the IDE to that line.
6.3.7 **"Open in IDE" buttons** — File-related tool cards in the Chat tab show an Open in IDE button that switches tabs and scrolls to the right line in one click. This is the bridge from "the agent is reading this" to "show me what it's reading."

### 6.4 Tab Management
6.4.1 **Standard tab UX** — Horizontal tabs with close X, overflow scrolling, drag to reorder, middle-click to close.
6.4.2 **Close-others / close-all** — Programmatic actions exposed by the store; UI for these is on the polish backlog.
6.4.3 **Persisted across restarts** — Reopening AutoDev brings back your open tabs and which one was active.

### 6.5 File Search
6.5.1 **Quick open with Ctrl/Cmd+P** — Pop up a fuzzy file picker that searches the current workspace.
6.5.2 **Fuzzy matching** — Type "uthcfg" to match `user/auth/config.rs`. Scoring favors filename matches over path matches and prefix matches over middle-of-string matches.
6.5.3 **Recent files** — When you open the search with an empty query, recent files appear.
6.5.4 **Keyboard navigation** — Arrow keys + Enter to open + Escape to close.

### 6.6 Symbol Outline
6.6.1 **Flat list of symbols** — Functions, classes, structs, enums, traits, modules, constants. Click a row to jump.
6.6.2 **Filter input** — A small search box filters the outline as you type.
6.6.3 **Tree-sitter upgrade planned** — The current implementation uses regex pattern matching. A future upgrade replaces this with tree-sitter for more accurate parsing.
6.6.4 **Go to definition** — A "resolve symbol" command navigates from a usage to the definition. Cross-file, currently regex-based.
6.6.5 **Find references** — A "find symbol references" command searches the whole workspace for usages.

### 6.7 The Git Sub-Panel
6.7.1 **Twenty-nine git operations** — Open repo, clone, status, diff, stage, unstage, discard, commit, push, pull (fast-forward only), fetch, branch list, branch create, branch delete, checkout, worktree list, worktree add, worktree remove, log, blame, stash list, stash push, stash apply, stash drop, create checkpoint, restore checkpoint, list checkpoints, delete checkpoint, compare checkpoints.
6.7.2 **Library-based git, not shell-based** — AutoDev talks to git through a Rust library that wraps libgit2 directly. This is faster and safer than shelling out to the `git` command and parsing its text output. It also means git always works without depending on having `git` installed on the user's machine.
6.7.3 **Changes tab** — Shows modified files with status badges. Each file has a per-file diff preview. Stage / Unstage / Discard buttons per file or bulk. Stage All and Unstage All actions.
6.7.4 **Staged section** — Inside the Changes tab. Files that are staged. A commit message textarea. A commit trailer field (defaulting to "AutoDev/PA"). A Commit button.
6.7.5 **History tab** — Commit log with author, date, summary, and short commit ID. An ASCII commit graph visualization. A diff viewer for any selected commit. A button to checkout that commit. A button to create a branch from that commit. A merge button (with conflict detection).
6.7.6 **Branches tab** — Local and remote branches sectioned separately. Current branch marked with a check icon in the accent color. Create-new button (with optional checkout). Delete button on hover. Merge button. Each branch shows ahead/behind counts versus its upstream.
6.7.7 **Stash tab** — List of stashes with their messages. Apply with or without pop. Drop. Create-new with a custom message and an include-untracked checkbox.
6.7.8 **Undo tab (Checkpoint Viewer)** — A timeline of file checkpoints grouped by file, newest first. Restore any checkpoint with inline confirmation. (A side-by-side compare-any-two UI is on the roadmap; the backend is ready.)
6.7.9 **Inline commit dialog** — Type a message, pick a trailer, click Commit. The trailer mechanism automatically tags AI commits with "Generated-by: AutoDev/<role>" so you can tell agent work apart from human work in `git log`.
6.7.10 **Fast-forward-only pull** — When you pull, AutoDev will only do a fast-forward merge. Non-fast-forward merges are left as a manual operation, because they need human judgment about how to resolve.

### 6.8 File Checkpoints
6.8.1 **Automatic before every write** — Every time the AI edits a file, AutoDev silently snapshots the prior version. You don't have to think about this; it just happens.
6.8.2 **50 versions per file** — The most recent fifty versions are kept; older ones are pruned automatically.
6.8.3 **One-click restore** — Click any checkpoint in the Undo tab to restore the file to that state.
6.8.4 **Audit trail** — Each checkpoint records the hash of the file before, the hash after, and the message ID that caused the change. So you can trace any version back to the exact agent turn that produced it.

### 6.9 Worktrees (Per-Task Isolation)
6.9.1 **Each worker task gets its own working copy** — When a Worker claims a task, AutoDev creates a fresh worktree from the latest main branch in a dedicated worktrees folder, on a fresh branch named after the task. The worker does all its work there, leaving your main checkout untouched.
6.9.2 **Worktree manager (Phase 4)** — A UI for listing active worktrees, seeing which agent owns each, and navigating into them is on the roadmap.
6.9.3 **Sparse checkout and symlinks** — Future work will use git's sparse checkout plus symlinks for `node_modules` and similar so that each worktree doesn't duplicate gigabytes of dependencies.

### 6.10 IDE Workspace State
6.10.1 **Open tabs + expanded folders persist** — Closing AutoDev and reopening it preserves the IDE state you left.
6.10.2 **Workspace folder selection** — A workspace folder is the project root all agents operate in. You pick it with a native OS folder picker. All agents (PA, Supervisor, Workers) treat it as their working directory.

### 6.11 Annotation Capabilities (Roadmap)
6.11.1 **Inline annotations on code lines** — Future work lets you attach notes to specific lines of code that the PA can read.
6.11.2 **PA-annotated notes** — In Review and Fast Dev modes, you'll be able to highlight a region and have the PA leave a note for itself.
6.11.3 **Delegate-mode annotations** — Annotations created in Delegate mode become part of the acceptance criteria when the PA creates a task for the fleet.

### 6.12 Repo Management
6.12.1 **Clone Repo flow** — Click the Clone Repo button, AutoDev fetches your AIRS-Foundry repos from GitHub (using your stored access token), you pick one, AutoDev clones it locally, the new repo appears in the selector dropdown, and the workspace folder switches to it. One-click clone-and-go.
6.12.2 **Multi-repo support** — You can clone many repos and switch between them from the selector without restarting.
6.12.3 **Per-repo identity for graphs** — The code graph engine identifies each repo by a hash of its absolute path, so two repos with the same name in different folders cannot accidentally share a graph database.

---

## 7. Recipes Panel

### 7.1 What a Recipe Is
7.1.1 **A recipe in plain terms** — A recipe is a small YAML file that says "every N minutes, wake up this AI agent with this model and these instructions, point it at the task board, and let it do its thing." Recipes are how AutoDev runs its fleet without any human in the loop.
7.1.2 **Recipes are for the fleet, not the PA** — AUTODEV.md is your personal instructions to the PA. Recipes are the instructions for fleet agents (Workers, Supervisor, QC). They are deliberately separate systems with different storage, different UIs, and different audiences.
7.1.3 **YAML structure** — A recipe specifies its version, title, schedule (a cron expression like `*/2 * * * *` for every two minutes), permission mode (autonomous, manual approval, smart approval, chat only), agent role, model, parameter list, the actual instructions text, and any extensions (MCP servers) it needs.
7.1.4 **Two flavors** — System recipes ship with AutoDev (Supervisor, the five default Workers, QC review cycle, QC browser check) and are marked locked. User recipes are anything you author or customize.

### 7.2 The Recipe Engine (Backend)
7.2.1 **YAML parser** — Reads recipe files and validates them against the recipe schema. Bad recipes give a clear error rather than silently misbehaving.
7.2.2 **Recipe runner** — Takes a parsed recipe and executes its instructions as an agent message — i.e., it spins up an agent with the recipe's model and instructions and sets it running.
7.2.3 **Cron scheduler** — The component that actually fires recipes on their schedule. Built on top of the `cron` crate and tokio's async runtime.
7.2.4 **Default system recipes** — Created automatically the first time the app launches.
7.2.5 **Per-agent sandbox** — Each recipe agent operates in its own folder so its scratch files don't pollute the user's workspace.
7.2.6 **Worker mode by default** — Recipes route through the heavyweight orchestrator path. If the orchestrator becomes unreachable for several consecutive health checks, they fall back to the lightweight Rust path automatically.
7.2.7 **Startup gate** — The first recipe tick after app launch waits for the orchestrator to report healthy (polling every 2 seconds for up to 45 seconds) so the very first tick does not race against the not-yet-ready orchestrator.
7.2.8 **Minimum tick floor of 10 seconds** — Even if a recipe's cron expression would fire faster (perhaps because a previous run took too long), AutoDev forces at least 10 seconds between ticks. This avoids the "tight loop" failure mode where a slow recipe would otherwise burn CPU.
7.2.9 **Error backoff** — On any recipe failure, the runner sleeps for 60 seconds before trying again, so a broken recipe doesn't spin endlessly.
7.2.10 **Workspace injection on every tick** — Every Worker tick prepends a workspace header to the instructions so the agent knows exactly which folder it should be working in, no matter what stale path mem0 might have recalled.

### 7.3 The Recipes Tab UI
7.3.1 **Split-pane layout** — A 220-pixel list of recipes on the left, recipe detail on the right.
7.3.2 **Recipe cards in the list** — Each card shows filename, schedule, permission mode, and an instructions preview snippet.
7.3.3 **Active recipe highlight** — The selected recipe gets a green border.
7.3.4 **YAML viewer with syntax highlighting** — The detail pane shows the raw YAML with proper coloring.
7.3.5 **Action buttons per recipe** — Run Now (triggers an immediate tick), Schedule (modify cron expression), Edit YAML (open in editor).
7.3.6 **New Recipe button** — In the toolbar; opens the recipe editor with an empty form.
7.3.7 **System / User sections** — Recipes are grouped: system-shipped at the top with a SYSTEM badge, user-authored below.

### 7.4 The Recipe Editor
7.4.1 **Structured form modal** — Rather than typing raw YAML, the editor presents fields: ID (a slug auto-generated from the title), Title, Description, Schedule, Permission Mode, Agent Role, Model, Instructions textarea.
7.4.2 **Save / Cancel / Delete** — Standard modal controls.
7.4.3 **System-recipe protection** — Editing a system recipe (e.g. the Supervisor) shows a warning and requires an explicit override. This prevents accidental damage to the core fleet.
7.4.4 **Version history panel** — Inside the editor, a small panel shows the last several backups of this recipe and offers a Restore button for each.
7.4.5 **Slug auto-generation** — Type a title, the ID field updates to a URL-safe slug. You can override the slug manually if needed.

### 7.5 AI-Assisted Recipe Generation
7.5.1 **Generate with AI panel** — A collapsible section in the editor. Pick a worker type, type a one-line description, hit generate. The AI produces a draft YAML recipe with sensible instructions for that worker type.
7.5.2 **Apply to form** — A button copies the generated YAML into the form so you can tweak before saving.
7.5.3 **Per-worker-type templates** — The AI is given different default instructions per type: backend Worker, frontend Worker, platform Worker, MCP Worker, cleanup Worker.

### 7.6 Recipe Versioning
7.6.1 **Backup on save** — Every time you save a recipe, the previous version is moved into a backups subfolder named with a timestamp.
7.6.2 **History button in editor** — Opens the version history panel showing the last several backups.
7.6.3 **Restore button per backup** — Replaces the current recipe with the chosen historical version.

### 7.7 Binding a Recipe to a Worker
7.7.1 **Inline binding from the worker table** — Each row in the Workers table has a recipe column with a small dropdown. Change the dropdown and the worker's bound recipe updates instantly.
7.7.2 **Visual highlight of current binding** — The selected recipe is shown in the accent color so the current binding is unmistakable.

### 7.8 Permission Modes
7.8.1 **Autonomous** — No prompts; the agent runs end-to-end. Used by all default fleet Workers.
7.8.2 **Manual Approval** — Prompts on every write or destructive action. Used by the PA in Review Mode.
7.8.3 **Smart Approval** — Auto-approves safe operations, prompts on dangerous ones. A middle ground for cautious users.
7.8.4 **Chat Only** — The agent cannot use any tools, only conversation. Useful for pure question-and-answer use.

### 7.9 Run History
7.9.1 **Timestamps and outcomes per recipe** — Each tick is recorded with its start time, end time, success/failure status, and duration.
7.9.2 **Live tick events** — When a Worker ticks, AutoDev emits `worker:tick-start` and `worker:tick-end` events so the UI can reflect activity in real time.

---

## 8. Visibility of AutoDev (Logs Panel)

### 8.1 The Logs Tab UI
8.1.1 **A central log viewer** — The Logs tab gives you one place to read every log AutoDev produces, including the desktop app's own logs and the logs of the background Python services. No need to hunt around the filesystem.
8.1.2 **Date picker** — Pick which day's log to view. Daily rotation means each day is its own file.
8.1.3 **Level filter** — A dropdown with ALL, ERROR, WARN, INFO, DEBUG, and TRACE. Pick ERROR when you want to triage; pick ALL when you want context.
8.1.4 **Free-text search** — A search box filters log lines by substring. Combine with the level filter for fast targeted browsing.
8.1.5 **Auto-scroll toggle** — When on, new log lines push the view downward so you're always seeing the most recent. Toggle off when you want to scroll back without being yanked back to the bottom.
8.1.6 **Auto-refresh** — Every three seconds the view pulls in newly-written log lines so the panel feels live.
8.1.7 **Copy All button** — Copies the visible log to the clipboard with a "Copied!" confirmation. Useful for pasting into a bug report.
8.1.8 **Export button** — Downloads the visible log as a `.log` file.
8.1.9 **Color-coded levels** — Errors are coral, warnings amber, info periwinkle, debug muted, trace dark. The colors are picked to be readable against the dark background without being garish.
8.1.10 **Structured display** — Each line is parsed into timestamp + module + message, then rendered in three aligned columns. This makes scanning much faster than reading wall-of-text logs.

### 8.2 The Log Backend
8.2.1 **Daily rotation** — A new log file is created each day so each day's history is contained.
8.2.2 **Seven-day retention** — Logs older than seven days are auto-deleted to keep disk usage bounded.
8.2.3 **Non-blocking writes** — Log writes happen on a background worker thread so they never slow down the foreground app. A guard ensures the worker thread is properly flushed at shutdown.
8.2.4 **Per-sidecar log files** — In addition to the main app log, each sidecar (Memory MCP, Core Orchestrator) gets its own log file, rotated at 20 MB.
8.2.5 **List and read commands** — The frontend talks to the backend via two commands: list available log dates, and read one specific date's contents.

### 8.3 Log Routing from Sidecars
8.3.1 **Captured stdout and stderr** — Both Python sidecars are spawned with their output piped back to AutoDev. Two background threads per sidecar read those streams line by line.
8.3.2 **Level-aware routing** — Python's INFO and DEBUG lines are routed to debug-level only (so they appear in the sidecar's own log but not the main log), while ERROR and WARNING are surfaced at error/warn level. This keeps the main log focused on signal.
8.3.3 **Long-line filter** — Lines longer than 2000 characters are skipped. This filters out giant system prompt echoes that would otherwise dominate the log.
8.3.4 **Per-spawn line cap** — Each spawn is capped at 50,000 lines. When the cap is reached, a single "cap reached" message is written and further lines are silently dropped.
8.3.5 **Known-noise demotion** — Windows-specific connection-reset patterns are demoted from info-level to debug-level so they appear in the sidecar log but not the main log.

### 8.4 System Events Table
8.4.1 **Structured events from system components** — In addition to text logs, AutoDev records structured events (component name, event type, severity, message, metadata) into the analytics database. Examples: "scheduler reported a recipe failure," "runner triggered fallback to Mode 1," "client observed consecutive health failures."
8.4.2 **Recent-events query** — A query returns up to 500 recent events, newest first, optionally filtered by component and severity.
8.4.3 **System logs in the Dashboard** — A widget in the Dashboard surfaces the most recent system events with severity badges.

### 8.5 Centralized Surfaces
8.5.1 **Three log streams in one place** — The main Rust app log, the Core Orchestrator log, and the Memory MCP log are all readable from the Logs tab.
8.5.2 **CLI access** — The headless command-line tool (`airsctl`) has commands to view, list, clear, and export the same logs, so you can grep AutoDev's logs from a terminal.

---

## 9. Settings Panel

### 9.1 Providers
9.1.1 **The Providers table** — A row per AI provider: Anthropic, OpenAI, Google Gemini, Ollama, Microsoft Azure. Columns are: provider name, masked API key (or "Add key" placeholder), the model in use, and status (Active or Off).
9.1.2 **Add / Test / Save flow** — Click a provider, enter your API key, click Test Connection. AutoDev sends a tiny test call to confirm. On success, the row shows Active.
9.1.3 **Secure storage** — API keys are never written to a plain config file. They go into your operating system's secure keychain (Windows Credential Manager, macOS Keychain, Linux Secret Service via libsecret). AutoDev only stores a reference.
9.1.4 **Ollama special handling** — Ollama doesn't need an API key because it's local. Instead, the row shows the host and port (default `localhost:11434`) and a "Local" badge. A refresh button re-fetches the list of models you've downloaded.

### 9.2 Model Router
9.2.1 **Mapping agent roles to models** — Beyond just "which provider," AutoDev has a table that maps agent roles to specific models. So Supervisor uses opus-tier (best reasoning), Worker Backend uses sonnet (balanced), Worker Cleanup uses haiku (cheapest). A "reason" column documents why.
9.2.2 **Per-Worker override** — Each Worker in the workers table can override the role default with its own model choice.
9.2.3 **Edit routing rules** — A button opens an editor where you can change the role-to-model mapping. Changes take effect immediately.

### 9.3 Model Configuration
9.3.1 **Per-model temperature and top-p sliders** — Stored persistently with reset buttons.
9.3.2 **Extended thinking toggle and budget slider** — For models that support thinking modes, a toggle plus a budget slider from 1024 to 32000 tokens.
9.3.3 **Reasoning effort dropdown** — For OpenAI o-series, low/medium/high.
9.3.4 **System prompt variants editor (roadmap)** — A planned editor for viewing and modifying the three PA-mode prompts (Fast Dev, Review, Delegate).

### 9.4 Key Vault
9.4.1 **OS-native secret storage** — Goes through whichever secret store your OS provides.
9.4.2 **Cross-platform support** — One uniform interface to Windows Credential Manager, macOS Keychain, and Linux libsecret.
9.4.3 **GitHub PAT storage** — Stored the same way as API keys. Get, store, delete commands.
9.4.4 **PAT card UI** — Shows stored/cleared status with a save button, a remove button, and an expiry warning when the token is nearing expiration.

### 9.5 GitHub MCP Integration
9.5.1 **GitHub MCP card** — Enable/disable toggle, organization name input, allowed-repos textarea.
9.5.2 **Persisted config** — Stored as JSON in the app settings database.
9.5.3 **Runtime activation** — When enabled, Workers can use the GitHub MCP tools to create issues, pull requests, manage labels, and post comments.

### 9.6 AUTODEV.md Settings (Roadmap)
9.6.1 **Filesystem sync toggle** — Optional toggle to mirror your AUTODEV.md content from the database to actual files on disk for version control.
9.6.2 **Auto-save toggle** — Save on every keystroke or save manually.
9.6.3 **Import from CLAUDE.md migration tool** — One-click import of existing CLAUDE.md content into AUTODEV.md.

### 9.7 Workspace Folder
9.7.1 **Workspace card in Settings** — Shows the currently-selected workspace folder with an ACTIVE badge, plus Change and Clear buttons.
9.7.2 **Native OS folder picker** — Click Change and the OS's standard folder-picker dialog opens.
9.7.3 **Persisted across sessions** — Your workspace choice survives app restarts.
9.7.4 **The folder all agents use** — PA, Supervisor, and every Worker all treat this folder as their working directory unless explicitly overridden.

### 9.8 Workers Sub-Tab
9.8.1 **Worker table** — Discussed in detail in section 4.7.
9.8.2 **Create Worker button** — Opens the modal for new Workers.

### 9.9 Services / Sidecar Management
9.9.1 **Sidecar Panel** — Lists all four services (Memory MCP, Core Orchestrator, AutoDev MCP, Redis) with their current status.
9.9.2 **Status indicators** — Tone-coded icons: aqua check for healthy, amber spinner for starting, red warning for error, gray dot for idle.
9.9.3 **Start / Stop / Restart buttons** — Available for sidecars AutoDev manages directly. The in-process AutoDev MCP server has no buttons (it's part of the app process), and Redis has no buttons (it's external).
9.9.4 **Resource usage modal** — Click a service name to see live CPU% and memory in bytes.
9.9.5 **Log viewer modal** — Click a service to see the last 500 lines of its log without leaving Settings.
9.9.6 **Manual refresh + auto-poll** — A refresh button plus a 10-second auto-poll keep the panel current.
9.9.7 **Required vs optional** — The Memory MCP and Core Orchestrator are required. Redis is optional (its absence only disables warm caching).

### 9.10 MCP Tool Browser
9.10.1 **Searchable list of tools exposed by AutoDev** — Type to filter the list of tools the in-process MCP server exposes to fleet workers.
9.10.2 **Collapsible JSON schema viewer** — Click any tool to see its full parameter schema. Useful for debugging and for understanding exactly what Workers can do.

### 9.11 Hooks Settings
9.11.1 **Hooks list with toggle switches** — Every configured hook with an on/off switch.
9.11.2 **Add Hook button** — Opens a form-based hook builder.
9.11.3 **Hook editor** — Action type, conditions, the body of the hook.
9.11.4 **Test button** — Runs the hook with mock context so you can verify it behaves before going live.
9.11.5 **Execution log viewer** — A panel showing recent hook runs with timestamps and outcomes (executed, failed, blocked, skipped).
9.11.6 **Hook log filter** — Filter by outcome category. Errors are highlighted.
9.11.7 **Export log button** — Save hook execution history as JSON for troubleshooting.

### 9.12 Skills Settings
9.12.1 **Skills palette and browser** — See section 11.2 for the full hooks-and-skills coverage.

### 9.13 Default Worker Budget
9.13.1 **Defaults for new tasks** — A panel in Settings lets you set the default token budget, dollar budget, and turn budget that new tasks inherit unless overridden.

### 9.14 Notification Preferences
9.14.1 **Per-event toggles** — Task complete, task failed, worker error, build failed, cost threshold. Each event type has its own on/off toggle.
9.14.2 **Desktop OS notifications** — For critical events, AutoDev raises an OS-level notification (the same kind of toast that other native apps produce).

### 9.15 PA Mode Default
9.15.1 **Global default PA mode** — Set what mode new sessions start in. Per-session override is always available from the Chat tab header.

### 9.16 AIRS Plugin Manager
9.16.1 **Plugin marketplace browser** — Card grid showing every available plugin: name, version, author, description, OS support, dependency requirements.
9.16.2 **Filters and search** — Filter by All / Installed / Built-in plus a tag search.
9.16.3 **Install / Uninstall / Verify** — Per-card actions.
9.16.4 **Plugin details overlay** — Click a card to see full details including a dependency check.
9.16.5 **Five built-in plugins** — Worker Backend, Worker Frontend, Worker DevOps, Worker QA, Worker Security. Each includes the dependencies and configuration appropriate for that worker type.
9.16.6 **Worker Setup Wizard** — A multi-step flow: pick a worker type, configure fleet ID + repos + token lifetime, generate a setup token, get a one-line command you email to a teammate.
9.16.7 **IT Staff Dashboard** — A management view for IT: stat cards (available, installed, not installed, failing), bulk verify / install, per-plugin actions, "Enroll Worker" button.
9.16.8 **Plugin manifest** — Each plugin has a `plugin.yaml` declaring its name, version, type, supported OSes, dependencies, install/verify/uninstall commands, and a config schema.
9.16.9 **Per-OS install scripts** — Auto-generated for each plugin: shell scripts for Linux and macOS, PowerShell for Windows.
9.16.10 **Signed setup tokens** — Tokens are HMAC-SHA256 signed with a time-to-live. They can be generated and validated programmatically.
9.16.11 **One-click worker setup** — A curl one-liner pulls down a script that downloads the CLI, installs the right plugin, runs the dependency installer, and registers the new worker with the fleet. Typical time: five minutes.

### 9.17 Auto-Updater Settings
9.17.1 **Update Settings panel** — Auto-check toggle, channel selector (stable/beta/nightly), manual "Check for Updates" button, update history viewer, data-backup-before-update toggle.
9.17.2 **Per-platform mechanism** — Windows uses Tauri's built-in updater; macOS uses Sparkle (industry-standard); Linux AppImage uses AppImageUpdate; Debian/Fedora packages use the system package manager.
9.17.3 **Background check** — Every four hours AutoDev silently checks whether an update is available.
9.17.4 **Incremental delta updates** — Updates ship as binary patches rather than full installers when possible, dramatically shrinking download size.
9.17.5 **Cryptographic signature verification** — Every update is checked against an Ed25519 signature compiled into the app at build time. Tampered updates are rejected.
9.17.6 **SHA-256 checksum verification** — A second integrity check on top of the signature.
9.17.7 **Atomic install** — Updates are unpacked to a temp location, then swapped into place atomically on the next restart, so a failure mid-install never bricks the app.
9.17.8 **Data persistence guarantees** — All user data (app settings, analytics, sessions, plans, skills, hooks) is verified present after every update. A pre-update restore point is created. Post-update cleanup removes old version files.
9.17.9 **Critical updates** — A security-critical update auto-opens the update dialog and cannot be dismissed.
9.17.10 **Three update channels** — Stable, beta, and nightly. Pick the one matching how bleeding-edge you want to be.
9.17.11 **Skip version** — You can skip a specific version (it won't be offered again).
9.17.12 **Update history viewer** — See past updates and when each was applied.
9.17.13 **Enterprise deployment** — Silent install flags (`/qn` for MSI, `/S` for NSIS), Group Policy / MDM documentation, staged rollouts, version pinning, internal update server support.

### 9.18 MCP Health Panel
9.18.1 **Per-MCP-server status** — Green / yellow / red indicators for every MCP server, with last successful ping timestamp and reconnect attempt count.
9.18.2 **Auto-reconnect with exponential backoff** — Every 30 seconds AutoDev pings each MCP server. After three consecutive failures the server is declared down, then AutoDev tries to reconnect with backoff starting at 5 seconds, doubling, capped at five minutes.

### 9.19 Memory Health Panel
9.19.1 **Live memory subsystem stats** — Vector store entry count, last recall latency in milliseconds, Redis warm-tier status.
9.19.2 **Purge button per worker** — Wipe an individual worker's memories from the vector store. Useful when migrating projects.
9.19.3 **Inspect modal** — Open the last 20 memories stored for any selected worker.

### 9.20 System Prompt Section (Roadmap)
9.20.1 **Variant editor** — Edit the three PA-mode base prompts (Fast Dev, Review, Delegate).

---

## 10. Graph Panel

### 10.1 The Concept
10.1.1 **A live map of your codebase** — AutoDev includes a built-in code knowledge graph. It analyzes every source file in your repo, extracts every function, class, type, and import, figures out which thing depends on which other thing, and produces both a database of facts and an interactive visualization.
10.1.2 **Why this matters for AI agents** — Without a graph, an AI agent answering "what calls this function?" has to grep through every file in the project, burning many thousands of tokens. With a graph, the answer is a single tiny query. The graph is also what powers QC's "smart" reviews — instead of reading every file in a pull request, the QC agent asks "what code is affected by this change?" and focuses only on those areas.
10.1.3 **Why this matters for humans** — A typical codebase has many hundreds of files and thousands of functions. Even an experienced developer struggles to hold the architecture in their head. The graph (especially the 3D city view, §10.27) externalises that mental model into a picture you can fly through.
10.1.4 **Eleven languages out of the box** — Python, JavaScript, TypeScript, Rust, Go, Java, C/C++, Ruby, PHP, Kotlin, Swift, Bash. Plus Jupyter notebooks (per-cell) and Vue single-file components (delegating their script blocks to the TypeScript extractor).
10.1.5 **Two views, one data** — The same graph data drives two completely different visualizations: a 2D force-directed graph that resembles a network diagram, and a 3D "city" where each community of related code becomes a district full of buildings. Both share the same underlying database; agents always read the data layer directly, never the visualization.
10.1.6 **Token-efficient queries** — Instead of "read this 500-line file to find a function," the agent asks "tell me about this function" and gets back the function's metadata, its callers, callees, and tests in roughly 100 tokens.

### 10.2 The Problems This View Replaces
10.2.1 **The "hairball" failure mode** — A naive force-directed graph of 900+ nodes produces a dense central blob of nearly-identical hollow circles that is impossible to read. The current design exists specifically to defeat this — by introducing community-based initial positioning, much stronger node repulsion, collision detection so circles never overlap, and convex hull polygons drawn behind each cluster.
10.2.2 **No visible community structure (the old version)** — Without colored hulls and community-based positioning, the user could not answer "which module does this function belong to?" from the picture alone. The new design fixes this by giving every community a distinct color, a hull polygon, and a gravitational anchor in the layout.
10.2.3 **No size hierarchy (the old version)** — Every node was the same size, so a heavily-used god class was indistinguishable from an `__init__.py` with one connection. The new design scales node radius by degree on a log scale, with a special glow ring on highly-connected "god" nodes.
10.2.4 **Undifferentiated edges (the old version)** — Every relationship type used to look identical, making it impossible to tell a function call from an import. Now each relationship type gets its own color and arrow style, and the noisiest type (structural containment) is hidden by default.
10.2.5 **All labels always on (the old version)** — Hundreds of overlapping text labels made the canvas unreadable. The new design only shows labels when the node is large enough on screen to actually carry text, and god-node labels are always visible regardless of zoom.

### 10.3 Core Design Principles
10.3.1 **Principle 1 — Community first** — The primary organizational unit a person reasons about in a codebase is the module or the cluster of related code, not the individual function. Every view (2D and 3D) makes community structure visible *first*, at every zoom level. You see modules before you see functions.
10.3.2 **Principle 2 — Hierarchy of attention** — Visual salience is matched to structural importance. God nodes (heavily connected) are the biggest and brightest; regular classes and functions are medium; file hubs are smaller and dimmer (they are mechanical, not semantic); isolated nodes are tiny and gray.
10.3.3 **Principle 3 — Edges carry information** — Every edge type encodes a specific relationship: a function call, a module import, an inheritance, a test coverage link. You should be able to see at a glance which is which. The defaults show the meaningful edges and hide the noisy structural ones.
10.3.4 **Principle 4 — Focus beats overview** — A whole codebase is too dense to understand in one look. The most valuable interaction in the graph is the "focus" — click a node, see only its neighborhood, dim everything else. The global view is for orientation; the focused view is for actually working.
10.3.5 **Principle 5 — Dark industrial aesthetic** — The graph looks like a live systems-engineering diagram, not a colorful bubble chart. Black background, monospace labels, muted secondary text, AIRS green for active states.
10.3.6 **Principle 6 — No hairball tolerance** — If the simulation produces a central mass of nodes piled on top of each other, the layout has failed. The defaults are tuned to actively prevent this through cluster-seeded starting positions, strong repulsion, and collision detection.

### 10.4 What the Graph Knows About
10.4.1 **Node types** — Files, classes, functions, types, tests, modules, imports. Each node carries a unique qualified name, the file it lives in, its start and end line numbers, its language, its parent symbol (for nested items), its parameters and return type if applicable, modifiers, a flag indicating whether it is a test, the worker who created or last modified it (if any), the task ID it was touched during, and a worker-lock flag indicating active editing.
10.4.2 **Edge types** — `CALLS` (function A calls function B), `IMPORTS_FROM` (file A imports specifically from file B), `IMPORTS` (file A imports an external package), `INHERITS` (class A inherits from class B), `IMPLEMENTS` (class A implements interface B), `CONTAINS` (file A contains function B), `TESTED_BY` (function A is tested by test B), `DEPENDS_ON` (module-level coupling), `REFERENCES` (a softer mention), `SEMANTICALLY_SIMILAR` (LLM-inferred resemblance).
10.4.3 **Naming convention** — Internally, all edge and node kinds are written in UPPER_SNAKE_CASE (e.g. `IMPORTS_FROM`, not `ImportsFrom`). This makes them safe to use directly as CSS class names, filter keys, and color-lookup keys without case conversion gymnastics.
10.4.4 **Confidence tiers** — Each edge is tagged as one of three: `EXTRACTED` (the source code literally says so, e.g. `import x`), `INFERRED` (a strong heuristic guess), or `AMBIGUOUS` (the relationship could be one of several things). The agent (and the visualization) can prefer high-confidence edges and de-emphasize uncertain ones.
10.4.5 **Communities** — Groups of related symbols. The graph engine runs a community-detection algorithm and assigns each community a numeric ID plus a meaningful name (e.g. "agent-core," "api-health," "memory-system") derived from the dominant directory and the most frequent non-trivial keyword in member node names.
10.4.6 **Cohesion score** — Each community gets a number from 0 to 1 indicating how tightly its members are connected to each other (a measure of "is this really one module or a random pile of code?"). Communities with very low cohesion are good candidates for refactoring.
10.4.7 **Flows** — Top-level entry points (a `main` function, an HTTP request handler, a CLI command) plus their downstream call chains. Each flow has a criticality score and a flag for whether it is test-covered.
10.4.8 **Snapshots** — A point-in-time freeze of the entire graph that can be compared against a later snapshot. This is what powers "show me what changed in the graph during task #142."
10.4.9 **Full-text search index** — A separate, fast text index lets agents find symbols by partial name matches (so the agent can ask "any function with 'health' in the name?" and get an answer in a millisecond).
10.4.10 **Multi-repo registry** — A small registry file lets AutoDev know about several repos at once and merge their graphs into a combined view.

### 10.5 How the Graph Is Built (The Pipeline)
10.5.1 **Stage 1 — Detection** — Walk the workspace folder, respecting `.gitignore` and skipping noisy directories (`node_modules`, `target`, `.git`, build outputs). Collect every code file matching a supported language extension.
10.5.2 **Stage 2 — Parsing** — Each file is parsed with tree-sitter, an industry-standard incremental parser, to produce a structured tree of the file's syntax. Some extractors also do a second regex pass for things tree-sitter cannot easily resolve (like call sites).
10.5.3 **Stage 3 — Per-language extraction** — One extractor per language walks the parse tree and emits nodes (the symbols) and edges (the relationships). Each extractor knows its language's quirks (see §10.6).
10.5.4 **Stage 4 — Content-hash caching** — Each file is hashed; if the hash matches a cached extraction result on disk, the cache is reused and the file is skipped. This makes incremental rebuilds very fast.
10.5.5 **Stage 5 — Parallel parsing** — Files are parsed concurrently on every available CPU core.
10.5.6 **Stage 6 — Build & reconcile** — The collected nodes and edges are deduplicated. Edge endpoints are normalized so a function name that appears with slight variations gets merged into a single canonical reference. Then everything is bulk-inserted into the database.
10.5.7 **Stage 7 — Community detection (four-phase)** — A modularity-aware algorithm finds tightly-connected clusters by (a) extracting strongly-connected components using only the heavy edge types (calls, imports, inheritance), (b) recursively splitting any community that grew larger than 25% of the graph using even stricter weight thresholds, (c) attaching previously-isolated nodes to the best-fit community by weighted vote of their neighbors, and (d) placing external module stubs in the community that imports them the most.
10.5.8 **Stage 8 — Cohesion scoring & naming** — For each community, compute its internal-vs-external edge ratio as a cohesion score, and assign it a human-readable name from its dominant directory prefix and most frequent keywords.
10.5.9 **Stage 9 — Analysis** — Surface god nodes, surprising cross-module connections, suggested investigation questions, and entry points / flows.
10.5.10 **Stage 10 — Report generation** — Render a comprehensive markdown report (`GRAPH_REPORT.md`) summarizing the codebase from the graph's point of view.
10.5.11 **Stage 11 — Export** — Optionally produce a JSON, HTML, or SVG export of the graph for sharing or embedding.
10.5.12 **Cache write safety** — If the cache directory cannot be written (read-only file system, full disk, server environment with no home directory) the failure is logged as a warning and the build continues without caching. No panics, no silent corruption.
10.5.13 **Extractor version stamping** — Each cached extraction result is stamped with an `EXTRACTOR_VERSION` number. Bumping the version invalidates every cached file so the next build re-parses from scratch. Used when an extractor's logic changes meaningfully.

### 10.6 Per-Language Extraction Behavior
10.6.1 **Python** — Extracts classes, functions, methods, type aliases. Resolves `from .module import name` relative imports to actual file paths inside the project so internal imports link to real file nodes (rather than dead-end `module::` stubs). Has a second regex pass that scans function bodies for call-expression patterns and emits `CALLS` edges. External (third-party) imports remain as module stubs with lower confidence.
10.6.2 **JavaScript / TypeScript** — Extracts named functions, arrow functions assigned to variables, classes, and import statements. Resolves relative paths (`./utils`, `../foo`) by probing for the actual file with extensions `.ts`, `.tsx`, `.js`, `.jsx`, `.mjs`, including `index` variants. Detects exported items via the `export` keyword.
10.6.3 **Rust** — Extracts `fn`, `struct`, `enum`, `trait`, `impl` blocks. Recognises `#[test]` attribute as well as the `test_` function-name convention to mark test nodes. Handles grouped use declarations (`use a::b::{c, d}`) by emitting one import edge per leaf.
10.6.4 **Go** — Extracts package-level `func` declarations (including those with method receivers), structs, and interfaces.
10.6.5 **Java** — Extracts classes, interfaces, enums, records. Extracts methods via access-modifier patterns. Walks superclass and super-interface clauses to emit `INHERITS` and `IMPLEMENTS` edges.
10.6.6 **C/C++** — Extracts classes, structs, and function definitions. Recognises `#include "local.h"` as a local include and resolves it to a real file path, while `#include <stdio.h>` is treated as external.
10.6.7 **Ruby** — Extracts classes, modules, instance methods, and class methods. Resolves `require_relative` to real file paths; `require` is treated as external.
10.6.8 **PHP** — Extracts classes, interfaces, traits, enums, and functions. Handles `use` namespace imports and resolves `require`/`include` calls.
10.6.9 **Kotlin** — Extracts classes, interfaces, objects, enum classes, functions (including `suspend`, `inline`, `operator`), and type aliases. Resolves package imports.
10.6.10 **Swift** — Extracts classes, structs, enums, protocols, extensions, actors, and functions. Extracts framework imports.
10.6.11 **Bash** — Extracts function definitions. Resolves `source` and dotted-file (`. ./script.sh`) statements to real script paths.
10.6.12 **Heuristic call detection** — Call-edge detection across all languages uses a regex pass against known function names defined in the same file. Cross-file calls require the import resolution from the same language to have already linked the two files. This is a heuristic, not a full type-aware analysis, but it dramatically beats having zero call data.

### 10.7 Incremental Updates
10.7.1 **File system watcher** — A debounced watcher fires when source files change in the workspace. Multiple rapid changes are batched into a single update.
10.7.2 **Only re-parse changed files** — Files whose content hash hasn't changed are skipped entirely.
10.7.3 **Dependent re-parse (one hop)** — When a file changes, files that import it are also re-analyzed so the cross-file edges stay accurate.
10.7.4 **Performance target** — Save-to-graph-refresh in under two seconds even on substantial codebases (~3,000 files).
10.7.5 **Live `graph:updated` events** — Whenever the graph rebuilds, an event fires and any open Graph tab refreshes automatically — no manual reload.
10.7.6 **Build progress streaming** — Both full and incremental builds emit `graph:build_progress` events with a phase name and percent so the UI can show a stage-by-stage progress bar (Detect → Extract → Build → Cluster → Analyze → Report → Export).
10.7.7 **Error-resilient loading state** — The "building" overlay is always cleared on completion or error so a build failure cannot leave the UI stuck in a permanent loading state.

### 10.8 Multi-Repo Database Isolation
10.8.1 **One database per repository** — Each repository's graph lives in its own database file, named after a hash of the repository's absolute path. So two different projects cannot accidentally share data and produce a polluted view.
10.8.2 **Storage layout** — Graph databases all live together inside a dedicated graphs folder under the AutoDev data directory; the hash filename means there is never a collision and no special characters from the path leak into the filename.
10.8.3 **Repo selection per command** — Every graph operation accepts the repo path so a build, an update, or a visualization read goes to the right database. Switching workspace folders in AutoDev cleanly switches which graph the UI is reading.
10.8.4 **Repo registry** — A small JSON registry lists every repo you've added with its alias, last-built timestamp, and node/edge counts.
10.8.5 **Connection pool** — Up to ten repo databases can be open at once; older ones are closed automatically (least-recently-used eviction).

### 10.9 The 22 Graph Tools (Exposed to Agents)
10.9.1 **Discovery tools (5)** — Minimum context for a task (returns about 100 tokens of "what's relevant"), architecture overview, stats, list communities, fetch the full graph report.
10.9.2 **Navigation tools (9)** — Query callers / callees / imports / tests / dependencies of a symbol, keyword search across symbol names, get all details of a single symbol, find shortest path between two symbols, explain a symbol with its top neighbors, summarize one file, list direct callers, list direct callees, list direct tests.
10.9.3 **Impact tools (5)** — Impact radius, detect changes with risk score, token-efficient review context (capped at 800 tokens), affected flows for a file, flow criticality rankings.
10.9.4 **Analysis tools (4)** — God nodes, surprising cross-module connections, auto-generated investigation questions, snapshot diff.
10.9.5 **AutoDev-native tools (4)** — Worker graph (subset authored by one worker), task graph (subset touched during one task), locked nodes, graph status / trigger rebuild.

### 10.10 MCP Workflow Prompts
10.10.1 **Review changes prompt** — A guided workflow for QC: detect changes → impact radius → review context → list tests.
10.10.2 **Architecture map prompt** — Overview → list communities → god nodes → read key files.
10.10.3 **Debug issue prompt** — Keyword search → query callers → get affected flows.
10.10.4 **Onboard developer prompt** — Read the graph report → list communities → explain each god node.
10.10.5 **Pre-merge check prompt** — Detect changes → impact radius → flow criticality → list tests.

### 10.11 2D View — The Layout Engine
10.11.1 **Force-directed simulation with tuned defaults** — The 2D view uses a force simulation in which nodes repel each other and edges pull connected nodes together. The defaults are tuned far stronger than the library defaults to defeat the "central blob" failure mode.
10.11.2 **Repulsion scales with importance** — Highly-connected nodes push their neighbors away harder than peripheral nodes, so god nodes get more breathing room and the rest of the graph spreads naturally around them.
10.11.3 **Edge-type-aware link distance** — `CONTAINS` edges (a file containing a function) pull tightly so functions stay near their parent file. `CALLS` and `IMPORTS_FROM` get longer distances so unrelated communities don't fuse together.
10.11.4 **Cluster gravity per community** — A custom "cluster force" gently pulls each node toward the centroid of its community, which keeps the community visually contiguous even after dragging.
10.11.5 **Collision detection** — Every node has a circular collider so circles never visually overlap; padding ensures even tightly-clustered nodes remain individually visible.
10.11.6 **Slow cooling** — The simulation "cools down" more slowly than the library default, which gives the layout extra time to settle into a clean arrangement rather than freezing too early.
10.11.7 **Cluster-seeded initial positions** — Before the simulation starts, every node is given an initial position near its community's centroid, with all community centroids arranged evenly around a circle. So even on the first frame the user already sees rough community separation.
10.11.8 **Centering force** — A weak pull toward the canvas center keeps drifting communities from floating off-screen entirely.

### 10.12 2D View — Render Modes
10.12.1 **Auto-mode (default)** — AutoDev picks the right rendering mode based on graph size. Under 300 nodes uses Full Mode. From 300 to 1,500 nodes uses File Mode. From 1,501 to 5,000 nodes uses Community Mode. Above 5,000 nodes also uses Community Mode with a warning explaining that a full view would be unreadable.
10.12.2 **Full Mode** — Every individual node and edge rendered. Each function, class, type appears as its own shape. Used for small repos where the full graph fits on a screen.
10.12.3 **File Mode** — Functions, classes, and types are collapsed into their parent file node. Each file becomes a larger circle scaled by how many symbols it contains, with a label showing the filename and a count summary inside. Clicking a file node drills into Full Mode filtered to just that file.
10.12.4 **Community Mode** — All symbols are collapsed into their community supernode. Each community is one large circle sized by member count and colored by its community color. Lines between communities are sized by how many cross-community edges they aggregate. Double-clicking a community drills into Full Mode for that community.
10.12.5 **Focus Mode** — Activated by clicking any node in any other mode. The selected node and its immediate neighbors are at full opacity; everything else dims to almost-invisible. This is the primary *working* view because it isolates "the code I'm thinking about right now."
10.12.6 **Impact Mode** — Activated from a node inspector's "Blast Radius" button or via the impact MCP tool. Buildings color-code by hop distance: epicenter is white-pulsing, 1-2 hops away is red, 3-4 hops is orange, 5+ hops is yellow-green. Test nodes that will run turn green; affected nodes with no test coverage turn red with an X overlay. Everything not affected fades to near-invisible.
10.12.7 **Manual override** — Users can override auto-mode at any time via the render-mode selector. The override sticks until they switch repositories.
10.12.8 **Progressive rendering for large graphs** — For graphs larger than a few hundred nodes, the canvas paints in stages: first the community hulls (so the user sees structure immediately), then god nodes and high-degree nodes, then low-degree nodes, then edges. This means a useful partial picture is on screen within 100ms even for very large graphs.

### 10.13 2D View — Node Visual Encoding
10.13.1 **Radius by degree (log-scaled)** — Bigger circles have more connections. The size formula uses a logarithm so the difference is visible across many orders of magnitude (degree 1 vs degree 50) without making god nodes consume the entire canvas. Capped at 22 pixels.
10.13.2 **Distinct shapes by kind** — Class is a square (rotated to a diamond visually), function is a circle, test is an upward triangle (pointing at what it covers), type is a narrow diamond, file is a square, module is a hexagon, import is a cross. Shapes are picked from D3's symbol library and sized by area so two shapes at the same area appear visually balanced.
10.13.3 **Color by community** — All nodes in the same community share a color drawn from the Tableau 10 categorical palette (designed specifically for clear color discrimination against dark backgrounds).
10.13.4 **Outline glow on god nodes** — Nodes with degree ≥ 15 get an additional outer ring at 25% opacity in their community color, so the "important" nodes are visually distinguishable even from a distance.
10.13.5 **Stroke modifications by state** — Default stroke is the node's community color at 50% opacity, half a pixel wide. Hover brightens the stroke to white at full opacity. Selection turns it white at two pixels with the outer ring. Worker-locked nodes get a striped pattern overlay and a warning-amber stroke.
10.13.6 **Opacity by confidence** — Nodes inferred from heuristics rather than directly extracted from the source are rendered slightly translucent so they read as "less certain."
10.13.7 **Isolated nodes** — Nodes with zero edges are drawn small and in muted gray, visible but de-emphasized so they don't compete with the structurally important nodes.

### 10.14 2D View — Edge Visual Encoding
10.14.1 **Color by relationship type** — `CALLS` is AIRS green (the most important relationship gets the brand color), `IMPORTS_FROM` is info blue, `INHERITS` is purple, `IMPLEMENTS` is teal, `TESTED_BY` is orange (test coverage is structurally important to QC), `DEPENDS_ON` is yellow, `REFERENCES` is gray, `CONTAINS` is near-invisible structural noise, `SEMANTICALLY_SIMILAR` is brown.
10.14.2 **Opacity by confidence** — Extracted edges are crisp (around 55% opacity), inferred edges are noticeably fainter (around 30%), ambiguous edges are barely visible (around 15%). When a node is selected, edges not connected to it dim further to almost invisible.
10.14.3 **Width by weight** — Thicker edges represent stronger or more frequent connections.
10.14.4 **Directed arrows** — Most edge types have arrowheads to show direction. `CONTAINS` has no arrow (containment isn't directional in a meaningful semantic sense); `SEMANTICALLY_SIMILAR` uses a double-headed marker because similarity is mutual.
10.14.5 **Default visibility table** — `CALLS`, `IMPORTS_FROM`, `INHERITS`, `IMPLEMENTS`, `TESTED_BY`, `DEPENDS_ON` are on by default. `REFERENCES`, `CONTAINS`, `SEMANTICALLY_SIMILAR` are off by default (they are either noisy or too speculative for a default view). All toggles are one-click in the toolbar.

### 10.15 2D View — Community Hull System
10.15.1 **What a community hull is** — A semi-transparent colored polygon drawn behind each community, marking the area its member nodes occupy. The single most impactful improvement over a raw force-directed graph: cluster structure becomes visible at a glance.
10.15.2 **Convex hull plus padding** — For each community with at least three members, the convex hull (smallest polygon containing all member positions) is computed, then expanded outward by about 28 pixels from the centroid. The result is a smooth padded boundary rather than tight against the outermost node.
10.15.3 **Smooth curves, not corners** — The hull is drawn as a closed Catmull-Rom spline (a smooth curve through the hull points) so the boundary reads as an organic blob rather than a sharp polygon.
10.15.4 **Subtle styling** — The fill is the community color at only 6% opacity so hulls do not overwhelm the nodes. The border is the community color at 35% opacity, 1.5 pixels wide, dashed.
10.15.5 **Hull layer below everything** — In the rendering stack, hulls go below edges and nodes so they read as background context.
10.15.6 **Throttled updates during simulation** — Recomputing every hull from current node positions every animation frame is expensive at 900 nodes. So hulls update only every third tick during the "hot" phase of the simulation and every tick once the simulation cools down.
10.15.7 **Overlap detection** — When two community hulls overlap by more than about 15% of the smaller hull's area, a small warning icon appears at the overlap centroid. Hovering it explains that the two communities are tightly coupled and may need a dependency review. This directly surfaces what the academic literature calls a "big ball of mud" anti-pattern.

### 10.16 2D View — Labels
10.16.1 **Zoom-dependent visibility** — A label only renders when its node, at the current zoom level, occupies at least about 8 pixels of screen radius. Tiny nodes at low zoom show no label. Zoom in and the labels appear progressively.
10.16.2 **God nodes always labeled** — Highly-connected nodes are exempt from the zoom rule. They always show their label so the user can always identify the architectural landmarks.
10.16.3 **Short labels by default** — The label is just the function or class name. The qualified name (including the file path) goes in the inspector panel, not on the canvas.
10.16.4 **Two-line labels at high zoom** — At a zoom scale above about 2.5x, labels become two lines showing both the name and the file basename, since there is now room for it.
10.16.5 **Collision-aware pruning** — At medium zooms the visible labels are checked for overlap; if two labels would collide, the one belonging to the lower-degree node is hidden so the more important label wins.
10.16.6 **Labels never intercept mouse events** — Labels are rendered with `pointer-events: none` so clicking a label is the same as clicking the node behind it.

### 10.17 2D View — Interaction Model
10.17.1 **Zoom and pan with constraints** — Standard mouse-wheel zoom (between 5% and 1200% magnification) and click-and-drag panning. A "fit to canvas" calculation pans and zooms so all nodes are visible on first render.
10.17.2 **Keyboard shortcuts when the canvas has focus** — `+` and `=` zoom in, `-` zooms out, `0` resets to fit, `F` fits to the current selection.
10.17.3 **Node hover** — Cursor turns into a pointer, the node's border brightens to white, the first-hop neighborhood subtly brightens, and after a short delay (~200ms) a tooltip appears under the cursor showing the node name, kind, language, community, file path, line range, and degree.
10.17.4 **Single click → Focus Mode** — Selected node and its direct neighbors stay at full opacity; everything else dims to 8% opacity. Connected edges glow; unconnected edges fade. The right panel switches to the node inspector. Clicking the empty canvas exits focus.
10.17.5 **Double click → Zoom to node** — Pans and zooms the canvas to center on the clicked node at 3x magnification, framing it and its direct neighbors clearly.
10.17.6 **Right-click context menu** — Opens a menu with: open in IDE tab, focus on this node, expand to two-hop neighborhood, show blast radius, copy qualified name, copy file path, find path to another node. "Find path" opens a small search input where you type a second node name and the canvas highlights the shortest path between them.
10.17.7 **Drag to reposition** — Grab any node with the mouse to move it. Releasing pins it in its new spot temporarily; the simulation warms back up briefly to settle the surrounding nodes.
10.17.8 **Canvas background click cancels modes** — Clicking the empty canvas exits Focus Mode, clears any path visualization, and exits Impact Mode.

### 10.18 2D View — Toolbar and Filters
10.18.1 **Repo selector** — Dropdown listing every repository registered in the multi-repo registry. Selecting a different repo loads that repo's graph database.
10.18.2 **Build Graph button** — Triggers a full rebuild (the full seven-stage pipeline). During the build the button shows a stage-by-stage spinner with the current percent and the canvas shows a build overlay.
10.18.3 **Update button** — Triggers an incremental update (just files that changed since the last build). Much faster than a full rebuild for everyday use.
10.18.4 **Export dropdown** — Pick from `graph.json`, `graph.html` (a standalone browser-openable visualization), `graph.svg`, or `GRAPH_REPORT.md`.
10.18.5 **Reset view button** — Re-fits the canvas to show all nodes.
10.18.6 **Delete button** — Wipes this repo's graph database. Confirmation prompt.
10.18.7 **Render mode selector** — Pick from Auto, Full, File, Community, Focus.
10.18.8 **Edge type toggles** — Inline checkboxes per edge kind. Click to show or hide each relationship type. Defaults already configured to show only meaningful edges; `CONTAINS` is off by default.
10.18.9 **Search input** — Live full-text search across every node in the graph (powered by the FTS index built during graph construction). Results appear as a dropdown; selecting one focuses the canvas on that node.
10.18.10 **Confidence filter** — Dropdown: show All / Extracted only / Extracted + Inferred / Ambiguous only. Switching to "Ambiguous only" surfaces the uncertain edges that need human review.
10.18.11 **Minimum-degree filter** — Numeric input. Hide all nodes whose degree is below this value. Setting it to 3 removes isolated and periphery nodes; setting it to 5 shows only the structurally important nodes. Powerful noise reduction for big repos.

### 10.19 2D View — Stats Bar
10.19.1 **A live one-line summary above the canvas** — Shows current node count, edge count, community count, average degree, density (a percentage), and the time of the last build (as a relative time like "14 minutes ago").
10.19.2 **Recent-build highlight** — The node count turns AIRS green when the graph was built or updated less than five minutes ago, signalling "this view is fresh."
10.19.3 **Filtered-state indicator** — When any filter is active (hiding nodes by degree, by edge kind, by community), the stats bar appends a "showing N of M nodes" message in warning amber so the user remembers a filter is on.

### 10.20 2D View — Node Inspector Panel
10.20.1 **Right-side panel, 300 pixels wide** — Always present, but only shows full details when a node is selected. Renders the selected node's metadata in a compact dark-theme layout.
10.20.2 **Node name and badges** — The qualified name in monospace, with badges underneath for kind (color-coded), language, and community (showing both the numeric ID and the human-readable name like "agent-core").
10.20.3 **File and line info** — Clickable file path that opens the IDE tab at the right line, plus the line range and degree count.
10.20.4 **Confidence badge** — Color-coded badge: green for extracted, amber for inferred, red for ambiguous.
10.20.5 **Agent and task attribution** — If the node was authored or last touched by a worker during a specific task, the worker slug and task ID are shown.
10.20.6 **Incoming section** — Lists incoming edges (callers, imports, tests). Shows the top 3 with a "Show all" toggle for the rest. Each row shows the source node name in its community color, the edge kind in the edge color, and the line number.
10.20.7 **Outgoing section** — Same structure as incoming but for edges going out from this node.
10.20.8 **Action buttons** — "Open in IDE" jumps to the file in the IDE tab. "Blast Radius" switches the canvas into Impact Mode for this node. "Find Path" opens an inline search to pick a second node and draw a path to it.

### 10.21 2D View — Communities Panel
10.21.1 **Below the inspector** — A list of every community with a color swatch, the community name, the number of nodes in it, and a cohesion score.
10.21.2 **Cohesion color coding** — Green for above 0.5 (tight community), amber for 0.25 to 0.5 (moderate), red for below 0.25 (loose / suspicious).
10.21.3 **Warning icon for very low cohesion** — Communities below 0.20 get a warning icon. Hovering it suggests the community may need to be split.
10.21.4 **Click to focus** — Clicking a community row zooms the canvas to its hull, dimming everything else. Useful for "show me the auth module."
10.21.5 **Hover to highlight** — Hovering a community row makes its hull glow on the canvas so you can spatially locate it before committing to a click.
10.21.6 **Show/hide toggle per community** — A checkbox lets you hide every node in one community from the canvas — useful when one mega-community is dominating the view.

### 10.22 2D View — Loading and Build States
10.22.1 **Empty state** — When no graph has been built for the current repo, the canvas shows a central illustration, a one-line description, and a "Build Graph" call-to-action button. Tells you the first build can take 30 to 90 seconds for large repos.
10.22.2 **Build-in-progress overlay** — During a full build, an overlay covers the canvas with a stage-by-stage progress display (Detect → Extract → Build → Cluster → Analyze → Report → Export), each stage showing as pending, running with a spinner, or done with a checkmark, plus a percentage and a cancel button.
10.22.3 **Incremental update strip (non-blocking)** — For incremental updates, the canvas stays usable; a thin status strip at the bottom shows "Updating graph — 3 files changed · 00:01 elapsed" and fades out after about four seconds once complete.
10.22.4 **Aggregation banner** — When the graph is so large that AutoDev auto-switches to community-aggregate rendering, a banner appears explaining "showing community aggregates instead of full nodes because the graph exceeds the render limit."

### 10.23 2D View — Floating Controls
10.23.1 **Zoom controls bottom-left** — Three stacked buttons: zoom in, zoom out, fit-to-canvas. Always visible.
10.23.2 **Edge legend bottom-right** — Compact toggleable legend listing each visible edge type with its color swatch. Clicking an item toggles that edge type's visibility — a redundant entry point to the toolbar's edge filter, optimised for the keyboard-free user.

### 10.24 The Graph Report
10.24.1 **Auto-generated markdown report** — At the end of every build, AutoDev generates a comprehensive markdown report summarizing the codebase from the graph's point of view.
10.24.2 **Sections of the report** — Statistics (node count, edge count, language distribution, average degree), god nodes (highly-connected architectural landmarks), community structure (each community's name, size, cohesion, top members), surprising connections (cross-module edges that defy expectations, with a "why" explanation per connection), suggested questions (auto-generated investigation prompts), entry points (where the codebase begins executing).
10.24.3 **Collapsible panel in the Graph tab** — The report renders inline as a collapsible accordion in the right panel.
10.24.4 **Regenerate button** — Forces a full rebuild and refreshes the report.

### 10.25 Export Formats
10.25.1 **JSON export** — A clean interchange format. Used both internally (for merging graphs across repos) and externally (for feeding the data to other tools).
10.25.2 **HTML export** — A single self-contained HTML file with an embedded D3 force-directed visualization. Dark theme matching AutoDev. Includes a search box, a node info panel, and a community legend. Can be opened in any browser without AutoDev installed. All user-provided strings are HTML-escaped to prevent script injection from malicious file or symbol names.
10.25.3 **SVG export** — A static vector image with a circular-per-community layout. Good for embedding in design documents or wiki pages.
10.25.4 **Markdown report export** — The graph report file itself, suitable for committing to the repository as documentation.

### 10.26 Multi-Repo Support
10.26.1 **Why multi-repo** — Real teams operate across several repositories. Often the most important architectural insights live at the seams *between* repos (e.g. which functions in repo A are called from repo B?).
10.26.2 **Repo registry file** — A small JSON file lists every repo you've registered with the graph engine, plus an alias, the database path, last update timestamp, and node/edge counts.
10.26.3 **Connection pool with cache eviction** — Up to ten repo databases can be open simultaneously; older ones are closed automatically as you switch between repos.
10.26.4 **Graph merging across repos** — A merge operation combines several repos' graphs into a single combined JSON. Each node is tagged with which repo it came from; symbols with the same qualified name are deduplicated.
10.26.5 **Multi-repo section in the Graph tab** — Shows registered repos with their stats. Buttons for "register repo" (opens a folder picker), "unregister," and "merge all graphs" (which opens the merged view as an HTML file in your browser).
10.26.6 **Cross-repo surprising connections** — When repos are merged, edges that cross repo boundaries are flagged as especially interesting in the graph report — they often indicate hidden dependencies between codebases that should be more explicit.

### 10.27 PA Graph Context Injection
10.27.1 **Automatic context for code questions** — When you ask the PA something code-related ("how does authentication work?"), AutoDev silently scores graph nodes against your question, finds the most relevant ones, traces two hops outward through the graph, and adds a few hundred tokens of "graph context" to the system prompt before the PA answers.
10.27.2 **Token-bounded** — The injected context is capped at 800 tokens so it never overwhelms the model's context window.
10.27.3 **Graceful absence** — If the current repo has no graph built yet, the context block is simply omitted. Nothing breaks; the PA just answers without graph context.
10.27.4 **Mode-aware** — Injection happens in Fast Development and Review modes. Delegate mode skips it because in Delegate mode the PA is creating tasks, not directly investigating code.

### 10.28 QC Graph Workflow
10.28.1 **The standard QC pattern** — The QC review recipe instructs the QC agent to use the graph rather than reading entire files. The standard sequence: detect changes, get the impact radius for high-risk nodes, list tests for those nodes, fetch the token-efficient review context. Only read raw files for nodes flagged as high-risk with no test coverage.
10.28.2 **Why this matters** — Without the graph, a typical PR review might consume 5,000+ tokens of file reads. With the graph, the same review consumes under 500 tokens of structured context. The QC pass rate stays high while the cost drops by an order of magnitude.

### 10.29 Worker Graph Integration
10.29.1 **Author and task stamping on every change** — Every node a worker creates or modifies is automatically stamped with the worker's slug and the task ID. This makes it possible to ask the graph "what did Worker Backend create yesterday?"
10.29.2 **Pre-task and post-task snapshots** — When a worker claims a task, the graph is snapshotted. When the task finishes, it is snapshotted again. The diff between the two snapshots is stored on the task record so the task card can show "this task added 1 new function and 3 new edges."
10.29.3 **Auto-update on file writes** — Whenever a worker writes or edits a file, the graph updates incrementally so the visualization always reflects current state. No batch rebuild needed.
10.29.4 **Live refresh event** — A `graph:updated` event fires whenever the graph changes, so any open Graph tab refreshes in real time.
10.29.5 **Worker locks visualized** — Nodes currently being modified by an active worker are marked with a worker-lock flag, which the visualization shows as a striped overlay and amber stroke. This makes it visually obvious where active edits are happening.

---

### 10.30 The 3D City View — Philosophy
10.30.1 **What the city view is** — A second, completely different visualization of the same graph data. Instead of nodes and edges, the city renders the codebase as an actual three-dimensional city you can fly around: each cluster of related code is a *district*, each class is an *office tower*, each function is a *low-rise building*, each function call is a *glowing tube* arcing through the air over the rooftops.
10.30.2 **Why a city** — Seventeen years of academic research (starting with the CodeCity work in 2008) found that humans navigate physical cities every day. We know intuitively that a taller building is more important, that a busy road carries more traffic, that an abandoned building is suspicious. By mapping code structure onto a city, the city's visual intuitions become free cognitive aids for understanding the code.
10.30.3 **A human comprehension tool, not a data store** — The 3D city is *only* for humans. Agents never see the city's coordinates, building heights, road geometry, district layouts, camera positions, or particle states. They keep querying the plain graph data through the same MCP tools as before. The city is a rendering skin over the same data.
10.30.4 **Spatial memory** — The layout is deterministic — given the same graph data, the city is laid out identically every session. So once you learn that "the agent district is in the upper-left," it stays there. Over time you build genuine spatial memory of your codebase.
10.30.5 **What the city answers at a glance** — "What is the biggest module?" → the tallest district. "Which function is most central?" → the glowing landmark tower. "What modules are tightly coupled?" → districts with thick boulevards of traffic between them. "Does this function have tests?" → green-lit windows. "What breaks if I change this?" → the blast-radius emergency overlay. "Where are the untested areas?" → dark unlit buildings on the edge of town at night.

### 10.31 The 3D City View — Code-to-City Mapping
10.31.1 **Community → city district** — Each community is a colored ground plate, raised slightly, with a low boundary wall and a billboard sign at the center showing the community's name (e.g. "AGENT DISTRICT," "API QUARTER," "MEMORY SECTOR," "TEST PRECINCT").
10.31.2 **Module / package → city block** — A raised concrete platform inside the district holds a cluster of related files.
10.31.3 **File → city block subdivision** — A darker ground tile within the block, holding the file's functions and classes as buildings.
10.31.4 **Class → office tower / skyscraper** — A tall box building. Classes are complex multi-method structures, mapped to multi-floor buildings.
10.31.5 **Function → low-rise building** — A shorter box building. Functions are simpler single-purpose structures.
10.31.6 **Test function → fire station** — An orange-accented building with an antenna. Tests are support infrastructure that protects other buildings.
10.31.7 **Type / interface → archway** — A flat arch structure. Types are definitional rather than operational.
10.31.8 **External import → airport or port** — A structure at the district boundary, signalling "this comes from outside the city."
10.31.9 **God node → central landmark tower** — The tallest building in the district, with a glowing beacon and a point-light at the top, casting light over the surrounding neighborhood. Architectural landmarks are visually unmistakable.
10.31.10 **Worker-locked node → building under construction** — Scaffolding overlay, yellow warning lights. Active edits show as construction zones.
10.31.11 **Isolated node → abandoned building** — Dark, unlit, faded texture. Disconnected code looks visually neglected.
10.31.12 **Function call edge → underground metro line** — A glowing tube arcing through the air over the rooftops (visible when relevant). Function calls are the rapid direct connections, like subway lines.
10.31.13 **Module import → main boulevard** — A wide flat road at street level connecting district entry points. Module imports are the primary supply route.
10.31.14 **Inheritance → elevated bridge** — An arched concrete bridge between parent and child class buildings. Inheritance is an elevated structural relationship.
10.31.15 **Tested-by edge → service road to a fire station** — A narrow gravel road connecting a function to its test. Tests protect specific buildings.
10.31.16 **Depends-on edge → supply route** — A medium-width paved road. Dependencies flow like supply trucks.
10.31.17 **Semantically-similar edge → pedestrian walkway** — A dotted cobblestone path. Informal connections, not hard infrastructure.
10.31.18 **Contains edge → no road** — Containment is expressed by the building's *position* within its block, not by drawing a line.
10.31.19 **Confidence tier → road surface** — Extracted edges become clean asphalt with painted lane markings. Inferred edges become gray cobblestone. Ambiguous edges become brown dirt paths with dashed edges.

### 10.32 The 3D City View — Building Generation
10.32.1 **Height encodes complexity** — Building height is computed from lines of code plus connection count. A 500-line god class becomes a skyscraper; a 5-line helper function becomes a single-story stub. The math is bounded so the tallest possible building is about 35 units high; the shortest is a half-unit pad.
10.32.2 **Width encodes incoming connections** — A building's width grows with how many things import it. A wide building means "many people depend on this — broad public API surface."
10.32.3 **Depth encodes outgoing connections** — A building's depth grows with how many things it calls or imports. A deep building means "this thing reaches out to a lot of other things."
10.32.4 **A building that is both wide and deep is a god node** — Visually unmistakable: a hulking cube of a building that everyone uses and that uses everyone else.
10.32.5 **Materials by kind** — Classes get a deep blue with slight metallic finish; functions get a darker matte material; tests get an amber-warm material; types get purple; files get neutral dark; modules get green.
10.32.6 **Procedural window lighting** — Each building has procedurally-placed window patterns generated from a hash of the node ID, so the same building always has the same window pattern across sessions. The window emission color tells you about test coverage.
10.32.7 **Window color = test coverage** — Fully-tested buildings have green-lit windows. Partially-tested buildings have amber windows. Untested buildings are dark. Worker-locked buildings get yellow construction lights. Isolated buildings get a faint eerie red glow. God nodes get a brilliant white beacon at the rooftop with a true point-light source.
10.32.8 **God node special treatment** — God nodes don't use the standard box. They use an octagonal tapered tower (truncated pyramid) with an animated bloom sphere at the top. The bloom emits actual light into the scene so the landmark is visible from far away.
10.32.9 **Instanced rendering for performance** — All Function buildings in the whole city are rendered as a single batched draw call; same for Class buildings, Test buildings, Type buildings. The GPU treats them as instances of one mesh with per-instance position, scale, and color. The result: thousands of buildings at a near-zero rendering cost.

### 10.33 The 3D City View — Layout Algorithm
10.33.1 **Squarified treemap** — The city is laid out with a "squarified treemap" — an algorithm that packs rectangles of varying sizes into a larger rectangle while keeping each rectangle as close to square as possible. This is the same algorithm used by disk-space visualizers and Voronoi maps.
10.33.2 **Three nested levels** — Level 0 is the entire city footprint. Level 1 packs communities into districts. Level 2 packs files within each district. Level 3 (at high zoom only) packs functions within each file.
10.33.3 **Padding becomes road space** — The algorithm leaves visible gaps between districts and between blocks. Those gaps become the road network — the road generator traces paths through them.
10.33.4 **Deterministic by sort order** — The treemap is seeded by community ID order, so identical graph data produces an identical city every session. This is what enables spatial memory.
10.33.5 **Web Worker offloading** — The full layout computation (treemap, building dimensions, road geometry, particle start positions) happens on a background thread, not the main UI thread, so opening the city view doesn't freeze the app.

### 10.34 The 3D City View — Roads
10.34.1 **Function-call tubes (elevated, glowing)** — Function calls render as quadratic-bezier curves arching above the rooftops, glowing in AIRS green. They appear when a building is selected or in Impact Mode, hidden by default at city-overview scale (there are too many to all show at once).
10.34.2 **Import boulevards (ground-level, aggregated)** — Imports between two districts are aggregated into a single multi-lane boulevard whose width is proportional to the number of imports. Instead of drawing every individual import edge as a separate line, the city shows the macro pattern: "this district sends a lot of imports to that district." Far more readable.
10.34.3 **Inheritance bridges (elevated, architectural)** — Class-inheritance edges render as literal arched bridges spanning from the parent class building to the child class building. The bridge is rendered as an extruded arch shape.
10.34.4 **Test service roads (narrow, gravel)** — A test-to-function edge becomes a narrow service road connecting the test (a fire station) to the function it covers.
10.34.5 **Dependency supply routes (medium asphalt)** — `DEPENDS_ON` edges become medium-width paved roads.
10.34.6 **Pedestrian similarity paths (dashed cobble)** — `SEMANTICALLY_SIMILAR` edges become dashed cobblestone walkways — informal, not infrastructure.
10.34.7 **Road surface = confidence tier** — Extracted edges are clean asphalt, inferred are cobblestone, ambiguous are dirt.

### 10.35 The 3D City View — Lighting and Atmosphere
10.35.1 **A staged scene, not a flat render** — The city has proper lighting: an ambient fill so even unlit buildings aren't pitch black; a directional "sun" angled across the city casting real shadows; a hemisphere light differentiating sky color from ground color; atmospheric fog so the city fades into darkness in the distance.
10.35.2 **Subtle grid on the ground plane** — The base ground has a faint city-planning grid pattern in dark gray every five units, evoking a city plan.
10.35.3 **District ground tinting** — Each district's ground is the community color at very low opacity (about 8%), so the district reads as a colored borough without overwhelming the buildings.
10.35.4 **District boundary walls** — A low (one-tenth of a unit) wall in the community color at higher opacity demarcates district edges from outside.
10.35.5 **District name signs** — A billboard text element at the center of each district displays the community's human-readable name in the community color. Billboards always face the camera regardless of viewpoint.
10.35.6 **Bloom post-processing** — God-node beacons and active call tubes emit light that visibly *blooms* in the rendered image. From far away the city looks like a real night skyline with glowing landmarks.
10.35.7 **Ambient occlusion** — Soft shadows accumulate in the corners where buildings meet streets. This adds real depth and prevents the city from looking flat.
10.35.8 **Vignette** — A subtle darkening of the edges of the frame gives the rendering a cinematic quality and focuses attention on the center.

### 10.36 The 3D City View — Camera & Navigation
10.36.1 **Google Maps-style controls** — Drag to pan across the city. Scroll to zoom in and out. Right-click and drag to orbit the camera around the focus point. The feel is intentionally close to Google Maps 3D so users are productive immediately.
10.36.2 **Zoom limits** — Minimum distance is about three units (street level inside a district); maximum is about 220 units (the whole city in view). Polar angle is clamped so the camera cannot go fully horizontal or past vertical.
10.36.3 **Damping for smoothness** — Camera movement is dampened so it feels gliding rather than twitchy.
10.36.4 **Camera preset keyboard shortcuts** — `0` resets to fit the whole city. `G` drops to ground-level perspective for a street-eye view. `T` snaps to a top-down overhead view. `F` zooms to the selected building's district. `B` zooms to a specific selected building at street level. Arrow keys pan across the city at the current altitude.
10.36.5 **Fly-to animation on building click** — Clicking a building triggers a smooth camera flight — the camera animates over about 1.2 seconds to a close-up position offset above and beside the building, with the camera target lerping to the building center. Uses a cubic easing curve so the motion feels natural.
10.36.6 **Minimap overlay** — A small 2D top-down map of the entire city sits in the top-left corner (about 120 by 120 pixels). A white rectangle on the minimap represents the current camera's view footprint. Click anywhere on the minimap to teleport the camera there.

### 10.37 The 3D City View — Level-of-Detail
10.37.1 **Why LOD matters** — At any moment, most of the city is far from the camera. Drawing thousands of detailed buildings at full quality wastes the GPU. So buildings render at four progressively-simpler levels depending on distance.
10.37.2 **LOD 0 (full)** — Camera within 8 units. Full geometry, window-shader active, building label visible, animated traffic particles on adjacent roads, service roads visible.
10.37.3 **LOD 1 (high)** — 8 to 25 units. Full geometry but no window shader (just solid color), building label still visible, roads visible but particles hidden.
10.37.4 **LOD 2 (medium)** — 25 to 60 units. Simplified geometry (fewer polygons, no bevels), no label, only district-level roads visible.
10.37.5 **LOD 3 (low)** — Beyond 60 units. Just a colored box drawn via instancing, no label, no roads at all.
10.37.6 **Automatic transitions** — As the camera moves, each building flips through LOD levels seamlessly. The user doesn't see the transitions because each level is designed to be visually consistent with the next.
10.37.7 **Frustum culling** — Buildings entirely outside the camera's view are not submitted to the GPU at all. Standard 3D rendering optimization, but worth noting because it's how a city of thousands of buildings runs at 60 fps on a midrange laptop.

### 10.38 The 3D City View — Interactions
10.38.1 **Click a building** — Camera flies to the building (§10.36.5), a white outline highlights the selection, adjacent buildings (the ones connected by edges) pulse subtly, the right panel switches to the node inspector with the same data as the 2D view, adjacent roads glow brighter, and unrelated buildings dim to about 30% opacity.
10.38.2 **Click a district's ground plane** — Camera pulls back to frame the entire district, the boundary wall brightens, the right panel switches to a community overview, every building in the district lights up while others dim.
10.38.3 **Hover a road** — A tooltip appears showing the edge kind (calls, imports-from, etc.), the source and target node names, the confidence tier, and for aggregated boulevards the total edge count rolled into that road.
10.38.4 **Right-click a building** — A context menu opens with: "Open in IDE Tab," "Focus in 2D Graph View" (toggles to the other view focused on this node), "Show Blast Radius," "Show Incoming Roads Only," "Show Outgoing Roads Only," "Find Path To...," "Copy qualified name," "Copy file path."
10.38.5 **Hover a district** — A tooltip with the community name, the node count, the dominant language, the cohesion score with color indicator, and the top three god nodes inside.
10.38.6 **Cursor change** — Hovering any interactive element (building, district, road) changes the cursor to a pointer so it's obvious what is clickable.

### 10.39 The 3D City View — Impact Mode (Emergency Overlay)
10.39.1 **What Impact Mode does** — When you select a building and ask "show me what breaks if I change this," the city transforms into an emergency-lockdown visualization. Disaster-movie language: the changed building strobes, affected buildings light up in graduated red/orange/yellow risk colors, unaffected buildings fade to almost invisible "ghost" silhouettes.
10.39.2 **Epicenter (changed building)** — White-pulsing strobe animation, a red-orange emergency light cone projecting up from the rooftop, a "CHANGING" billboard label.
10.39.3 **High risk (one or two hops away)** — Solid red emissive fill (in the AutoDev danger color), flickering intensity simulating power instability, a warning-triangle billboard at rooftop level.
10.39.4 **Medium risk (three or four hops)** — Solid orange emissive, steady illumination.
10.39.5 **Low risk (five or more hops)** — Dim yellow-green, not urgent.
10.39.6 **Test buildings that will run** — Green pulse plus a "TESTING" billboard. These are the tests that should be re-run after the change.
10.39.7 **Affected nodes with no test coverage** — Red fill with a giant X overlay texture and a "NO TESTS" billboard. The most important diagnostic — these are the gaps in coverage that put the change at risk.
10.39.8 **All unaffected buildings** — Dimmed to about 5% opacity. They become silhouettes in the background so the affected ones stand out.
10.39.9 **Impact paths as particle streams** — Glowing red particle streams trace the dependency paths from the epicenter through intermediate buildings to each affected target. Particle speed is faster than normal traffic; color gradients from red to orange based on hop count.
10.39.10 **The Impact Side Panel** — The right panel switches to an "emergency dispatch" style readout: counts by risk level, the list of tests that should run, the list of nodes with no test coverage, and action buttons to "Run Affected Tests" (triggers the test runner with the right filter) or "Create Test Tasks" (creates Delegate-mode tasks for each uncovered node).

### 10.40 The 3D City View — Traffic Particles
10.40.1 **A living city, not a static map** — Particles travel along roads showing the direction and volume of data and control flow. This is the most "alive" element of the city — it transforms a static diagram into a system you can watch breathing.
10.40.2 **GPU-shader particles, not individual objects** — Particles are rendered as a single big point cloud with a custom shader that advances each particle along a path using a time uniform. Thousands of particles run at near-zero GPU cost.
10.40.3 **Default visibility rules** — Function-call particles appear only on a selected/focused building (too many to show always). Module-import particles run by default on district boulevards. Inheritance particles are off (too static to be informative). Test-coverage particles run by default. Dependency particles run by default.
10.40.4 **Particle speed by edge weight** — Heavier (more important) edges get faster particles. The user immediately sees that the auth-to-database road is heavily trafficked compared to a lone debug-print connection.
10.40.5 **Color by edge kind** — Calls are green, imports are blue, inheritance is purple, tests are orange, dependencies are yellow.
10.40.6 **Fade in and out** — Each particle fades in at the start of its road, fades out at the end, and loops, so the visual feel is continuous flow rather than little dots popping in.

### 10.41 The 3D City View — Performance Budget
10.41.1 **Target frame rate** — Sustained 30 frames per second or higher on a mid-range developer machine, even with the full city visible and particles active.
10.41.2 **Draw call budget** — Roughly seventy draw calls for a city with around 930 buildings: one instanced draw per kind of building, one for district ground, one for ground plane, one for district walls, one for boulevard roads, one for bridge roads, one for the entire particle system, plus a handful for individually-rendered god-node buildings and active call tubes. Well under the typical 200-call ceiling at which 3D rendering starts to slow down.
10.41.3 **Pre-computation in a background thread** — All city geometry (layout, building dimensions, road geometry, particle start positions) is computed once when graph data loads, on a worker thread, so opening the city view never blocks the UI. Typical pre-compute time: under 500 ms for 930 nodes.
10.41.4 **Frustum culling** — Buildings outside the camera's view are not submitted to the GPU.
10.41.5 **Progressive shadow scope** — Real-time shadows are only cast by buildings within 40 units of the camera target. Distant buildings get ambient occlusion instead. Cheaper, still visually convincing.
10.41.6 **Memoized geometry** — The expensive geometry computation is cached and only re-runs when the underlying graph data actually changes, not on every UI state update.

### 10.42 Dual-View Toggle (City ↔ 2D Graph)
10.42.1 **One toggle button, two visualizations** — A pill-shaped toggle in the top-right of the canvas switches between the 2D force-directed graph and the 3D city. Active mode has the AIRS green background, inactive is dark gray.
10.42.2 **Animated transition** — Going from graph to city: the D3 canvas fades out over 300 ms, the 3D canvas fades in, the camera resets to default city overview, and buildings animate up from the ground (their Y-scale grows from zero to one over 600 ms, staggered by district so the city visibly "constructs itself"). Going back: buildings compress down, the 3D canvas fades out, the 2D canvas fades in.
10.42.3 **Selection sync** — Both views share the same selected-node state. Select a building in the city view, toggle to the graph view, and the same node is selected and focused there. The two views are different lenses on the same model.

### 10.43 Agent Isolation Guarantee
10.43.1 **What agents see** — Agents see the plain graph data through MCP tools: node qualified names, edge kinds, confidence tiers, community assignments, line numbers, file paths.
10.43.2 **What agents never see** — Three-dimensional positions, building dimensions, road geometry, district tile layouts, camera positions, particle states, level-of-detail level, post-processing state. None of this leaves the visualization layer.
10.43.3 **No new MCP tools for the city view** — The 3D city does not add a single MCP tool. The existing 22 graph tools (§10.9) work identically whether the user is looking at the 2D graph or the 3D city. The city is a pure rendering skin over the same data the agents already query.
10.43.4 **Why this matters** — It guarantees that adding (or removing) the city view cannot affect agent behavior. The agent fleet keeps working the same way regardless of which view the user prefers, and visualization changes can ship without any risk of breaking agent tools.

---

## 11. Dashboard Panel

### 11.1 What the Dashboard Is
11.1.1 **A real-time fleet management dashboard** — The Dashboard tab is your "is everything OK and what is it costing me?" view. Real-time cost tracking, token analytics, worker efficiency, system health, and predictive analytics, all in one place.
11.1.2 **Why it exists** — Without the Dashboard, you would have to guess at "how much did I spend today?", "is this worker pulling its weight?", and "are we trending toward an API outage?" The Dashboard answers those without having to write SQL.
11.1.3 **Data collection pipeline** — Every cost, token, task completion, and system event flows into a SQLite analytics database. Background aggregation jobs roll the data up by hour and by day. The Dashboard reads from the rolled-up tables for snappy rendering.

### 11.2 What's Stored
11.2.1 **Token usage table** — Every model call recorded with prompt tokens, completion tokens, thinking tokens (for thinking-capable models), total cost in dollars, the session, agent type, provider, model, and request type.
11.2.2 **Daily costs roll-up** — Per-day totals: total dollars, total tokens, session count, message count, tool-call count, plus per-provider and per-agent-type breakdowns as JSON.
11.2.3 **Worker stats roll-up** — Per-worker daily: tasks completed, tasks failed, time spent, dollars spent, average duration, success rate, last heartbeat.
11.2.4 **API failures log** — Every API call that failed: timestamp, provider, error type (rate limit / auth / timeout / model error), error message, retry count, whether resolved.
11.2.5 **System events log** — Structured event records covering all major components.

### 11.3 The Eight Dashboard Widgets
11.3.1 **Cost Meter** — Today's cost vs yesterday with a percentage change indicator. A progress bar against your daily budget, color-coded (green/yellow/red). Click for a drill-down modal that breaks the cost down by provider, by agent, and by task type.
11.3.2 **Token Usage** — Today's tokens with the prompt-vs-completion ratio, a 7-day line chart, a model-distribution pie chart, top 5 sessions ranked by token use. Also tracks thinking tokens and context window utilization percentage.
11.3.3 **Worker Status** — Active workers vs total, workers added today, an overall health indicator. Click for a modal with the full worker list and an "Add Worker" form.
11.3.4 **Session Activity** — Sessions created today, currently-active sessions, messages sent today, tool calls executed today.
11.3.5 **Model Usage** — Bar chart of token distribution by model. Modal drill-down shows cost per model in dollars per million tokens.
11.3.6 **Worker Leaderboard** — Top workers ranked by success rate. Average task duration per worker type. Cost per task per worker. Trending up/down icons indicate whether each worker is improving or regressing.
11.3.7 **API Failures** — Recent errors with timestamps and categorization. A modal drill-down shows per-provider failure rates with trend indicators. The widget also surfaces the auto-retry success rate so you can tell if AutoDev's resilience is working.
11.3.8 **System Health** — Per-component status, uptime percentages, response times, incident count, mean-time-to-recovery. Surfaces session crash tracking and subagent error categorization.

### 11.4 Real-Time Updates
11.4.1 **Event subscription** — The Dashboard subscribes to cost-update and api-failure events so widgets refresh the moment something happens.
11.4.2 **Budget alerts** — A budget-alert system raises a warning at 100% of budget and a critical alert at 110%.

### 11.5 Cost Analytics Features
11.5.1 **Drill-down dimensions** — Per session, per worker, per provider, per model, per task type.
11.5.2 **Cost projections** — "At your current rate, your monthly cost will be $X." Computed from a 7-day rolling average.
11.5.3 **Budget alerts** — Configurable thresholds with desktop notifications.

### 11.6 Token Analytics Features
11.6.1 **Token usage by agent type** — Stacked breakdown: PA vs Workers vs QC.
11.6.2 **Prompt vs completion ratio** — A rough proxy for how chatty your agents are.
11.6.3 **Thinking tokens** — Separately tracked for thinking-capable models.
11.6.4 **Token efficiency** — Tokens spent per successfully-completed task. A measure of how productive your fleet is.
11.6.5 **Context window utilization** — How much of the available context window your sessions actually use.
11.6.6 **Subagent token tracking** — Subagent token use is rolled into their parent so the leaderboard reflects total burn.

### 11.7 Worker Analytics Features
11.7.1 **Tasks per worker per day**.
11.7.2 **Success rate per worker**.
11.7.3 **Average task duration**.
11.7.4 **Cost per task**.
11.7.5 **Utilization rate** — Percentage of time the worker was actively working versus idle.
11.7.6 **Idle time tracking** — Helps identify workers that are over-provisioned.

### 11.8 System Health
11.8.1 **Sidecar uptime / downtime**.
11.8.2 **Provider API availability**.
11.8.3 **Tool execution failure rates**.
11.8.4 **Session crash tracking** with mean-time-to-recovery.
11.8.5 **Subagent error categorization**.

### 11.9 Export and Reporting
11.9.1 **CSV export** for any data range — for feeding into Excel or financial models.
11.9.2 **JSON export** for programmatic consumption.
11.9.3 **PDF report generation** (weekly, monthly).
11.9.4 **Scheduled reports** delivered via email.
11.9.5 **Custom date range selection** — Pick any start and end date.

### 11.10 Predictive Analytics
11.10.1 **Monthly cost prediction** based on current usage.
11.10.2 **Capacity planning** — "You need X more workers to keep up with current task creation rate."
11.10.3 **Optimal model recommendation** — "For tasks like Y, switching to Ollama would save you $Z per month."
11.10.4 **Subagent depth tracking** with safety-limit indicators.

### 11.11 Dashboard Sidebar
11.11.1 **A collapsible left rail visible from any tab** — Shows cost / token summary, active sessions, and worker health at a glance, without forcing you to leave whatever tab you're in.

---

## 12. Cross-Cutting Subsystems

### 12.1 The 21 Orchestrator State Machines
12.1.1 **Agent Lifecycle FSM** — Tracks an agent from created → registered → starting → active → stopping → stopped.
12.1.2 **Session Lifecycle FSM** — Tracks a session from created → warming → active → idle → closing → closed.
12.1.3 **Task Execution FSM** — Tracks a task from created → planning → executing → reviewing → completed/failed.
12.1.4 **Tool Execution FSM** — Tracks a tool call from pending → executing → completed/failed/cancelled.
12.1.5 **Memory Store FSM** — Tracks a memory write from idle → storing → stored/failed.
12.1.6 **Memory Recall FSM** — Tracks a memory read from idle → recalling → recalled/failed.
12.1.7 **MCP Server FSM** — Tracks an MCP connection from disconnected → connecting → connected → error.
12.1.8 **MCP Tool Discovery FSM** — Tracks tool discovery from idle → discovering → discovered/failed.
12.1.9 **A2A Server FSM and A2A Request FSM** — Track the agent-to-agent server and per-request lifecycles.
12.1.10 **Workflow Execution FSM** — Tracks a multi-step workflow from created → running → completed/failed.
12.1.11 **Cache Tier FSM** — Tracks cache entries through cold → warming → hot → stale → evicted.
12.1.12 **System Startup and System Shutdown FSMs** — Track the boot and shutdown sequences as a whole.
12.1.13 **Agent Subprocess FSM, Agent Config FSM, Session Runtime FSM, Prompt Reload FSM** — Track other agent-related lifecycles.
12.1.14 **File Operations FSM and Authentication FSM** — Track file I/O and auth.
12.1.15 **All emit events with correlation IDs** — Every transition becomes an event. Events flow through the SSE bridge to the Rust shell, which mirrors them as Tauri events, which the React UI reflects in real time. This is how the Dashboard, the Live Monitor, and every health indicator stay current.

### 12.2 The Skills System (Detailed)
12.2.1 **What a skill is** — A markdown file with a YAML header at the top. The header gives the skill its name, description, category, parameter list, version, author, and tags. The body is a prompt template with `{{parameter}}` placeholders.
12.2.2 **Parameter types** — String, integer, boolean, enum (with a fixed value list).
12.2.3 **Five categories** — coding, debugging, devops, fleet, custom.
12.2.4 **Twenty-plus built-in skills** — Discussed by name in section 3.6.
12.2.5 **Skill execution flow** — Find the skill by name, validate the parameters against its schema, substitute the placeholders, inject the rendered prompt into the agent's context.
12.2.6 **Skills Palette** — Cmd+Shift+S to open. Search by name. Filter by category. Preview each skill. Click Execute.
12.2.7 **Dynamic parameter modal** — Form fields are generated from the parameter schema. Required vs optional indicators. Default values pre-filled. Tooltips.
12.2.8 **Slash command autocomplete** — Type `/` in the chat to see matching skills.
12.2.9 **Execution history** — A record of every skill run with its inputs and outcome.
12.2.10 **Mode tagging** — A skill can specify which PA modes it's valid in.

### 12.3 The Hooks System (Detailed)
12.3.1 **What a hook does** — A hook is a small action that runs automatically when something happens. For example: "every time the agent writes a Rust file, run `cargo fmt` on it." Or "every time a worker completes a task, send a Slack notification." Hooks are how you wire AutoDev into the rest of your toolchain without changing any source code.
12.3.2 **Six categories of triggers** — Agent lifecycle (session start, session end, agent loop start, agent loop end, on error), tool execution (pre-tool-use, post-tool-use, on-tool-error, on-permission-request), file system (pre-file-read, post-file-write, pre-file-delete, on-checkpoint-create, on-checkpoint-restore), git (pre-commit, post-commit, pre-push, on-branch-switch, on-merge-conflict), plan management (pre-plan-step, post-plan-step, on-plan-activate, on-plan-complete, on-plan-abandon), fleet-specific (on-task-claim, on-task-complete, on-task-fail, on-qc-review-start, on-qc-approve, on-qc-fail).
12.3.3 **Hook configuration** — Hooks live in a JSON configuration file. Each hook has an ID, a type (trigger), an enabled flag, a priority (higher fires first), a set of conditions, and an action.
12.3.4 **Action types** — Shell (run a shell command with variable substitution), HTTP (POST or GET to a URL with a JSON body), notification (a desktop OS notification), log (write to a custom log file), block (prevent the action from happening, optionally with a message), emit_event (raise a custom event for the UI to catch).
12.3.5 **Conditions** — Match by tool name, file pattern, repo path, command pattern (regex), session type (PA vs fleet), agent mode, time range, or a custom Rhai expression that has access to context variables.
12.3.6 **Hook execution context** — Every hook receives a JSON object describing what triggered it: the hook type, timestamp, session, agent mode, the tool call (if any), the current repo, the current plan (if any). Variables can be referenced in shell commands and HTTP bodies via `{{variable}}` syntax.
12.3.7 **Priority-based ordering** — When multiple hooks match an event, they fire in priority order.
12.3.8 **Sandboxed execution** — Hooks run in an isolated context so a misbehaving hook doesn't take down the whole app.
12.3.9 **Hooks UI** — A Settings panel with toggle switches per hook. An add-hook form. A conditions editor. A test button with a mock context.
12.3.10 **Hook Log Viewer** — A panel showing recent executions with filterable outcomes (executed, failed, blocked, skipped). Expandable to see full output and error context.
12.3.11 **Persisted in SQLite** — Hook configurations survive restarts.

### 12.4 Notification System
12.4.1 **A bell icon in the title bar** with an unread count.
12.4.2 **Notification types** — Task complete, task failed, worker error, build failed, cost threshold.
12.4.3 **Persistence** — Notifications stay in a Zustand store; cleared on dismiss or after 24 hours.
12.4.4 **Desktop OS notifications for critical events** — System-level toast notifications.
12.4.5 **Toast pipeline (in-app)** — Beyond persistent notifications, AutoDev has an in-app toast stack pinned to the top-right of the window for transient announcements. Toasts auto-dismiss after a configurable duration; pressing Escape clears the stack.
12.4.6 **Severity-colored toasts** — Each toast has a severity (error, warning, info, success) mapped to the AutoDev palette so the user can register the urgency at a glance.
12.4.7 **Worker-specific toasts** — Status transitions into `error`, `cooldown`, or `offline` trigger a toast that includes the worker name, the reason (from the structured worker:status payload), and the cooldown countdown if relevant.
12.4.8 **Toast deduplication** — Identical `{slug, status}` events arriving within a short window (about five seconds) are deduplicated so retry storms cannot spam the user.

### 12.5 Memory Architecture (Detailed)
12.5.1 **Hot tier** — An in-memory ordered cache. Fastest, smallest, in-process.
12.5.2 **Warm tier** — Redis or Memurai. Fast, larger, in a separate process.
12.5.3 **Cold tier** — mem0 (or MongoDB, ChromaDB, Google Vertex AI). Slowest, largest, persistent.
12.5.4 **Three memory types** — Episodic (recent interactions, short TTL), semantic (facts about the world, medium TTL), procedural (accumulated wisdom, long TTL).
12.5.5 **Advanced subsystems** — Token budget (auto-degrade context when over limit), compression engine (multi-level memory compression by age), staleness tracker (temperature-based freshness scoring), query expander (sub-query generation for better recall), BM25 hybrid search (lexical + semantic fusion), context scorer (8-factor utility ranking), event journal (append-only write-ahead log with BLAKE3 hashing), contradiction detector (scope-aware), continuity packet (tiny session-state for instant warm-up), predictive stager (pre-fetch memories likely needed next), paradigm integration (multi-signal relevance), real-time metrics with alerting.
12.5.6 **mem0 OSS backed by Qdrant** — In the standard configuration, mem0 (the long-term memory framework) is set up in its open-source mode using a local Qdrant vector store for semantic search. This means memory works without depending on mem0's hosted service.
12.5.7 **Stable per-worker memory lanes** — Every fleet worker uses a deterministic memory-session identifier derived from its slug (e.g. `worker-backend-...`) so its accumulated memories persist across every tick instead of being lost to a fresh random session on each cron fire. This gives Workers genuine continuity ("I already did this on the previous tick").
12.5.8 **Continuity packet fast-boot** — Instead of running a full vector-store recall on every wake-up, a tiny continuity packet (a few kilobytes of recent state) is stored in the warm cache and loaded as a seed. The full recall only runs if the packet's signal is insufficient. This is what keeps recall under 200ms when the warm tier is up.
12.5.9 **Slow-recall threshold alerting** — A configurable threshold (default 1 second) triggers a `Slow recall` log warning. Recalls consistently above this number usually mean the warm cache is empty or down.
12.5.10 **Session SSE skip for headless** — Streaming events from headless sessions (worker recipes that never had a UI to begin with) are deliberately not broadcast over the SSE bus. The earlier behaviour generated a flood of "session not found" warnings on every tick; the headless prefix check skips them silently.

### 12.6 Agent-to-Agent Protocol
12.6.1 **Supervisor ↔ Worker coordination** — A standard protocol in the Orchestrator lets agents send each other structured messages.
12.6.2 **Agent registry with agent cards** — Each agent advertises its capabilities through a card-shaped data structure so other agents can discover what it does.
12.6.3 **Used in worker mode only** — The PA in Mode 1 doesn't use this — it's not needed when a human is doing the coordination.

### 12.7 Permission System
12.7.1 **Three modes** — Manual approval (prompt every time), autonomous (no prompts), smart approval (prompt only for dangerous things).
12.7.2 **State machine for the dialog** — Each pending permission has its own state machine that tracks pending → approved/denied/timeout.
12.7.3 **Permission inspector** — A planned layer that can reject specific commands before they reach the permission dialog (e.g., a regex that blocks `rm -rf /`).
12.7.4 **Permission mode is independent of PA mode** — PA Mode (Fast Dev / Review / Delegate) controls *which tools are registered*; Permission Mode (ManualApproval / SmartApproval / Autonomous) controls *whether each tool execution waits for an Allow/Deny click*. They are separate sliders so a user can, for example, run in Fast Dev with Manual Approval for an especially sensitive task.
12.7.5 **SmartApproval auto-execute rules** — In SmartApproval, read-only tools (`read_file`, `list_directory`, `grep`, `glob`, `github_create_issue`) auto-execute without a confirmation prompt. Write-side tools (`write_file`, `edit_file`, `bash`, `github_create_pr`, `github_merge_pr`, `planner` with destructive actions like `abandon`) always ask. The intent is "let the obvious read-only stuff flow, pause on anything that changes the world."
12.7.6 **Hard-blocked operations (no override, ever)** — A small list of operations is *always* blocked regardless of permission mode and cannot be overridden by the user — most notably `git push origin main`. The associated tool card renders in a `BLOCKED` state with no Allow button at all, just a policy explanation. This is the safety net that protects the protected branch from accidental agent damage even in Autonomous mode.
12.7.7 **Permission mode toggles in multiple places** — The user can change permission mode from the ChatHeader dropdown (next to the PA sub-mode), from the StatusBar's left section, or by right-clicking any pending tool card. Redundant entry points so the toggle is never more than one click away when something is waiting.

### 12.8 Context Compaction
12.8.1 **Auto-compact at 80% of context** — When a session's conversation is filling up the model's context window, AutoDev silently summarizes older messages to keep things from overflowing.
12.8.2 **Replaced by three-tier memory for the PA** — In Phase 4 the PA stopped relying on compaction and started using persistent memory instead. Compaction remains as a fallback.

### 12.9 SQLite Migrations
12.9.1 **Versioned schemas per database** — Each database tracks its schema version. On app launch, any pending migrations run automatically.
12.9.2 **Wrapped in transactions** — Migrations run inside explicit transactions so a failure halfway through leaves the database in a consistent state.
12.9.3 **Idempotent data migrations** — One-time fixes (like "rename all old model slugs to new ones") run safely on every launch.

### 12.10 Cost Tracker
12.10.1 **Per-model pricing data** — Every model carries its cost per million input tokens and per million output tokens. The cost of any individual call is computed from those numbers.
12.10.2 **In-memory + database recording** — Cost is added to the current session's in-memory total and also written to the analytics database for the Dashboard.
12.10.3 **Provider inferred from model name** — If a model name starts with "claude" it's Anthropic, "gpt"/"o1"/"o3" is OpenAI, "gemini" is Google. This means costs are attributed correctly without extra metadata.
12.10.4 **Non-fatal recording** — A failure to write a cost row never breaks the agent run.

### 12.11 Worker Stats Recording
12.11.1 **Per-tick stats** — After every Worker tick, AutoDev records whether it succeeded, failed, or did nothing, plus the time spent and the tokens used.
12.11.2 **Token extraction from Worker mode** — When a Worker call returns through the Orchestrator, the underlying token counts are extracted from the response and recorded.

### 12.12 Validation Module
12.12.1 **What this module does** — A shared validation library used by both the inline "did your write produce something valid?" check on `write_file` and `edit_file`, and by the standalone `check_file` tool that agents can call on demand. It returns structured diagnostics (file, line, severity, message) so agents can fix issues with surgical precision.
12.12.2 **Tier 1 — syntax** — The fastest tier. JSON, YAML, and TOML are parsed in-process by Rust so the check is essentially free. Python files are validated via `py_compile`. JavaScript and TypeScript files use `node --check`. The result is a list of syntax errors with line numbers.
12.12.3 **Tier 2 — imports** — A pure-Rust import cross-referencer. For Python files it parses out every `import` and `from ... import` statement and checks each against the project's `requirements.txt` (or `pyproject.toml`) plus a built-in set of about 220 Python standard-library modules — so `import json` doesn't get flagged but `import nonexistent_package` does. For JavaScript and TypeScript it does the same against `package.json` plus a set of around 35 Node.js built-ins. This catches the "agent wrote code that imports a library not in the manifest" failure mode.
12.12.4 **Tier 3 — types** — A heavier, optional pass that runs the project's actual type checker. For Python it auto-detects whether `mypy` or `pyright` is installed in the project's virtual environment. For TypeScript it runs `tsc --noEmit`. For JavaScript it runs `eslint` if configured. Tier 3 only runs when the agent explicitly asks for it (e.g. via the `check_file` tool with `checks=["types"]`).
12.12.5 **Venv-aware Python resolution** — When Python checks need to run, the module first looks for a project-local virtual environment (a `.venv` or `venv` next to the manifest) and uses that interpreter. So a project using uv-managed dependencies gets checked against its own installed packages rather than whatever Python happens to be on the system PATH.
12.12.6 **Manifest auto-discovery** — Tier 2 and Tier 3 walk upward from the file being checked, looking for the nearest manifest (`requirements.txt`, `pyproject.toml`, `package.json`, etc.). This is what lets the agent run `check_file` on a deeply-nested source file and have the import check work against the right project's manifest.
12.12.7 **Inline diagnostics in tool results** — When the inline check after a `write_file` or `edit_file` finds issues, the tool result's main output stays compact ("File written.") but the validation findings are formatted as a small bullet list appended below, so the agent reads them as part of the same response and can immediately fix them in the next turn.
12.12.8 **Standalone `check_file` tool** — In addition to the inline check, agents can call `check_file(path, checks)` directly. The `checks` parameter is a list of which tiers to run; defaults to `["syntax", "imports"]`. The tool returns `ToolResult::error` (rather than success) when any check finds errors, which signals the agent loop that it has something to fix before claiming the task is done.

### 12.13 Provider Layer Detail
12.13.1 **Unified completion request shape** — Model, messages, tools, temperature, top-p, reasoning effort, thinking budget, max tokens.
12.13.2 **Connection state** — Each provider's connection has a small state machine: disconnected → testing → connected → error.
12.13.3 **Streaming parser** — Provider-agnostic; emits a normalized stream of text deltas, message deltas, tool-use starts, tool-use deltas, and a final done event.

### 12.14 Update Server API
12.14.1 **Versioned releases endpoint** — Returns the latest version, release date, release notes, a critical flag (for forced updates), and per-platform download URLs (both full installer and incremental patch) with signatures.
12.14.2 **Per-channel data** — Stable, beta, and nightly channels each have their own latest version.

### 12.15 Memory MCP Server Tools (6)
12.15.1 **`store_memory`** — Save content with optional topic, tags, and an "infer" flag for whether to auto-extract facts.
12.15.2 **`search_memories`** — Find memories by query with limit, offset, filters, min score, session scope ratio.
12.15.3 **`get_memory`** — Fetch one memory by ID.
12.15.4 **`update_memory`** — Modify an existing memory.
12.15.5 **`delete_memory`** — Remove a memory.
12.15.6 **`list_memories`** — List with pagination and filtering.

### 12.16 Bridges in the Orchestrator
12.16.1 **Unified tool handler** — One dispatcher that routes tool calls to the right backend (memory, MCP, internal, AutoDev).
12.16.2 **MCP bridge** — Discovers tools from external MCP servers and integrates them into the function-calling pipeline.
12.16.3 **Memory compiler** — Specialized handler for memory-specific tool calls.
12.16.4 **Adobe compiler** — Specialized handler for Adobe creative tools (excluded from the AutoDev deployment).
12.16.5 **Agent delegation bridge** — Implements agent-to-agent task delegation.

### 12.17 Ecosystem Detection & Build Validation
12.17.1 **What this layer does** — When a fleet worker scaffolds a new project (writes a `package.json`, a `Cargo.toml`, etc.), the runner needs to verify the code actually compiles and runs before claiming the task is "done." A dedicated subsystem detects which build ecosystem the workspace uses, runs the right install command, then runs the right validate command, and surfaces the result as a task status.
12.17.2 **Detected ecosystems** — Node.js (a `package.json` is present), Python via pip (a `requirements.txt` without other lockfiles), Python via Poetry (`pyproject.toml` + `poetry.lock`), Python via uv (`pyproject.toml` + `uv.lock`) — the modern alternative now first-class — Rust via Cargo (`Cargo.toml`), and Go (`go.mod`). Detection happens in priority order so a Python project that uses both Poetry and uv resolves correctly.
12.17.3 **Install commands per ecosystem** — Node uses `npm install` (with fallback to `pnpm install` or `yarn install` based on lockfile presence). Python pip uses `pip install -r requirements.txt`. Poetry uses `poetry install`. uv uses `uv sync`. Rust uses `cargo build`. Go uses `go mod tidy`.
12.17.4 **Validate commands per ecosystem** — Node uses `npm run build` or `npx tsc --noEmit`. Python uses `python -m pytest` (or the Poetry / uv variant). Rust uses `cargo test`. Go uses `go test ./...`.
12.17.5 **Sentinel files per ecosystem** — A small marker that proves install actually completed: a `node_modules` directory for Node (more reliable than the old `node_modules/.package-lock.json` check that no longer existed in modern npm), `.venv/pyvenv.cfg` for Python, a Cargo build marker for Rust, `go.sum` for Go.
12.17.6 **Worker recipe build-validation step** — Every Worker recipe includes a mandatory Build Validation section instructing the agent to detect the ecosystem, run install if the sentinel is missing, run validate, and on a non-zero exit write the stderr to a `build.log` and mark the task `build_failed` instead of `qc_review`.
12.17.7 **Build Validation precedes Stop Contract in the prompt** — In earlier recipe drafts, Build Validation appeared *after* the "Stop Contract" section, which meant a clever LLM could decide it had satisfied the Stop Contract and skip the validation step. The new ordering puts Build Validation first so the agent always reads it before considering itself done.
12.17.8 **Server-side guard as a backstop** — Even if the agent skips validation, the runner's `guard_and_validate` step (see 4.6.10) re-checks the sentinel and demotes the task to `build_failed` if dependencies were never installed. So workers cannot accidentally ship broken code by misreading their recipe.

### 12.18 Centralized Log Collection
12.18.1 **A single logging pipeline** — All log lines (from the Rust app, sidecar processes, and the agent loop) funnel through one collector that handles formatting, sanitization, and storage.
12.18.2 **String sanitization** — Every log line is run through a sanitization step that handles UTF-16 surrogate characters (which used to crash Python's default StreamHandler on Windows) and strips control sequences that would break log viewers.
12.18.3 **UTF-8 with replace fallback** — File handles are opened with `errors='replace'` so a bad character never corrupts the log file; instead the offending byte is replaced with the Unicode replacement character. Sidecar processes are spawned with `PYTHONIOENCODING=utf-8:replace` and `PYTHONUTF8=1` to apply the same policy at the source.
12.18.4 **Log circuit breaker** — A defense against accidental log floods: if more than about 2,500 lines arrive in a 10-second window, the collector opens its circuit for a 2-second cooldown and drops further input. A single warning line is emitted explaining what just happened. This prevents a misbehaving sidecar or a debug-verbose library from filling the disk in seconds.
12.18.5 **API key masking** — Provider initialization logs are filtered so the `api_key` field is never serialized. This stops live API keys from leaking into log files that might be shared for debugging.
12.18.6 **Per-source isolation** — Even though everything funnels through one collector, each sidecar gets its own dedicated log file (e.g. `core-orchestrator.log`, `memory-mcp.log`) so the user can grep one sidecar's history without seeing every other sidecar's chatter.
12.18.7 **Daily rolling with retention** — The main app log rolls daily and keeps seven days of history. Older files are auto-deleted to keep disk usage bounded.

### 12.19 Transport Resilience (Rust ↔ Orchestrator)
12.19.1 **The Windows 10054 problem** — On Windows, idle HTTP keep-alive connections are sometimes silently reset by the operating system after a period of inactivity. The Rust shell's HTTP client (reqwest) historically had no idea this happened and would try to reuse a dead pooled connection on its next request, causing a sudden "error sending request" failure at exact cron boundaries.
12.19.2 **Connection pool tuning** — The HTTP client is now configured with a small pool (one idle connection per host) and a short idle timeout (about 30 seconds) so that connections are reaped before Windows reaps them. This dramatically reduces the chance of picking a dead connection.
12.19.3 **Single retry on connection errors** — On any `is_connect()` or `is_timeout()` error from the underlying client, the request is retried once after a brief delay with a fresh connection. Real outages still surface; transient pool issues self-heal silently.
12.19.4 **Mode-1 fallback model name resolution** — When the orchestrator is unreachable and the Worker falls back to the lightweight Rust agent engine, the same model identifier needs to work. A small helper queries the orchestrator's model registry endpoint at boot, caches the `registry_key → canonical_name` map, and uses it during fallback so the same model is addressed by both modes. (This was a real bug: the Rust fallback used to send the short key while the orchestrator used the full LiteLLM name, producing model-not-found errors during fallback.)
12.19.5 **Idempotent MCP server registration** — The Rust shell tells the orchestrator about the in-app MCP server on every startup. If the orchestrator already knew about it from a previous run, it answers with various flavors of "already exists" responses (HTTP 400, 409, or 500 depending on the version). The Rust client treats any of these as success rather than an error, so app restarts are safe and the log stays clean.

### 12.20 Windows Shell Compatibility
12.20.1 **The Unix-defaults problem** — LLMs trained predominantly on Unix examples generate commands using `head`, `tail`, `grep`, and similar utilities that do not exist in Windows CMD or PowerShell. Running these blindly produces an `exit_code: 255` "is not recognized" error, which the agent interprets as a hard failure.
12.20.2 **Platform-aware tool description** — The `bash` tool description includes a platform hint when running on Windows, warning the agent to avoid Unix-only utilities and suggesting PowerShell-compatible alternatives.
12.20.3 **Command rewriting** — A pre-processor catches common Unix-pipe patterns (` | head -N`, ` | tail -N`, ` | grep PATTERN`) and rewrites them to PowerShell equivalents (`Select-Object -First N`, `Select-Object -Last N`, `Select-String PATTERN`) before spawning the process. Rewritten commands are logged at debug level so developers can verify.
12.20.4 **Structured "did you mean" hint on exit 255** — If a command fails with exit 255 and the error contains "is not recognized," the tool returns a structured suggestion block explaining the Unix→PowerShell mismatch. The agent uses this in its next reasoning step to self-correct.

---

## 13. Distribution, CLI, Installers

### 13.1 The Headless CLI (`airsctl`)
13.1.1 **What it is** — A separate command-line binary alongside the desktop app. Lets you do most of what the desktop app does, but from a terminal — useful for automation, CI/CD pipelines, and worker PCs that don't need a GUI.
13.1.2 **Shared backend** — The CLI and the desktop app read and write the same databases, so a task created in the CLI shows up in the desktop UI and vice versa.
13.1.3 **Global flags** — Config path, working directory, API key, provider, model, non-interactive mode (for CI), output format (json / table / plain), quiet, verbose, version, help.
13.1.4 **`airsctl agent`** — Run / status / stop / list / logs. Run accepts a prompt either inline or from a file, with an optional session ID.
13.1.5 **`airsctl task`** — Create / list / get / assign / claim / complete / fail / delete.
13.1.6 **`airsctl worker`** — Register / list / status / heartbeat / unregister.
13.1.7 **`airsctl config`** — Init / get / set / list / validate / import / export.
13.1.8 **`airsctl plan`** — Create / list / get / add-step / update-step / activate.
13.1.9 **`airsctl system`** — Status / logs / sidecars / backup / restore.
13.1.10 **`airsctl analytics`** — Cost / tokens / workers / export.
13.1.11 **`airsctl logs`** — View / list / clear / export.
13.1.12 **`airsctl plugin`** — List / install / uninstall / update / verify / run / info / config.
13.1.13 **`airsctl worker-setup`** — Generate / validate / redeem setup tokens.
13.1.14 **`airsctl graph`** — Query / path / explain / build / update / report / diff (partly stubbed; full functionality on the roadmap).
13.1.15 **REPL mode** — `airsctl repl` opens an interactive shell with command history.
13.1.16 **Non-interactive output** — JSON for any command, designed for piping into other tools. Exit codes 0 (success), 1 (error), 2 (partial success).
13.1.17 **Configuration file** — A TOML file in your home directory with sections for core, providers, CLI, and worker settings.
13.1.18 **Environment variables** — Standard env vars can override config (`AIRSDEV_API_KEY`, `AIRSDEV_PROVIDER`, and `ANTHROPIC_API_KEY` as a fallback).

### 13.2 The AIRS Plugin System
13.2.1 **Plugins as installation units** — A plugin bundles everything a worker PC needs: dependencies, configuration, install scripts, verification. You install one plugin to get one Worker type up and running.
13.2.2 **Plugin types** — Worker setup (installs Rust, Python, Node, etc.), environment (clones repos, sets up git credentials, configures IDE), security (installs static analysis tools, CVE scanners, configures access controls), custom (your own).
13.2.3 **Built-in plugins** — Five out of the box: Worker Backend, Worker Frontend, Worker DevOps, Worker QA, Worker Security.
13.2.4 **Plugin manifest** — Each plugin has a YAML file declaring its metadata, dependencies, install/verify/uninstall commands per OS, and a config schema.
13.2.5 **Per-OS install scripts** — Auto-generated at install time for Linux/macOS (shell script) and Windows (PowerShell).
13.2.6 **Signed setup tokens** — Tokens are cryptographically signed with a time-to-live so they expire automatically and cannot be forged.
13.2.7 **One-click worker setup flow** — IT staff click "Add Worker," pick a type, generate a token, email the resulting one-liner to a teammate. The teammate runs the one-liner; five minutes later their machine is a fully-configured Worker showing up in the fleet panel as Active.

### 13.3 Platform Installers
13.3.1 **Windows MSI** — Built using WiX v4, the modern Windows installer toolkit. Includes a check-and-auto-install for WebView2 (the rendering engine Tauri uses on Windows), a firewall exception, file associations for `.autodev` files, an environment variable, and code signing.
13.3.2 **Windows NSIS** — An alternative installer format for environments that prefer NSIS to MSI.
13.3.3 **macOS DMG** — Signed and notarized app bundle. Drag-and-drop installation. Hardened runtime with the right entitlements for WebKit JIT, network access, filesystem access, and subprocess spawning.
13.3.4 **Linux AppImage** — A single-file universal Linux build that works across distributions without installation. Includes an AppDir structure with an AppRun entry point and a desktop file.
13.3.5 **Linux .deb package** — For Debian and Ubuntu. Includes control file, postinst and prerm scripts.
13.3.6 **Linux .rpm package** — For Fedora and RHEL. Includes a spec file.
13.3.7 **Desktop integration** — Linux packages install a `.desktop` file so AutoDev shows up in the application menu. Post-install scripts update MIME associations, desktop database, and icon cache.
13.3.8 **Build pipeline** — Cross-platform build scripts plus GitHub Actions workflows for multi-platform releases and PR validation.
13.3.9 **Automated release creation** — A release script invokes `gh release create` to upload all the platform installers to a GitHub release in one step.
13.3.10 **Code signing** — Windows via signtool, macOS via codesign + notarytool.
13.3.11 **SHA-256 checksums** — Generated for every artifact so users can verify their download.

### 13.4 Auto-Updater (Recap)
13.4.1 **Auto-updater details covered in section 9.17**.

### 13.5 Enterprise Distribution
13.5.1 **Silent install flags** — MSI's `/qn` and NSIS's `/S` for headless install on managed Windows machines.
13.5.2 **Group Policy / MDM documentation** — Detailed instructions for IT departments rolling out AutoDev to many users at once.
13.5.3 **Staged rollouts** — Control which percentage of users get a new version.
13.5.4 **Version pinning** — Lock specific users on a known-good version.
13.5.5 **IT Staff Runbook** — A documentation file covering pre-deployment checklist, system requirements, network requirements, step-by-step install guide per platform, silent deployment, troubleshooting, updates, uninstall.

---

## 14. Design System & UI v2.0

### 14.1 Design Tokens
14.1.1 **Color palette** — The aesthetic is "industrial minimal." Backgrounds are near-black (#0A0A0A primary, #111111 surface, #161616 raised, #1A1A1A overlay). Borders are dark gray (#222222 default, #333333 focused). The single accent color is AIRS green (#00FF88), with a dimmer variant for hover states. Danger is red (#FF4444), warning is amber (#FFAA00), info is blue (#4E79A7), success is green (#59A14F). Text comes in three weights: primary (#F0F0F0), secondary (#888888), disabled (#444444).
14.1.2 **Fonts** — IBM Plex Mono for code, numbers, agent names, tool calls, metric values. IBM Plex Sans for prose, UI labels, descriptions. Both are open-source-font-license, bundled locally so there's no CDN dependency.
14.1.3 **Border radius** — 2 pixels everywhere, by design. The sharp corners are part of the industrial feel.
14.1.4 **Spacing base unit** — 8 pixels. Everything in the design system uses multiples (4, 8, 16, 24, 32, 48).
14.1.5 **Typography scale** — Six sizes: 9 pixels (uppercase metric labels), 10 pixels (section headers and badges), 11 pixels (sidebar items, roster rows, card content), 12 pixels (tab labels, code), 13 pixels (chat messages, input text), 16 pixels (metric values, headings).
14.1.6 **Tailwind extension** — The tokens are wired into Tailwind CSS so the same values are used consistently across every component.

### 14.2 The Six-Tab Layout
14.2.1 **Chat, AutoDev, IDE, Graph, Recipes, Settings** plus the Dashboard sidebar.

### 14.3 PA Mode Toggle (Recap)
14.3.1 **Covered in section 3.2**.

### 14.4 Tool Call Card System (16 Variants)
14.4.1 **`read_file` card** — Filename, line range, byte count, content preview (first 3 lines).
14.4.2 **`write_file` card** — Filename, byte count written, validation chip (green ✅ 0 errors or yellow ⚠️ N errors).
14.4.3 **`edit_file` card** — Filename, diff stats (+N / -N lines), validation chip.
14.4.4 **`bash` card** — Command (first 100 chars), exit code chip, output (last 20 lines, expandable). Special handling: if the command was `cargo test` or `npm test`, a test summary chip appears. Rust and Python error codes are extracted from stderr into colored chips.
14.4.5 **`check_file` card** — Filename and per-tier (syntax, imports, types) result chips.
14.4.6 **`append_file` card** — Filename and bytes appended.
14.4.7 **`grep` card** — Pattern, match count, first 5 matches with line numbers and highlighted match positions.
14.4.8 **`glob` card** — Pattern and file count, with an expandable sortable table (name, size, modified) and filter by extension.
14.4.9 **`list_directory` card** — Path and entry count with a sortable table.
14.4.10 **`create_task` card** — Task title, ID, and status badge.
14.4.11 **`update_task_status` card** — Task ID and old → new status transition.
14.4.12 **`spawn_subagent` card** — Subagent slug, working directory, truncated task description.
14.4.13 **`spawn_parallel_subagents` card** — Number of agents spawned and their working directories.
14.4.14 **`planner` card** — Plan title, step count, collapsible step list.
14.4.15 **Graph tool cards** — Result count and first 3 nodes.
14.4.16 **Generic fallback card** — Tool name, JSON input, JSON output for any tool without a specialized renderer.
14.4.17 **Common card behavior** — Collapsed by default, expand on click. The hint text is rendered in a small `<details>` element below the card. File-related cards include an "Open in IDE" button.
14.4.18 **State machine per card** — Every card transitions through `pending-approval` → `running` → `success` / `error` / `denied` / `blocked-by-policy`. Each state has its own border color, status badge, and footer (Allow / Deny buttons only in `pending-approval`; spinner with elapsed timer in `running`; result content in `success` / `error`; denial note in `denied`; policy explanation with no override in `blocked-by-policy`).
14.4.19 **Pending card never collapses** — A card waiting for the user's decision always stays expanded so it cannot be missed. Resolved cards collapse to a single-line summary like `✏ edit_file · engine.rs · +3 -0 · ✓ Done` so the chat history stays scannable.
14.4.20 **Test summary chip** — When a `bash` card runs a recognized test runner (cargo test, npm test, pytest), the result section adds a green test summary chip on top of the raw output: e.g. `✅ 47 passed · 0 failed · 2 skipped · 1.24s`. Saves the agent (and the user) from parsing pages of test framework output.
14.4.21 **Rust error code chips** — For `cargo check` and `cargo build` output, the bash card's error renderer identifies Rust error codes (e.g. `E0433`) and renders them as clickable chips. Clicking a chip jumps the IDE tab to the offending line.
14.4.22 **IDE diff sync** — When the IDE tab is open and showing the same file that a write or edit card is targeting, the diff card and the IDE code viewer stay in sync — accepting changes in one is reflected in the other.
14.4.23 **MCP card iconography** — Tool cards for external MCP-server tools (GitHub, Slack, etc.) carry a distinct `⚡` icon in the header and show the MCP server name on the left, so the user can tell at a glance "this is talking to GitHub, not editing local files." After success, if the MCP call produced a remote resource (issue URL, PR URL), the result section includes a clickable link.
14.4.24 **Inline command editing** — On a `pending-approval` bash card, the user can edit the command text directly before clicking Allow. This is the safe-edit-then-run pattern — the user can fix a small typo or add a missing flag without rejecting and re-asking.
14.4.25 **Destructive command warning band** — When a `bash` command matches a destructive-pattern regex (`rm -rf /...`, `dd if=...`, `>:`, etc.), a warning band above the Allow button forces the user to acknowledge the danger before approving.

### 14.5 Plan Display Cards (Recap)
14.5.1 **Covered in section 3.10**.

### 14.6 Notification System (Recap)
14.6.1 **Covered in section 12.4**.

### 14.7 Cost and Token Visualization
14.7.1 **CostBar in Chat tab footer** — The running session cost in dollars plus token counts in/out.
14.7.2 **Per-session sparkline** in the Dashboard sidebar — Cost over the last 10 sessions.
14.7.3 **Token budget ring** — A colored ring (green → yellow → red) that fills as you approach your budget.
14.7.4 **Worker cost leaderboard** in the AutoDev Panel.

### 14.8 Memory Health Panel (Recap)
14.8.1 **Covered in section 3.9.10**.

### 14.9 Keyboard Shortcuts
14.9.1 **Cmd/Ctrl+T** — New chat session.
14.9.2 **Cmd/Ctrl+W** — Close active session.
14.9.3 **Cmd+Shift+S** — Open skills palette.
14.9.4 **Cmd/Ctrl+P** — IDE quick-open file.
14.9.5 **Ctrl+S** — Save in AUTODEV.md editor.
14.9.6 **Esc** — Close modal, collapse expanded card.

### 14.10 Frontend Dependencies
14.10.1 **Recharts** — Line, bar, pie, and donut charts.
14.10.2 **Prism syntax highlighter** — Code highlighting in tool result cards.
14.10.3 **ansi-to-react** — Renders terminal ANSI color codes inside the chat.
14.10.4 **date-fns** — Lightweight date formatting (e.g., "2 hours ago").
14.10.5 **lucide-react** — The icon library used throughout the UI.
14.10.6 **Monaco editor** — The VS Code editor, used in the IDE tab.

---

## 15. Operational Hardening (from End-to-End Runs)

> These features were designed and implemented in response to live fleet runs (notably the 5 May, 6 May, and 14 May 2026 calendar-app end-to-end exercises). They are real shipped behaviors, not aspirational items. Each one closes a specific class of failure the team observed in production, so the list reads as a catalogue of safety nets, visibility tools, and policy primitives.

### 15.1 Workspace Path-Jail (Escape Prevention)
15.1.1 **The problem this solves** — During a real fleet run, a Worker agent's frontend recipe wrote three project files into AutoDev's own source tree instead of into the user's project folder. The agent had been told the right workspace in plain English inside the prompt, but no code was enforcing it. AutoDev now refuses to ever let that happen again.
15.1.2 **What a path-jail is** — A small, load-bearing safety check that every file-modifying tool runs before it touches the disk. It takes the path the agent is about to write to, expands it to a full absolute path (resolving things like `..` and symbolic links), and verifies the result is still inside the configured workspace. If it is not, the tool refuses the call and returns a message the agent can read and self-correct from.
15.1.3 **Every writing tool is jailed** — `write_file`, `edit_file`, `append_file`, and the shell tool's working directory are all gated by the same check. There is no back-door: a tool cannot opt out, and the check runs *before* any file is opened, created, or read for diffing.
15.1.4 **Shell redirect target heuristic** — The bash tool also scans the command string for output redirects (`>`, `>>`, `tee <file>`) and runs each redirect target through the same jail. This is a best-effort defense — it does not parse here-docs or process substitution — but it catches the common case where a shell command would otherwise drop a file outside the workspace through the side-door of a redirect.
15.1.5 **Windows-aware path comparison** — On Windows, paths are compared case-insensitively and only at component boundaries, so a sibling folder like `C:\foobar` is not mistakenly accepted just because the workspace is `C:\foo`. The Windows verbatim prefix `\\?\` is stripped before comparison so two equivalent paths do not falsely disagree.
15.1.6 **Tolerant of files that do not yet exist** — Since the agent is often *about to create* a new file, the jail walks up to the nearest existing ancestor folder, canonicalizes that, and re-attaches the remaining path. The check still succeeds for a not-yet-existing path inside the workspace; it still fails for a not-yet-existing path outside it.
15.1.7 **Refusal message is self-correcting** — When the jail rejects a path it returns a structured error naming both the offending resolved path and the workspace root, then tells the agent how to retry (use a relative path or an absolute path inside the workspace). This wording is designed for the LLM to read and immediately fix, rather than burn a turn confused.
15.1.8 **Workspace-mismatch banner** — At app startup, AutoDev compares the current working directory the app launched from against the workspace the user configured. If they differ, a dismissible warning banner appears across the top of the window explaining the two paths and offering a "Set workspace" button that reopens the Settings → Workspace flow.
15.1.9 **Stray-files banner** — At startup AutoDev also walks its own source folders (depth-limited to avoid scanning huge build trees) and looks for files that look like worker-generated artifacts (progress logs, scaffold files, React components in places where they cannot belong). If any are found, a dismissible banner lists them with size and reason so the user can investigate or clean up.

### 15.2 Per-Task Headless QC Dispatcher
15.2.1 **The problem this solves** — A single QC Agent reviewing tasks one at a time becomes a bottleneck the moment several Workers finish in close succession. The fleet ends up with a queue of "ready to review" tasks waiting on one reviewer. AutoDev now fans out QC review across many short-lived helper agents in parallel.
15.2.2 **The QC dispatcher recipe** — A lightweight recipe that runs on a fast cadence whose only job is to look at the task board, find tasks waiting in `qc_review`, and spawn a fresh ephemeral reviewer agent for each one. The dispatcher itself does no reviewing — it allocates work.
15.2.3 **Ephemeral QC reviewer agents** — Each spawned reviewer is a one-shot subagent dedicated to a single task. It reads the diff, reads the task's expected outcome, runs whatever checks it needs, then either approves the task or rejects it with a written reason. Once the verdict lands in the database, the subagent terminates and is reclaimed.
15.2.4 **Concurrency cap (default 6)** — At most six reviewers are in flight at once. The cap protects the user's wallet (every reviewer costs tokens), the LLM provider's rate limits, and the local machine's CPU. The cap is conservative on purpose and can be raised when needed.
15.2.5 **Cost guard ($5/hour ceiling)** — The dispatcher tracks the rolling per-hour cost of QC review across all spawned reviewers. When the running total approaches a configurable ceiling (default $5/hour), the dispatcher pauses new spawns until the window slides forward. This stops a runaway loop of rejections-and-retries from burning a credit card while the user is asleep.
15.2.6 **`under_review_by` task column** — The task table has a column recording which reviewer subagent (if any) currently owns a given task. The dispatcher uses this to avoid double-spawning a reviewer for a task that is already being looked at, and the Kanban board uses it to show a small avatar of the reviewing subagent on the card.
15.2.7 **`task_qc_cost_usd` task column** — Each task records the cumulative dollar cost of all QC review attempts against it. Tasks that have been rejected and re-reviewed several times accumulate this number, and the board can show a small cost chip on cards that have already cost more than they probably should.
15.2.8 **Stranded reviewer reconciler** — If a reviewer subagent crashes or its host process dies mid-review, the orphan-detector notices the dangling `under_review_by` reference, clears it, and returns the task to the front of the QC queue so the next dispatcher tick picks it up.
15.2.9 **QC ROI panel** — A small dashboard widget shows the cumulative ROI of the QC system: tasks approved per dollar spent, average review time, the rejection rate, and a sparkline of QC cost over time. This lets the user judge whether the cost guard ceiling needs raising or whether reviewer prompts need tightening.
15.2.10 **`spawn_qc_review` builtin tool** — A first-class tool the dispatcher uses to spawn one reviewer subagent against one task. Internally it wires up the meta-arguments (working directory, session id, the task id under review) so the reviewer starts with everything it needs and writes its verdict back through the standard task-update path.
15.2.11 **Rejected task feedback loop** — A rejected task gets a structured `rejection_reason` written to it, a `retry_count` incremented, and goes back to a "Rejected" column on the board. The Supervisor recipe re-dispatches rejected tasks back to the same Worker that produced them, so the same agent gets the chance to fix its own work with the reviewer's feedback in hand.

### 15.3 Build Evidence Gate (No QC Without a Real Build)
15.3.1 **The problem this solves** — A Worker can write code that "looks right," declare the task complete, and push it to QC review. If the project never actually builds, the reviewer is reading dead code. The build evidence gate refuses to let a Worker move a task to `qc_review` unless that Worker has recently produced a successful build.
15.3.2 **The session-keyed ring** — AutoDev keeps a small in-memory ring buffer of recent build outcomes, indexed by the Worker's session id. Whenever the Worker's shell runs a build command (cargo build, npm run build, etc.) and the build exits cleanly, the success is stamped into the ring with a timestamp and the command that produced it.
15.3.3 **The "EXIT:0" sentinel** — The gate looks for an explicit `EXIT:0` marker from a recent enough build to count as fresh evidence. Stale evidence (older than the configured window) does not count, and a build that printed warnings but returned non-zero does not count.
15.3.4 **Refusal at the status update boundary** — When the Worker tries to call `update_task_status(id, "qc_review")` without recent build evidence in the ring, the tool refuses with a structured message explaining why and instructing the Worker to run its build, see it pass, then retry the status change. This makes the build a *prerequisite* of moving work forward rather than a polite suggestion.
15.3.5 **Build_failed task status** — Tasks that fail their build attempt are visibly marked with a dedicated `build_failed` status, so the Kanban board surfaces them rather than letting them silently stall.

### 15.4 Backend ↔ Frontend Interface Contract
15.4.1 **The problem this solves** — In a fleet where the backend Worker and the frontend Worker are separate agents working in parallel, the two are extremely good at drifting out of sync — one renames an endpoint, the other still calls the old one, and nothing notices until a human runs the app. AutoDev now makes the interface between them a first-class, machine-checkable artifact.
15.4.2 **`INTERFACE.md`** — A markdown document at the root of the project that captures the agreed-upon API contract between backend and frontend: endpoints, request and response shapes, authentication assumptions, error codes. It is written in a structured-enough format that it can be parsed, but readable enough that a human reviewer can sanity-check it at a glance.
15.4.3 **`write_interface_spec` tool** — A builtin tool the Supervisor uses to author or update `INTERFACE.md`. The Supervisor decides the contract once, writes it through this tool, and from that point on both Workers consult the same document.
15.4.4 **Interface drift detection in diff analysis** — Whenever a Worker is about to commit a change that touches API surface, the diff analyzer compares the change against the recorded `INTERFACE.md` and flags drift: a removed endpoint, a renamed field, a changed type. The drift is surfaced to QC as a structured signal that pre-empts the "we shipped, but the two halves do not match" failure mode.
15.4.5 **Tier 2.6 API contract validation** — The third validation tier ("types") is extended with a sixth sub-tier that specifically validates frontend code's API calls against the backend's declared contract. A frontend that calls `/api/v1/events` when the backend exposes `/api/v2/events` fails the check loudly.
15.4.6 **Tier 2.5 export cross-reference** — A pure-Rust import↔export matcher that scans the project, indexes everything each module exports, and reports any import that does not resolve to a known export. This catches the common "we imported a symbol that does not exist" class of build break before the slow compiler does.

### 15.5 Fleet Smoke Recipe & End-to-End Probe
15.5.1 **The problem this solves** — Even after all the per-task checks pass, the only true test that a built application *works* is to actually run it and confirm it serves a request. AutoDev now bakes that into a first-class recipe and tool.
15.5.2 **Fleet smoke recipe** — A scheduled recipe that boots the user's application (using the project's start command), waits a moment for it to become reachable, and confirms it responds before tearing it back down. The recipe is the canonical "is the whole thing actually alive?" check, and it runs on its own cadence alongside the other recipes.
15.5.3 **`e2e_probe` builtin tool** — A timeout-wrapped HTTP GET that the smoke recipe uses to hit a known endpoint and assert that the response body contains a specific expected substring. It is intentionally minimal: one URL, one expected-text match, one short timeout. Pass means alive; fail means the smoke recipe records a failure with the exact error for the Supervisor to act on.

### 15.6 Real-Time Worker Visibility (Live Monitor Upgrades)
15.6.1 **The problem this solves** — In a real run the Live Monitor would display "Idle" for a Worker that was in the middle of a long Mode 2 invocation, because the tool calls happening inside the Orchestrator's LivingAgent session were not being relayed back to the desktop app in real time. The user had no way to see what the Worker was actually doing.
15.6.2 **Mode 2 streaming via `invoke_worker_streaming`** — The desktop app now invokes Mode 2 Workers through a streaming endpoint that emits server-sent events as the LivingAgent runs. Every tool call, tool result, and intermediate model output is bridged into the same in-app event channel the Live Monitor already listens on, so a Mode 2 worker's activity scrolls in the UI exactly the same as a Mode 1 worker's.
15.6.3 **Lazy slug registration** — When an event arrives for a Worker the frontend has never heard of, the agent live store auto-creates a record for it on the fly rather than dropping the event. This eliminates a class of "the agent ran but never appeared in the UI" bugs that came from the frontend being initialized before the backend had registered every Worker.
15.6.4 **Worker requests panel state split** — The Worker Requests dropdown was originally driven by a single piece of state that tried to mean both "the panel is open" and "this specific request row is expanded." Those two intents are now two pieces of state, so opening the panel does not auto-expand the wrong row and expanding a row does not collapse the panel.

### 15.7 Sidecar Crash Diagnostics
15.7.1 **The problem this solves** — When the Core Orchestrator dies, the desktop app already knew how to fall back to Mode 1, but the user had no easy way to see *why* the Orchestrator died. The Python traceback lived in a sidecar log file and required digging.
15.7.2 **Sidecar crash banner** — When the auto-restart circuit breaker opens (the Orchestrator has crashed too many times in a row), AutoDev displays a danger-tinted banner at the top of the window with a one-line summary, the restart count, the window over which the crashes occurred, and the exact timestamp.
15.7.3 **Captured stderr tail** — The banner folds the last forty lines of the crashed sidecar's standard-error output into a click-to-expand block. The Python traceback (or whatever the cause turns out to be) is one click away rather than a hunt through log files.
15.7.4 **"Copy crash report" button** — A single button bundles the stderr tail, the sidecar id, the restart count and window, an ISO timestamp, and the host's user-agent string into a markdown-formatted report and copies it to the clipboard. The button briefly flips to "✓ Copied" so the user knows it worked. The intended workflow is paste-into-issue-tracker.

### 15.8 Per-Recipe Fallback Policy
15.8.1 **What a fallback policy is** — Every recipe now declares whether it is safe to run on the lightweight Mode 1 engine when Mode 2 (Core Orchestrator) is unreachable. The two policies are `Mode2Only` (the default) and `Mode1Capable`.
15.8.2 **`Mode2Only` recipes** — Recipes that depend on MCP tools, durable memory, or the agent-to-agent protocol cannot meaningfully run on the Rust engine, so when Mode 2 is down they record a structured "fallback skipped per policy" outcome and set the Worker's status to `fallback_idle`. The board shows them as gracefully paused, not failed.
15.8.3 **`Mode1Capable` recipes** — Recipes that only need the basic builtin tool set (e.g. a cleanup recipe that just runs `cargo fmt`) are allowed to drop down to the Rust engine and continue working. Their Worker status shows as `mode1_active` in amber so the user can see at a glance that the fleet is operating in degraded mode.
15.8.4 **Backward-compatible defaults** — Existing recipes on disk that do not declare a policy deserialize as `Mode2Only` without any migration step. Promoting a recipe to `Mode1Capable` is a deliberate four-item checklist (no MCP-only tools used, prompts make sense without memory, etc.) documented in a Mode 1 fallback recipes guide.

### 15.9 Worker Idle Pre-Flight
15.9.1 **The problem this solves** — A Worker that wakes up every two minutes to check the task board and finds nothing for it to do should not call the LLM at all. AutoDev now short-circuits these "nothing to do" ticks before any expensive token spend.
15.9.2 **Idle pre-flight check** — Before the runner invokes the Worker's LLM, it queries the task board for anything assignable to that Worker. If there is genuinely nothing, the tick is logged as idle, the Worker's status is set accordingly, and the runner returns. No prompt is built, no API call is made, no tokens are spent.
15.9.3 **Pre-flight tool capability check** — A second pre-flight runs after the agent is ensured but before the invoke fires: it asks the Orchestrator how many tools the LivingAgent has. If the answer is zero (which is what happens when the MCP bridge has broken), the runner refuses to burn tokens on a tool-less Worker and forces the Mode 1 fallback path immediately.

### 15.10 TCP-Alive Health Probe (Smarter Fallback)
15.10.1 **The problem this solves** — The Core Orchestrator can be alive but very busy serving a long-running tick, in which case its health-check endpoint stops responding for tens of seconds. The old fallback logic would declare it dead and switch the entire fleet to Mode 1 unnecessarily.
15.10.2 **Two probes, not one** — The watchdog now runs two probes side by side: the existing HTTP `/health` request, and a raw TCP probe of the Orchestrator's port. Both have to fail before the watchdog declares the Orchestrator dead.
15.10.3 **The "busy not dead" pattern** — When `/health` has failed six times in a row but the TCP probe still connects, the watchdog logs that the Orchestrator is busy and *keeps the fleet on Mode 2*. When the TCP probe also fails, only then does it switch to Mode 1. This eliminates almost all premature fallbacks during real long-running ticks.
15.10.4 **No double-execution risk** — The runner also enforces a "do not silently fall back from a failed Mode 2 invocation to Mode 1 mid-tick" rule. If a Mode 2 invocation is in flight when the connection drops, the tick is aborted and re-tried on the next cron beat instead of being retried on the Rust engine, which could otherwise re-execute side effects.

### 15.11 Structured Log Error Taxonomy
15.11.1 **The problem this solves** — When something fails in production, a human reading the logs has to spot the failure among thousands of routine lines and then guess at the category. AutoDev now tags every recoverable failure with a stable, grep-friendly identifier.
15.11.2 **`kind=` field on every error log line** — Critical error sites emit a structured `kind=` field naming the class of failure. For example, an invoke failure logs `kind=invoke_failed`, a watchdog-triggered restart logs `kind=watchdog_restart`, an Orchestrator crash logs `kind=orch_crash`.
15.11.3 **Nine canonical kinds** — A central error-kinds module defines nine constants covering the known recoverable-failure classes. New kinds can be added but must be unique and snake-case, enforced by a unit test that fails the build if a duplicate or malformed kind is introduced.
15.11.4 **`LOG_ERROR_KINDS.md` reference** — A developer-facing document maps each kind to its meaning, the triage owner, and the first source location that emits it, plus a four-step process for adding a new kind. This is the canonical "what does this error tag mean?" reference.
15.11.5 **QC decision events as toasts** — A dedicated `qc:decision` event fires whenever the QC agent (or a reviewer subagent) approves or rejects a task. The frontend turns this into a visible toast — green check with task title for approvals, red warning with the rejection reason for rejections — so the user gets immediate feedback when the QC tier acts.

### 15.12 Log Retention & Rotation
15.12.1 **Daily rotation** — The desktop app's main log file rolls over automatically at UTC midnight to a new dated file (`autodev-YYYY-MM-DD.log`). Old days do not interfere with today's log, and `tail` against the live file always sees just one day's worth of events.
15.12.2 **Fourteen-day default retention** — At startup, a retention pass walks the logs folder and deletes any matching log file older than the retention window. This prevents the logs folder from quietly growing forever on a long-running install.
15.12.3 **`AUTODEV_LOG_RETENTION_DAYS` environment override** — The retention window is a single environment variable away from being changed. Setting it to a larger value keeps more history; setting it to `0` disables deletion entirely (for example for a customer support scenario where you want logs preserved indefinitely).
15.12.4 **Defensive filename matching** — The retention sweeper only deletes files whose names match the canonical `autodev-…log` pattern. Any other file the user happens to have stashed in the logs folder is left alone.
15.12.5 **Single summary line** — The sweep emits one info-level line per startup: how many files were dropped, how many bytes were reclaimed, and how many errors (if any) occurred. The summary lands in the freshly opened daily file so a future debugger can correlate retention activity with whatever else happened that day.
15.12.6 **Cron-slip warnings demoted under five seconds** — Sub-five-second scheduler slips were previously logged as warnings, polluting the INFO stream. Slips that small are now logged at DEBUG level; only genuine multi-second slips remain visible at the default verbosity.
15.12.7 **Cached runtime config** — The runtime-configuration loader now reads from disk and logs its config exactly once per host lifetime (via a one-shot cache) instead of once per caller. This removes a noisy class of repeated info lines that previously dominated the log.

### 15.13 Service Auto-Start Robustness
15.13.1 **Redis / Memurai standalone binary fallback** — The sidecar manager's auto-start used to try only Windows-service commands (`net start`, `sc start`). When Memurai is installed as a standalone binary rather than registered as a service, those commands fail silently. The fallback now also checks well-known binary install paths (`C:\Program Files\Memurai\memurai.exe`, the Chocolatey path) and spawns the binary directly if found, with the spawned process tracked so it gets cleaned up on app shutdown.
15.13.2 **Localhost-trust for the in-app MCP server** — The in-process MCP server originally required a bearer-token authorization header on every request, with the token persisted to disk and propagated through a YAML config to the Orchestrator. Because the server only ever listens on `127.0.0.1` and only its sibling process inside the same app talks to it, this auth was pure friction. It has been removed in favor of localhost-only trust, eliminating an entire class of 401-Unauthorized cascade failures.
15.13.3 **Subagent default-model resolution** — When the LLM omits the `model` field while spawning a subagent, the subagent manager now resolves a sensible default (the fleet's currently active model) instead of forwarding an empty string that the Orchestrator would reject. This closes the silent-failure path where a subagent would be created, immediately fail with "Model '' not found in registry," and disappear from the caller's view.

### 15.14 Dashboard Responsiveness
15.14.1 **Sixty-second board refresh** — The AutoDev panel's auto-refresh interval was previously three minutes, which is a long time to wait when a fleet run is producing visible state every few seconds. It is now sixty seconds, giving a comfortable cap on how long the board can be stale even when the event-driven path is not firing.
15.14.2 **Loud failure when events cannot fire** — The internal `emit_task_changed_global` helper used to fail silently if the Tauri app handle was not yet installed. It now emits a warning so a developer reading the log can spot the race condition where a task DB write happens before the frontend listener has mounted, rather than blaming "the board is stale" on the polling interval.

---

## 16. Ruflo-Inspired Features

> A separate body of work brings ideas from Ruflo (a different agent-orchestration project the AIRS team has studied) into AutoDev, adapted to the Tauri/Rust/React stack. All of these features were implemented inside AutoDev's own code base — none of them required changes to the bundled Core Orchestrator or Memory MCP Server. They add learning, safety, planning, and visibility capabilities the fleet did not previously have.

### 16.1 Semantic Task Routing (Worker Affinity Learning)
16.1.1 **The idea** — Instead of the Supervisor picking which Worker handles which task using fixed keyword rules forever, AutoDev *learns* over time which Worker actually performs best for which kind of task. A Worker that historically nails backend tickets gets preferred for the next backend ticket; a Worker that has a 30% rejection rate on frontend tickets gets routed around when there is a stronger candidate.
16.1.2 **Routing history table** — Every time the Supervisor assigns a task, the assignment is recorded along with the task type, the Worker chosen, the timestamp, and (once the task finishes) the outcome (success, failure, or reassigned), how long it took, and how many tokens it consumed. This becomes the dataset the router learns from.
16.1.3 **The static fast path** — Before any historical lookup, the router checks a small set of hand-written keyword rules ("backend", "fastapi", "pydantic" → worker-backend, etc.). These resolve in well under a millisecond and handle the common cases instantly without touching the database.
16.1.4 **The historical layer** — When the static layer is uncertain (or the static answer disagrees with the historical track record), the router queries the routing-history table and computes a success rate per Worker per task type. A Worker needs at least three historical entries of a given type to count; otherwise it gets a neutral score so brand-new Workers are not starved of work.
16.1.5 **The scoring formula** — Each Worker's score for a task type is 60% its success rate and 40% the inverse of its average duration ranking (faster is better, as long as it succeeds). Scores are normalized across active Workers and the highest scorer wins.
16.1.6 **The override rule** — A high-confidence static match wins outright *unless* the same Worker has a poor historical score for that pattern, in which case the router defers to history. This prevents the system from forever sending obviously-backend tasks to a Worker that has demonstrably been bad at backend.
16.1.7 **`suggest_worker` MCP tool** — Exposed as a first-class MCP tool the Supervisor uses before each `assign_task`. The tool returns a Worker slug and a confidence score, and identifies which method (static, historical, or default) produced the recommendation so the Supervisor can reason about it.
16.1.8 **Automatic outcome wiring** — When any task transitions to `done`, `failed`, or is reassigned, the corresponding routing-history row's outcome is filled in automatically. The Supervisor does not have to remember to "close the loop" — the system does it.
16.1.9 **Worker Affinity Stats panel** — A collapsible table on the AutoDev panel shows, per Worker, how many tasks of each type it has completed, the success rate per type, and an overall success rate. This lets the user see at a glance which Worker is actually good at what, instead of just trusting the routing decisions blindly.

### 16.2 PID-Based Orphan Detection
16.2.1 **The problem this solves** — If a Worker process dies mid-task (segfault, OOM kill, user closing the app uncleanly), the task it was working on is left marked as `in_progress` with a Worker assigned. Without intervention, the old system waited four hours before a stale-lock timeout released the task. AutoDev now releases it within the next Supervisor poll (roughly two minutes).
16.2.2 **PID columns on the tasks table** — When a Worker claims a task, the task row records the operating-system process id of the Worker, plus the hostname of the machine the Worker is running on. This turns "is this Worker still alive?" from a guess into a check.
16.2.3 **Cross-platform PID liveness check** — The detector uses the `sysinfo` crate to ask the operating system whether a given PID is currently a running process. Works identically on Windows, macOS, and Linux, with no unsafe direct OS calls.
16.2.4 **PID reuse protection** — If the PID is alive but the process at that PID is something else entirely (the OS recycled the number), the detector verifies the process name looks like AutoDev or `airsctl` before trusting that the Worker is genuinely still working. A mismatch counts as orphaned.
16.2.5 **TTL fallback for legacy tasks** — Tasks that were claimed before the PID columns existed (or by code paths that did not record a PID) still get the original four-hour stale-lock TTL as a backstop, so nothing is left hanging indefinitely.
16.2.6 **`claim_task` MCP tool** — A new first-class MCP tool that Workers call to take a task. Internally it records the Worker's PID and hostname alongside the status change, so the orphan detector has the data it needs from the moment the task is claimed.
16.2.7 **Hooked into the Supervisor poll** — Orphan reconciliation runs on the same cadence as the Supervisor's normal poll. Every two minutes the detector sweeps all claimed tasks, checks the recorded PIDs, releases anything that is genuinely abandoned, and fires a `task:orphans-released` event so the board updates immediately.
16.2.8 **`force_release_task` command** — A manual override exposed to the UI so the user can release a stuck task by hand from a small Orphan Alert toast or the task card itself, without waiting for the next reconciliation tick.
16.2.9 **Orphan alert toast** — When the detector releases orphans, a small toast appears on the AutoDev panel ("⚠️ 2 tasks released — worker process exited unexpectedly. Tasks returned to Ready queue.") and auto-dismisses after ten seconds so the user notices but is not nagged.

### 16.3 Diff Risk Scoring for QC Agent
16.3.1 **What `analyze_diff` is** — A dedicated tool that turns a raw git diff into a structured analysis: a risk score, a change classification (feature, bugfix, refactor, security, etc.), a one- or two-sentence human-readable summary, a list of specific review notes, and a list of concrete checks the reviewer should run. The QC Agent calls this once per PR and uses the output to drive its decision.
16.3.2 **Two-pass design (structural + LLM)** — The first pass parses the diff in pure Rust and computes a *structural* risk score from facts alone: total churn, file count breadth, paths touched, deletion-to-addition ratio, whether config files or binary assets are changed. No tokens are spent. The second pass makes a single LLM call to classify the diff and produce the prose summary. Splitting the work this way keeps the expensive part minimal and keeps the structural signal reliable.
16.3.3 **The risk factors** — Five hand-picked factors contribute to the structural score: high churn (over 500 lines), critical paths (`auth`, `security`, `migration`, `db/`, `schema`), file breadth (over ten files), config or binary changes (`.env`, `.toml`, `.yaml`, `Dockerfile`), and deletions-heavy ratios. Each factor adds a weighted contribution; the total is clamped to 1.0.
16.3.4 **Four risk levels** — The score maps to four buckets: low (<0.25), medium (<0.50), high (<0.75), critical (≥0.75). The QC recipe's instructions key off these levels — critical diffs are flagged for human review and never silently approved.
16.3.5 **Compact LLM prompt** — The classifier does *not* send the full diff text (which would burn tokens on a long PR). It sends a structured summary: the file list with line counts, the key changed paths, the structural risk factors, and a small sample of added and deleted lines. This keeps the call cheap even for very large changes.
16.3.6 **Suggested checks** — The classifier returns a `suggested_checks` array, which the QC agent treats as its test plan. For example, a diff that touches auth middleware gets a suggested check like "verify token validation still rejects expired tokens." The Worker doing the QC review then runs each of those checks before deciding.
16.3.7 **`analyze_diff` MCP tool** — The capability is wired into the MCP server as a Supervisor- and QC-allowed tool with a clean schema (repo path, base branch, head branch in). Internally it generates the diff using the existing git module, then hands off to the analyzer.
16.3.8 **Diff Analysis card** — When a task is in `qc_review` and a diff analysis is available, the task card on the board renders a small `DiffAnalysisCard` showing the colored risk badge, the classification tag, the summary text, and a collapsible list of review notes. The card persists alongside the task so a user can see why QC made its call.
16.3.9 **Analysis stored with the task** — The structured analysis JSON is saved into a `diff_analysis` column on the tasks table so the same analysis can be re-displayed without re-running the LLM call, and so the audit trail of QC's reasoning is preserved.

### 16.4 Budget Circuit Breaker
16.4.1 **What it does** — Each task carries a budget — a maximum token count, a maximum dollar cost, and a maximum number of agent turns. The agent engine checks the budget before *every* turn and halts the loop the moment any limit is exceeded, instead of silently burning through tokens on a Worker that got stuck in a loop.
16.4.2 **Defaults** — Worker tasks default to 100,000 tokens, $0.50, and twenty turns. Supervisor tasks default to 50,000 tokens, $0.25, and ten turns. Both can be overridden per-task by the Supervisor. An explicit "unlimited" budget exists for the rare task that genuinely should run unbounded, but even unlimited budgets keep the turn cap as a final guard.
16.4.3 **Input validation** — Setting a budget rejects `NaN`, infinity, negatives, and values above a hard ceiling (1,000,000 tokens, $100, 100 turns). Errors are returned as opaque constant strings (`BUDGET_INVALID_TOKENS`, `BUDGET_LIMIT_EXCEEDED`) rather than echoing back the rejected number, which mirrors a defensive pattern from the Ruflo reference implementation.
16.4.4 **Pre-emptive halt in the agent loop** — At the start of every turn, the engine increments the turn count, checks the cumulative budget, and aborts the loop with a specific reason (`BUDGET_EXCEEDED_TOKENS`, `BUDGET_EXCEEDED_USD`, or `BUDGET_EXCEEDED_TURNS`) before any LLM call goes out. After each LLM response, the cumulative token and cost counters are updated and persisted to the task row.
16.4.5 **Escalation to `needs_info`, not failure** — A task that hits its budget is *not* marked failed. It is moved to the existing `needs_info` status with a `WorkerRequest` automatically filed against the Supervisor explaining that the Worker halted at *n* tokens / *$x* USD and asking for either a budget increase or a scope reduction. The Supervisor decides; the Worker waits. This treats budget exhaustion as a planning problem, not a defect.
16.4.6 **Supervisor `set_task_budget` MCP tool** — A first-class tool the Supervisor uses to attach a budget to a task before assigning it. Budgets travel with the task; the assigned Worker sees and is enforced against them.
16.4.7 **Budget bar in the UI** — Task cards show a `BudgetBar` widget while the task is active or in `needs_info`: green when under half the budget, yellow at half, orange at 80%, red above 95%, and a hard "EXCEEDED" badge when the breaker has tripped. Cost and tokens are shown together.
16.4.8 **Settings default budgets** — A Settings group lets the user set the fleet-wide default per-task budget (tokens, USD, and a warning threshold) so they do not have to remember the constants. Defaults persist in the runtime configuration file.
16.4.9 **Daily budget summary** — A dashboard-side command aggregates all task budgets across the day: total tokens allocated, total dollars allocated, total spent, and the list of tasks that hit their limits. This gives the user a single number for "how much could the fleet possibly cost today?"

### 16.5 Phase-Based Task Decomposition
16.5.1 **What it does** — When the Supervisor decomposes a big piece of work (an "epic") into subtasks, it can now organize those subtasks into named phases — Foundation → Core → Integration → QA, for example. Subtasks in a later phase do not start until the prior phase reaches its milestone, which gives the user a visible, ordered progress arc instead of a flat soup of independent tickets.
16.5.2 **Phase columns on the task row** — Each task has optional `phase` (the name), `phase_order` (1, 2, 3, 4 — controls sequencing), `milestone` (an optional label that fires when the phase is complete), and `epic_id` (which epic this task belongs to). Tasks without these columns behave exactly as before; the phasing is fully opt-in.
16.5.3 **Blocker dependencies** — Each task can also list a `blocked_by` array of task IDs that must be done before this one can start. Workers cannot pick up a task while any blocker is open. This is a separate mechanism from phases and can be used inside or outside an epic.
16.5.4 **Automatic unblock on completion** — When a task is marked done, the database checks every task that listed this one as a blocker. Anything that is now fully unblocked fires a `task:unblocked` event, and the scheduler immediately kicks the relevant Workers so freshly-unblocked work starts within the next tick rather than waiting for the next cron beat.
16.5.5 **Automatic phase completion** — When the last task in a phase finishes, a `task:phase-complete` event fires carrying the epic id, the phase name, and the milestone label. The UI uses this to celebrate the milestone and to surface the fact that the next phase has just unlocked.
16.5.6 **`Epic` task type** — Epics are represented as a special task type rather than a separate table. An epic has no assigned Worker and no phase; its subtasks reference it through `epic_id`. When all subtasks complete, the epic auto-transitions to done, so the epic record itself becomes a natural marker of "this entire body of work is finished."
16.5.7 **Supervisor MCP tools for planning** — Four tools are exposed: `create_epic`, `create_phased_task` (with blockers), `get_epic_progress` (returns the phase summary), and `get_ready_tasks_in_phase` (so the Supervisor can find what's assignable right now without scanning the whole board). These are the building blocks the Supervisor uses to lay out a multi-phase plan.
16.5.8 **Epic Phase View** — A dedicated UI component renders the epic's progress as a list of phase rows with progress bars, percent complete, and status indicators (✅ complete, 🟡 in progress, 🔒 blocked). Clicking a phase expands the individual task cards within it. Live updates come from the `task:phase-complete` and `task:unblocked` events so the view stays current without polling.
16.5.9 **Epics tab on the AutoDev panel** — Alongside the existing Kanban columns, the task board has a dedicated "Epics" view that lists every epic with its phased progress at a glance. Individual task cards that belong to an epic also carry a small badge that deep-links into the epic phase view.

### 16.6 MCP Connection Health Monitor
16.6.1 **The problem this solves** — When a user has configured external MCP servers (GitHub, Slack, a custom toolserver), there is no built-in signal when one of them drops. The fleet just silently fails on its next tool call to that server. The health monitor turns dropped MCP connections into a visible, actionable state.
16.6.2 **Per-server health record** — For each configured MCP server, the monitor tracks the connection state (Connected, Degraded, Disconnected, Reconnecting), the timestamp of the last ping, the timestamp of the last *successful* ping, a count of consecutive failures, an average latency, and the last error message if any.
16.6.3 **Periodic ping loop** — Every thirty seconds (configurable) the monitor pings every registered MCP server. For HTTP/SSE servers it sends a small health request; for stdio servers it sends a JSON-RPC `ping`. Both have a five-second timeout. Latency is recorded on success; the failure type (timeout, connection refused, invalid response) is recorded on failure.
16.6.4 **State transitions** — After three consecutive failures, the server flips from Connected to Disconnected and a `mcp:connection-lost` event fires. If a request succeeds but takes more than two seconds, the server flips to Degraded and a `mcp:connection-degraded` event fires. Successful pings reset the failure counter and return the server to Connected.
16.6.5 **Auto-reconnect with exponential backoff** — A Disconnected server enters Reconnecting and is retried after a ten-second delay. HTTP servers can be revived by another successful ping; stdio servers (which may need their entire process restarted) emit a `mcp:requires-restart` event so the UI can prompt the user. Repeated failures use exponential backoff up to a five-minute ceiling.
16.6.6 **Live registry updates** — When the user adds an MCP server in Settings the monitor is told about it in real time and starts pinging on the next cycle; when they remove a server the monitor unregisters it. No restart needed.
16.6.7 **MCP Health Panel** — A sidecar-panel widget shows every registered MCP server with a colored status badge: 🟢 Connected with average latency, 🟡 Degraded with the slow latency, 🔴 Disconnected with the failure count and a "Reconnect" button, 🔄 Reconnecting with the current attempt. The Reconnect button forces an immediate retry instead of waiting for the next cycle.
16.6.8 **Status-bar MCP indicator** — The window's bottom status bar gets a small `⚠️ MCP` badge in orange or red whenever *any* configured server is Disconnected, so the user notices even if the Settings tab is not open. Clicking the indicator opens the health panel.

### 16.7 Browser Automation QC Skill
16.7.1 **What it adds** — A dedicated skill (`browser-test`) that gives the QC Agent the ability to run visual-regression and end-to-end tests against frontend changes by driving a browser through Playwright. This is *not* a built-in agent tool — it is a markdown skill the agent invokes — which means it requires no changes to the agent engine, the tool registry, or the MCP server.
16.7.2 **Playwright availability check** — Because the skill depends on Playwright being installed, a small Rust command (`check_playwright_available`) verifies the install, surfaces the Playwright version, lists the available browsers (Chromium, Firefox, WebKit), and confirms `npx` is on the PATH. The UI uses this to know whether the skill can run.
16.7.3 **One-click browser install** — `install_playwright_browsers` invokes `npx playwright install <browser>` and streams the install log through the existing bash-tool streaming path so the user sees progress. Defaulting to Chromium covers most of the visual-regression use case.
16.7.4 **The skill content** — The system-installed `~/.autodev/skills/browser-test/SKILL.md` walks the agent through how to use the existing `bash` tool with Playwright commands: verify the install, take a screenshot of a URL, run a Playwright test file, run a specific project (browser), and shape the output (screenshot path + pass/fail summary + specific failures with selectors and expected-vs-actual values).
16.7.5 **QC recipe integration** — The QC recipe's instructions are extended so that for frontend PRs (Worker type `worker-frontend`, or files changed are `.tsx`, `.ts`, `.css`) the agent automatically considers invoking `/browser-test`. If the PR ships its own Playwright tests, the agent runs them; if not and risk is high, the agent runs Playwright against a configured staging URL.
16.7.6 **Browser Testing settings group** — A new Settings panel inside the QC settings shows the Playwright install status, an Install button, an editable Staging URL, and a toggle to enable browser testing for frontend PRs. The toggle defaults to off until Playwright is confirmed installed, and a warning banner appears if the dependency is missing.
16.7.7 **Skill capability flag** — Skills can declare a `requires_capability` list (`["playwright"]` in this case). The Skills Palette shows a small "⚠️ Requires setup" badge next to any skill whose capability is not currently available, so the user is never surprised by a skill silently failing because a dependency is not installed.
16.7.8 **System-skill protection** — The `browser-test` skill is marked `is_system = true` so it cannot be accidentally deleted from the Skills Palette UI; the user can disable it, but the canonical skill file is owned by AutoDev.

---

## 17. Deferred & Future Items

> These appear in the source documentation but are marked as planned, deferred, or partially implemented. They are tracked here so a future codebase reconciliation pass can confirm their state.

15.1 **Plan-management hook integration** — Hooks tied to plan lifecycle events are partly wired; full integration is pending.
15.2 **Auto-worktree per subagent** — Subagents currently inherit their parent's working directory; auto-creating a fresh worktree per code-modifying subagent is planned.
15.3 **Inline subagent display in chat messages** — Subagent monitor lives in a sidebar today; embedding it inline in chat is a small follow-up.
15.4 **Recipe rewrites to use subagents** — Worker recipes can use the subagent tools but standard recipes have not yet been rewritten to take advantage of them.
15.5 **Per-worker subagent cost tracking** — Subagent costs flow into analytics but aren't yet broken out per worker in the Dashboard.
15.6 **CLI/Tauri shared core** — The CLI uses HTTP to the orchestrator today; sharing the agent engine and tool registry with the Tauri app via a shared library is on the roadmap.
15.7 **Dedicated sidecar configuration editor** — Port and auto-start configuration per service is planned (the Demo Mode toggle covers the only current use case).
15.8 **Multi-line REPL input** — The CLI REPL is single-line today; multi-line input plus full command-tree tab completion plus syntax highlighting are deferred.
15.9 **Memory TTL on working-directory memories** — Per-memory time-to-live is documented; the Python-side implementation is pending.
15.10 **Memory namespace scoping by project hash** — Documented as the next deeper fix on memory contamination across projects.
15.11 **Tree-sitter symbol extraction** — IDE outline currently uses regex; a tree-sitter upgrade is planned.
15.12 **Two-checkpoint side-by-side comparator UI** — Backend ready, frontend UI deferred.
15.13 **Keyboard shortcut for undo / file-level checkpoint restore** — Backend ready, hotkey wiring deferred.
15.14 **Auto-restart with exponential backoff for the in-process MCP server** — The other sidecars have this; the in-process server is reliable enough that the backoff was deferred.
15.15 **AUTODEV.md filesystem sync toggle** — Optional sync from database to disk is planned.
15.16 **CLAUDE.md import / migration tool** — One-click import from CLAUDE.md format is planned.
15.17 **System prompt variant editor in Settings** — Editor for the three PA-mode prompts is planned.
15.18 **3D City Mode 4 — JSCity-style inner function nesting** — The city view's roadmap mentions a future enhancement where deeply nested functions render as sub-buildings inside their parent's roof, mirroring how JSCity handles nested closures. Not yet wired into the building generator.
15.19 **3D City labels with collision avoidance at street level** — The city's billboard label system already collision-avoids at district level; per-building label collision at street zoom is documented but pending.
15.20 **Cross-view "Open in IDE" parity** — Both the 2D graph and the 3D city expose an "Open in IDE" action. A small follow-up will unify the deep-link behaviour so jumping from one view to the other carries focus through automatically.
15.21 **Redis sidecar auto-spawn** — Today AutoDev only probes Redis/Memurai for liveness and assumes the user has it installed externally. A planned enhancement bundles a Redis binary directly with the installer and spawns it as a managed Tauri sidecar so warm-tier memory works out of the box without separate setup.
15.22 **Frontend "build_failed" colour mapping** — The `build_failed` task status exists in the database and on the runner side, but the UI's task store needs an explicit color for it (currently it may render with a generic muted colour rather than a distinctive warning shade).
15.23 **Build-validation post-tick wiring** — The ecosystem detector and the sentinel-check helpers are in place. Wiring them into the runner's post-tick flow as an automatic guard requires a small refactor to avoid colliding with the stranded-task reconciliation step; this is queued for the next runner pass.

---

*This list is derived from the AutoDev master plan, the development plan, and the project README. Items appear as described in those documents. A later reconciliation pass will verify each item against the actual codebase and update statuses accordingly.*
