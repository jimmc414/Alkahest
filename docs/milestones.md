# ALKAHEST: Milestone Plan

**Version:** 1.0.0
**Date:** 2026-02-16
**Status:** Complete — all milestones delivered
**Companions:** requirements.md v1.0.0, architecture.md v0.1.0

---

## 1. How to Use This Document

All 16 milestones (M0-M15) are complete. This document is now a historical record of the build plan and acceptance criteria used during development.

Each milestone produced a working, testable artifact. Milestones were sequential — milestone N+1 was not begun until milestone N passed all acceptance criteria. Within a milestone, tasks were ordered by dependency.

"Acceptance criteria" means automated tests unless stated otherwise. "Visual verification" means a human looks at the output and confirms it matches the described behavior — these were converted to screenshot regression tests when the renderer stabilized.

Architecture references (e.g., ARCH 5.2) point to architecture.md sections. Requirement references (e.g., REQ 4.1.2) point to requirements.md sections.

### Completion Summary

| Milestone | Name | Key Deliverable |
|-----------|------|-----------------|
| M0 | Toolchain and Empty Window | Rust/WASM/WebGPU pipeline, egui debug panel |
| M1 | Static Voxel Rendering | Ray march shader, free-orbit camera, single-light shadows |
| M2 | Gravity Simulation | Double-buffered compute pipeline, checkerboard conflict resolution |
| M3 | Multi-Material and Reactions | Data-driven rule engine, 10 materials, 15+ rules |
| M4 | Thermal System | Heat diffusion, phase transitions, convection |
| M5 | Multi-Chunk World | Chunk state machine, cross-chunk simulation, octree rendering |
| M6 | Pressure and Structural | Pressure accumulation, rupture/explosions, structural collapse |
| M7 | Player Tools and UI | Brush system, material browser, cross-section view, first-person camera |
| M8 | Save/Load | LZ4 compressed binary format, auto-save, subregion export |
| M9 | Material Expansion (200+) | 200+ materials, 2000+ rules, balancing test suite |
| M10 | Rendering Polish | Ambient occlusion, 64 dynamic lights, volumetric transparency, LOD |
| M11 | Performance and Stress | 1M voxels at 60 FPS, graceful degradation |
| M12 | Modding Support | Mod loader, validation, conflict resolution, example mod |
| M13 | Audio | Procedural spatialized audio driven by simulation state |
| M14 | Extended Materials (500+) | 561 materials, 11,995 rules across 8 categories |
| M15 | Electrical System | Charge propagation, logic gates, resistance heating, 10 electrical materials |

---

## 2. Milestone 0: Toolchain and Empty Window

**Status: Complete**

**Goal:** Confirm the full build pipeline works end-to-end before writing any engine code. This milestone has zero game logic. It exists to surface toolchain problems early.

**Deliverable:** A Rust project that compiles to WASM, loads in a browser, acquires a WebGPU device and surface, clears the screen to a solid color, and displays an egui panel showing GPU adapter info and frame time.

### Tasks

0.1. Initialize the Rust workspace with cargo. Create crate structure: `alkahest-core` (library), `alkahest-web` (WASM entry point). Configure wasm-pack and wasm-bindgen.

0.2. Add wgpu as a dependency. Write the WebGPU initialization sequence: request adapter, request device with compute and render features, configure the surface for the canvas element.

0.3. Write a render loop using requestAnimationFrame. Each frame: get the current surface texture, create a command encoder, submit a render pass that clears to a color, present the surface.

0.4. Integrate egui via egui-wgpu. Render a debug panel showing: adapter name, backend (WebGPU vs. WebGL fallback), frame time in ms, and FPS.

0.5. Set up the CI pipeline: cargo clippy, cargo fmt --check, cargo test (no meaningful tests yet, but the harness runs), wasm-pack build. Confirm the WASM binary loads in Chrome and Firefox.

### Acceptance Criteria

- `wasm-pack build --release` succeeds with zero warnings.
- The WASM module loads in Chrome 120+ and Firefox 130+ and displays a colored canvas with the egui debug panel.
- The egui panel reports a WebGPU adapter (not WebGL fallback).
- Frame time is below 2 ms (we're doing nothing — this just validates there's no gross overhead in the WASM-WebGPU bridge).

---

## 3. Milestone 1: Static Voxel Rendering

**Status: Complete**

**Goal:** Render a single chunk of static voxels using the ray march pipeline described in ARCH 7.1. No simulation. The voxels are hardcoded in a buffer and never change. This milestone validates the core rendering approach.

**Deliverable:** A 32×32×32 chunk of voxels displayed on screen via GPU ray marching, with a free-orbit camera and basic direct lighting from a single hardcoded light source.

### Tasks

1.1. Define the voxel data layout (ARCH 3.1): 8 bytes per voxel, packed as specified. Write Rust-side code to create a 32³ voxel buffer with a test pattern — a floor of "stone," a pile of "sand" sitting on it, and a few "light" emitter voxels. Upload this buffer to a GPU storage buffer.

