# Third-party notices

Iris-owned application code and documentation are licensed under GPL-3.0-or-later.
This does not replace third-party licenses or copyrights.

| Material                                   | License / notice                                                                                                                                                                                                                       |
| :----------------------------------------- | :------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Standalone RAW wrapper                     | [MIT](components/raw-decoder/LICENSE)                                                                                                                                                                                                  |
| rawler                                     | LGPL-2.1; original package notices retained in the RAW source archive                                                                                                                                                                  |
| Material Symbols Rounded                   | [Apache-2.0](apps/shell/src/ui/icons/LICENSE); provenance in the icon directory                                                                                                                                                        |
| Noto Sans SC / Noto Serif SC               | SIL OFL 1.1; font package notices copied during frontend builds                                                                                                                                                                        |
| React / React DOM / npm dependencies       | Original package licenses; versions in lockfiles                                                                                                                                                                                       |
| Tauri CLI 2.12.1 NSIS template             | MIT OR Apache-2.0; [provenance](apps/shell/installer/README.md), [MIT](apps/shell/installer/upstream/LICENSE-MIT.txt), [Apache-2.0](apps/shell/installer/upstream/LICENSE-APACHE-2.0.txt); Iris UI adaptations remain GPL-3.0-or-later |
| YuNet                                      | [MIT](models/LICENSE-YuNet.txt)                                                                                                                                                                                                        |
| MediaPipe conversions                      | [Apache-2.0](models/LICENSE-MediaPipe.txt); provenance limits in [model documentation](models/README.md)                                                                                                                               |
| BasicSR NIQE data and algorithm references | [Apache-2.0](models/LICENSE-BasicSR.txt)                                                                                                                                                                                               |
| ONNX Runtime                               | [MIT](models/LICENSE-ONNXRuntime.txt), [additional notices](models/ThirdPartyNotices-ONNXRuntime.txt)                                                                                                                                  |
| Optional DINOv3 weights                    | [Meta DINOv3 license](apps/shell/src/ui/licenses/DINOv3.txt), not GPL                                                                                                                                                                  |
| WebView2 Rust bindings                     | [MIT](apps/shell/portable/licenses/WebView2-Rust-LICENSE.txt); Microsoft SDK/runtime terms are separate                                                                                                                                |
| Downloaded HEIC, RAW, DirectML runtimes    | Provisioning manifests, original notices and applicable corresponding sources                                                                                                                                                          |

Rust dependencies are identified by each Cargo.lock; their original package notices remain authoritative. Portable packaging collects a dependency license index. This inventory is not a completed compatibility audit for every binary/model combination.

The traditional renderer uses LittleCMS/lcms2 (MIT), wgpu (MIT OR Apache-2.0), image (MIT OR Apache-2.0), tiff (MIT), and Rayon (MIT OR Apache-2.0). Their pinned package licenses remain authoritative. RAW PPG demosaicing calls rawler's implementation within the existing standalone RAW component; it is not relicensed as Iris-owned code. Algorithm references and implementation limits are recorded in [the rendering implementation notes](docs/color-engine-implementation.md).

The optional local retrieval pipeline uses Hugging Face Tokenizers (Apache-2.0), pinned in Cargo.lock. Compatible ONNX retrieval weights and tokenizer assets are supplied separately by the user and retain their own licenses; Iris does not download them or accept their terms automatically.

Private photographs, optional weights, downloaded runtimes and research corpora are not in this repository. Corpus preparation records per-file licenses and attribution locally; Iris's GPL license grants no photograph rights.

## Colour temperature formula reference

The white-balance illuminant polynomials reference Colour's CIE daylight and Kang 2002 implementations. The [original notice](crates/iris-render/src/white_balance/COLOUR-LICENSE.txt) is also reproduced below so that the existing binary package notice bundle carries it.

Copyright 2013 Colour Developers

Redistribution and use in source and binary forms, with or without modification, are permitted provided that the following conditions are met:

1. Redistributions of source code must retain the above copyright notice, this list of conditions and the following disclaimer.

2. Redistributions in binary form must reproduce the above copyright notice, this list of conditions and the following disclaimer in the documentation and/or other materials provided with the distribution.

3. Neither the name of the copyright holder nor the names of its contributors may be used to endorse or promote products derived from this software without specific prior written permission.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS “AS IS” AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE
