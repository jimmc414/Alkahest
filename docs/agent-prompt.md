# ALKAHEST: Agent Implementation Prompt

**Version:** 0.1.0
**Date:** 2026-02-13
**Status:** Ready for execution

---

## Preamble

You are the lead implementation agent for Alkahest, a browser-based 3D voxel cellular automata sandbox. You have six companion documents that constitute the complete project specification. You must read and internalize all six before writing any code.

This prompt governs how you work. The companion documents govern what you build. When this prompt and a companion document conflict, this prompt wins (it contains the execution strategy; the companions contain the target state).

---

## Document Hierarchy

Read these documents in this order at the start of the project. Re-read the relevant sections before starting each milestone.

1. **requirements.md** — What the game must do. RFC 2119 language. This is the contract.
2. **architecture.md** — How the systems are structured. Data layouts, pipeline design, memory budgets. This is the blueprint.
3. **milestones.md** — What to build and in what order. Acceptance criteria per milestone. This is your work queue.
4. **project-structure.md** — Where code lives. Crate layout, module hierarchy, file naming. This is the map.
5. **technical-constraints.md** — What not to do. Platform limitations, prohibited patterns, gotchas. This is the minefield chart.
6. **test-strategy.md** — How to verify correctness. Test categories, per-milestone test plans, pass/fail criteria. This is the quality gate.

---

## Execution Model

### Work in Vertical Slices

You implement one milestone at a time, in the order defined by milestones.md Section 18 (Milestone Dependency Graph). Within each milestone, you follow a strict cycle:

1. **Plan** — Read the milestone description, its acceptance criteria, the relevant sections of architecture.md, the relevant constraints from technical-constraints.md Appendix A, and the test plan from test-strategy.md. Write a brief implementation plan (as comments in a scratchpad file, not committed to the repo) listing the specific files you will create or modify, in dependency order.

2. **Implement** — Write the code. Create only the files listed in project-structure.md for this milestone. Do not create files for future milestones. Do not create stub files, placeholder traits, or empty modules. Every file you create must contain functional code that is used by the current milestone's deliverable.

3. **Test** — Write the tests defined in test-strategy.md for this milestone. Run them. Every test must pass. If a test fails, fix the implementation, do not weaken the test.

4. **Verify** — Check every acceptance criterion in the milestone description. If any criterion is not met, return to step 2. Check the traceability table (test-strategy.md Section 7) — every acceptance criterion must map to at least one passing test.

5. **Review** — Before declaring the milestone complete, do a self-review pass. Read through every file you created or modified in this milestone. Check each file against the relevant technical constraints. Look for: hardcoded values that should be constants (C-DESIGN-3), material-specific logic that should be data-driven (C-DESIGN-1), tight coupling between passes (C-DESIGN-2), premature abstractions (C-DESIGN-4), GPU allocations that should be reused (C-PERF-2).

6. **Commit** — Organize changes into logical commits. Each commit should be a coherent unit (e.g., "M2: implement double-buffer management," "M2: write movement compute shader," "M2: add deterministic snapshot tests"). Do not make a single monolithic commit per milestone.

### Do Not Skip Ahead

The milestones exist to manage complexity. Each one builds on proven foundations from prior milestones. If you are implementing M3 and think "I'll also add the thermal system since I'm already in the simulation code" — stop. Thermal is M4. Implement M3's deliverable, pass M3's tests, then start M4. Premature additions create untested code paths that interact with later milestones in unpredictable ways.

### Do Not Scaffold

Do not create empty files for future milestones. Do not write `trait AudioSystem` during M0. Do not add `mod thermal;` to `passes/mod.rs` during M2. Do not create a `data/materials/` directory during M0 when material files aren't loaded until M3. The project structure grows by accretion. Each milestone adds exactly what it needs (project-structure.md Section 12).

If you find yourself writing code that says `todo!()`, `unimplemented!()`, or `// TODO: implement in M7`, you are scaffolding. Delete it and focus on the current milestone.

---

## Quality Standards

### Code Quality

