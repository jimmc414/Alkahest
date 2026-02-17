# ALKAHEST: Test Strategy

**Version:** 1.0.0
**Date:** 2026-02-16
**Status:** Complete
**Companions:** requirements.md, architecture.md, milestones.md, project-structure.md, technical-constraints.md

---

## 1. How to Use This Document

This document defines what is tested, how it is tested, and what constitutes pass/fail for each subsystem and milestone. All 191 tests are implemented as `#[cfg(test)]` blocks within each crate's source files (see project-structure.md Section 7).

All 15 milestones are complete. The full test suite of 191 tests covers all milestones cumulatively.

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

Each milestone lists the test categories required, specific areas of focus, and test counts. All tests have been implemented and are part of the CI suite.

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

### 4.14 Milestone 13: Audio

**Test categories:** Functional (scanner behavior, system lifecycle).

**Actual tests (10 in alkahest-audio):**
- Audio system enabled/disabled toggle and no-op behavior
- Scanner: activity resets idle counter, neighbor activation, event decay, max event cap, intensity scaling
- Scanner: multiple simultaneous event categories
- System lifecycle: register, update, clear

### 4.15 Milestone 14: 500+ Materials

**Test categories:** Rule validation (balancing suite extended).

**Actual tests:** The M9 balancing tests now validate all 561 materials and 11,998 rules. Tests `test_material_count_minimum` and `test_rule_count_minimum` enforce the target counts.

### 4.16 Milestone 15: Electrical

**Test categories:** Unit tests (buffer layout, balancing, shader data).

**Actual tests (5 in alkahest-sim, 4 in alkahest-rules):**
- `test_charge_slot_size` — verifies charge buffer is 128 KB per chunk (32^3 * 4 bytes)
- `test_charge_slot_byte_offset` — verifies byte offset calculations for charge buffer slots
- `test_electrical_materials_exist` — verifies electrical materials (wire, resistor, switch, AND gate) are present in the material table
- `test_electrical_conductivity_valid` — verifies all electrical materials have valid conductivity values
- `test_electrical_conductivity_out_of_range_rejected` — verifies conductivity out of range is rejected by validator
- `test_electrical_resistance_out_of_range_rejected` — verifies resistance out of range is rejected by validator
- `test_electrical_cfl_stability` — verifies electrical CFL stability condition is checked
- `test_cfl_stability_validated` — verifies combined thermal+electrical CFL validation
- `test_activation_threshold_out_of_range_rejected` — verifies activation threshold bounds checking

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

As implemented in `ci/test.sh`:

- Lint (WASM target): `cargo clippy --workspace --exclude alkahest-bench --target wasm32-unknown-unknown -- -D warnings`
- Lint (native, bench only): `cargo clippy -p alkahest-bench -- -D warnings`
- Format: `cargo fmt --all -- --check`
- Unit tests (CPU-only, excludes web/bench): `cargo test --workspace --exclude alkahest-web --exclude alkahest-bench`

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

Instead, coverage is measured by scenario coverage: every material interaction that ships in the game has at least one deterministic test exercising it. Every milestone's acceptance criteria (milestones.md) maps to at least one automated test. The mapping is maintained in the traceability table below.

