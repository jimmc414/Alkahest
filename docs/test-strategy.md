# ALKAHEST: Test Strategy

**Version:** 0.1.0-draft
**Date:** 2026-02-13
**Status:** Skeleton — test cases to be written alongside implementation
**Companions:** requirements.md, architecture.md, milestones.md, project-structure.md, technical-constraints.md

---

## 1. How to Use This Document

This document defines what is tested, how it is tested, and what constitutes pass/fail for each subsystem and milestone. It does not contain individual test cases — those are written during implementation and live in the `tests/` directory (see project-structure.md Section 7).

Each milestone must pass all tests defined for it and all tests from prior milestones before it is considered complete. Tests are additive: the test suite only grows. A passing M5 build runs all M2, M3, M4, and M5 tests.

---

## 2. Test Categories

Alkahest uses five categories of tests. Each serves a different purpose and catches a different class of bug.

### 2.1 Deterministic Snapshot Tests

**What they verify:** Given an initial voxel state and a rule set, running N simulation ticks produces a specific known-good output state.

**How they work:** A test fixture defines an initial voxel configuration as a compact description (e.g., "place sand at (16,31,16), stone floor at y=0"). The test harness (`alkahest-sim/test_harness.rs`) initializes the GPU buffers, runs exactly N simulation ticks, reads back the voxel buffer, and compares it byte-for-byte against a stored reference snapshot.

**When snapshots are created:** The first time a test runs with no existing snapshot, it writes the output as the new reference. Subsequent runs compare against this reference. If a code change intentionally alters simulation behavior (e.g., tuning a diffusion rate), the developer explicitly regenerates affected snapshots and commits them with a justification in the commit message.

**What they catch:** Non-determinism (C-SIM-1, C-SIM-2, C-SIM-4), simulation regressions, pass ordering bugs (C-SIM-3), cross-chunk boundary errors (C-SIM-7).

**Location:** `tests/determinism/`

**Introduced at:** M2. Extended at every milestone that modifies simulation behavior.

### 2.2 Rule Validation Tests

**What they verify:** The rule loader and validator correctly accept valid rule files and reject invalid ones with specific, actionable error messages.

**How they work:** Each test provides a RON material or rule file (inline or from a fixture file). The test calls the loader and validator and asserts either success or a specific error variant. For rejection tests, the error message is checked for the expected diagnostic content.

**What they catch:** Schema parsing bugs, validator false positives/negatives, missing validation cases (C-DATA-1 through C-DATA-4), energy conservation violations (C-DATA-3), CFL stability miscalculation (C-SIM-5).

**Location:** `tests/rules/`

**Introduced at:** M3. Extended at M4 (CFL check), M9 (balancing), M12 (mod loading).

### 2.3 Integration Tests

**What they verify:** Multiple subsystems work together correctly in end-to-end scenarios that mirror real gameplay.

**How they work:** Each test sets up a multi-subsystem scenario (e.g., load rules, create a world, run simulation, save, load, verify state). Integration tests run on GPU and may take longer than unit tests. They are tagged `#[ignore]` for fast CI runs and explicitly included in the full test suite.