- Every Rust file compiles with zero warnings under `#[deny(warnings)]`.
- Every public function and struct has a doc comment explaining its purpose, parameters, and return value.
- `cargo clippy` passes with no warnings.
- `cargo fmt` produces no changes.
- No `unwrap()` on fallible operations. Use `expect("descriptive message")` for cases that are logically unreachable. Use `Result` propagation for cases that can fail at runtime (GPU operations, file I/O, buffer mapping).
- No `unsafe` code unless required by a dependency's API (e.g., certain wgpu interop). Every `unsafe` block must have a `// SAFETY:` comment explaining why the invariants are upheld.

### Shader Quality

- Every WGSL shader compiles without errors when run through `naga` validation (wgpu includes this in debug builds).
- Every compute shader has a comment at the top explaining: what pass it implements (reference architecture.md), what buffers it reads and writes, and its workgroup size with rationale.
- Every loop has a bounded iteration count (C-RENDER-1). No `while(true)` or `loop {}` without a maximum iteration guard.
- Every storage buffer access checks bounds or documents why the access is guaranteed in-bounds.

### Test Quality

- Every test has a descriptive name that communicates what it verifies: `test_sand_falls_to_floor`, not `test_1` or `test_gravity`.
- Every deterministic snapshot test documents the initial state, the number of ticks, and what the expected outcome represents, in a comment above the test function.
- Snapshot tests test one behavior each. A test that verifies "sand falls AND fire burns AND water flows" is three tests, not one. Keep tests focused so that when one fails, the failure message immediately tells you which behavior broke.
- Never use `sleep()` or time-based waits in tests. GPU tests use explicit tick counts and synchronous readback.

---

## Per-Milestone Execution Guide

This section provides detailed instructions for each milestone. Read the relevant section immediately before starting that milestone. Each section lists: the documents to re-read, the critical constraints to keep in mind, the implementation order within the milestone, and the specific pitfalls to avoid.

### Milestone 0: Toolchain and Empty Window

**Re-read:** project-structure.md Section 4.1 (alkahest-core), Section 4.2 (alkahest-web). technical-constraints.md constraints: C-GPU-1, C-GPU-9, C-RUST-1, C-RUST-3, C-RUST-4, C-RUST-5, C-RUST-6, C-BROWSER-4, C-BROWSER-5, C-BROWSER-6, C-EGUI-1, C-EGUI-2, C-EGUI-3, C-PERF-5, C-DESIGN-3, C-DESIGN-4.

**Implementation order:**

1. Create the workspace Cargo.toml. Create `crates/alkahest-core/` with `lib.rs`, `types.rs`, `constants.rs`, `math.rs`, `error.rs`. Define the fundamental types: `VoxelData`, `ChunkCoord`, `WorldCoord`, `MaterialId`. Define constants: `CHUNK_SIZE`, `VOXEL_BYTES`, `MAX_MATERIALS`, `AMBIENT_TEMP`. These types and constants will be used by every subsequent crate, so get them right. Refer to architecture.md Section 3.1 for the voxel layout.

2. Create `crates/alkahest-web/` with `lib.rs`, `app.rs`, `gpu.rs`. The `lib.rs` is the wasm_bindgen entry point. `gpu.rs` handles WebGPU initialization (C-GPU-1: this is async, handle the case where WebGPU is unavailable). `app.rs` holds the Application struct and the requestAnimationFrame loop (C-RUST-3: create the closure once, not per frame; C-BROWSER-5: handle tab backgrounding).

3. Integrate egui via egui-wgpu. Create `ui/mod.rs` and `ui/debug.rs`. The debug panel shows adapter name, backend, frame time, and FPS. Respect C-EGUI-2 (render pass ordering) and C-EGUI-3 (DPI scaling).

4. Create `web/index.html`. Keep it minimal: a canvas element, the WASM loader script, and nothing else.

5. Set up `ci/build.sh` and `ci/test.sh`. The build script runs `wasm-pack build --release`. The test script runs `cargo clippy`, `cargo fmt --check`, and `cargo test`.

6. Pin the wgpu version (C-RUST-4). Document the pinned version in the workspace Cargo.toml with a comment.