| Milestone Acceptance Criterion | Test File | Test Function(s) |
|---|---|---|
| M0: Voxel data packing (8 bytes) | `crates/alkahest-core/src/math.rs` | `test_pack_unpack_zeros`, `test_pack_unpack_typical_voxel`, `test_pack_unpack_max_values`, `test_pack_unpack_negative_velocities`, `test_pack_unpack_vel_x_boundary_cases`, `test_voxel_data_size` |
| M0: Temperature quantization | `crates/alkahest-core/src/math.rs` | `test_temp_roundtrip_ambient`, `test_temp_edge_cases`, `test_phase_as_f32` |
| M0: Coordinate conversions | `crates/alkahest-core/src/math.rs` | `test_world_to_chunk_positive`, `test_world_to_chunk_negative`, `test_world_to_local_positive`, `test_world_to_local_negative`, `test_chunk_local_roundtrip` |
| M1: Render lighting | `crates/alkahest-render/src/lighting.rs` | `test_light_config_default`, `test_light_config_size`, `test_gpu_point_light_size` |
| M1: Ambient occlusion | `crates/alkahest-render/src/ao.rs` | `test_ao_range` |
| M1: Sky rendering | `crates/alkahest-render/src/sky.rs` | `test_sky_colors_valid` |
| M1: Transparency compositing | `crates/alkahest-render/src/transparency.rs` | `test_volume_clamp` |
| M1: Octree construction | `crates/alkahest-render/src/octree.rs` | `test_empty_octree`, `test_rebuild_all_empty`, `test_rebuild_with_occupied_chunks`, `test_corner_offset` |
| M2: Direction system (26 neighbors) | `crates/alkahest-core/src/direction.rs` | `test_all_directions_count`, `test_all_directions_unique`, `test_direction_kinds`, `test_all_gravity_directions_go_down`, `test_down_offset`, `test_no_zero_offset`, `test_gravity_directions_order` |
| M2: Double-buffer management | `crates/alkahest-sim/src/buffers.rs` | `test_chunk_buffer_size`, `test_slot_byte_offset`, `test_pool_slot_allocation_and_free`, `test_descriptor_data_layout` |
| M2: Checkerboard conflict resolution | `crates/alkahest-sim/src/conflict.rs` | `test_checkerboard_no_conflict`, `test_gravity_schedule_first_is_down`, `test_gravity_schedule_length`, `test_gravity_schedule_parities_alternate`, `test_movement_schedule_length`, `test_movement_schedule_parities`, `test_movement_schedule_has_lateral`, `test_movement_schedule_has_rise`, `test_movement_uniforms_size`, `test_movement_schedule_length` |
| M2: Deterministic PRNG | `crates/alkahest-sim/src/rng.rs` | `test_deterministic`, `test_different_inputs_differ`, `test_hash_to_float_range`, `test_distribution` |
| M2: Test harness infrastructure | `crates/alkahest-sim/src/test_harness.rs` | `test_prng_determinism_across_ticks`, `test_prng_symmetry_broken` |
| M2: Camera modes | `crates/alkahest-web/src/camera.rs` | `test_orbit_eye_position`, `test_camera_mode_toggle`, `test_camera_state_size`, `test_sim_speed_4x` |
| M2: Tool system | `crates/alkahest-web/src/tools/mod.rs` | `test_tool_state_default`, `test_active_tool_names`, `test_enabled_toggle`, `test_sim_speed_quarter` |
| M3: Material definitions load | `crates/alkahest-rules/src/loader.rs` | `test_load_single_material`, `test_load_single_rule`, `test_valid_materials_load`, `test_valid_rules_load`, `test_load_all_merges`, `test_new_materials_load` |
| M3: Material ID uniqueness | `crates/alkahest-rules/src/validator.rs` | `test_duplicate_material_id_rejected` |
| M3: Rule validation | `crates/alkahest-rules/src/validator.rs` | `test_nonexistent_material_ref_rejected`, `test_property_exceeds_quantization_rejected`, `test_energy_from_nothing_rejected`, `test_infinite_loop_detected`, `test_malformed_ron_rejected` |
| M3: Material table access | `crates/alkahest-core/src/material.rs` | `test_material_table_get`, `test_material_id_mapping` |
| M3: GPU rule data compilation | `crates/alkahest-rules/src/compiler.rs` | `test_gpu_data_format`, `test_remap_assigns_contiguous_ids` |
| M4: CFL stability check | `crates/alkahest-rules/src/validator.rs` | `test_cfl_stability_validated`, `test_thermal_conductivity_range` |
| M5: Chunk map spatial queries | `crates/alkahest-world/src/chunk_map.rs` | `test_chunk_map_spatial_queries`, `test_clear_removes_all` |
| M5: Chunk state transitions | `crates/alkahest-world/src/state_machine.rs` | `test_sleep_after_idle_ticks`, `test_register_and_update`, `test_activity_resets_idle` |
| M5: Dispatch list | `crates/alkahest-world/src/dispatch.rs` | `test_dispatch_list_construction`, `test_dispatch_list_active_only`, `test_fp_movement_in_empty_world` |
| M5: Terrain generation | `crates/alkahest-world/src/terrain.rs` | `test_terrain_chunk_size`, `test_terrain_deterministic`, `test_terrain_generates_stone_sand_water`, `test_air_chunk_above_terrain`, `test_fill_detection_all_air` |
| M6: Structural integrity | `crates/alkahest-sim/src/structural.rs` | `test_structural_empty_chunk`, `test_structural_flood_fill_connected`, `test_structural_flood_fill_disconnect`, `test_structural_flood_fill_bounded`, `test_structural_mixed_materials` |
| M7: Brush system | `crates/alkahest-web/src/tools/brush.rs` | `test_brush_shape_cycle`, `test_brush_radius_increase_clamp`, `test_brush_radius_decrease_clamp`, `test_brush_auto_shape_on_radius_increase`, `test_brush_reset_shape_on_radius_zero`, `test_cube_voxel_count_r4`, `test_sphere_voxel_count_r8`, `test_brush_shape_gpu_values` |
| M7: Material browser search | `crates/alkahest-web/src/ui/browser.rs` | `test_browser_search_water`, `test_browser_search_no_results`, `test_browser_search_case_insensitive`, `test_browser_search_empty`, `test_browser_sorted_by_id` |
| M8: Save/load round-trip | `crates/alkahest-persist/src/save.rs` | `test_save_load_roundtrip`, `test_save_header_fields_correct` |
| M8: Save format validation | `crates/alkahest-persist/src/format.rs` | `test_header_size`, `test_chunk_has_content` |
| M8: Load error handling | `crates/alkahest-persist/src/load.rs` | `test_invalid_magic_rejected`, `test_unsupported_version_rejected`, `test_truncated_file_rejected`, `test_file_too_small_rejected`, `test_valid_header_no_warnings` |
| M8: Compression | `crates/alkahest-persist/src/compress.rs` | `test_compress_decompress_roundtrip`, `test_compressed_size_sanity`, `test_fill_encode_decode`, `test_fill_detection_mixed`, `test_is_fill_rejects_non_fill`, `test_save_produces_valid_binary` |
| M8: Save compatibility | `crates/alkahest-persist/src/compat.rs` | `test_rule_hash_deterministic`, `test_rule_hash_changes_on_modification`, `test_rule_hash_mismatch_warns` |
| M8: Subregion export | `crates/alkahest-persist/src/subregion.rs` | `test_subregion_filters_correctly`, `test_subregion_output_is_loadable` |
| M8: Save/load with fill optimization | `crates/alkahest-persist/src/save.rs` | `test_save_load_fill_optimization`, `test_save_load_empty_world` |
| M9: Balancing (no self-replication) | `crates/alkahest-rules/src/balancing.rs` | `test_no_self_replication` |
| M9: Balancing (no runaway temp) | `crates/alkahest-rules/src/balancing.rs` | `test_no_runaway_temperature` |
| M9: Balancing (combustion exhausts) | `crates/alkahest-rules/src/balancing.rs` | `test_all_combustion_exhausts` |
| M9: Balancing (no oscillation) | `crates/alkahest-rules/src/balancing.rs` | `test_no_multi_step_oscillation` |
| M9: Balancing (category coverage) | `crates/alkahest-rules/src/balancing.rs` | `test_category_coverage` |
| M9: Interaction matrix distribution | `crates/alkahest-rules/src/balancing.rs` | `test_interaction_matrix_distribution` |
| M9: Material/rule count targets | `crates/alkahest-rules/src/balancing.rs` | `test_material_count_minimum`, `test_rule_count_minimum` |
| M9: Default behaviors | `crates/alkahest-rules/src/defaults.rs` | `test_multiple_categories`, `test_category_ranges` |
| M12: Mod manifest parsing | `crates/alkahest-core/src/mod_manifest.rs` | `test_mod_manifest_parse`, `test_mod_manifest_malformed_rejected` |
| M12: Mod loading and merging | `crates/alkahest-rules/src/loader.rs` | `test_mod_load_example`, `test_merge_mod_into_base` |
| M12: Mod ID validation | `crates/alkahest-rules/src/validator.rs` | `test_mod_valid_id_accepted`, `test_mod_id_below_range_rejected`, `test_mod_duplicate_with_base_detected_after_merge` |
| M12: ID remapping | `crates/alkahest-rules/src/migration.rs` | `test_remap_assigns_contiguous_ids`, `test_remap_preserves_base_ids`, `test_remap_cross_references`, `test_remap_rules_updates_all_refs`, `test_remap_idempotent`, `test_remap_reverse_lookup` |
| M12: Mod materials pass balancing | `crates/alkahest-rules/src/balancing.rs` | `test_mod_materials_pass_balancing` |
| M13: Audio system lifecycle | `crates/alkahest-audio/src/lib.rs` | `test_audio_system_disabled_noop`, `test_enabled_toggle`, `test_max_events_cap` |
| M13: Acoustic event scanning | `crates/alkahest-audio/src/scanner.rs` | `test_activity_resets_idle`, `test_neighbor_activation_on_activity`, `test_event_decay`, `test_intensity_scaling`, `test_absorption_bias_relative_order`, `test_multiple_categories`, `test_max_events_cap` |
| M15: Charge buffer layout | `crates/alkahest-sim/src/buffers.rs` | `test_charge_slot_size`, `test_charge_slot_byte_offset` |
| M15: Electrical materials exist | `crates/alkahest-rules/src/balancing.rs` | `test_electrical_materials_exist` |
| M15: Electrical conductivity valid | `crates/alkahest-rules/src/balancing.rs` | `test_electrical_conductivity_valid` |
| M15: Electrical CFL stability | `crates/alkahest-rules/src/balancing.rs` | `test_electrical_cfl_stability` |
| M15: Electrical validation | `crates/alkahest-rules/src/validator.rs` | `test_electrical_conductivity_out_of_range_rejected`, `test_electrical_resistance_out_of_range_rejected`, `test_activation_threshold_out_of_range_rejected` |
| M15: Explosives rules | `crates/alkahest-rules/src/loader.rs` | `test_gunpowder_rule_loads` |

---

## 8. Debugging Support

Tests exist to catch bugs, but debugging GPU simulation code requires additional tooling beyond pass/fail tests.

### 8.1 Debug Buffer

A small (4 KB) GPU storage buffer reserved for diagnostic writes from any shader (C-GPU-10). The test harness reads this buffer after each tick and includes its contents in failure reports. Shaders can write voxel coordinates, intermediate values, or pass identifiers into this buffer to trace execution. The debug buffer is active in debug builds and compiled out in release builds.

### 8.2 Voxel State Dump

On deterministic test failure, the harness dumps both the expected and actual voxel buffers as binary files and generates a human-readable diff showing which voxels differ, their coordinates, and the old vs. new values for each field (material ID, temperature, pressure, velocity, flags). This diff is the primary debugging artifact for simulation regressions.

### 8.3 Visual Diff

On visual regression test failure, the harness produces three images: the expected screenshot, the actual screenshot, and a difference image highlighting pixels that exceed the tolerance threshold. All three are saved alongside the test output for manual inspection.