1.2. Write the ray march compute (or fullscreen fragment) shader. The shader receives camera position and orientation as uniforms, casts one ray per pixel, and steps through the 32³ grid using DDA. On hit, output the voxel's color. On miss, output a sky color.

1.3. Implement the free-orbit camera: mouse drag to rotate, scroll to zoom, middle-mouse to pan. Camera state lives on the CPU and is uploaded as a uniform buffer each frame.

1.4. Add basic direct lighting: one hardcoded point light. The ray march shader, on hitting a voxel, traces a shadow ray toward the light source through the same grid. If unoccluded, apply diffuse shading (normal is derived from which face of the voxel the primary ray entered). If occluded, apply ambient-only.

1.5. Add chunk-edge visualization: render a faint wireframe cube around the 32³ region so the chunk boundary is visible during development.

### Acceptance Criteria

- A 32³ volume renders on screen with correct perspective and no visual artifacts (no holes in surfaces, no z-fighting).
- Camera orbit, zoom, and pan are responsive and correct (no axis inversion, no gimbal lock at poles).
- Shadow from the point light is visible and geometrically correct: a voxel tower casts a shadow on the floor behind it.
- Frame time is below 4 ms for the 32³ chunk at 1920×1080. (This is generous — if it's above 4 ms for a single chunk, the ray marcher has a fundamental problem.)
- Visual verification: take reference screenshots of the test scene from 4 camera angles. These become the baseline for rendering regression tests.

---

## 4. Milestone 2: Simulation Loop — Gravity Only

**Status: Complete**

**Goal:** Get the simulation compute pipeline running with the simplest possible physics: sand falls due to gravity. This validates double buffering, the compute dispatch pipeline, and the simulation-to-renderer data flow.

**Deliverable:** Sand voxels fall downward and pile up on stone floor voxels. The player sees voxels move in real time.

### Tasks

2.1. Implement double buffering (ARCH 5.1): allocate two voxel state buffers on the GPU. The simulation reads from one and writes to the other. Swap each tick. The renderer reads from whichever was most recently written.

2.2. Write the movement compute shader (ARCH 5.2, Pass 2 only). For this milestone, implement only downward gravity: each sand voxel checks the voxel directly below it. If empty, swap positions. If occupied by another sand voxel, check the two downward-diagonal positions and swap if empty.

2.3. Implement the checkerboard sub-pass pattern (ARCH 5.3) for conflict resolution: process even-column voxels first, then odd-column voxels. This prevents two sand voxels from both trying to fall into the same empty cell.

2.4. Wire the simulation dispatch into the frame loop: each frame, dispatch the simulation compute shader, then dispatch the render pass reading from the updated buffer.

2.5. Add a simulation tick counter to the egui debug panel. Add pause/resume (spacebar) and single-step (period key) controls.

2.6. Create a deterministic test harness: a Rust function that initializes a known voxel configuration, runs N simulation ticks on the GPU, reads back the result, and compares against a known-good snapshot. Write at least 3 test cases: (a) single sand voxel falls to floor, (b) column of sand collapses into a pile, (c) sand on a ledge avalanches diagonally.

### Acceptance Criteria

- Sand voxels fall and pile on the floor with no visual glitches (no flickering, no voxels stuck mid-air, no interpenetration).
- Pausing stops all movement. Single-step advances exactly one tick (verified by the tick counter incrementing by 1).
- The deterministic test harness passes: running the same initial state for N ticks produces bit-identical output across 10 consecutive runs.
- Frame time remains below 8 ms with the full 32³ chunk active (32,768 voxels being simulated and rendered every frame).

---

## 5. Milestone 3: Multi-Material and Basic Reactions

**Status: Complete**

**Goal:** Introduce the data-driven rule engine and multiple materials. This is the milestone where the material definition schema and interaction matrix are designed, loaded from files, and uploaded to the GPU. The simulation moves from hardcoded behavior to data-driven behavior.

**Deliverable:** At least 10 materials with distinct physical behaviors and at least 15 pairwise interaction rules, all defined in external data files, simulated on the GPU, and visually distinguishable on screen.

### Starting Material Set

The following 10 materials exercise all the physics categories that later milestones will expand:

Air (empty), Stone (static solid), Sand (falling granular), Water (flowing liquid), Oil (flowing liquid, lighter than water, flammable), Fire (short-lived, consumes flammable materials, emits light), Smoke (rising gas, dissipates over time), Steam (rising gas, condenses to water when cooled), Wood (static solid, flammable), Ash (falling granular, byproduct of combustion).

### Tasks

3.1. Design the material definition file schema (ARCH 6.2). Write the 10 material definitions in RON format. Each definition specifies all properties listed in ARCH 6.2.

3.2. Design the interaction rule file schema (ARCH 6.3). Write at least 15 interaction rules covering: fire + wood → fire + ash (combustion), fire + oil → fire + smoke (oil combustion), fire + water → steam (extinguishing), water + hot voxel → steam (evaporation), steam + cold region → water (condensation), sand displaces water (density), oil floats on water (density), smoke rises and dissipates to air after N ticks.

3.3. Write the rule compiler (CPU, Rust): parse material and rule files, validate (ARCH 17.3), build the interaction lookup texture and rule buffer, upload to GPU.

3.4. Rewrite the movement shader to be density-driven: instead of hardcoded "sand falls," the shader reads density from the material table. Denser materials displace lighter ones. Liquids flow laterally. Gases rise. This generalizes movement to work for any material defined in the data files.

3.5. Write the reaction compute shader (ARCH 5.2, Pass 3): for each voxel, check all face-adjacent neighbors against the interaction matrix. If a matching rule is found, apply the rule's outputs (material conversion, byproduct spawning, temperature deltas).

3.6. Wire Pass 2 (movement) and Pass 3 (reactions) into the simulation loop in the correct order.

3.7. Add distinct colors per material so they're visually distinguishable. Fire and emitter materials should emit light (extend the renderer to support multiple point lights sourced from emissive voxels — a simplified version that scans for emissive voxels on the CPU and uploads their positions as a light list, not per-voxel lighting yet).

3.8. Extend the deterministic test harness with reaction tests: (a) fire placed adjacent to wood produces ash and smoke, (b) water poured on fire produces steam, (c) sand sinks through water.

### Acceptance Criteria

- All 10 materials behave as described when placed in the world.
- Fire spreads along a row of wood, leaving ash behind. Smoke rises from the combustion.
- Water extinguishes fire and produces steam. Steam rises and eventually condenses if it reaches a cooler region (top of the chunk).
- Sand sinks through water; oil floats on water.
- Modifying a material definition in the RON file and reloading changes the in-game behavior without recompiling the engine.
- All 15+ interaction rules are present in the data files and exercised by the test harness.
- Rule validation rejects a malformed rule file with a clear error message.
- Frame time below 10 ms at 1920×1080 with a 32³ chunk containing a mix of active materials.

---

## 6. Milestone 4: Thermal System

**Status: Complete**

**Goal:** Temperature becomes a live simulation property. Materials conduct, radiate, and respond to heat. This is the first field-propagation system and validates the ARCH 5.2 Pass 4 pipeline.

**Deliverable:** Heat flows through materials at material-dependent rates. Temperature-driven state transitions (melting, boiling, ignition) work correctly.

### Tasks

4.1. Write the thermal diffusion compute shader (ARCH 8.1): each voxel's temperature moves toward the weighted average of its neighbors' temperatures, scaled by thermal conductivity. Implement the entropy drain toward ambient (ARCH 8.2).

4.2. Validate the CFL stability condition at rule-load time (ARCH 8.1): if the global diffusion rate combined with any material's conductivity would cause oscillation, log a warning and clamp.

4.3. Add temperature-driven state transitions to the reaction pass: ice (new material) melts to water above 273 K, water boils to steam above 373 K, stone melts to lava (new material) above 1500 K, lava cools to stone below 1200 K (hysteresis gap to prevent oscillation at the boundary).

4.4. Add the "heat gun" and "freeze" tools (REQ 7.1.6): player can paint temperature changes onto existing voxels. Implement as commands in the player command buffer (ARCH 14.1).

4.5. Implement convection approximation (ARCH 8.3): heated liquids and gases receive an upward velocity bias.

4.6. Add a temperature visualization mode: a toggle that overlays a heatmap (blue-to-red gradient) on all voxels, replacing their material color. This is essential for debugging thermal behavior and useful as a gameplay feature.

4.7. Extend the test harness: (a) a metal rod with one end heated reaches thermal equilibrium across its length, (b) ice in a warm environment melts to water, (c) water on lava boils to steam and the lava cools to stone, (d) heated gas rises (convection).

### Acceptance Criteria

- A line of metal voxels heated at one end shows a visible temperature gradient propagating to the other end over multiple seconds. The gradient stabilizes (no oscillation).
- Placing ice near fire: ice melts to water, water flows, water near fire boils to steam, steam rises.
- Lava placed on stone: stone adjacent to lava heats up. If it crosses the melting threshold, it becomes lava (chain reaction). The reaction eventually stops as thermal energy dissipates.
- The heatmap visualization correctly shows temperature distribution.
- Frame time below 12 ms with thermal diffusion active across a full 32³ chunk.

---

## 7. Milestone 5: Multi-Chunk World

**Status: Complete**

**Goal:** Expand from a single 32³ chunk to a multi-chunk world. This is a major architectural milestone — it introduces the chunk state machine, sparse allocation, cross-chunk neighbor lookups, and the activity scan. Expect this to be the most difficult milestone.

**Deliverable:** A world of at least 8×4×8 chunks (256×128×256 voxels) with simulation and rendering working correctly across chunk boundaries.

### Tasks

5.1. Implement the chunk hash map (ARCH 4.1): a CPU-side data structure mapping chunk coordinates to GPU buffer offsets. Allocate a large GPU buffer (or buffer pool) subdivided into chunk-sized slots.

5.2. Implement chunk state machine (ARCH 4.2): Unloaded, Loaded-Static, Loaded-Active, Loaded-Boundary. Write the state transition logic on the CPU side.

5.3. Modify the simulation shaders to handle chunk boundaries: when a voxel at the edge of a chunk needs to read a neighbor, the shader must index into the adjacent chunk's buffer region. This requires passing a chunk neighbor table (for each chunk in the dispatch list, the buffer offsets of its 26 neighboring chunks) as a storage buffer.

5.4. Implement halo loading (ARCH 5.4): each workgroup loads the 1-voxel border from adjacent chunks into shared memory before running the simulation kernels. Profile shared memory usage and validate it fits within hardware limits.

5.5. Implement the activity scan pass (ARCH 5.2, Pass 5): a compute shader that checks whether any voxel in a chunk changed between the read and write buffers. Write per-chunk dirty flags to a staging buffer for CPU readback.

5.6. Implement async GPU readback (ARCH 12.2) for the activity scan results. Wire the readback into the chunk state machine: active chunks with no changes for 8 consecutive ticks transition to static.

5.7. Modify the renderer to ray march across multiple chunks. The ray must traverse the chunk grid, skip empty/unloaded chunks, and enter the per-voxel DDA within each occupied chunk it hits. This is where the rendering octree (ARCH 4.4) becomes necessary — build a minimal octree from loaded chunks to accelerate empty-space skipping.

5.8. Implement chunk loading/unloading based on camera distance: chunks beyond a configurable render distance are unloaded. Chunks entering render distance are allocated and initialized to air.

5.9. Create a large test scene: a terrain-like floor (stone and sand) spanning multiple chunks, with a pool of water and a forest of wood columns. Verify that fire spreads across chunk boundaries, water flows across chunk boundaries, and heat conducts across chunk boundaries without discontinuities.

5.10. Update the debug panel: show total loaded chunks, active chunks, boundary chunks, and total active voxel count.

### Acceptance Criteria

- A voxel placed at one chunk's edge correctly interacts with voxels in the adjacent chunk. No visible seams or simulation discontinuities at chunk boundaries.
- Fire lit in one chunk spreads into wood in an adjacent chunk without interruption.
- Water poured at a chunk boundary flows smoothly across it.
- Heat conducts across chunk boundaries without temperature discontinuity.
- Chunks with no activity transition to static and are excluded from simulation dispatch. The debug panel shows active chunk count dropping as activity settles.
- The test scene (256×128×256 world) renders at 60 FPS when most chunks are static and activity is localized to ~4-8 active chunks.
- The deterministic test harness works across chunk boundaries: a multi-chunk initial state produces identical results across runs.

---

## 8. Milestone 6: Pressure and Structural Integrity

**Status: Complete**

**Goal:** Add the final two physics subsystems. Pressure enables explosions and sealed-container behavior. Structural integrity enables realistic collapse of unsupported structures.

**Deliverable:** Enclosed pressurized containers that rupture when overstressed, and structures that collapse when their supports are destroyed.

### Tasks

6.1. Implement pressure accumulation and diffusion (ARCH 9.1, 9.2): the local enclosure heuristic (all 6 face-adjacent neighbors non-empty → enclosed), pressure equalization between enclosed voxels.

6.2. Implement rupture (ARCH 9.3): when pressure exceeds structural integrity, the voxel is destroyed and a blast wave propagates outward. Add blast wave propagation as a pressure + velocity impulse spreading radially over multiple ticks.

6.3. Add new materials to exercise pressure: Gunpowder (ignites from fire, generates extreme pressure on combustion), Sealed-Metal (high structural integrity, used to build pressure vessels), Glass (medium integrity, shatters at moderate pressure into a "glass shards" granular material).

6.4. Implement structural bond tracking (ARCH 10.1, 10.2): when a solid voxel is destroyed, trigger a local flood-fill on the CPU to check for disconnected components. If found, flag the disconnected voxels as falling.

6.5. Implement thermal bond weakening (ARCH 10.3): reduce bond strength when temperature exceeds a material-specific weakening threshold.

6.6. Test scenarios: (a) a sealed metal box filled with gunpowder. Light the gunpowder. Pressure builds until the box ruptures, sending a blast wave outward. (b) A stone arch. Destroy the keystone. The unsupported side collapses. (c) A wooden bridge. Set fire to the center. As wood burns to ash, the bridge weakens and the unsupported span falls.

### Acceptance Criteria

- A sealed metal container filled with gunpowder, when ignited, builds pressure over multiple ticks (visible in the debug voxel hover info) and eventually ruptures, scattering debris outward.
- Blast waves displace nearby loose materials (sand, water) and shatter glass.
- A stone arch with the keystone removed: the unsupported side falls within 1–2 seconds (flood-fill latency is acceptable). The supported side remains standing.
- A bridge with its center burned out: the unsupported spans on either side of the gap fall. The portions still connected to the abutments remain standing.
- Pressure does not leak through solid walls (verified by building a sealed chamber and confirming pressure equalizes inside but does not propagate outside).
- Frame time below 16 ms with pressure and structural systems active during an explosion event in a multi-chunk world.

---

## 9. Milestone 7: Player Tools and UI

**Status: Complete**

**Goal:** The game becomes playable as a sandbox. All core player interactions described in REQ 7 are implemented with a polished UI.

**Deliverable:** A complete tool palette, material browser, brush system, cross-section view, and simulation speed controls.

### Tasks

7.1. Implement the full brush system (REQ 7.1.3): single voxel, cube, sphere brushes with adjustable radius. Brush preview (ghost overlay showing where voxels will be placed before clicking).

7.2. Implement the material browser (REQ 7.2): a searchable, categorized panel listing all loaded materials. Clicking a material selects it as the active brush material. Each entry shows name, category, phase, and a short description.

7.3. Implement the directional force tool (REQ 7.1.7): "wind gun" that applies velocity to voxels in a cone.

7.4. Implement the cross-section view (REQ 6.2.2, ARCH 7.4): a slider that moves a clip plane along any axis, revealing the interior of the simulation. The clip plane is passed as a uniform to the ray march shader.

7.5. Implement simulation speed controls (REQ 7.1.5): a slider from 0.25x to 4x speed (adjusting ticks per frame). Display current speed in the HUD.

7.6. Implement the voxel hover info panel (REQ 8.1.4): show material name, temperature, pressure, velocity for the voxel under the cursor. Uses the GPU pick buffer (ARCH 14.2).

7.7. Implement keyboard shortcuts for all tools (REQ 8.1.3). Add a help overlay showing keybindings.

7.8. Implement the first-person camera mode (REQ 6.2.3): WASD + mouse look, with collision against solid voxels (simple ray-cast collision, not full physics).

7.9. Polish the HUD (REQ 8.1.2): FPS, active voxel count, simulation tick rate, current tool, current material, all displayed non-intrusively.

### Acceptance Criteria

- All brush shapes (single, cube, sphere) place and remove voxels correctly at all radii tested (1, 4, 8, 16).
- Brush preview accurately reflects what will be placed.
- Material browser search works (typing "wa" filters to "Water," "Wax," etc.).
- Cross-section view reveals the interior along each axis without visual artifacts.
- Simulation speed slider smoothly changes tick rate. 0.25x is noticeably slow; 4x is noticeably fast.
- Hover info panel updates in real time as the cursor moves and shows correct values.
- First-person camera does not clip through solid voxels.
- All actions are reachable via keyboard shortcuts.

---

## 10. Milestone 8: Save/Load and Persistence

**Status: Complete**

**Goal:** Players can save their worlds and load them back. This completes the core sandbox feature set.

**Deliverable:** Binary save/load with LZ4 compression, auto-save, and subregion export.

### Tasks

8.1. Implement the save file format (ARCH 13.1): header, chunk table, per-chunk LZ4-compressed voxel data.

8.2. Implement save: serialize all loaded chunks to the binary format. Run on a Web Worker (ARCH 12.3) to avoid blocking the main thread. Write to the File System Access API (with IndexedDB fallback).

8.3. Implement load: read the save file, validate the header (check magic number, format version, rule set hash), decompress chunks, upload to GPU, rebuild the chunk state machine and octree.

8.4. Implement rule set hash compatibility check (ARCH 13.3): on load, if the rule set hash doesn't match, display a warning dialog.

8.5. Implement auto-save (REQ 7.3.5): configurable interval (default: 5 minutes), non-blocking.

8.6. Implement subregion export (REQ 7.4.1): player selects a bounding box in the world, and only the chunks within that box are saved to a file.

8.7. Test: save a world with active simulation (fire, flowing water, pressurized container), load it back, verify simulation continues correctly from the saved state. The tick counter in the loaded save matches the saved tick count.

### Acceptance Criteria

- Save and load round-trip produces identical simulation state (verified by deterministic test: save at tick N, load, run M more ticks, compare against running the original N+M ticks without save/load).
- Save file size for a 256×128×256 world with ~30% fill is under 50 MB compressed.
- Saving does not cause a frame rate hitch (Web Worker isolation).
- Loading a save with a mismatched rule set hash shows a warning but still loads.
- Auto-save triggers at the configured interval without a visible frame drop.
- Subregion export produces a valid save file that can be loaded into a fresh world.

---

## 11. Milestone 9: Material Expansion and Balancing

**Status: Complete**

**Goal:** Scale from 10 materials to 200+ materials with a dense interaction matrix. This is the milestone where AI-assisted content generation (REQ 11.1.1) becomes the primary workflow.

**Deliverable:** 200+ materials, 2,000+ interaction rules, organized into the categories defined in REQ 5.2.2.

### Tasks

9.1. Define category-level default behaviors: "all metals conduct heat well and have high structural integrity," "all organics are flammable below a threshold," "all gases rise if lighter than air." These defaults reduce per-pair authoring.

9.2. Use LLM-assisted generation to produce candidate materials and interaction rules in bulk. The workflow: generate batches of 20–30 materials per category, review for physical plausibility and gameplay interest, adjust properties, add to the data files.

9.3. Use LLM-assisted generation to produce candidate interaction rules for material pairs that should have interesting behavior. Review, adjust probabilities and thresholds, add to the rule files.

9.4. Build a balancing test suite: automated scenarios that check for degenerate behaviors. Tests include: (a) no material self-replicates without energy input, (b) no reaction chain produces unbounded temperature, (c) all combustion reactions eventually exhaust fuel or oxygen, (d) no two materials enter an infinite oscillating loop (A→B→A).

9.5. Build a "reaction catalog" visualization: an offline tool (can be a simple HTML page) that renders the interaction matrix as a grid. Materials on both axes, cells colored by interaction type (combustion=red, dissolution=blue, no interaction=gray). This helps identify gaps (pairs that should interact but don't) and clusters (too many similar interactions).

9.6. Playtest each category to verify emergent behavior is interesting. Specific focus areas: can a player build a functioning furnace (fuel + ore → metal)? Can a player create a water purification chain (dirty water + filter material → clean water)? Can a player build a basic explosive device (fuel + oxidizer + containment)?

### Acceptance Criteria

- 200+ materials defined in data files, each with all required properties (REQ 5.1.1).
- 2,000+ interaction rules defined, passing validation.
- The balancing test suite passes with zero degenerate behaviors detected.
- The reaction catalog visualization shows a well-distributed interaction matrix (no single category dominating all interactions).
- Five documented emergent "recipes" that arise from the interaction rules but were not explicitly designed (discovered during playtesting).
- Frame time impact of 200 materials vs. 10 materials is less than 2 ms (the interaction lookup is O(1) per voxel pair regardless of material count — this test verifies the lookup texture approach scales).

---

## 12. Milestone 10: Rendering Polish

**Status: Complete**

**Goal:** Bring the visual quality up to release standard. The ray marcher works but looks basic after Milestone 1. This milestone adds the visual features that make the simulation readable and appealing.

**Deliverable:** Ambient occlusion, multiple dynamic lights, volumetric transparency, and LOD for distant chunks.

### Tasks

10.1. Implement voxel ambient occlusion (ARCH 7.2): compute a neighbor-occupancy value per voxel in a small radius. Use this to darken crevices and ground contact areas.

10.2. Implement multiple dynamic point lights from emissive voxels (ARCH 7.2): scan active chunks for emissive voxels, select the N brightest/nearest, upload their positions and colors as a light buffer. The ray march shader traces shadow rays for each light. Target: 64 simultaneous lights (REQ 6.1.3).

10.3. Implement volumetric transparency (ARCH 7.3): rays continue through transparent voxels (water, glass, gas), accumulating color and opacity via front-to-back compositing. Water should have visible depth-dependent color absorption (darker blue at greater depth).

10.4. Implement LOD rendering for distant chunks (ARCH 15.2): the octree stores averaged color/density at higher levels. The ray marcher terminates at a coarser octree level for distant rays.

10.5. Implement sky rendering: a simple gradient or procedural sky dome, visible through transparent materials and in empty space above the world. Serves as ambient lighting color source.

10.6. Performance pass: profile the full rendering pipeline with a complex scene (200+ materials, multiple light sources, water and gas volumes). Identify and optimize bottlenecks. Target: 60 FPS at 1920×1080 with a multi-chunk world.

### Acceptance Criteria

- Ambient occlusion is visually correct: corners and crevices are darker, exposed surfaces are brighter.
- 64 simultaneous emissive voxels produce visible, correctly colored, correctly shadowed lighting.
- Water volume has visible depth: looking down into a pool, the bottom is progressively darker. Submerged objects are tinted.
- Gas clouds (smoke, steam) are visually distinct from solid materials: semi-transparent, soft-edged.
- Distant chunks render at reduced detail without visible pop-in when transitioning between LOD levels.
- Full scene with all visual features enabled: 60 FPS at 1920×1080 on target hardware (RTX 4060-class).

---

## 13. Milestone 11: Performance and Stress Testing

**Status: Complete**

**Goal:** Validate REQ 3.2.1 (1M voxels at 60 FPS) and REQ 3.2.3 (graceful degradation). This is a dedicated optimization and hardening milestone.

**Deliverable:** Documented performance characteristics and working graceful degradation.

### Tasks

11.1. Build a benchmark suite: standardized test scenes at increasing voxel counts (100K, 250K, 500K, 1M, 2M, 3M active voxels). Each scene has a fixed camera position and a mix of material types. Benchmark records: simulation time, render time, total frame time, GPU memory usage.

11.2. Profile the simulation pipeline per-pass (movement, reactions, thermal, pressure, activity scan) at each voxel count. Identify which passes scale worst.

11.3. Optimize the worst-scaling passes. Common targets: reduce shared memory usage in halo loading, reduce branch divergence in the reaction shader, batch chunk dispatches more aggressively.

11.4. Implement graceful degradation (ARCH 15.4): frame time monitor that reduces simulation tick rate, then render resolution, then LOD thresholds, when the frame budget is exceeded. Implement the UI indicator showing degraded performance mode.

11.5. Test on minimum-spec hardware: integrated GPU (Intel Iris or AMD Radeon integrated). Document the achievable voxel count and visual quality at 30 FPS on integrated graphics.

11.6. Test on target-spec hardware (RTX 4060-class): validate 1M active voxels at 60 FPS.

11.7. Test on high-end hardware (RTX 4090-class): validate 3M active voxels at 60 FPS.

### Acceptance Criteria

- RTX 4060-class: 1M active voxels sustained at 60 FPS with all physics subsystems active. (REQ 3.2.1)
- RTX 4090-class: 3M active voxels sustained at 60 FPS. (REQ 3.2.2)
- Graceful degradation activates smoothly: no frame drops below 30 FPS. The simulation rate reduction is visible in the HUD but the game remains responsive.
- Benchmark suite results are documented and committed to the repository as a performance baseline.

---

## 14. Milestone 12: Modding Support

**Status: Complete**

**Goal:** External users can create and load custom materials and rule sets.

**Deliverable:** Documented mod format, mod loader with validation, conflict resolution, and at least one example community mod pack.

### Tasks

12.1. Finalize and document the material and rule file schemas (REQ 10.1.2). Write a modding guide with examples.

12.2. Implement the mod loader (ARCH 17.1): detect mod directories/zips, load materials into the reserved ID range, merge rules, log conflicts.

12.3. Implement multi-mod loading with configurable load order (REQ 10.1.4). Last-loaded-wins for conflicting rules, with warnings.

12.4. Implement mod validation (ARCH 17.3, REQ 10.1.3): reject malformed files, detect infinite loops, validate property ranges.

12.5. Create an example mod pack (20+ new materials, 50+ new rules) that demonstrates the modding capabilities.

12.6. Test: load the example mod alongside the base game. Verify new materials work correctly and interact with base materials as expected. Verify removing the mod returns the game to base behavior.

### Acceptance Criteria

- The example mod loads successfully and new materials are playable.
- A malformed mod file is rejected with a specific, actionable error message.
- Two mods with conflicting rules both load; the later-loaded mod's rules take precedence; warnings are logged for each conflict.
- The modding guide is sufficient for someone unfamiliar with the project to create a simple material pack (test with a fresh reader).

---

## 15. Milestone 13: Audio

**Status: Complete**

**Goal:** Procedural audio driven by simulation state.

**Deliverable:** Spatialized audio for fire, water, steam, explosions, and structural collapse.

### Tasks

13.1. Implement a simulation audio scanner: each frame, scan active chunks for acoustic events (fire density, water flow velocity, recent rupture events, recent collapse events). Output a list of audio source positions and types.

13.2. Implement procedural audio generators using the Web Audio API: crackling (fire), hissing (steam), flowing (water), rumble (collapse), boom (explosion). Each generator takes an intensity parameter driven by the simulation data.

13.3. Implement spatial audio: attenuate and pan audio sources based on distance and direction from the camera.

13.4. Mix and master: ensure simultaneous audio sources don't clip. Implement a priority system that fades out quiet/distant sources when the total source count is high.

### Acceptance Criteria

- Standing near fire produces a crackling sound. Moving away attenuates it.
- A waterfall (water flowing over an edge) produces a flowing sound proportional to flow rate.
- An explosion produces a boom followed by debris rumble.
- 20+ simultaneous audio sources play without clipping or stuttering.
- Audio can be disabled in settings with zero CPU cost (the scanner is skipped).

---

## 16. Milestone 14: Extended Material Set (500+)

**Status: Complete** — Final count: 561 materials, 11,995 interaction rules.

**Goal:** Hit the REQ 5.2.1 target of 500 materials and REQ 4.3.6 target of 10,000 interaction rules.

**Deliverable:** The full material library with dense interaction matrix, balanced and playtested.

### Tasks

14.1. Continue the AI-assisted generation workflow from Milestone 9, scaling from 200 to 500+ materials.

14.2. Scale the interaction rule set from 2,000 to 10,000+ rules.

14.3. Expand the balancing test suite to cover new materials and interaction chains.

14.4. Build a "discovery guide" system: in-game hints that suggest interesting material combinations for players who are overwhelmed by the material count.

14.5. Final balancing pass: run the full balancing test suite, playtest all categories, fix degenerate behaviors.

### Acceptance Criteria

- 500+ materials defined and validated.
- 10,000+ interaction rules defined and validated.
- Balancing test suite passes.
- The reaction catalog visualization shows no major gaps (categories with fewer than 50 materials each have proportional interaction coverage).
- Frame time impact remains negligible (confirmed by re-running the Milestone 11 benchmark suite).

---

## 17. Milestone 15: Electrical System

**Status: Complete**

**Goal:** Implement the electrical subsystem (REQ 4.4.5).

**Deliverable:** Electrical conductivity, signal propagation, resistance heating, and logic materials.

### Tasks

15.1. Add electrical charge as a simulation field (requires finding bits in the voxel layout or expanding voxel size — evaluate the performance impact of going from 8 to 10 bytes per voxel).

15.2. Implement charge propagation in the field propagation pass: charge flows through conductive materials, attenuates based on resistance, and generates heat proportional to current × resistance.

15.3. Add conductive materials: Copper Wire (high conductivity), Resistor-Paste (medium conductivity, high heat generation), Insulator-Coat (zero conductivity).

15.4. Implement logic materials (REQ 5.3): Signal-Sand (conducts only when receiving signal from two adjacent sources — AND gate analog), Toggle-ite (flips between two states when pulsed — memory cell analog). These must work purely through the existing automata rules, not a separate system.

15.5. Test: build a simple circuit (power source → wire → resistor → wire → ground). Verify current flows and the resistor heats up. Build a logic gate from Signal-Sand and verify correct truth table behavior.

### Acceptance Criteria

- Current flows through conductive materials and stops at insulators.
- Resistive materials heat up proportionally to current. Sufficient current can melt them (interaction with thermal system).
- A short circuit (direct connection between power and ground through low-resistance material) generates extreme heat, potentially causing a fire/explosion (interaction with pressure system).
- Signal-Sand AND gate produces correct output for all 4 input combinations.
- Toggle-ite retains state after the input pulse ends.

---

## 18. Milestone Dependency Graph

```
M0 (Toolchain)
 └─► M1 (Static Rendering)
      └─► M2 (Gravity Simulation)
           └─► M3 (Multi-Material + Reactions)
                ├─► M4 (Thermal System)
                │    └─► M5 (Multi-Chunk) ◄── hardest milestone
                │         ├─► M6 (Pressure + Structural)
                │         │    └─► M7 (Player Tools + UI)
                │         │         └─► M8 (Save/Load)
                │         │              └─► M9 (200+ Materials)
                │         │                   └─► M14 (500+ Materials)
                │         ├─► M10 (Rendering Polish)
                │         └─► M11 (Performance + Stress)
                │              └─► M12 (Modding)
                └─► M13 (Audio) [optional, can start after M5]
                └─► M15 (Electrical) [optional, can start after M6]
```

Critical path: M0 → M1 → M2 → M3 → M4 → M5 → M6 → M7 → M8 → M9 → M14.

M10 (Rendering Polish) and M11 (Performance) can run in parallel with M6–M8 if multiple work streams are available. M12 (Modding), M13 (Audio), and M15 (Electrical) are off the critical path and can be scheduled based on priorities.

---

## 19. Risk Assessment Per Milestone (with Retrospective)

| Milestone | Risk Level | Primary Risk | Retrospective |
|---|---|---|---|
| M0 | Low | wgpu WASM compilation issues; mitigate by testing early. | Completed as expected. |
| M1 | Medium | Ray march performance; if DDA in a 32³ grid is too slow, the entire rendering approach needs rethinking. | DDA performance was well within budget. |
| M2 | Medium | Checkerboard conflict resolution may produce visible artifacts; budget extra time for tuning. | Checkerboard approach worked; deterministic tests validated correctness. |
| M3 | Medium | Rule engine design is load-bearing — a schema mistake here propagates to every later milestone. | Schema proved stable through M14's 11,995 rules. |
| M4 | Low | Thermal diffusion is well-understood math. CFL stability is the only subtle issue. | Completed as expected. |
| M5 | High | Cross-chunk simulation is the hardest single problem. Halo loading, chunk state machine, and multi-chunk rendering all land at once. | Confirmed as the most complex milestone. Cross-chunk boundary handling required careful testing. |
| M6 | Medium | Structural flood-fill on CPU may be too slow for large structures. | Flood-fill radius limiting proved sufficient. |
| M7 | Low | UI work; no major unknowns. | Completed as expected. |
| M8 | Low | Serialization; LZ4 is well-supported in Rust. | Completed as expected. |
| M9 | Medium | Balancing 200 materials is a design challenge, not a technical one. | AI-assisted generation workflow scaled well. Balancing test suite caught degenerate behaviors. |
| M10 | Medium | 64 dynamic lights with shadow rays may blow the frame budget. | 64 lights achieved with per-pixel shadow ray budget. |
| M11 | Medium | Performance targets may not be achievable on minimum-spec hardware. | 1M voxels at 60 FPS achieved on target hardware. Graceful degradation implemented. |
| M12 | Low | Mod loading is straightforward given the existing data-driven architecture. | Data-driven architecture paid off — mod system required minimal engine changes. |
| M13 | Low | Audio is isolated; failure here doesn't affect the rest of the game. | Completed as expected. Procedural audio adds significant atmosphere. |
| M14 | Medium | Same as M9, scaled up. Interaction combinatorics at 500 materials may surface degenerate behaviors missed at 200. | Scaled to 561 materials / 11,995 rules. Balancing suite expanded accordingly. |
| M15 | Medium | Expanding the voxel layout may have non-obvious performance consequences. | Electrical pass added as 5th simulation pass. 10 dedicated electrical materials with charge-gated rules. |