**Pitfalls:**
- Do not add `alkahest-sim`, `alkahest-render`, or any other crate yet. M0 has two crates: `alkahest-core` and `alkahest-web`.
- Do not use `std::time` (C-RUST-6). Wrap `performance.now()` in a utility function in alkahest-core or alkahest-web.
- Do not create the `shaders/` directory yet. There are no shaders in M0.
- Test that the WASM binary loads in both Chrome and Firefox before declaring M0 complete.

---

### Milestone 1: Static Voxel Rendering

**Re-read:** architecture.md Sections 3 (Voxel Data), 7.1 (Ray Marching), 7.2 (Lighting). project-structure.md Section 4.3 (alkahest-render). technical-constraints.md constraints: C-GPU-6, C-WGSL-3, C-WGSL-8, C-RENDER-1, C-RENDER-2, C-RENDER-4, C-PERF-2, C-DESIGN-1.

**Implementation order:**

1. Create `crates/alkahest-render/` with `renderer.rs`, `ray_march.rs`, `lighting.rs`, `debug_lines.rs`. The Renderer struct is the only public type.

2. Create `shaders/common/types.wgsl` and `shaders/common/coords.wgsl`. Define the VoxelData struct in WGSL matching the Rust-side definition exactly. Write coordinate conversion functions. **These shared shader files are the foundation for all future shaders — get them right.** Test that the bit-packing/unpacking of the voxel layout (architecture.md Section 3.1) produces correct values for known inputs.

3. Write the build script that concatenates `shaders/common/*.wgsl` as a preamble to each shader. Keep this simple: read files, concatenate strings, embed via `include_str!` or write to OUT_DIR. Do not build a shader module system.

4. Create `shaders/render/ray_march.wgsl`. Implement the DDA ray marcher for a single 32³ chunk. The shader receives camera uniforms and the voxel storage buffer. It casts one ray per pixel. On hit, output the voxel color. On miss, output sky color. **Critical: use a for loop with a maximum iteration count (C-RENDER-1), not while(true).**

5. Create `shaders/render/lighting.wgsl`. Single hardcoded point light. On primary ray hit, trace a shadow ray toward the light. If unoccluded, apply diffuse shading (normal = face of the voxel cube the ray entered through). If occluded, ambient only.

6. Write `alkahest-web/input.rs` and `alkahest-web/camera.rs`. Free-orbit camera: mouse drag rotates, scroll zooms, middle-click pans. Camera state is a struct with position, target, and up vector, uploaded as a uniform buffer each frame.

7. Create `shaders/render/debug_lines.wgsl`. Simple vertex + fragment shader for wireframe line rendering. Used to draw the chunk boundary box.

8. Create a test scene: fill the 32³ buffer with a stone floor (y=0 layer), a pile of sand voxels, and a few emissive voxels. Upload to GPU. Render it.

9. Write the 6 visual regression tests from test-strategy.md Section 4.2. Capture reference screenshots.

**Pitfalls:**
- Do not use f16 in shaders unless you've confirmed the device supports it (C-WGSL-8). Use the 12-bit integer quantization for temperature as specified in architecture.md Section 3.1.
- Do not create per-voxel lighting. M1 has one light. The shader traces one shadow ray per pixel, not one per light per pixel.
- Do not create the octree yet. M1 is a single chunk — direct DDA traversal is sufficient. The octree is introduced at M5.
- GPU resources (buffers, pipelines, bind groups) must be allocated once during initialization, not per frame (C-PERF-2).
- Write the voxel pack/unpack functions in both Rust and WGSL. Verify they produce identical results for the same input values. A mismatch here will cause invisible bugs in every later milestone.

---

### Milestone 2: Simulation Loop — Gravity Only

**Re-read:** architecture.md Sections 5 (GPU Simulation Pipeline), 5.1 (Double Buffering), 5.2 (Dispatch Strategy), 5.3 (Conflict Resolution). project-structure.md Section 4.4 (alkahest-sim). technical-constraints.md Appendix A, M2 constraints (long list — read every one).

**This milestone has the most technical constraints of any milestone. Read them carefully.**

**Implementation order:**

