<div align="center">
  <h1>Iris · 伊人</h1>
  <p>Local-first AI photo culling for portraits, weddings and events. Desktop, CLI and MCP.</p>
  <p>
    <a href="LICENSE"><img alt="License: GPL v3 or later" src="https://img.shields.io/badge/License-GPLv3%2B-blue.svg"></a>
    <img alt="Platforms: Windows, macOS and Linux" src="https://img.shields.io/badge/Platform-Windows%20%7C%20macOS%20%7C%20Linux-lightgrey">
    <a href="https://github.com/UynajGI/Iris"><img alt="GitHub stars" src="https://img.shields.io/github/stars/UynajGI/Iris?style=social"></a>
  </p>
  <p><strong>Language:</strong> <a href="README.md">English</a> | <a href="zh-CN/README.md">简体中文</a></p>
  <p><a href="https://github.com/UynajGI/Iris/releases/tag/v0.1.0-beta">Download Beta</a> · <a href="docs/README.md">Documentation</a> · <a href="https://github.com/UynajGI/Iris/issues">Report an issue</a></p>
</div>

## 📋 Contents

* [Features](#-features)
* [Quick start](#-quick-start)
* [CLI and MCP](#-cli-and-mcp)
* [Development](#-development)
* [Documentation](#-documentation)
* [Topics](#-topics)
* [Contributing](#-contributing)
* [License](#-license)

Reviewing portrait sessions involves repeated checks for focus, eye state and similar frames. Iris provides local analysis and grouping to help you review those photographs. Models suggest; you make the final selection. Marking a photo as rejected does not delete the original file.

## ✨ Features

| Workflow | Capabilities |
| :--- | :--- |
| Browse and review | Overview, single-photo review and group comparison; JPEG, PNG, WebP and platform-dependent HEIC/RAW support |
| Assisted culling | Face and eye observations, sharpness and exposure scores, similar-photo groups and expandable analysis details |
| Mark and organize | Keep, undecided and reject decisions, star ratings, color labels, batch changes and undo |
| Export and recover | Copy, CSV and XMP export; quarantine preview, confirmation and restore |
| Agent access | Standalone stdio MCP binary with allowed-directory boundaries and write auditing; no desktop window required |

Inference runs locally. Release packages include the default models. Optional models require separate installation and license review, and are not enabled automatically. Scores are decision aids; no independently labeled evaluation supports an accuracy claim yet.

## 🚀 Quick start

**1. Download and extract.** Choose your OS and processor from [v0.1.0-beta](https://github.com/UynajGI/Iris/releases/tag/v0.1.0-beta). Extract the entire package to a writable directory and retain its folder structure. No source checkout or development toolchain is needed. The release includes `SHA256SUMS.txt` for download verification.

| Package | Requirements and beta scope |
| :--- | :--- |
| Windows x64 | WebView2 Runtime and Microsoft Visual C++ 2015–2022 x64 Runtime; includes HEIC runtime, ExifTool and the independent RAW converter |
| macOS arm64 / x64 | Apple Silicon or Intel respectively; unsigned and not notarized; HEIC and ExifTool are not bundled |
| Linux x64 | Ubuntu 22.04 baseline, GTK 3 and WebKitGTK 4.1; HEIC and ExifTool are not bundled |

**2. Launch from the extracted directory.** Windows: open PowerShell in that directory and run:

```powershell
powershell -NoProfile -File .\Launch.ps1
```

macOS: open `Iris.app`, or open Terminal in the extracted directory and run:

```bash
bash Launch.command
```

Linux: open Terminal in the extracted directory and run:

```bash
bash Launch.sh
```

macOS may require confirmation in Privacy & Security because this beta is not notarized. All release packages use CPU inference. macOS/Linux include JPEG, PNG and WebP analysis and bounded RAW fallback conversion. Automatic updates are not enabled. See the [release notes](docs/releases/v0.1.0-beta.md) for platform limits.

**3. Verify the workflow.** When the project screen opens, choose a folder of authorized photo copies, confirm the scan scope and start analysis. Results appear in the photo list and analysis details, where you can review and mark them. Native builds, CPU inference and package self-checks passed on all four CI targets; interaction on other computers remains part of beta testing.

## 📖 CLI and MCP

Packages include `iris-cli`, `iris-daemon` and `iris-mcp` (`.exe` on Windows). The CLI scans, analyzes, marks and exports; MCP connects through standard input/output to a compatible agent.

Windows, from the extracted directory:

```powershell
.\iris-cli.exe --help
.\iris-mcp.exe --help
```

macOS / Linux, from the directory containing the binaries:

```bash
./iris-cli --help
./iris-mcp --help
```

On macOS the binaries are inside `Iris.app/Contents/MacOS/`. See the [MCP guide](docs/mcp.md) for configuration, allowed directories and write operations. Technical guides are primarily in Chinese; commands and configuration keys are unchanged.

## 🔧 Development

Install Git, GNU Make, Rust stable, Node.js 22+, [uv](https://docs.astral.sh/uv/getting-started/installation/) and the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) for your OS. Use GNU Make in your terminal (`make --version`), including on Windows. Then run:

```bash
git clone https://github.com/UynajGI/Iris.git
cd Iris
make setup
make check
make test
```

Use the same commands in PowerShell on Windows. `make setup` prepares an isolated backend environment, installs locked dependencies and configures local Lefthook hooks. No environment activation is required. Successful commands return exit code zero.

The toolchain file selects native Rust stable. Windows GNU builds can select `RUSTUP_TOOLCHAIN=stable-x86_64-pc-windows-gnu`. Run `make help` for all tasks, `make models` for default models, and `make codegraph` to install optional project-local CodeGraph. Indexing is explicit with `make codegraph-init`. See the [tools index](tools/README.md) for builds, model provisioning and packaging, and the [desktop guide](apps/shell/README.md) for native development. Photographs, downloaded weights, runtimes and local reports are excluded from Git.

## 📚 Documentation

| Guide | Contents |
| :--- | :--- |
| [Documentation index](docs/README.md) | All topics and module maps |
| [Architecture](docs/architecture.md) | Core, daemon, CLI, MCP and Tauri boundaries |
| [Design](DESIGN.md) / [Product](PRODUCT.md) | Approved interaction, visual system and scope |
| [Validation](docs/validation.md) / [CI](docs/ci-verification.md) | Evidence and remaining coverage |
| [Release process](docs/releasing.md) | Versions, tags, native builds and publishing |
| [Status](docs/development-status.md) / [Handoff](docs/HANDOFF.md) | Implementation and remaining acceptance work |

## 📌 Topics

[`photo-culling`](https://github.com/topics/photo-culling) · [`photography`](https://github.com/topics/photography) · [`local-first`](https://github.com/topics/local-first) · [`rust`](https://github.com/topics/rust) · [`tauri`](https://github.com/topics/tauri) · [`mcp`](https://github.com/topics/mcp)

## 🤝 Contributing

Read [CONTRIBUTING.md](CONTRIBUTING.md) for changes and feedback, [SECURITY.md](SECURITY.md) for private vulnerability reports, and [AUTHORS.md](AUTHORS.md) for attribution. Pull requests should describe the resulting behavior, relevant checks and tests not run.

## 📄 License

Iris-owned code and documentation use **GPL-3.0-or-later**; see [LICENSE](LICENSE). Third-party code, the independent MIT RAW wrapper, fonts, icons and models retain their own licenses. See [third-party notices](THIRD_PARTY_NOTICES.md) and [licensing details](docs/licensing.md). The software is provided without warranty.
