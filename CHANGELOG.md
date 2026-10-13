# Changelog

Generated from Conventional Commits by git-cliff. Installation instructions and
known limitations are maintained separately in [release notes](docs/releases/).

## v0.1.0-beta5

### Documentation

- **selection:** record five-feature delivery and CI evidence ([32815ce](https://github.com/UynajGI/Iris/commit/32815ce808162f7d51afba758880af157d2783f2))

### Features

- **selection:** upgrade selection workflow to beta4 ([219bf9b](https://github.com/UynajGI/Iris/commit/219bf9bcced0ecf69b3a000ca8bdadc3724e61cf))

- **selection:** complete group intent and original detail workflow ([a26ae97](https://github.com/UynajGI/Iris/commit/a26ae977ff92af46a3caef086a6433e2f211683f))

- **installer:** apply approved dark wizard design ([b6eec20](https://github.com/UynajGI/Iris/commit/b6eec20bc1515a577939e69d56ca135b183a4bea))

- **support:** add help diagnostics and version synchronization ([ecca88e](https://github.com/UynajGI/Iris/commit/ecca88e1dcf718b80407a1436d8d9a4c00921739))

- **selection:** save filter presets and open export folders ([2ada30a](https://github.com/UynajGI/Iris/commit/2ada30a719bb085d4e0d3694ef3b2b8dbf04f438))

### Fixes

- **selection:** bound comparison candidates before requesting evidence ([172aaf9](https://github.com/UynajGI/Iris/commit/172aaf96517c71a0600dc5c70c2fa23e73b1f023))

- **ci:** retry transient quality tool downloads ([e3cbac0](https://github.com/UynajGI/Iris/commit/e3cbac096cd1617f504441e3ac98344904649bb2))

- **selection:** align CI regressions and initialize RAW region decoding ([848bd7c](https://github.com/UynajGI/Iris/commit/848bd7cb370372f0a6813ec020a80342094cc402))

- **selection:** allow direct main-face confirmation and correction reset ([cdc8c9e](https://github.com/UynajGI/Iris/commit/cdc8c9ec079df3080cc93cdbf7ab0280c05e46f9))

- **api:** sync selection contract with CI-generated schema ([b50de3b](https://github.com/UynajGI/Iris/commit/b50de3bb4988d3de5cf16bd4cb495106137ee480))

- **selection:** preserve overlapping intent and refresh adopted groups ([226207c](https://github.com/UynajGI/Iris/commit/226207ce2a04d53310cf1b16ae653bcac2709bd4))

- **xmp:** resolve file subjects without project-relative shortcuts ([5d726ad](https://github.com/UynajGI/Iris/commit/5d726ad35f76f68f662af93a87aeb473e5a0cb93))

- **xmp:** keep exported subjects free of local directory paths ([0141561](https://github.com/UynajGI/Iris/commit/0141561616da62247e292f9bbc4b5c1b2c811928))

- address beta5 review and contract version drift ([5a93d98](https://github.com/UynajGI/Iris/commit/5a93d9873464fb97c904a74019c40037d0131d7a))

### Other

- Merge pull request #17 from UynajGI/codex/beta4-selection

feat(selection): upgrade selection workflow to beta4 ([f3732d8](https://github.com/UynajGI/Iris/commit/f3732d8924ac743a3d5486ef39d6e552aa1c11a2))

- Merge pull request #18 from UynajGI/codex/selection-workflow

feat(selection): 补齐原图细节与组内选片工作流 ([20850bf](https://github.com/UynajGI/Iris/commit/20850bfbe2dcc9070961759598ace56ae87f5d32))

## v0.1.0-beta3

### Development

- standardize formatting and harden maintenance checks ([6dc9b20](https://github.com/UynajGI/Iris/commit/6dc9b200c1d951d7871307abef0f241fcc8ca6f6))

- **deps:** bump actions/download-artifact from 4.3.0 to 8.0.2 ([01a3a97](https://github.com/UynajGI/Iris/commit/01a3a97dd860d91cf23a4e8248b8e0e7019d4633))

- **deps:** integrate dependency update ([653b46a](https://github.com/UynajGI/Iris/commit/653b46ad37de0accb4ffbe02fbb8ffc01d9bfc1d))

- **deps:** bump actions/setup-node from 4.4.0 to 7.1.0 ([2322d06](https://github.com/UynajGI/Iris/commit/2322d06a6de1201323f0be2918436568a2047d6d))

- **deps:** integrate dependency update ([0b264b2](https://github.com/UynajGI/Iris/commit/0b264b2286f1b1f584aee326d27732c4c33e99e9))

- **deps:** bump astral-sh/setup-uv from 6.8.0 to 10.2.0 ([addb5a2](https://github.com/UynajGI/Iris/commit/addb5a233242853e0313b876dd76c9240feabc73))

- **deps:** integrate dependency update ([36b12bc](https://github.com/UynajGI/Iris/commit/36b12bcd6cb172cde1ece87a6082f4016d4dbf76))

- **deps:** bump actions/checkout from 4.4.0 to 7.0.1 ([18be247](https://github.com/UynajGI/Iris/commit/18be247bf4d1a1a522510c7071be24a9a28377fb))

- **deps:** integrate checkout update ([8cf022a](https://github.com/UynajGI/Iris/commit/8cf022a9481bef778f821bbe30ed245a5d745f5d))

- **deps-dev:** bump lefthook from 2.1.17 to 2.2.0 ([57a7fcf](https://github.com/UynajGI/Iris/commit/57a7fcf3ff860eff81ed06c26746139c7946068a))

- **deps:** integrate lefthook update ([537ce0f](https://github.com/UynajGI/Iris/commit/537ce0ff62a70fd1eb3ea6de09825f0b69ed535a))

- **deps-dev:** bump @types/node from 22.20.5 to 26.6.4 in /apps/shell ([4c7a3c2](https://github.com/UynajGI/Iris/commit/4c7a3c2f6400eafffeb446982c084a770d4e840c))

- **deps:** integrate dependency update ([014c0e7](https://github.com/UynajGI/Iris/commit/014c0e765aa863e60583c44e506e49b7d0387e43))

- **deps:** bump actions/setup-python from 5.6.0 to 7.0.0 ([d595b62](https://github.com/UynajGI/Iris/commit/d595b627f16b2ef78bce2df8c2435d63e9682f64))

- **deps:** integrate Python action update ([c058f4c](https://github.com/UynajGI/Iris/commit/c058f4c7e7fc83ded99efd756c84c8d9aa77a193))

- **deps:** bump base64 from 0.22.1 to 0.23.1 ([a399a65](https://github.com/UynajGI/Iris/commit/a399a65acb9ed174d9c7362208b394ed186e6021))

- **deps:** integrate dependency update ([047341a](https://github.com/UynajGI/Iris/commit/047341a4d5548cd9770860a834211595dc87b068))

- **deps:** bump nalgebra from 0.34.2 to 0.35.0 ([c6526bc](https://github.com/UynajGI/Iris/commit/c6526bc23a948e1c510f7fded4622f4100ed5581))

- **deps:** integrate dependency update ([731868d](https://github.com/UynajGI/Iris/commit/731868d3a7249ccab59aca914a3cc78893ec41a7))

- **deps:** bump tower-http from 0.6.11 to 0.7.1 ([9a9ce07](https://github.com/UynajGI/Iris/commit/9a9ce0734372660c4f95a8038abd7d853be29419))

- **deps:** integrate dependency update ([e93b39e](https://github.com/UynajGI/Iris/commit/e93b39e4537fa6e1cb355b6f5c47786a188e17dc))

- **deps:** bump base64 from 0.22.1 to 0.23.1 in /apps/shell/src-tauri ([ad1f1bf](https://github.com/UynajGI/Iris/commit/ad1f1bf438e0dfb53f9490419e99e75d7a801df6))

- **deps:** integrate native base64 update ([de17743](https://github.com/UynajGI/Iris/commit/de1774324c1204451c3257d1269d72947ae1e199))

- **deps:** bump utoipa from 5.5.0 to 6.0.0 ([1a83a4b](https://github.com/UynajGI/Iris/commit/1a83a4b6bdd5bde58465e5378845d8affb1c05f3))

- **deps:** integrate dependency update ([4c65a48](https://github.com/UynajGI/Iris/commit/4c65a4865c1393f3d4db4b4c25824a4e0546664c))

- **deps:** bump ort from 2.0.0-rc.10 to 2.0.0-rc.13 ([9ddb097](https://github.com/UynajGI/Iris/commit/9ddb09785751633c54580e91d1e3ae65df6c6dee))

- **deps:** integrate ort with pinned runtime API compatibility ([c6ffa63](https://github.com/UynajGI/Iris/commit/c6ffa6316b97b42a3d7b3595c1db1a4b02148149))

- **deps-dev:** bump typescript from 5.9.3 to 7.0.2 in /apps/shell ([1cb6fb1](https://github.com/UynajGI/Iris/commit/1cb6fb1c3dd9e2987bcd43eac713245602c25c47))

- **deps:** integrate TypeScript update ([187afd3](https://github.com/UynajGI/Iris/commit/187afd34fd2a5befb91b5d26679bf6bdb8c5f33a))

- **deps:** integrate upgrades with compiler and runtime compatibility ([3f037b5](https://github.com/UynajGI/Iris/commit/3f037b599652c030ff0c30605e164b250bdc8060))

- **deps:** bump actions/download-artifact from 4.3.0 to 8.0.2 ([cc9b2a5](https://github.com/UynajGI/Iris/commit/cc9b2a5fa5f6afb4f5d88d6b90af7fd713abfff9))

- **deps:** integrate refreshed PR #1 ([c68464e](https://github.com/UynajGI/Iris/commit/c68464eb1e00b1c2d5d2adcb837108e3d5cd2e17))

- **deps:** bump actions/setup-node from 4.4.0 to 7.1.0 ([f2ddaf6](https://github.com/UynajGI/Iris/commit/f2ddaf6a32ab768e9c9da8cb7cbd52d0c6e7e5b9))

- **deps:** integrate refreshed PR #2 ([cc98729](https://github.com/UynajGI/Iris/commit/cc9872935d52c928f41d61e7ecb6b8e209420251))

- **deps:** bump astral-sh/setup-uv from 6.8.0 to 10.2.0 ([0dea73a](https://github.com/UynajGI/Iris/commit/0dea73a9860b77898533a47d2d3733a618280c2f))

- **deps:** integrate refreshed PR #3 ([ae87a4e](https://github.com/UynajGI/Iris/commit/ae87a4e9a322b17742a1c7b24164d5f35862392d))

- **deps:** bump actions/checkout from 4.4.0 to 7.0.1 ([9384afc](https://github.com/UynajGI/Iris/commit/9384afc2ecd0d2dd6d518317bb0ef9e85053d19c))

- **deps:** integrate refreshed PR #4 ([621ff82](https://github.com/UynajGI/Iris/commit/621ff827773425ed0c0025483f9ddb2f109a4134))

- **deps-dev:** bump lefthook from 2.1.17 to 2.2.0 ([a14f50e](https://github.com/UynajGI/Iris/commit/a14f50e227c8b9d8fc583238a319ea383086c73e))

- **deps:** integrate refreshed PR #5 ([65af754](https://github.com/UynajGI/Iris/commit/65af7545fae7bf73cc82cf300d340f0555e3dfac))

- **deps-dev:** bump @types/node from 22.20.5 to 26.6.4 in /apps/shell ([326787d](https://github.com/UynajGI/Iris/commit/326787d558b67be2a44728bce345fa216ca08488))

- **deps:** integrate refreshed PR #6 ([17ef0ef](https://github.com/UynajGI/Iris/commit/17ef0efda47537ac271e62b76f7074e268d99913))

- **deps-dev:** bump typescript from 5.9.3 to 7.0.2 in /apps/shell ([e31b2a2](https://github.com/UynajGI/Iris/commit/e31b2a2a29910e6cc9848297f7e27b3446ee4352))

- **deps:** integrate refreshed PR #7 ([bf65c2a](https://github.com/UynajGI/Iris/commit/bf65c2aa4935c51029044681a6f660409b390d83))

- **deps:** bump actions/setup-python from 5.6.0 to 7.0.0 ([a4e2f57](https://github.com/UynajGI/Iris/commit/a4e2f57505ffd4d12c5ee6e277fc21d2086fee55))

- **deps:** integrate refreshed PR #8 ([dd3bc7c](https://github.com/UynajGI/Iris/commit/dd3bc7cb9c5d9da34853343619dd3f10576a2e07))

- **deps:** bump base64 from 0.22.1 to 0.23.1 ([9c88ce3](https://github.com/UynajGI/Iris/commit/9c88ce38bd1c2040dc6afc612083e5530418fdb4))

- **deps:** integrate refreshed PR #9 ([228bc70](https://github.com/UynajGI/Iris/commit/228bc70e021191e87732195e1ad9b01b6b2235fb))

- **deps:** bump ort from 2.0.0-rc.10 to 2.0.0-rc.13 ([1a0cb0a](https://github.com/UynajGI/Iris/commit/1a0cb0a1a9dc7fd66c0dc3479cb357c7edb8ad0f))

- **deps:** integrate refreshed PR #10 ([3735205](https://github.com/UynajGI/Iris/commit/37352055211a04831f82a64c15219968a81c52d6))

- **deps:** bump utoipa from 5.5.0 to 6.0.0 ([7e46482](https://github.com/UynajGI/Iris/commit/7e464824a36e182e2669838321d1984e1bb57ced))

- **deps:** integrate refreshed PR #11 ([ed3dcc3](https://github.com/UynajGI/Iris/commit/ed3dcc3e22b4470d1619518322781b13e2ba3cad))

- **deps:** bump nalgebra from 0.34.2 to 0.35.0 ([44d4530](https://github.com/UynajGI/Iris/commit/44d45305a14005be18a7b831b875fccfe7724e9a))

- **deps:** integrate refreshed PR #12 ([01c4c12](https://github.com/UynajGI/Iris/commit/01c4c123efc1c50a0da98094af75beac8fb92997))

- **deps:** bump tower-http from 0.6.11 to 0.7.1 ([a6c7314](https://github.com/UynajGI/Iris/commit/a6c7314c5e33603e502d548f3c69013c7f00280a))

- **deps:** integrate refreshed PR #13 ([31af392](https://github.com/UynajGI/Iris/commit/31af3922c32907cd0630ada9bd8f3642289a3794))

- **deps:** bump base64 from 0.22.1 to 0.23.1 in /apps/shell/src-tauri ([03df09b](https://github.com/UynajGI/Iris/commit/03df09be319b70a70a1aa4ef90065f15af510939))

- **deps:** integrate refreshed PR #14 ([e2c62cb](https://github.com/UynajGI/Iris/commit/e2c62cb627c9dc642ef706a8293bf2bbf776c6c3))

- automatically merge verified low-risk Dependabot updates ([5a63a01](https://github.com/UynajGI/Iris/commit/5a63a01770706917ad518f1d261ec443b9d82f60))

- reconcile public fixture verification changes ([ee457ca](https://github.com/UynajGI/Iris/commit/ee457ca9653ab8ec49db2e61bdfaca684bee04f3))

- **store:** reproduce historical schemas without v6 tables ([046e87b](https://github.com/UynajGI/Iris/commit/046e87b3ea0367ea30de41de794d6fe36f046c45))

### Documentation

- align maintenance guide with cleared complexity debt ([67e33f5](https://github.com/UynajGI/Iris/commit/67e33f5da8fe65c6b363d97f1c230c353a90e631))

- reconcile dependency integration evidence and project handoff ([bde2876](https://github.com/UynajGI/Iris/commit/bde2876bc9d59e09c718b10bb3c9a0884e4d041c))

- align desktop setup and API generation with current tooling ([97b02d1](https://github.com/UynajGI/Iris/commit/97b02d1a11e8954c01c640274a0d39f448b74693))

- record public fixture coverage and validation boundaries ([f1543ff](https://github.com/UynajGI/Iris/commit/f1543ff77920a5986b0ff8676b2adbee3b3983a3))

- explain expanded research fixture workflow ([46b2b76](https://github.com/UynajGI/Iris/commit/46b2b76ebbe8ee6d1ebbe5d57279279f6890ac5e))

- record partial public corpus regression boundaries ([75c7306](https://github.com/UynajGI/Iris/commit/75c730617e7b36cb8c1cb1b8759186b88b74b64f))

- reconcile public regression evidence with review fixes ([d4d50c6](https://github.com/UynajGI/Iris/commit/d4d50c69441aa683e3d428c9c1c77b70479560c0))

### Features

- **shell:** refine photo workbench interactions and simplify project entry ([bcdebaa](https://github.com/UynajGI/Iris/commit/bcdebaab4a739d8e6a0ae84b42212ea7d2365f12))

- **selection:** improve comparison evidence and preview responsiveness ([aacdfb1](https://github.com/UynajGI/Iris/commit/aacdfb1b1ab4defb6e8472e2ee291a0ee18c40ab))

- **selection:** improve performance and desktop lifecycle ([9dc0887](https://github.com/UynajGI/Iris/commit/9dc088783c79dbf9013b23748b622cbf98b0971a))

### Fixes

- preserve OpenAPI validity and Windows ONNX process teardown ([8a63ee1](https://github.com/UynajGI/Iris/commit/8a63ee1b00c40f76c6f3a12298ac8366edd553cf))

- **tooling:** isolate changelog Git subprocess environments ([c25196c](https://github.com/UynajGI/Iris/commit/c25196c5ebf2fd38e8d16e617da3e1f4b1a849f8))

- **tooling:** isolate source export fixtures from hook Git context ([1f60122](https://github.com/UynajGI/Iris/commit/1f601226cebc9256e47fb60c99dc6b3c431704b8))

- **tooling:** retain fixture catalogs when preparing research samples ([f3249ad](https://github.com/UynajGI/Iris/commit/f3249ad60ffe72c42a43505ef576cd90267884d4))

- address selection and dependency merge review findings ([4a6204a](https://github.com/UynajGI/Iris/commit/4a6204a24243f1f1a2dc76f4104568ed4def5008))

- **tooling:** preserve partial public fixture verification results ([fd60391](https://github.com/UynajGI/Iris/commit/fd60391934cbfbad3770c477e70cb79714389f7e))

- **store:** invalidate old engine groups only once ([4d39e6b](https://github.com/UynajGI/Iris/commit/4d39e6b301c11a2911d0cda3a877ae6e1975397c))

- **vision:** avoid automatic device selection on CPU runtimes ([e786965](https://github.com/UynajGI/Iris/commit/e7869650d339b77d9db1272c7ba43f1560f0504a))

- **vision:** retain dynamic runtime environment until process exit ([7181864](https://github.com/UynajGI/Iris/commit/71818643a0e36c0a8d8aab904482a1177e46713a))

- **vision:** preserve native environment cleanup on macOS ([b5a30e2](https://github.com/UynajGI/Iris/commit/b5a30e25a4d602013715099e8bea6d4059d4d38e))

### Other

- Merge pull request #15 from UynajGI/codex/dependency-integration

build(deps): integrate verified dependency upgrades and maintenance cleanup ([e944c1b](https://github.com/UynajGI/Iris/commit/e944c1b325cbc51c627bfe3daf25bae029019513))

- Merge pull request #16 from UynajGI/codex/dependabot-automerge

feat: polish selection workflow and gate Dependabot auto-merges ([7b7a11a](https://github.com/UynajGI/Iris/commit/7b7a11a0aad475c319324a064d46f697605d7263))

### Refactoring

- split workbench and services with enforced quality gates ([ed5b390](https://github.com/UynajGI/Iris/commit/ed5b390fcb8f11eb1a06b88350219e20ae32e1e5))

- eliminate remaining complexity warnings ([60a5103](https://github.com/UynajGI/Iris/commit/60a5103d28aa3d0a0ad3aec25bd8168cb7edd2e6))

- **tooling:** split public fixture orchestration without behavior changes ([77d52b7](https://github.com/UynajGI/Iris/commit/77d52b7d72b727b24bf62819b39a370b70946a01))

## v0.1.0-beta2

### Development

- unify development setup and tasks under Make ([a4ab8d7](https://github.com/UynajGI/Iris/commit/a4ab8d7ab5b2ff70c4822da82cf090c6822b359b))

### Documentation

- standardize GitHub presentation and contribution guides ([9b917b7](https://github.com/UynajGI/Iris/commit/9b917b7adde83276638876a231eb1eaac04db68a))

- record GitHub rendering and format verification ([ea5355d](https://github.com/UynajGI/Iris/commit/ea5355dfd52496969dbb8ee4f82b18e72c939027))

### Features

- **desktop:** polish UI and automate cross-platform installers ([183bc45](https://github.com/UynajGI/Iris/commit/183bc454561ff189db945b631fd661a3d67e5ea9))

### Fixes

- **security:** confine model artifact paths and redact scan diagnostics ([b293982](https://github.com/UynajGI/Iris/commit/b29398251317c649d71be8dda109e5cb0fbcbfde))

## v0.1.0-beta

### Development

- harden repository checks and complete review documentation ([7132dab](https://github.com/UynajGI/Iris/commit/7132dabf41fe76cc5f460cc5f33bab1fbcb01a0d))

- allow native beta rehearsal before tagging ([2bb99c4](https://github.com/UynajGI/Iris/commit/2bb99c4f4a7872e7ee68dd008f780c5095e0e380))

- include dependency sources and verify Unix beta packages ([f5246cb](https://github.com/UynajGI/Iris/commit/f5246cb4242dbbe42307a2b796dcfcc2e9d820dd))

### Documentation

- simplify development prerequisites ([cae5168](https://github.com/UynajGI/Iris/commit/cae51687fb84c3d0c3b29b2bc51ec204a313dfdd))

- distinguish beta rehearsal evidence and platform support ([1d54881](https://github.com/UynajGI/Iris/commit/1d548816d06824e9426d4331aef90c301e8e9422))

- record successful native beta package verification ([c7115ec](https://github.com/UynajGI/Iris/commit/c7115ec1d3fb71f8b00a4f18e57b407e68adf2f9))

### Features

- open-source IrisVision under GPL-3.0-or-later ([c28901f](https://github.com/UynajGI/Iris/commit/c28901ff406234ca92bae1f0ef812426b93ca26d))

- **devtools:** add portable development task commands ([97cbb2f](https://github.com/UynajGI/Iris/commit/97cbb2f675972d0322320e43f39333a6bf68e9e4))

- **release:** prepare Iris beta and native platform builds ([e0df482](https://github.com/UynajGI/Iris/commit/e0df4829824761ad84292abe9e40dd8c564bc57b))

### Fixes

- **test:** wait for updater fixture request headers on Windows ([dedae44](https://github.com/UynajGI/Iris/commit/dedae441fbb9c345f664efc5513e4b2b341af895))

- **build:** retain GCC runtime notices after MSYS2 package split ([42c1602](https://github.com/UynajGI/Iris/commit/42c16023a33a7b011af7427bcd4479d5751e1ac6))

- **desktop:** provide the required transparent Unix icon resource ([5a631a3](https://github.com/UynajGI/Iris/commit/5a631a32bbbf2a3144ac978c8895dc68702aa919))

- **ci:** use a literal cache save condition ([4b328ab](https://github.com/UynajGI/Iris/commit/4b328ab41e76584825dfac3eb06cb3e1cfddf296))

- **packaging:** normalize Windows short paths before containment checks ([6654c3f](https://github.com/UynajGI/Iris/commit/6654c3f618c409a4d3490302e4e97d5115f658d4))

- **runtime:** release macOS ONNX environment before native destructors ([210eaef](https://github.com/UynajGI/Iris/commit/210eaef78c7ba513980249c9625c3875bcf29863))

- **packaging:** compare normalized source content before export ([c997197](https://github.com/UynajGI/Iris/commit/c997197a57fd8d8f028ea770457eafb319f98937))