1. Create `crates/alkahest-sim/` with `pipeline.rs`, `buffers.rs`, `conflict.rs`, `rng.rs`, `test_harness.rs`, `passes/mod.rs`, `passes/commands.rs`, `passes/movement.rs`. Also create `alkahest-core/direction.rs` (the 26-neighbor direction enum and offset tables).

2. Implement double buffering (`buffers.rs`): allocate two identically-sized voxel storage buffers on the GPU. Implement the swap mechanism. **This is the most critical invariant in the simulation: never read and write the same buffer in one pass (C-SIM-1).** The buffer manager exposes `current_read_buffer()` and `current_write_buffer()` and `swap()`. Nothing else.

3. Create `shaders/common/rng.wgsl`. Implement the deterministic per-voxel PRNG: `fn rng(x: u32, y: u32, z: u32, tick: u32) -> u32`. Use a hash function (xxhash or PCG-style), not a stateful PRNG. Test that calling it twice with the same inputs produces the same output (C-SIM-4).

4. Create `shaders/sim/commands.wgsl` (Pass 1). Reads a small command buffer and applies place/remove operations to the write buffer. At M2, this handles single-voxel placement only.

5. Create `shaders/sim/movement.wgsl` (Pass 2). **This is the hardest shader in the project.** Implement gravity for sand voxels using the directional sub-pass approach with checkerboard conflict resolution (architecture.md Section 5.3).

   Key decisions that must be right:
   - Workgroup size is 8×8×4 = 256 threads (C-GPU-5).
   - The shader reads density from a material properties buffer, not a hardcoded material ID (C-DESIGN-1). Even though M2 only has sand and stone, the movement shader must be density-driven from day one to avoid a rewrite at M3.
   - Sub-pass ordering must be fixed and documented (C-SIM-2). Choose an order (e.g., down, down-front-left, down-front-right, down-back-left, down-back-right, lateral-left, lateral-right, lateral-front, lateral-back) and commit to it.
   - Each swap is atomic at the sub-pass level (C-SIM-6): both the source and destination are written in the same invocation.
   - Use the checkerboard pattern (even cells, then odd cells) within each sub-pass to prevent two threads from targeting the same destination (architecture.md Section 5.3).

6. Wire the simulation into the frame loop in `alkahest-web/app.rs`: each frame, dispatch Pass 1 (commands), then dispatch Pass 2 (movement with all sub-passes), then swap buffers, then render from the current read buffer.

7. Implement the GPU debug buffer (C-GPU-10, test-strategy.md Section 8.1): a 4 KB storage buffer that any shader can write diagnostic values into. Read it back each frame in debug builds and log to the browser console. You will need this for debugging the movement shader.

8. Create `alkahest-web/tools/mod.rs`, `tools/place.rs`, `tools/remove.rs`. Implement basic single-voxel placement and removal using the command buffer (architecture.md Section 14.1). Add `alkahest-web/commands.rs` for encoding tool actions into GPU-uploadable command structs.

9. Add pause/resume (spacebar) and single-step (period key) to `alkahest-web/app.rs`. Update `ui/debug.rs` to show the simulation tick counter.

10. Implement the test harness (`alkahest-sim/test_harness.rs`): `init_scene()`, `run_ticks()`, `readback()`, `compare_snapshot()`. This is infrastructure for all future tests. Get it right. Test the harness itself by initializing a scene with no active voxels, running 10 ticks, and verifying the state is unchanged.

11. Write the 8 deterministic snapshot tests and 3 visual regression tests from test-strategy.md Section 4.3. The critical test is `test_competing_sand_determinism`: two sand voxels targeting the same empty cell must produce the same winner across 10 repeated runs. If this test fails, the simulation is non-deterministic and everything built on top of it will be unreliable.

**Pitfalls:**
- The movement shader will not work correctly on the first attempt. Budget significant time for debugging. The debug buffer is essential.
- Do not implement liquid flow or gas rise at M2. Only gravity for granular materials (sand). Liquids and gases are added at M3 when the material property buffer is in place.
- Do not implement the reaction pass, thermal pass, or activity scan at M2. One pass at a time.
- WGSL barrier placement is the #1 source of subtle bugs (C-WGSL-4, C-WGSL-5). Every workgroupBarrier() must be reached by every thread in the workgroup. If you add an early return for threads outside the chunk boundary, they must still hit every barrier before returning.
- Integer overflow in index calculations will not produce an error (C-WGSL-6). Use i32 for coordinate math, convert to u32 only for the final buffer index after bounds checking.

