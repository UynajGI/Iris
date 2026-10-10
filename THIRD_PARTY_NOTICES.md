# Third-party notices

Iris-owned application code and documentation are licensed under GPL-3.0-or-later.
This does not replace third-party licenses or copyrights.

| Material | License / notice |
| :--- | :--- |
| Standalone RAW wrapper | [MIT](components/raw-decoder/LICENSE) |
| rawler | LGPL-2.1; original package notices retained in the RAW source archive |
| Material Symbols Rounded | [Apache-2.0](apps/shell/src/ui/icons/LICENSE); provenance in the icon directory |
| Noto Sans SC / Noto Serif SC | SIL OFL 1.1; font package notices copied during frontend builds |
| React / React DOM / npm dependencies | Original package licenses; versions in lockfiles |
| Tauri CLI 2.12.1 NSIS template | MIT OR Apache-2.0; [provenance](apps/shell/installer/README.md), [MIT](apps/shell/installer/upstream/LICENSE-MIT.txt), [Apache-2.0](apps/shell/installer/upstream/LICENSE-APACHE-2.0.txt); Iris UI adaptations remain GPL-3.0-or-later |
| YuNet | [MIT](models/LICENSE-YuNet.txt) |
| MediaPipe conversions | [Apache-2.0](models/LICENSE-MediaPipe.txt); provenance limits in [model documentation](models/README.md) |
| BasicSR NIQE data and algorithm references | [Apache-2.0](models/LICENSE-BasicSR.txt) |
| ONNX Runtime | [MIT](models/LICENSE-ONNXRuntime.txt), [additional notices](models/ThirdPartyNotices-ONNXRuntime.txt) |
| Optional DINOv3 weights | [Meta DINOv3 license](apps/shell/src/ui/licenses/DINOv3.txt), not GPL |
| WebView2 Rust bindings | [MIT](apps/shell/portable/licenses/WebView2-Rust-LICENSE.txt); Microsoft SDK/runtime terms are separate |
| Downloaded HEIC, RAW, DirectML runtimes | Provisioning manifests, original notices and applicable corresponding sources |

Rust dependencies are identified by each Cargo.lock; their original package notices remain authoritative. Portable packaging collects a dependency license index. This inventory is not a completed compatibility audit for every binary/model combination.

Private photographs, optional weights, downloaded runtimes and research corpora are not in this repository. Corpus preparation records per-file licenses and attribution locally; Iris's GPL license grants no photograph rights.
