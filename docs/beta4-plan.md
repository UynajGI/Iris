# beta4 selection upgrades

Baseline: published `v0.1.0-beta3`, commit `286a3fe1374311b5a31ada4a62d0d824d591ce04`.

Status: backend and frontend scope implemented and locally verified on `codex/beta4-selection`; source version is `0.1.0-beta4`, with installers not yet published. The application remains a local photo-selection tool. Cross-platform release, independent accuracy labels and additional hardware are separate validation boundaries, not claimed by this checklist.

## Required scope and acceptance

| Upgrade                          | Backend                                                                                                                                                                    | Frontend                                                                                                                 | Evidence required                                                                                                                                            |
| -------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Browse-aware analysis            | Reprioritize queued work by current photo, comparison members and nearby photos; retain bounded workers, pause/cancel, no duplicate execution and fair background progress | Publish current browsing context without starting analysis implicitly                                                    | Queue ordering, concurrent claims, changing context, fairness and lifecycle tests; response latency under analysis                                           |
| On-demand detail review          | Use higher-resolution subject regions for difficult/uncertain cases; cache with source and method identity; retain explicit preview/full-detail/uncertain distinctions     | Request and display detail evidence; allow manual review; never equate unavailable detail with defects                   | Synthetic sharp/blurred region tests, source invalidation, bounded resource use, public portrait examples and RAW limitations                                |
| Indexed filters and sorts        | Page common filters, score/date/size/name sorting in SQLite before loading full analysis JSON; preserve marks, stale-result semantics and stable ordering                  | Preserve navigation and selected scope when filters change                                                               | Database migration/rollback protection; equivalence with existing semantics, ties, paging, stale and missing analyses; large synthetic catalog measurements  |
| Scene and burst hierarchy        | Separate scene candidates from strict bursts and file duplicates; handle time gaps/missing timestamps explicitly; stable groups and user correction                        | Browse scenes and their bursts with clearly distinct meanings; support corrections                                       | Group boundary, missing-time, incorrect-merge and correction persistence tests; no accuracy claim without labels                                             |
| Difference-oriented comparison   | Provide meaningful group differences and evidence reliability; reuse detail review                                                                                         | Corresponding subject crops, linked zoom/pan and keyboard comparison; uncertain subject matching remains user-controlled | Backend comparison tests and frontend interaction tests; actual UI verification with public fixtures                                                         |
| Optional natural-language search | Local image/text embedding provider, persistent compatible index, bounded/cancellable indexing, optional model status and clear unavailable behavior                       | Optional search and relevant result context; indexing must not block browsing                                            | Tokenization/provider compatibility and ranking tests, cache invalidation and endpoint integration; no automatic model-license acceptance or cloud inference |
| Incremental computation          | Reuse existing weight-only rescoring; keep detection, grouping, detail and retrieval cache dependencies distinct                                                           | Show useful readiness rather than treating unfinished optional stages as a failed photo                                  | Settings/source/model invalidation tests; reused stages actually avoid expensive work                                                                        |

## Implementation order

1. [x] Indexed catalog queries and regression coverage.
2. [x] Browse-aware backend queue and context endpoint.
3. [x] Detail evidence and incremental persistence.
4. [x] Scene hierarchy, correction persistence and comparison evidence.
5. [x] Optional local image/text search backend.
6. [x] Regenerated OpenAPI/frontend contract and all backend checks.
7. [x] Frontend browsing context, scene navigation, detail/linked comparison and search.
8. [x] Public-fixture and synthetic scale/performance acceptance, with explicit unmeasured resource/native/RAW limits.
9. [x] Version `0.1.0-beta4`, reconciled release/handoff documentation and public-source gates.

## Boundaries

- Use synthetic fixtures or explicitly configured public photographs within their terms; never use personal photographs or fall back to `test-photos/`.
- Preserve beta3's runtime compatibility fixes and explicit optional-model license boundaries. No machine-wide changes, new cloud inference or implicit acceptance of model licenses.
- Algorithm suggestions, human decisions and physical file operations remain separate. Uncertain, unavailable and stale are not negative judgments about photographs.
- Do not claim that more workers or larger batches are faster without end-to-end evidence. The installed beta3 DirectML graph supports batch one.
- Performance acceptance records hardware, source identity, fixture manifest, cache state, first-use latency, switching P95, throughput and resource peaks. Generated reports stay ignored.
- Version naming and local verification do not imply a public release. Publication/push is a separate authorized action; beta3 remains immutable.

## Progress evidence

On 2026-10-12 the working tree was clean and HEAD matched the published beta3 tag before the development branch was created. Backend implementation preceded frontend integration. Version manifests and lockfiles now identify beta4; the published download still points to beta3.

- Indexed queries preserve beta3 sorting/filtering behavior, ties, stale/missing evidence and page boundaries. A 10,000-photo synthetic experiment returned identical ids at 31.1ms median versus 2173ms for full materialization then sorting. This is a strategy oracle, not a separately built beta3 binary.
- Queue tests cover dynamic context, concurrent exactly-once claims, revision ordering, expiration and fairness. Quality, optional DINOv3 and retrieval indexing use the same priority policy.
- Detail tests cover synthetic sharp/blurred regions, native decode bounds, source invalidation, corrupt cache recovery and reuse after quality-weight changes. JPEG/PNG/WebP have native-region measurements; RAW/HEIC remains explicitly preview-only.
- Scene tests cover capture-time and appearance boundaries, stable ids, correction persistence/reset, availability/scope checks, filtered paging and contained strict bursts. A photo-to-scene lookup avoids scanning every burst for every scene. Comparison tests retain unknown/ambiguous observations rather than inventing identity or scores.
- Retrieval tests cover tokenizer Unicode/special tokens/truncation, manifest hashes and explicit license state, compatible fingerprinted persistence, ranking and protocol cancellation/deadlines. An explicit synthetic ONNX dual-encoder test ran against the local runtime. Real optional retrieval weights were not installed or accepted; semantic accuracy is unclaimed.
- Frontend navigation tests and three public-portrait browser checks cover coalesced browse hints, linked/independent views, original-region measurement, keyboard decisions, scene correction/scope navigation and disabled-model behavior. API declarations are regenerated.
- 541 public portraits: import 645ms, basic analysis 38.6 photos/s, foreground preview under thumbnail load P95 32.4ms, browser switching during active analysis P95 73ms. Reports, source/binary hashes and caveats are recorded in [performance-plan.md](performance-plan.md). Resource peaks, native WebView2 and full-resolution RAW were not measured.
- Verification totals and release boundaries are maintained in [validation.md](validation.md), [HANDOFF.md](HANDOFF.md) and [beta4 source notes](releases/v0.1.0-beta4.md). Local search preparation is documented in [local-search.md](local-search.md).