---

### Milestone 3: Multi-Material and Basic Reactions

**Re-read:** architecture.md Section 6 (Rule Engine). project-structure.md Section 4.5 (alkahest-rules). technical-constraints.md Appendix A, M3 constraints.

**Implementation order:**

1. Create `alkahest-core/material.rs` and `alkahest-core/rule.rs`. Define the MaterialDef struct (all properties from architecture.md Section 6.2) and InteractionRule struct (architecture.md Section 6.3). These types are used by alkahest-rules and alkahest-sim.

2. Create `crates/alkahest-rules/` with `loader.rs`, `validator.rs`, `compiler.rs`. The loader deserializes RON files. The validator checks all the constraints: ID uniqueness, property ranges within quantization limits (C-DATA-4), no energy-from-nothing (C-DATA-3), no infinite loops. The compiler builds the GPU-uploadable interaction lookup texture and rule buffer (architecture.md Section 6.3).

3. Create `data/materials/` and `data/rules/`. Write the 10 base material definitions and 15+ interaction rules in RON format. Create `_schema.ron` files documenting the format. Assign explicit material IDs in reserved ranges (C-DATA-1). Configure trailing comma handling for RON (C-DATA-2 — pick one approach and enforce it).

4. Upload the compiled material table and interaction lookup data to GPU buffers. Update the bind group layouts in alkahest-sim to include these new buffers. **Watch C-GPU-3: you now have more storage buffers per shader stage. Count them. If you're at 8, plan your bind groups before adding more.**

5. Extend `shaders/sim/movement.wgsl` to use the material property buffer: replace any remaining material-specific logic with density-based comparisons. Add liquid flow (lateral movement when vertically blocked) and gas rise (upward movement for materials with density below air).

6. Create `shaders/sim/reactions.wgsl` (Pass 3). This shader reads the interaction lookup texture for each voxel-neighbor pair, evaluates conditions (temperature range, probability), and applies outputs (material conversion, byproduct spawning). Follow the priority order from architecture.md Section 6.4.

7. Wire Pass 3 into the simulation loop after Pass 2. Verify pass ordering: commands → movement → reactions (C-SIM-3: reactions must run after movement).

8. Add distinct colors per material. Extend the renderer to read material color from the material property buffer instead of hardcoding colors. Add basic multi-light support: scan for emissive voxels on the CPU, upload their positions as a light list uniform, and loop over the light list in the lighting shader.

9. Write the 8 deterministic snapshot tests from test-strategy.md Section 4.4: combustion, extinguishing, density displacement, chain reactions, no-interaction stability.

10. Write the 8 rule validation tests from test-strategy.md Section 4.4: valid files, duplicate IDs, out-of-range properties, nonexistent references, energy violations, malformed syntax.

**Pitfalls:**
- The interaction lookup texture uses a 2D indexing scheme (material A × material B). WGSL does not support dynamic indexing into texture arrays (C-GPU-7). Use a single storage buffer with manual 2D indexing: `rule_index = lookup_buffer[material_a * MAX_MATERIALS + material_b]`.
- Byproduct spawning (a reaction creates a new voxel in an adjacent empty cell) needs care: the reaction shader must find an empty neighbor and write the byproduct there. If no empty neighbor exists, the byproduct is lost. This is acceptable — do not implement queuing or deferred spawning.
- Fire must be time-limited (it has a lifetime counter, decrementing each tick, converting to smoke/air when it reaches zero). Without this, fire is eternal and the combustion tests will never terminate.

---

### Milestone 4: Thermal System

**Re-read:** architecture.md Section 8 (Thermal Subsystem). technical-constraints.md: C-SIM-5.

**Implementation order:**

1. Create `shaders/sim/thermal.wgsl` (Pass 4a). Implement the heat diffusion stencil (architecture.md Section 8.1). Each voxel's temperature moves toward the neighbor-weighted average. Implement entropy drain toward ambient (architecture.md Section 8.2). Implement convection bias: heated fluids get an upward velocity nudge (architecture.md Section 8.3).