**What they catch:** Cross-crate interface mismatches, data flow bugs (e.g., rule compiler produces a buffer the sim shader can't read), save/load round-trip corruption, chunk lifecycle bugs.

**Location:** `tests/integration/`

**Introduced at:** M5. Extended at M6, M8.

### 2.4 Visual Regression Tests

**What they verify:** The rendered image for a known scene and camera position matches a reference screenshot within an acceptable tolerance.

**How they work:** A test fixture defines a voxel scene and camera state. The renderer produces a frame into an offscreen texture. The texture is read back and compared pixel-by-pixel against a reference image. A perceptual difference metric (e.g., per-pixel RMSE across RGB channels) must be below a threshold. The threshold is intentionally loose (allowing for minor floating-point rendering differences across GPUs) — these tests catch gross regressions (broken ray marcher, missing materials, lighting inversion), not pixel-exact correctness.

**When references are created:** Same approach as deterministic snapshots — generated on first run, committed, regenerated explicitly when rendering changes are intentional.

**What they catch:** Renderer regressions, broken shader changes, missing bind group updates, incorrect octree construction (holes in rendering), transparency compositing errors.

**Location:** `tests/integration/` (visual regression tests are a subset of integration tests)

**Introduced at:** M1 (basic rendering). Extended at M5 (multi-chunk), M10 (lighting, AO, transparency).

### 2.5 Performance Benchmark Tests

**What they verify:** Frame time, simulation time, and memory usage are within budget for standardized test scenes.

**How they work:** Benchmark scenes at defined voxel counts (100K, 250K, 500K, 1M, 3M) are run for a fixed number of ticks. Per-frame timings are recorded for each simulation pass and the renderer. Results are compared against a stored baseline. A regression exceeding 10% on any metric fails the test (REQ 11.1.5).

**What they catch:** Performance regressions from code changes, memory leaks, quadratic scaling bugs, GPU occupancy issues.

**Location:** `tests/benchmarks/` (scene definitions), `alkahest-bench/` (runner)

**Introduced at:** M11. Run on every subsequent commit as part of the full CI suite.

---

## 3. Test Infrastructure

### 3.1 GPU Test Environment

Deterministic snapshot tests, integration tests, and visual regression tests all require a GPU. The CI pipeline must run on a machine with a WebGPU-capable GPU (or use wgpu's native Vulkan/Metal backend for headless testing). Tests use wgpu in native mode (not WASM) for CI speed and debuggability. The assumption is that wgpu's native backend produces identical simulation results to its web backend for the same shader code — if this assumption is violated, it must be documented and addressed.

A dedicated CI GPU runner is required from M2 onward. Before M2 (at M0), only compilation and lint tests run, which need no GPU.

### 3.2 Snapshot Storage

Reference snapshots (voxel state binaries and reference screenshots) are stored in the repository under `tests/snapshots/`. They are binary files and will inflate the repository over time. Use Git LFS for snapshot files once total size exceeds 100 MB. Each snapshot file is named `{test_name}_{milestone}.bin` or `{test_name}_{milestone}.png`.

### 3.3 Test Harness API

The test harness in `alkahest-sim/test_harness.rs` exposes (at minimum):

- `init_scene(voxel_placements, rule_set) → SimState` — set up GPU buffers with the given initial configuration.
- `run_ticks(state, n) → SimState` — advance the simulation by exactly N ticks.
- `readback(state) → Vec<u8>` — copy the current voxel buffer from GPU to CPU.
- `compare_snapshot(actual, reference_path) → Result<(), DiffReport>` — byte comparison with a human-readable diff report on failure.

This API is the foundation for all deterministic and integration tests. It is introduced at M2 and must not change its interface in later milestones (new methods can be added, existing signatures must remain stable).

### 3.4 Test Tagging

Tests are tagged with attributes to support selective execution:

- `#[test]` — standard Rust tests. Run on every CI push.
- `#[ignore]` — GPU-dependent or slow tests. Run in the full CI suite (nightly or pre-merge).
- Custom tags via a test framework (e.g., `#[cfg(feature = "gpu_tests")]`) to separate GPU tests from pure-CPU tests.

The fast CI path (every push) runs: compilation, clippy, rustfmt, unit tests (no GPU). The full CI path (pre-merge, nightly) runs: everything plus GPU tests, visual regression, and benchmarks.

---

## 4. Per-Milestone Test Plan

Each milestone lists the test categories required, specific areas of focus, and minimum test counts. Actual test case definitions are written during the milestone and committed alongside the implementation code.

### 4.1 Milestone 0: Toolchain

**Test categories:** Compilation only.

**Focus areas:**
- `cargo build --target wasm32-unknown-unknown` succeeds with zero warnings.
- `cargo clippy` passes.
- `cargo fmt --check` passes.
- The WASM binary loads in a headless browser test runner (Puppeteer or Playwright) without JavaScript errors.

**Minimum tests:** 0 Rust test functions. CI pipeline validates compilation and lint. Browser load test is a CI script, not a Rust test.

### 4.2 Milestone 1: Static Rendering

**Test categories:** Visual regression.

**Focus areas:**
- A 32³ test scene renders correctly from 4 camera angles (front, top, corner, close-up).
- The shadow from a voxel tower is visible and geometrically plausible.
- An empty scene (all air) renders the sky color without artifacts.
- Camera extremes: fully zoomed in (single voxel fills screen), fully zoomed out (chunk is a small cube), camera at world origin, camera at extreme distance.

**Minimum tests:** 6 visual regression tests. No deterministic snapshot tests yet (no simulation).

### 4.3 Milestone 2: Gravity Simulation

**Test categories:** Deterministic snapshot, visual regression.

**Focus areas — deterministic:**
- Single sand voxel falls to the floor in exactly the expected number of ticks.
- Column of 16 sand voxels collapses into a pile with the correct final shape.
- Sand on a ledge avalanches diagonally to form the expected angle of repose.
- Two sand voxels competing for the same empty cell: one wins deterministically, the other takes the diagonal path. Verify the same voxel wins across 10 repeated runs (determinism).
- Pause and resume: run 50 ticks, pause, run 0 ticks (verify no state change), resume, run 50 more ticks. Result must match running 100 ticks without pause.
- Single-step: verify that each step advances the tick counter by exactly 1 and produces the expected intermediate state at ticks 1, 2, and 3.

**Focus areas — visual regression:**
- Sand falling animation: reference screenshots at tick 0, tick 10, tick 50 (final settled state).

**Minimum tests:** 8 deterministic snapshot tests, 3 visual regression tests.

**Critical constraint tests:** Verify C-SIM-1 (double buffer correctness) by running the "two competing sand voxels" test — if the double buffer is broken, the result will depend on thread execution order and fail the determinism check.

### 4.4 Milestone 3: Multi-Material and Reactions

**Test categories:** Deterministic snapshot, rule validation.

**Focus areas — deterministic:**
- Fire + wood → ash + smoke. Place fire adjacent to a 4-voxel wood block. After N ticks, all wood is consumed, ash occupies the former wood positions (or below, having fallen), smoke has risen above.
- Water + fire → steam. Place water above fire. After N ticks, fire is extinguished, steam exists above the former fire position.
- Sand sinks through water. Place a layer of sand above a layer of water. After N ticks, sand is below water.
- Oil floats on water. Place oil below water. After N ticks, oil is above water.
- Chain reaction: fire + wood + oil. Fire ignites wood, burning wood ignites adjacent oil, oil fire produces smoke. Verify the cascade completes.
- No interaction: stone adjacent to water for 100 ticks. No state change.

**Focus areas — rule validation:**
- Valid 10-material file loads without error.
- File with duplicate material ID is rejected with error mentioning the duplicate ID.
- File with material property exceeding quantization range (C-DATA-4) is rejected.
- Rule referencing a nonexistent material ID is rejected.
- Rule that produces energy from nothing (C-DATA-3) is rejected.
- Malformed RON syntax is rejected with a parse error, not a panic.

**Minimum tests:** 8 deterministic snapshot tests, 8 rule validation tests.

### 4.5 Milestone 4: Thermal System

**Test categories:** Deterministic snapshot, rule validation.

**Focus areas — deterministic:**
- Thermal equilibrium: a line of 16 metal voxels, left end at 1000 K, right end at 300 K. After N ticks, temperature gradient is monotonically decreasing left-to-right. After M ticks (M >> N), all voxels are within 5 K of each other (equilibrium reached).
- Entropy drain: a single hot voxel (2000 K) surrounded by air. After N ticks, temperature has decayed toward ambient (293 K). The decay curve is monotonic (no oscillation, validating CFL stability).
- Phase transition — ice melts: ice voxel at 250 K adjacent to fire. After N ticks, ice has become water. Water's temperature is near 273 K (just above melting point, not instantly hot).
- Phase transition — water boils: water voxel on top of lava. After N ticks, water becomes steam.
- Lava chain reaction: lava placed on stone. Adjacent stone heats up. If stone crosses melting threshold, it becomes lava. The chain propagates outward and eventually stops as thermal energy dissipates.
- Convection: heated water at the bottom of a column rises. After N ticks, the hot water voxels are higher than their starting position.

**Focus areas — rule validation:**
- CFL stability check: a material with thermal conductivity so high that `diffusion_rate * conductivity * 26 > 1.0` is rejected or clamped at load time (C-SIM-5).

**Minimum tests:** 8 deterministic snapshot tests, 2 rule validation tests.

### 4.6 Milestone 5: Multi-Chunk World

**Test categories:** Deterministic snapshot, integration, visual regression.

**Focus areas — deterministic:**
- Cross-boundary gravity: sand at position (0, 31, 16) in chunk (1,0,0) falls into chunk (1,0,0) at y=0, which has a stone floor. Same behavior as intra-chunk falling — no discontinuity.
- Cross-boundary reaction: fire at (31, 5, 16) in chunk (0,0,0) ignites wood at (0, 5, 16) in chunk (1,0,0). Combustion proceeds identically to intra-chunk combustion.
- Cross-boundary thermal: heat conducts from chunk (0,0,0) into chunk (1,0,0) without temperature discontinuity at the boundary.
- Chunk activation: place a single sand voxel in an otherwise static world. Verify that only the containing chunk and its immediate neighbors are in Active/Boundary state. All other chunks remain Static.
- Chunk sleep: after sand settles to rest, verify the chunk transitions to Static within the settling threshold (8 ticks of no change).
- Activity scan accuracy: run a scene where all activity stops. After the settling period, verify that zero chunks are in the Active state (C-SIM-8 — no false negatives).

**Focus areas — integration:**
- Chunk load/unload cycle: move the camera from one region to another. Verify chunks near the old position are unloaded and chunks near the new position are loaded. No crashes, no dangling references.
- Large world initialization: create a 256×128×256 world with terrain. Verify the world loads in under 5 seconds and renders without missing chunks.

**Focus areas — visual regression:**
- Multi-chunk scene rendered from a high-altitude camera: no visible seams between chunks.
- Cross-section view through a multi-chunk world: interior is revealed correctly at chunk boundaries.

**Minimum tests:** 8 deterministic snapshot tests, 3 integration tests, 3 visual regression tests.

### 4.7 Milestone 6: Pressure and Structural Integrity

**Test categories:** Deterministic snapshot, integration.

**Focus areas — deterministic:**
- Sealed container pressure: build a sealed metal box (6 faces, each face is a solid wall of metal voxels). Place a gas-producing reaction inside. After N ticks, all voxels inside the box have nonzero pressure. Voxels outside the box have zero pressure (no leak).
- Rupture: same sealed box, but run until pressure exceeds the metal's structural integrity. Verify the box breaks: at least one wall voxel is destroyed and pressure drops to near zero within M ticks (blast wave dissipates).
- Blast wave: rupture a pressurized container. Verify that loose materials (sand, water) within a radius are displaced outward. Materials beyond the blast radius are unaffected.
- Glass shatter: glass voxels in the blast radius shatter into glass-shard granular material.
- Structural collapse — keystone: build a 3-voxel arch (two pillars, one bridge voxel on top). Destroy the bridge voxel. Verify that if neither pillar was touching the bridge voxel's remaining neighbors, the disconnected voxel(s) fall. (Minimal scenario — the arch test from milestones.md acceptance criteria is the full version.)
- Structural collapse — thermal: a wooden beam supporting a stone block. Heat the beam until it ignites. After combustion completes (beam is now ash, which has zero structural integrity), the stone block falls.
- No false collapse: a stone block resting on a stone floor. Destroy a voxel that is not part of the support path. The block does not fall.

**Focus areas — integration:**
- Pressure + thermal interaction: heat a sealed container. Verify pressure increases (gas expansion). Continue heating until rupture.

**Minimum tests:** 9 deterministic snapshot tests, 2 integration tests.

### 4.8 Milestone 7: Player Tools and UI

**Test categories:** Integration (functional, not snapshot-based).

**Focus areas:**
- Brush placement: place a sphere of sand with radius 8. Count the placed voxels. Verify the count matches the expected volume of a discrete sphere (within ±5% due to voxelization).
- Brush removal: remove a cube of voxels with radius 4 from a filled region. Verify the removed region is empty and surrounding voxels are unaffected.
- Heat tool: apply the heat gun to a region. Verify temperature increases for affected voxels and does not change for unaffected voxels.
- Push tool: apply the force tool to a group of sand voxels. Verify they receive velocity in the expected direction.
- Material browser search: programmatically invoke the search with query "wat" and verify "Water" is in the filtered results and "Stone" is not.
- Simulation speed: set speed to 0.25x. Run for 1 wall-clock second. Verify tick count is approximately 15 (60 FPS × 0.25). Set speed to 4x. Verify tick count is approximately 240.

**Minimum tests:** 8 integration tests. (UI layout and visual polish are verified manually, not automated.)

### 4.9 Milestone 8: Save/Load

**Test categories:** Integration, deterministic snapshot.

**Focus areas:**
- Round-trip correctness: create a world with active simulation (fire, flowing water, a pressurized container). Save at tick N. Load the save. Run M more ticks. Separately, run the original world for N+M ticks without save/load. Compare the final states byte-for-byte. They must be identical.
- Format validation: load a save with an incorrect magic number. Verify it is rejected with a clear error.
- Rule set mismatch: save with rule set hash A, modify a rule, load with rule set hash B. Verify a warning is emitted but the save loads.
- Subregion export: export a 3-chunk bounding box from a larger world. Load the export into a fresh world. Verify the exported chunks are present and surrounding chunks are empty.
- Compression effectiveness: save a 256×128×256 world with ~30% fill. Verify the file size is under 50 MB (REQ 7.3.4).
- Corrupt data: truncate a save file mid-chunk. Verify the load fails gracefully with an error message, not a panic or crash.

**Minimum tests:** 8 integration tests.

### 4.10 Milestone 9: Material Expansion (200+)

**Test categories:** Rule validation (balancing suite).

**Focus areas:**
- No self-replication: for every material, place a single voxel in an empty world. Run 1000 ticks. Verify the voxel count has not increased (no material creates copies of itself without consuming another material).
- No runaway temperature: for every reaction rule that produces a temperature delta, verify it also consumes or transforms at least one input. Run a worst-case scenario (a region densely packed with the most exothermic materials) for 1000 ticks. Verify temperature does not exceed the quantization ceiling (8000 K).
- All combustion exhausts: ignite every flammable material. Verify combustion terminates (all fuel is consumed within 5000 ticks for a 32³ region).
- No infinite oscillation: for every pair (A, B) where A→B and B→A both exist as reaction rules, verify they are gated by different conditions (e.g., different temperature thresholds) so they cannot cycle indefinitely at any temperature.
- Category coverage: verify every category has at least 25 materials (REQ 5.2.3 says 50, but M9 targets 200 total, so 25 per category is proportional; M14 raises this to 50).

**Minimum tests:** 5 automated balancing tests (each scans all materials/rules). These are parametric tests, not individual test cases per material.

### 4.11 Milestone 10: Rendering Polish

**Test categories:** Visual regression, performance benchmark.

**Focus areas — visual regression:**
- Ambient occlusion: a scene with deep crevices and open surfaces. Reference screenshot verifies crevices are darker.
- Multiple dynamic lights: 8 light-emitting voxels of different colors. Reference screenshot verifies each light contributes visible, correctly colored illumination.
- Volumetric water: a pool of water with a stone floor. Reference screenshot from above verifies depth-dependent color darkening.
- Gas transparency: a smoke cloud between the camera and a solid object. Reference screenshot verifies the solid object is visible through the smoke with reduced opacity.
- LOD transition: a scene with near and far chunks. Reference screenshot from a medium distance verifies no visible pop-in or seams between LOD levels.

**Focus areas — performance:**
- 64 simultaneous lights: measure frame time. Must be under 16.6 ms at 1920×1080.
- Large water/gas volume: measure frame time with transparency compositing active. Identify the cost of transparent voxels vs. opaque.

**Minimum tests:** 6 visual regression tests, 2 performance benchmarks.

### 4.12 Milestone 11: Performance and Stress

**Test categories:** Performance benchmark.

**Focus areas:**
- Scaling curve: run the benchmark suite at 100K, 250K, 500K, 1M, 2M, 3M active voxels. Record per-pass simulation time and total frame time. Verify that simulation time scales approximately linearly with active voxel count (no quadratic blowup).
- Absolute targets: 1M at 60 FPS on RTX 4060-class (REQ 3.2.1). 3M at 60 FPS on RTX 4090-class (REQ 3.2.2).
- Memory usage: at 1M active voxels, GPU memory usage is under 500 MB. WASM memory usage is under 200 MB.
- Graceful degradation: artificially load the simulation beyond the hardware's capacity (e.g., 5M voxels on an RTX 4060). Verify that frame rate stays above 30 FPS (degradation engaged) and the simulation tick rate indicator reflects reduced tick rate.
- No memory leaks: run a session that creates and destroys many chunks (simulating player exploration). After 10 minutes, GPU memory usage is not significantly higher than the initial state with the same number of loaded chunks.

**Minimum tests:** 5 benchmark scenarios. Results are recorded as JSON baselines. Future runs flag regressions exceeding 10%.

### 4.13 Milestone 12: Modding

**Test categories:** Rule validation, integration.

**Focus areas — rule validation:**
- Valid mod loads successfully. New materials are accessible and functional.
- Mod with duplicate material ID (colliding with base game) is rejected.
- Mod with material ID in the base game range (outside the reserved mod range) is rejected.
- Two mods with conflicting rules: later-loaded mod wins. Warnings are logged.
- Malformed mod file: rejected with a clear error, does not crash the application.

**Focus areas — integration:**
- Load example mod, place new materials, verify interactions with base materials work.
- Unload mod (or restart without mod). Verify base game returns to normal behavior.
- Save a world containing modded materials. Load the save with the mod active: works correctly. Load the save without the mod: warning is emitted, modded materials are replaced with a fallback (air or a placeholder).

**Minimum tests:** 6 rule validation tests, 4 integration tests.

### 4.14 Milestones 13–15 (Optional)

Test plans for optional milestones follow the same structure. Key focus areas:

**M13 (Audio):** No automated audio tests. Manual verification that audio sources are spatialized and attenuated correctly. A test that verifies the audio scanner produces the correct number and type of audio sources for a known scene (functional test, not auditory).

**M14 (500+ Materials):** Extends M9 balancing suite. Same 5 parametric tests, now scanning 500+ materials and 10,000+ rules. Verify frame time impact is under 2 ms vs. the M9 baseline.

**M15 (Electrical):** Deterministic snapshot tests for circuit behavior. A wire carrying current heats a resistor. An AND gate produces correct output for all 4 input combinations. A short circuit causes thermal overload.

---

## 5. Pass/Fail Criteria

### 5.1 Test Failure Blocks Merge

Any test failure in the CI suite blocks the pull request from merging. There are no "known failures" or "flaky test" exceptions. If a test is genuinely flaky (intermittent failure unrelated to code changes), it is either fixed or removed. Flaky tests erode trust in the suite and train developers to ignore failures.

### 5.2 Snapshot Regeneration Requires Justification

If a code change causes a deterministic snapshot test or visual regression test to fail, the developer must either fix the regression or regenerate the snapshot with a commit message explaining why the behavior change is intentional. Bulk snapshot regeneration ("update all snapshots") without per-test justification is not permitted.

### 5.3 Performance Regression Threshold

A performance benchmark regression of more than 10% on any metric blocks merge (REQ 11.1.5). If the regression is intentional (e.g., a new simulation pass adds expected cost), the baseline is updated with justification.

### 5.4 Zero Tolerance for Panics

Any panic or crash in any test (including GPU validation errors, WASM traps, and out-of-bounds accesses) is a blocking failure regardless of which test was running. The application must fail gracefully with error messages, never with panics (C-RUST-5).

---

## 6. CI Pipeline Structure

### 6.1 Fast Path (Every Push)

- Compilation: `cargo build --target wasm32-unknown-unknown`
- Lint: `cargo clippy -- -D warnings`
- Format: `cargo fmt --check`
- Unit tests (CPU-only): `cargo test` (excludes GPU-dependent tests)
- WASM binary size check: fail if compressed size exceeds 10 MB

Target: under 5 minutes.

### 6.2 Full Path (Pre-Merge and Nightly)

- Everything in the fast path.
- GPU tests: deterministic snapshots, integration tests, visual regression.
- Performance benchmarks: run all scenes, compare against baselines.
- Browser smoke test: load WASM in headless Chrome and Firefox via Playwright, verify canvas renders without JavaScript errors.

Target: under 30 minutes.

### 6.3 Release Path (Pre-Release)

- Everything in the full path.
- Extended benchmark: all scenes at all voxel counts, 3 runs each, variance analysis.
- Cross-browser test: Chrome, Firefox, Edge (Safari if WebGPU is available).
- Save file compatibility: load saves from the previous release version and verify they load without error (backward compatibility smoke test, starting from M8 onward).

---

## 7. Test Coverage Philosophy

Alkahest does not target a line-coverage percentage. Coverage metrics incentivize trivial tests and miss the bugs that matter in a GPU-driven simulation (non-determinism, race conditions, visual artifacts, performance regressions).

Instead, coverage is measured by scenario coverage: every material interaction that ships in the game has at least one deterministic test exercising it. Every milestone's acceptance criteria (milestones.md) maps to at least one automated test. The mapping is maintained in a traceability table:

| Milestone Acceptance Criterion | Test File | Test Function |
|---|---|---|
| M2: sand falls to floor | `tests/determinism/single_chunk.rs` | `test_sand_falls_to_floor` |
| M2: deterministic across runs | `tests/determinism/single_chunk.rs` | `test_competing_sand_determinism` |
| M3: fire + wood → ash + smoke | `tests/determinism/reactions.rs` | `test_fire_wood_combustion` |
| ... | ... | ... |

This table is populated as test cases are written during each milestone. Gaps in the table (acceptance criteria with no corresponding test) are treated as blocking issues for milestone completion.

---

## 8. Debugging Support

Tests exist to catch bugs, but debugging GPU simulation code requires additional tooling beyond pass/fail tests.

### 8.1 Debug Buffer

A small (4 KB) GPU storage buffer reserved for diagnostic writes from any shader (C-GPU-10). The test harness reads this buffer after each tick and includes its contents in failure reports. Shaders can write voxel coordinates, intermediate values, or pass identifiers into this buffer to trace execution. The debug buffer is active in debug builds and compiled out in release builds.

### 8.2 Voxel State Dump

On deterministic test failure, the harness dumps both the expected and actual voxel buffers as binary files and generates a human-readable diff showing which voxels differ, their coordinates, and the old vs. new values for each field (material ID, temperature, pressure, velocity, flags). This diff is the primary debugging artifact for simulation regressions.

### 8.3 Visual Diff

On visual regression test failure, the harness produces three images: the expected screenshot, the actual screenshot, and a difference image highlighting pixels that exceed the tolerance threshold. All three are saved alongside the test output for manual inspection.