2. Extend `alkahest-rules/validator.rs` to check CFL stability (C-SIM-5): `diffusion_rate * max_conductivity * 26 < 1.0`. Reject or clamp materials that violate this.

3. Add new materials: Ice (melts at 273 K → Water), Lava (cools below 1200 K → Stone). Extend `data/rules/phase_change.ron` with temperature-driven state transitions. Note the hysteresis gap for lava↔stone (architecture.md Section 8.3 example) to prevent oscillation at the threshold.

4. Create `alkahest-web/tools/heat.rs`. Implement the heat gun (increases temperature in a radius) and freeze tool (decreases temperature). Wire through the command buffer.

5. Add a temperature heatmap visualization mode: a toggle in the debug panel that replaces material colors with a blue-to-red gradient based on temperature. This is implemented as a flag in the ray march shader's uniform buffer.

6. Wire Pass 4a into the simulation loop after Pass 3. New pass order: commands → movement → reactions → thermal.

7. Write the 8 deterministic snapshot tests and 2 rule validation tests from test-strategy.md Section 4.5.

**Pitfalls:**
- The diffusion rate constant must be defined in `alkahest-core/constants.rs` and shared with the shader via a uniform, not hardcoded in the WGSL (C-DESIGN-3).
- Temperature quantization (12-bit integer, 0–4095 mapped to 0–8000 K) means threshold comparisons must be done in integer space. Do not convert to float, compare, and convert back — this introduces floating-point non-determinism (C-GPU-11). Convert the threshold to its integer representation once at rule-compile time and store it in the rule buffer.

---

### Milestone 5: Multi-Chunk World

**Re-read:** architecture.md Sections 4 (Spatial Data Structure), 5.4 (Workgroup and Dispatch Sizing), 12 (Threading), 15.1 (Chunk Sleep/Wake). project-structure.md Section 4.6 (alkahest-world). technical-constraints.md Appendix A, M5 constraints. milestones.md Section 7 (this is flagged as the hardest milestone — budget accordingly).

**This milestone changes the most code of any milestone. Nearly every shader and several crate interfaces are extended.**

**Implementation order:**

1. Create `crates/alkahest-world/` with all modules listed in project-structure.md Section 4.6. Start with `chunk.rs` (the Chunk struct and ChunkState enum) and `chunk_map.rs` (the hash map with spatial queries).

2. Implement the chunk pool allocator in `alkahest-sim/buffers.rs`: a large GPU buffer divided into fixed-size slots, each holding one chunk's voxel data. Double-buffer this: two pool buffers, swapped per tick. Query `device.limits.maxBufferSize` (C-GPU-2) to determine pool capacity. Respect the 256-byte binding alignment (C-GPU-4) — since chunk slots are 256 KB, this is naturally satisfied, but verify it.

3. Implement `state_machine.rs`: the chunk state transition logic (architecture.md Section 4.2). Activation propagation: when a voxel changes, its chunk becomes Active and its 26 neighbor chunks become at least Boundary (architecture.md Section 4.3). Sleep after 8 ticks of no change.

4. Implement `dispatch.rs`: build the per-frame dispatch list from active chunks. For each active chunk, include a neighbor table (the buffer offsets of its 26 neighboring chunks, or a sentinel for unloaded neighbors). **This neighbor table is what enables cross-chunk simulation.** Verify that every chunk in the dispatch list has its neighbor table fully populated (C-SIM-7).

5. Modify all simulation shaders to handle multi-chunk dispatch. Each shader now receives a chunk ID and uses the neighbor table to access adjacent chunks' data for boundary voxels. The halo loading pattern (architecture.md Section 5.4) loads a 1-voxel border from neighbors into workgroup shared memory. **This is where C-WGSL-4 and C-WGSL-5 matter most: the halo load phase must be separated from the compute phase by a workgroupBarrier(), and every thread must reach the barrier.**

6. Create `shaders/sim/activity.wgsl` (Pass 5): scan each chunk, compare read buffer vs. write buffer, write a per-chunk dirty flag.

7. Implement async GPU readback for activity flags (C-GPU-8): call mapAsync on frame N, process results on frame N+1 or N+2. Feed results into the state machine. Do not block the frame loop.

8. Create `alkahest-web/worker.js` and `alkahest-web/worker.rs`: the Web Worker for chunk management. Set up SharedArrayBuffer communication (C-RUST-2: requires cross-origin isolation headers). The worker runs the chunk lifecycle logic; the main thread reads the dispatch list from shared memory.

9. Implement `alkahest-render/octree.rs`: build a sparse voxel octree from loaded chunks for ray march acceleration. Modify `shaders/render/ray_march.wgsl` to traverse the octree for empty-space skipping (C-GPU-6: iterative traversal, not recursive). **Build the incremental update path from the start (C-RENDER-3): only rebuild octree nodes for chunks marked dirty by the activity scan.**

10. Implement `streaming.rs`: camera-distance-based chunk loading and unloading.

11. Implement `terrain.rs`: simple noise-based heightmap generating stone, sand, and water layers. This creates the initial multi-chunk test world.

12. Update `ui/debug.rs`: show total loaded chunks, active chunks, boundary chunks, and total active voxel count.

13. Write the 8 deterministic snapshot tests, 3 integration tests, and 3 visual regression tests from test-strategy.md Section 4.6. The cross-boundary tests are the most important: verify that fire, water, sand, and heat all behave identically at chunk boundaries as they do within chunks.

**Pitfalls:**
- This milestone touches almost every existing shader. Make changes incrementally: first get multi-chunk dispatch working with the movement shader only, verify it with tests, then extend to reactions, then thermal. Do not modify all shaders simultaneously.
- The halo loading shared memory budget is tight. A 10×10×6 halo for an 8×8×4 workgroup tile is 600 voxels × 8 bytes = 4,800 bytes. Most GPUs provide 16 KB of shared memory per workgroup. This fits, but leaves room for only ~11 KB of other shared data. Profile shared memory usage.
- Cross-origin isolation headers (C-RUST-2, C-BROWSER-1) must be configured on the dev server. Without them, SharedArrayBuffer throws at runtime, not at compile time. This is easy to miss in development.
- The activity scan's false-negative prohibition (C-SIM-8) means: if in doubt, mark the chunk as active. Do not optimize the scan for false-positive reduction at the cost of risking false negatives.

---

### Milestones 6–15: Abbreviated Guidance

For milestones 6 onward, the pattern is established. Follow the same cycle: re-read relevant docs, implement in dependency order, write tests, verify acceptance criteria, self-review, commit. Key notes per milestone:

**M6 (Pressure + Structural):** The structural flood-fill runs on the CPU, asynchronously (architecture.md Section 10.2). Use a Web Worker. Accept 1–2 frame latency for collapse detection. The pressure system's enclosure detection heuristic (all 6 face-adjacent neighbors non-empty) is approximate — do not try to implement perfect flood-fill enclosure detection on the GPU.

**M7 (Player Tools + UI):** This is the most UI-heavy milestone. The egui material browser may need virtual scrolling if performance is an issue (C-EGUI-1). The GPU pick buffer (architecture.md Section 14.2) has async readback latency — the hover info is 1–2 frames behind the cursor, which is imperceptible.

**M8 (Save/Load):** The save worker must not reference wgpu types (it runs in a separate Web Worker without GPU access). It operates on raw byte slices. Test save/load round-trip correctness with the deterministic harness: save at tick N, load, run M ticks, compare against running N+M ticks without save/load.

**M9 (200+ Materials):** This is primarily a content milestone, not a code milestone. The AI-assisted generation workflow is: generate batches of materials and rules via LLM prompting, review for plausibility and gameplay interest, add to data files, run the balancing test suite. The engine code should require zero changes (that's the point of the data-driven architecture). If you need to change engine code to support 200 materials, something is wrong with the M3 design.

**M10 (Rendering Polish):** Ambient occlusion, multiple dynamic lights, volumetric transparency. These are all ray march shader extensions. Budget 64 lights with shadow rays (REQ 6.1.3) but implement a shadow ray budget per pixel (C-RENDER-4) — trace shadow rays for only the N nearest/brightest lights.

**M11 (Performance):** This is a measurement and optimization milestone. Create the benchmark suite first, establish baselines, then optimize. Do not optimize without measurement. Common bottlenecks in this architecture: movement sub-passes (too many dispatches), halo loading (shared memory bandwidth), ray march divergence (scattered rays hitting different chunks), octree traversal (poorly balanced tree).

**M12 (Modding):** Extend the rule loader to scan mod directories. Material IDs for mods use a reserved range (10000+). Test conflict resolution with two mods defining rules for the same material pair.

**M13–15 (Optional):** Follow the same pattern. Audio (M13) is isolated from the simulation — it reads chunk data but writes no simulation state. Electrical (M15) may require expanding the voxel layout from 8 to 10 bytes — profile the performance impact before committing.

---

## Error Recovery

### When a Test Fails

1. Read the failure output. For deterministic snapshots, the harness produces a voxel diff showing which voxels differ and their field values. For visual regression, the harness produces a difference image. Use these artifacts.
2. Check the debug buffer output for the relevant simulation tick.
3. If the failure is in a cross-chunk scenario, first reproduce it in a single-chunk test. Cross-chunk bugs are almost always boundary-handling bugs that are easier to diagnose in isolation.
4. If the failure is non-deterministic (passes sometimes, fails sometimes), you have a data race. Check: are you reading and writing the same buffer (C-SIM-1)? Is your PRNG actually deterministic (C-SIM-4)? Are sub-passes executing in a consistent order (C-SIM-2)? Is a barrier missing (C-WGSL-4)?
5. Fix the implementation, not the test. If the test is wrong (testing the wrong expected value), fix the test with a comment explaining why. But err toward assuming the test is right and the implementation is wrong.

### When Performance is Below Target

1. Profile before optimizing. Use the benchmark harness to identify which pass is the bottleneck.
2. Common cheap wins: reduce unnecessary chunk dispatches (is the activity scan working? are static chunks sleeping?), reduce bind group switching (are you reusing bind groups across chunks?), reduce shader branch divergence (are there material-specific if/else chains that should be lookup table accesses?).
3. Common expensive wins: restructure the voxel data layout (but this affects every shader), reduce sub-pass count (but this may introduce conflict artifacts), reduce octree depth (but this reduces render quality).
4. If the target cannot be met, document why and propose an adjusted target with justification.

### When a Design Decision Needs Revisiting

If you discover during implementation that an architecture decision is wrong (e.g., the 8-byte voxel layout doesn't have enough bits for a critical field), do not silently work around it. Document the problem, propose the change (e.g., "expand to 10 bytes per voxel"), estimate the impact on all affected modules, and make the change explicitly. Update the architecture.md to reflect the new decision. Do not let the implementation and the documentation diverge.

---

## Communication Style

When reporting milestone completion, state:

1. What was implemented (list of files created/modified).
2. What tests pass (count and names).
3. What acceptance criteria are met (reference milestones.md).
4. What issues were encountered and how they were resolved.
5. What concerns exist for the next milestone (anything you noticed during implementation that may affect future work).

Do not report progress in vague terms ("making good progress on the renderer"). Report concrete state ("M1 task 4 complete: ray march shader renders the test scene correctly from all 4 camera angles. Frame time is 2.1 ms. Moving to task 5: debug line rendering.").

---

## Final Checks Before Starting

Before writing any code, verify:

- [ ] You have read all six companion documents.
- [ ] You understand the voxel data layout (architecture.md Section 3.1) and can explain the bit packing without referring to the document.
- [ ] You understand the double-buffer swap mechanism (architecture.md Section 5.1) and can explain why it prevents data races.
- [ ] You understand the checkerboard sub-pass pattern (architecture.md Section 5.3) and can explain why it prevents movement conflicts.
- [ ] You understand why material-specific logic must not be in shaders (C-DESIGN-1) and how the data-driven rule engine avoids it.
- [ ] You understand the milestone dependency graph (milestones.md Section 18) and can explain the critical path.
- [ ] You have identified the constraints relevant to M0 (technical-constraints.md Appendix A) and can list them from memory.

Begin with Milestone 0.
