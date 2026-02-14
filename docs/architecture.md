# ALKAHEST: Architecture Document

**Version:** 0.1.0-draft
**Date:** 2026-02-13
**Status:** Draft
**Companion:** requirements.md v0.1.0

---

## 1. Document Purpose

This document describes the technical architecture for Alkahest, a browser-based 3D voxel cellular automata sandbox. It covers system decomposition, data structures, GPU pipeline design, memory management, and key technical decisions. It does not contain implementation code. All section references (e.g., REQ 4.1.2) point to the companion requirements.md.

---

## 2. System Overview

Alkahest is structured as a split CPU/GPU application. The CPU side (Rust compiled to WASM) handles world management, input processing, chunk lifecycle, rule loading, and orchestration. The GPU side (WGSL compute and render shaders via WebGPU) handles the simulation tick and rendering. The critical design constraint is that per-voxel work never touches the CPU at runtime — the CPU manages chunks and dispatches work, but all voxel-level logic runs on the GPU.

### 2.1 High-Level Module Decomposition

The application is divided into six major subsystems:

**World Manager (CPU):** Owns the sparse voxel octree. Manages chunk allocation, activation, deactivation, and streaming. Determines which chunks contain active voxels and builds the dispatch list for the GPU each frame.

**Rule Engine (CPU + GPU):** Loads material definitions and interaction rules from data files at startup. Compiles the interaction matrix into GPU-friendly lookup textures/buffers. Provides the simulation shaders with all data needed to evaluate voxel interactions without CPU round-trips.

**Simulation Pipeline (GPU):** Executes the cellular automata tick. Reads from the current voxel state buffer, evaluates neighbor interactions using the rule lookup data, and writes to the next-state buffer. Runs as a sequence of compute shader dispatches.

**Renderer (GPU):** Reads the current voxel state buffer and produces the final image. Uses ray marching against the sparse octree for primary visibility, with additional passes for lighting, transparency, and post-processing.

**Input and UI (CPU):** Handles player input, tool selection, brush application, camera control, and HUD rendering. Brush operations write into a command buffer that the simulation pipeline consumes.

**Persistence (CPU):** Handles save/load serialization, compression, and file I/O.

### 2.2 Data Flow Per Frame

A single frame proceeds in this order:

1. **Input Processing (CPU):** Read player input events. Update camera state. If the player is placing/removing voxels, write modification commands into a GPU-accessible command buffer.
2. **Chunk Management (CPU):** Evaluate which chunks are active (contain non-static voxels or are near recently modified regions). Update the dispatch list. Load/unload chunks based on camera position and activity.
3. **Simulation Dispatch (GPU):** Execute the simulation compute shaders over all active chunks. This consumes the command buffer (applying player modifications), then runs the automata tick. Double-buffer swap occurs after all dispatches complete.
4. **Activity Scan (GPU):** A lightweight compute pass scans each chunk to determine if any voxels changed state. Writes per-chunk activity flags back to a CPU-readable buffer. This feeds into the next frame's chunk management step.
5. **Render (GPU):** Ray march the voxel data to produce the final image. Lighting and post-processing passes follow.
6. **UI Overlay (CPU/GPU):** Composite the HUD, material browser, and debug info over the rendered image.

---

## 3. Voxel Data Representation

### 3.1 Per-Voxel Layout

Each voxel is 8 bytes, packed as follows:

| Field | Size | Type | Description |
|---|---|---|---|
| Material ID | 16 bits | u16 | Index into the material table. 0 = empty/air. Supports up to 65,535 material types. |
| Temperature | 16 bits | f16 | Degrees Kelvin, half-precision float. Range ~0–65504 K covers all gameplay-relevant temperatures. |
| Velocity | 24 bits | 3x i8 | Signed 8-bit per axis. Interpreted as voxel-units-per-tick. Range: -128 to +127 voxels/tick per axis. |
| Pressure | 8 bits | u8 | 0–255 abstract pressure units. |
| Flags | 8 bits | u8 | Bitfield: bit 0 = active/static, bit 1 = updated-this-tick, bit 2 = bonded, bits 3-7 = reserved/material-specific metadata. |

Total: 80 bits = 10 bytes. This exceeds the 8-byte target, so an alternative packing is used: temperature is quantized to 12 bits (0–4095 mapped to a gameplay-relevant range of 0–8000 K at ~2 K resolution), and pressure is reduced to 6 bits (0–63), yielding:

| Field | Bits |
|---|---|
| Material ID | 16 |
| Temperature | 12 |
| Velocity X | 8 |
| Velocity Y | 8 |
| Velocity Z | 8 |
| Pressure | 6 |
| Flags | 6 |

Total: 64 bits = 8 bytes per voxel. This aligns naturally to GPU memory access patterns.

### 3.2 Why 8 Bytes Matters

At 8 bytes per voxel, a 32x32x32 chunk (32,768 voxels) occupies 256 KB. A 64x64x64 chunk (262,144 voxels) occupies 2 MB. GPU workgroup shared memory is typically 16–48 KB, so a 32³ chunk fits partially in shared memory with a halo region for neighbor lookups. The 8-byte alignment also enables efficient 64-bit atomic operations if needed for concurrent write resolution.

### 3.3 Struct-of-Arrays vs. Array-of-Structs

The voxel data uses a **hybrid layout**. Within a single chunk, data is stored as Array-of-Structs (each voxel is a contiguous 8-byte word) because the simulation shader reads all fields of a voxel together during neighbor evaluation. However, across chunks, the world manager stores chunk metadata (bounding box, activity flags, LOD level) in separate arrays for efficient CPU-side scanning.

The rationale: AoS is better when every field is read together (which is the case for the simulation tick — you need material, temperature, pressure, and velocity of each neighbor simultaneously). SoA would be preferable if passes frequently accessed only one field across all voxels, but the simulation is not structured that way.

---

## 4. Spatial Data Structure

### 4.1 Chunked World with Sparse Allocation

The world is divided into a uniform grid of 32x32x32 chunks. The chunk grid itself is stored as a hash map (chunk coordinate → chunk data pointer), not a dense 3D array. This means only chunks that contain at least one non-air voxel consume memory.

The world coordinate space supports up to 32,768 chunks per axis (15-bit signed coordinates), giving a theoretical maximum world size of 1,048,576 × 1,048,576 × 1,048,576 voxels. In practice, memory limits this to whatever the player's hardware can support, but the addressing scheme does not impose an artificial ceiling.

### 4.2 Chunk States

Each chunk exists in one of four states:

**Unloaded:** No memory allocated. The chunk is either empty or beyond the active range.

**Loaded-Static:** Voxel data is in GPU memory, but the chunk is not dispatched for simulation. All voxels are in a resting state (no movement, no active reactions, temperature in equilibrium with neighbors). The chunk is still rendered.

**Loaded-Active:** Voxel data is in GPU memory and the chunk is included in every simulation dispatch. At least one voxel changed state in the last N ticks (where N is a configurable settling threshold, recommended value: 8 ticks).

**Loaded-Boundary:** The chunk itself may be static, but it borders an active chunk. It must be readable by the simulation shaders (for neighbor lookups at chunk edges) but does not need its own interior simulated. This is an optimization to avoid activating large static regions just because one neighboring chunk is active.

### 4.3 Chunk Activation Propagation

When a voxel changes state (due to player action or simulation), the containing chunk transitions to Active. All 26 neighboring chunks transition to at least Boundary. If the state change is near a chunk edge (within 1 voxel of the boundary), the adjacent chunk on that face also transitions to Active, because the change may propagate across the boundary on the next tick.

After a chunk has had zero voxel state changes for N consecutive ticks, it transitions from Active to Loaded-Static. Its boundary neighbors are re-evaluated and may also transition to static.

### 4.4 Octree for Rendering (Separate from Simulation)

The simulation operates on the flat chunk grid — the compute shaders need uniform, predictable memory access patterns, not tree traversal. However, the renderer uses a sparse voxel octree built from the chunk data for efficient ray marching. This octree is rebuilt incrementally: when a chunk's voxel data changes, only that chunk's octree nodes are updated.

The octree serves two purposes: accelerating empty-space skipping during ray marching, and providing level-of-detail (LOD) for distant chunks (a single octree node can represent a 4x4x4 or 8x8x8 region with an average color/density for distant rendering).

---

## 5. GPU Simulation Pipeline

### 5.1 Double Buffering

Two identically-sized voxel buffers exist on the GPU: Buffer A and Buffer B. On even ticks, the simulation reads from A and writes to B. On odd ticks, it reads from B and writes to A. The renderer always reads from whichever buffer was most recently written (the "current" buffer).

This eliminates read-write hazards: a voxel's neighbors are guaranteed to reflect the previous tick's state, not a partially-updated current tick. This is essential for deterministic simulation.

### 5.2 Simulation Dispatch Strategy

The simulation runs as multiple compute shader dispatches per tick, in a fixed order:

**Pass 1 — Player Command Application:** A compute shader reads the player command buffer (place/remove/heat/push operations) and writes the modifications into the current write buffer. This runs first so that player actions are immediately visible in the simulation tick.

**Pass 2 — Movement and Gravity:** Handles voxel displacement: falling, floating, flowing. This is the most complex pass because it involves voxels swapping positions, which creates write conflicts (two voxels may want to move into the same empty cell). The conflict resolution strategy is described in section 5.3.

**Pass 3 — Reactions and State Transitions:** Evaluates the interaction matrix for all adjacent voxel pairs. Produces byproducts, triggers state changes (melting, igniting, dissolving). This pass reads the output of Pass 2 (post-movement positions).

**Pass 4 — Field Propagation:** Diffuses temperature, pressure, and (optionally) electrical charge across neighbors. This is a straightforward stencil operation with material-dependent diffusion rates.

**Pass 5 — Activity Scan:** Each workgroup scans its chunk and writes a single flag: 1 if any voxel in the chunk changed between the read buffer and write buffer, 0 otherwise. This flag array is read back by the CPU for chunk state management.

### 5.3 Movement Conflict Resolution

The fundamental problem: if voxel A (sand) wants to fall into empty cell C, and voxel B (also sand) also wants to fall into cell C, only one can win. On a GPU, both threads execute simultaneously.

**Chosen approach: Directional sub-passes with deterministic priority.**

The movement pass is split into sub-passes, each handling movement in one direction: down, down-diagonal (4 sub-directions), lateral (4 sub-directions). Within each sub-pass, voxels are processed in a checkerboard pattern (even cells, then odd cells) so that no two simultaneously-processed voxels can target the same destination. This is analogous to the red-black Gauss-Seidel pattern used in parallel fluid solvers.

The cost is that movement requires ~6-10 sub-passes per tick instead of 1, but each sub-pass is cheap (simple conditional swap) and fully parallelizable. The deterministic ordering ensures reproducibility (REQ 4.3.7).

### 5.4 Workgroup and Dispatch Sizing

Each compute dispatch processes one chunk. A chunk is 32x32x32 = 32,768 voxels. The workgroup size is 8x8x4 = 256 threads (a common sweet spot for GPU occupancy). Each dispatch therefore has 128 workgroups per chunk (32,768 / 256).

For neighbor lookups at chunk boundaries, each workgroup loads a 1-voxel halo from adjacent chunks into shared memory. The halo for an 8x8x4 workgroup tile is (8+2)(8+2)(4+2) = 600 voxels × 8 bytes = 4,800 bytes, well within shared memory limits.

The total dispatch count per tick is: (number of active chunks) × (number of simulation sub-passes). For a scene with 100 active chunks and 10 sub-passes, that's 1,000 dispatches per tick. WebGPU dispatch overhead is low enough that this is feasible at 60 Hz, but dispatch batching (encoding multiple dispatches in a single command buffer) is still important.

### 5.5 Stochastic Behavior and Determinism

Some interactions require randomness (e.g., fire has a probability of spreading per tick, not a certainty). The simulation uses a deterministic PRNG seeded per-voxel from its world coordinates and the current tick number. This ensures that given identical initial state and tick count, the simulation produces identical results across runs and across hardware (REQ 4.3.7). The PRNG choice should be a fast, low-state-size function suitable for GPU execution, such as PCG or a hash-based approach (xxhash applied to coordinates + tick).

---

## 6. Rule Engine

### 6.1 Data-Driven Interaction Rules

All material behaviors are defined in external data files (RON format recommended for Rust ecosystem compatibility, but JSON is also acceptable). The engine hard-codes zero material-specific logic. This means adding a new material or changing an interaction never requires recompiling the engine.

### 6.2 Material Definition Schema

Each material definition specifies:

**Identity:** Unique u16 ID, string name, category tag, brief description.

**Physical properties:** Phase, density, thermal conductivity, melting point, boiling point, ignition point, structural integrity, viscosity (for liquids), angle of repose (for powders), flammability, electrical conductivity (optional), color, emission intensity.

**State transition rules:** A list of conditions under which this material transforms into another material. Each rule specifies a trigger (temperature threshold, pressure threshold, contact with specific material) and a result (new material ID, energy released/absorbed).

### 6.3 Interaction Matrix

Pairwise interactions between materials are stored separately from individual material definitions. Each interaction rule specifies:

**Inputs:** Material A ID, Material B ID.

**Conditions:** Temperature range, pressure range, probability per tick, minimum neighbor count of material B.

**Outputs:** What material A becomes, what material B becomes, any byproduct material spawned in an adjacent empty cell, temperature delta applied to both, pressure delta applied to both.

The interaction matrix is compiled at load time into a GPU-friendly format: a 2D texture where each texel at coordinates (material_A, material_B) contains an index into a rule buffer. Since not all 65535² pairs have rules, this uses a sparse representation: a hash map on the CPU resolves to a packed rule array on the GPU, with a small indirection texture.

For 500 materials with 10,000 defined interactions, the lookup texture is 500×500 = 250,000 entries (indices or "no rule" sentinels), at 4 bytes each = 1 MB. The rule data buffer (10,000 rules at ~32 bytes each) = 320 KB. Total rule data on GPU: ~1.3 MB. This is negligible.

### 6.4 Rule Priority and Conflict Resolution

When multiple rules could apply to a voxel in a single tick (e.g., it's simultaneously above its melting point AND in contact with a reactive material), rules are evaluated in a fixed priority order:

1. Pressure rupture (highest priority — explosions override everything).
2. Chemical reactions (material-pair interactions).
3. State transitions (temperature-driven phase changes).
4. Movement (gravity, flow).

Within the same priority level, if multiple rules match, the first match in the rule file's declaration order wins. This is simple, predictable, and debuggable.

---

## 7. Rendering Pipeline

### 7.1 Primary Visibility: Octree Ray Marching

The renderer casts one ray per screen pixel from the camera through the scene. Each ray traverses the sparse voxel octree using a DDA (Digital Differential Analyzer) variant adapted for hierarchical grids. Empty octree nodes are skipped entirely (the ray jumps to the far side of the empty region), which is the primary performance optimization — most of the world volume is empty.

When a ray enters a leaf-level octree node (a 32³ chunk), it switches to per-voxel DDA traversal within that chunk, reading directly from the voxel state buffer. The first non-empty voxel hit is the primary surface.

### 7.2 Lighting

**Direct lighting** uses point lights emitted by voxels with non-zero emission (fire, lava, glowing materials). Because the scene is fully volumetric, shadow rays are traced from the hit point toward each light source through the same octree. Hard shadows are acceptable for the initial implementation; soft shadow approximation via jittered multi-sample is a future optimization.

**Ambient occlusion** is computed per-voxel using a precomputed count of occupied neighbors in a small radius (e.g., 3x3x3 or 5x5x5). This can be calculated as a byproduct of the simulation activity scan or as a separate lightweight compute pass. Screen-space AO is an alternative that avoids per-voxel storage.

**Global illumination** is deferred to a later milestone. The recommended approach is voxel cone tracing, which integrates naturally with the existing octree. For initial release, a simple ambient term plus direct lighting is sufficient.

### 7.3 Transparency

Transparent and semi-transparent materials (water, glass, gases) require special handling because ray marching naturally supports volumetric transparency — the ray simply continues through transparent voxels, accumulating color and opacity (front-to-back compositing). This avoids the depth-sorting problems that plague polygon-based transparent rendering.

For gases and fog, the ray accumulates a density integral over the traversed volume, with color and opacity derived from the material properties. This produces natural-looking volumetric effects without a separate particle system.

### 7.4 Cross-Section View

The slice/cross-section view (REQ 6.2.2) is implemented by clipping the ray marcher: rays that would hit voxels on the clipped side of the plane are ignored, revealing the interior. The clip plane is a uniform passed to the ray march shader, requiring no geometry changes. The exposed interior face of clipped voxels is shaded with a distinct edge highlight to make the cut plane visually clear.

---

## 8. Thermal Subsystem Design

### 8.1 Heat Diffusion Model

Temperature propagation uses a discrete approximation of the heat equation. Each tick, a voxel's temperature is updated based on the weighted average of its 26 neighbors' temperatures, scaled by the thermal conductivity of both the voxel and each neighbor.

The update formula (conceptually):

    new_temp = current_temp + diffusion_rate × Σ(neighbor_conductivity × (neighbor_temp - current_temp))

The diffusion_rate is a global simulation parameter that controls how fast heat propagates. It must be kept below a stability threshold (related to the CFL condition) to prevent temperature oscillation. For a 26-neighbor 3D stencil, the stability condition constrains diffusion_rate relative to the maximum thermal conductivity in the material set. This constraint is validated at rule-load time.

### 8.2 Entropy / Heat Dissipation

A global entropy factor slowly drains temperature toward an ambient baseline (e.g., 293 K / 20°C) in the absence of heat sources. This prevents the world from accumulating unbounded thermal energy over long simulation runs. The entropy rate is configurable and should be subtle enough that players don't notice unless they're observing an isolated hot object over many seconds.

### 8.3 Convection Approximation

True convection (fluid movement driven by temperature gradients) is approximated by biasing the movement pass: heated liquid and gas voxels receive an upward velocity bias proportional to their temperature above ambient. Cooled fluids receive a downward bias. This produces visually convincing convection currents without solving the full Navier-Stokes equations.

---

## 9. Pressure Subsystem Design

### 9.1 Pressure Accumulation

Pressure is tracked per-voxel as a 6-bit value (0–63). When voxels are added to an enclosed volume (or when the contents are heated and expand), the pressure value of all voxels in the enclosed region increases. Detecting "enclosed volume" is the hard part.

### 9.2 Enclosure Detection

Full flood-fill enclosure detection per tick is too expensive for the GPU. Instead, the system uses a local heuristic: a voxel is considered "enclosed" if all 6 face-adjacent neighbors are non-empty. Pressure diffuses between enclosed voxels, equalizing within a contiguous enclosed region over multiple ticks. This is an approximation — it won't perfectly handle complex non-convex enclosures — but it produces correct behavior for the common cases (sealed containers, underground chambers, pressurized pipes).

A more accurate approach (deferred to a later milestone) would run a GPU-accelerated connected-component labeling pass to identify distinct enclosed volumes and assign uniform pressure to each.

### 9.3 Rupture

When a voxel's pressure exceeds its material's structural integrity, it is destroyed (converted to debris/fragments of appropriate material) and the pressure is released as a radial blast wave. The blast wave is implemented as a spherical front of high-pressure, high-velocity voxels that propagates outward, attenuating with distance. Each affected voxel receives a velocity impulse away from the rupture point.

---

## 10. Structural Integrity Subsystem Design

### 10.1 Bond Graph (Simplified)

Full rigid-body structural simulation is out of scope. Instead, structural integrity uses a simplified connected-component model. Adjacent solid voxels of compatible material types are considered "bonded." The bond graph is not explicitly stored per-voxel (that would require too much memory); instead, structural evaluation is triggered only when a voxel is destroyed or weakened.

### 10.2 Collapse Evaluation

When a solid voxel is destroyed (by heat, reaction, or player action), a local flood-fill propagates from the destroyed voxel to identify any connected component of solid voxels that is no longer connected to a "grounded" voxel (one resting on the world floor or on a sufficiently large stable structure). If an isolated component is found and its total weight (sum of voxel densities) exceeds its aggregate bond strength to remaining neighbors, the component is "released" — all its voxels are flagged as falling and enter the movement simulation.

This flood-fill is expensive, so it runs on the CPU asynchronously, triggered by specific events (destruction of structural voxels), not every tick. The GPU flags candidate events; the CPU processes them with a one-to-few-frame latency, which is imperceptible to the player.

### 10.3 Thermal and Chemical Bond Weakening

Bond strength between two voxels is reduced when either voxel's temperature exceeds a material-defined weakening threshold. Corrosive reactions similarly reduce bond strength. These reductions are applied during the reaction pass and may trigger a collapse evaluation if the reduced bond strength falls below the load threshold.

---

## 11. Memory Budget

### 11.1 Voxel Data

At 8 bytes per voxel and 32³ voxels per chunk (32,768 voxels), each chunk's voxel data is 256 KB. With double-buffering, each chunk consumes 512 KB of GPU memory for simulation.

Target: 1 million active voxels ≈ 31 active chunks. GPU memory for active voxel data: 31 × 512 KB ≈ 16 MB.

However, loaded-static and boundary chunks also occupy memory (single-buffered, 256 KB each). A typical scene might have 200 loaded chunks total: 200 × 256 KB + 31 × 256 KB (double-buffer overhead) ≈ 59 MB for voxel data.

### 11.2 Rule Data

Interaction lookup texture (500×500×4 bytes) + rule buffer (10,000 × 32 bytes): ~1.3 MB. Negligible.

### 11.3 Render Buffers

Octree acceleration structure: estimated 50–100 MB depending on world complexity. Framebuffer (2560×1440 RGBA32F): ~15 MB. G-buffer (if used): ~30 MB additional. Light accumulation buffer: ~15 MB.

### 11.4 Total GPU Memory Estimate

Voxel data (~60 MB) + rule data (~1.3 MB) + octree (~100 MB) + render buffers (~60 MB) + overhead (~30 MB) ≈ **250 MB** for a typical scene with 1M active voxels. This is well within the 4–8 GB available on mid-range GPUs.

### 11.5 CPU (WASM) Memory

The WASM module manages chunk metadata, the dispatch list, the rule compiler, save/load buffers, and UI state. The chunk hash map with 200 loaded chunks is negligible. Save/load serialization may require a full copy of voxel data in CPU memory: up to 200 × 256 KB = 50 MB. The REQ 5.2.5 limit of 4 GB system RAM for the WASM module is easily met.

---

## 12. Threading and Async Model

### 12.1 Web Worker Architecture

The main thread runs the render loop, input handling, and UI. A dedicated Web Worker runs the world manager and chunk lifecycle logic. Communication between the main thread and the worker uses SharedArrayBuffer for the chunk state table and postMessage for infrequent control signals (load/unload/save events).

The GPU submission (compute dispatches and render passes) is initiated from the main thread, which holds the WebGPU device and command encoder. The worker thread communicates which chunks to dispatch via the shared chunk state table — the main thread reads this table each frame to build the dispatch list.

### 12.2 Async GPU Readback

The activity scan results (per-chunk dirty flags) must be read back from GPU to CPU. This uses WebGPU's mapAsync on a staging buffer. Because mapAsync is asynchronous and introduces a 1–2 frame latency, the chunk state machine tolerates stale activity data: a chunk that becomes static might be dispatched for 1–2 extra frames, which is harmless (the simulation will simply produce no changes). This avoids any GPU pipeline stalls.

### 12.3 Save/Load Threading

Serialization and compression run on a dedicated Web Worker to avoid blocking the main thread or simulation. The save worker receives a snapshot of the chunk hash map (which chunks exist and their coordinates), then reads voxel data from SharedArrayBuffer, compresses it, and writes to an IndexedDB or File System Access API handle.

---

## 13. Save File Format

### 13.1 Structure

Save files use a custom binary format with the following layout:

**Header (64 bytes):** Magic number ("ALKA"), format version (u16), rule set hash (u64, for compatibility validation), tick count (u64), chunk count (u32), world seed (u64), camera state (position + orientation, 28 bytes), padding.

**Chunk Table:** Array of (chunk_coordinate: i16×3, compressed_data_offset: u64, compressed_data_size: u32) entries. One entry per saved chunk.

**Chunk Data Blocks:** Each chunk's 256 KB voxel data, compressed individually using LZ4 (fast decompression, reasonable ratio). Chunks that are entirely one material type are stored as a single (material_id, fill_flag) pair instead of full voxel data (run-length special case).

### 13.2 Compression Rationale

LZ4 is chosen over zstd or gzip because decompression speed matters more than compression ratio for save files — loading a save must stream chunks in fast enough to not stall the renderer. LZ4 decompresses at ~4 GB/s on modern CPUs. Individual chunk compression allows random-access loading: the game can load chunks near the camera first and stream distant chunks in the background.

### 13.3 Rule Set Compatibility

The save file header includes a hash of the rule set that was active when the save was created. On load, if the current rule set hash differs, the game displays a warning that simulation behavior may differ. The game does not refuse to load — it simply warns. Material IDs are stored in the save file, not material names, so a rule set change that reassigns IDs will produce incorrect results. A migration tool (offline, not part of the engine) can remap IDs between rule set versions.

---

## 14. Input and Tool System

### 14.1 Brush Application Pipeline

Player brush operations (place, remove, heat, push) are not applied directly to the voxel buffer from the CPU. Instead, the CPU writes a command into a GPU-accessible command buffer:

Each command is a struct: (tool_type: u8, brush_shape: u8, center_position: i32×3, radius: u16, material_id: u16, intensity: f16, direction: i8×3).

The simulation pipeline's first compute pass (Pass 1, section 5.2) reads this command buffer and applies the operations to the voxel data. This keeps all voxel writes on the GPU and avoids CPU-GPU synchronization for player actions.

The command buffer is small (max ~64 commands per frame for the most aggressive player input) and double-buffered alongside the voxel data.

### 14.2 Picking / Hover Query

When the player hovers over the world, the UI needs to display the material name and properties of the voxel under the cursor. This requires a GPU-to-CPU readback of a single voxel's data. The renderer writes the hit voxel's world coordinates and material ID into a 1×1 buffer during the ray march pass. This buffer is read back asynchronously (same pattern as the activity scan). The 1–2 frame latency for hover info is imperceptible.

---

## 15. Performance Optimization Strategies

### 15.1 Chunk Sleep / Wake

The most impactful optimization is not simulating static chunks. In a typical sandbox scene, the player modifies a small region while the rest of the world is at rest. If 90% of chunks are static, the simulation workload drops by 90%. The activity scan (Pass 5) and chunk state machine (section 4.2) implement this.

### 15.2 LOD for Rendering

Distant chunks do not need per-voxel ray marching. The octree naturally supports LOD: at higher tree levels, a single node represents a larger volume. The ray marcher can terminate early at coarser octree levels for rays that originate from a distant camera position relative to the chunk. This reduces ray traversal steps for distant geometry at the cost of visual fidelity, which is acceptable because distant voxels are sub-pixel anyway.

### 15.3 Temporal Coherence in Rendering

Between frames, most voxels don't move. The renderer can exploit this by caching the primary ray hit results and only re-tracing rays for pixels whose underlying chunk was marked dirty by the activity scan. This is a form of temporal reprojection and can reduce ray tracing workload by 50–80% in scenes with localized activity. Implementation is complex (requires motion vectors for camera movement) and is deferred to post-launch optimization.

### 15.4 Graceful Degradation

If the frame time exceeds the 60 FPS budget (16.6 ms), the engine reduces the simulation tick rate before reducing rendering quality. The sequence:

1. Reduce simulation to every other frame (30 simulation ticks per second, still rendering at 60 FPS).
2. Reduce simulation to every 4th frame (15 ticks per second).
3. Reduce render resolution (render at 75% resolution, upscale).
4. Reduce octree LOD thresholds (lower visual quality for distant chunks).

The player is informed of degraded simulation rate via a UI indicator.

---

## 16. WebGPU Considerations

### 16.1 Buffer Size Limits

WebGPU implementations may impose maximum buffer sizes (commonly 256 MB per buffer). The voxel double-buffer for all loaded chunks must fit within this limit. At 512 KB per active chunk (double-buffered), 256 MB supports ~500 active chunks (16 million active voxels), which exceeds the performance target. If more loaded chunks are needed (static + boundary), they can be placed in separate buffers partitioned by chunk region.

### 16.2 Shader Compilation Latency

WGSL shader compilation at application startup can take several seconds for complex compute shaders. The engine should compile all shader variants asynchronously during loading screen display and cache compiled pipelines using WebGPU's pipeline caching hints. First-frame jank from lazy compilation must be avoided.

### 16.3 Browser Differences

WebGPU behavior varies between Chrome (Dawn), Firefox (wgpu), and Safari (WebKit). The engine should target the WebGPU specification without relying on implementation-specific behavior. Subgroup operations (useful for optimizing reductions in the activity scan) are not universally available and must be feature-detected, with a fallback path.

### 16.4 WASM-WebGPU Interop

The Rust WASM module accesses WebGPU through the wgpu crate (which targets both native Vulkan/Metal and web WebGPU). This provides a unified API for potential native builds. The wgpu web backend translates Rust API calls into JavaScript WebGPU calls via wasm-bindgen. Overhead is measurable but acceptable for the dispatch-level operations the CPU performs (it's not per-voxel).

---

## 17. Modding Architecture

### 17.1 Mod Loading

A mod is a directory (or zip archive) containing material definition files and interaction rule files in the same format the base game uses. Mods are loaded after the base rule set. Mod materials receive IDs in a reserved range (e.g., 1000+ for base materials, 10000+ for mod materials) to avoid collisions.

### 17.2 Rule Conflict Resolution

If two mods define a rule for the same material pair (A, B), the later-loaded mod's rule wins (last-write-wins by load order). The mod loader logs a warning for every conflict so modders can detect unintentional overrides.

### 17.3 Validation

On load, the mod validator checks: all referenced material IDs exist, temperature thresholds are within the quantization range, density values produce valid gravity behavior, and no rule creates an infinite reaction loop (material A → B and B → A in the same conditions with no energy gate). Validation failures reject the mod with a specific error message, per REQ 10.1.3.

---

## 18. Key Technical Risks and Mitigations

**Risk: Movement conflict resolution introduces visible artifacts (voxels "vibrating" or failing to settle).** Mitigation: Extensive testing of the directional sub-pass approach with adversarial configurations (large volumes of mixed-density fluids). Fallback: increase sub-pass count or switch to a sequential CPU-side movement solver for small active regions.

**Risk: 26-neighbor stencil with halo loading exceeds shared memory on some GPUs.** Mitigation: Profile on minimum-spec hardware early. Fallback: reduce to 6-neighbor (face-only) evaluation for the thermal and pressure passes, keeping 26-neighbor only for reaction detection.

**Risk: WebGPU compute shader precision varies across implementations, breaking determinism.** Mitigation: Avoid floating-point arithmetic in the simulation where possible (use fixed-point or integer math for critical state transitions). Temperature is the one f16 field; accept that f16 rounding may differ and don't rely on exact temperature values for determinism — use threshold comparisons with sufficient margin.

**Risk: The interaction matrix grows superlinearly with material count, making 500 materials unbalanceable.** Mitigation: Most material pairs should have no interaction (the default). The 10,000 rule target covers only the interesting pairs. Category-level default rules (e.g., "all metals conduct heat well") reduce per-pair authoring. AI-assisted generation and playtesting (REQ 11.1.1) is specifically aimed at this problem.

**Risk: Browser WebGPU implementations regress or diverge during development.** Mitigation: Run CI tests against Chrome Canary and Firefox Nightly. Design the abstraction layer (via wgpu) to allow a native build as a fallback distribution path.

---

## Appendix A: Technology Stack Summary

| Component | Technology | Rationale |
|---|---|---|
| Core language | Rust | Memory safety, WASM compilation, wgpu ecosystem |
| WASM toolchain | wasm-pack + wasm-bindgen | Mature Rust-to-WASM pipeline |
| GPU API | WebGPU (via wgpu) | Modern compute shader support in browsers; portable to native |
| Shader language | WGSL | Required by WebGPU; wgpu also accepts SPIR-V for native |
| Data format (rules) | RON | Rust-native, human-readable, serde-compatible |
| Compression (saves) | LZ4 | Fast decompression, acceptable ratio |
| UI framework | egui (via egui-wgpu) | Pure Rust, WebGPU-integrated, immediate-mode |
| Audio (future) | Web Audio API | Browser-native, no additional dependencies |

## Appendix B: Glossary of Architecture Terms

- **Activity scan:** A lightweight GPU compute pass that determines which chunks had voxel state changes, used to transition chunks between active and static states.
- **Boundary chunk:** A chunk adjacent to an active chunk that must be readable for neighbor lookups but is not itself simulated.
- **Checkerboard pattern:** A parallel processing technique where alternating cells are processed in separate sub-passes to prevent write conflicts.
- **Command buffer (player):** A small GPU buffer containing player tool operations to be applied at the start of each simulation tick.
- **DDA (Digital Differential Analyzer):** An algorithm for efficiently stepping a ray through a regular grid, visiting each cell the ray passes through.
- **Double buffering:** Maintaining two copies of the voxel state so the simulation can read from one while writing to the other, preventing data races.
- **Halo region:** A border of neighboring voxels loaded into workgroup shared memory to enable neighbor lookups without global memory access at tile boundaries.
- **Rule set hash:** A hash of all loaded material and interaction definitions, used to detect compatibility between save files and the current rule configuration.
- **Staging buffer:** A GPU buffer with CPU-read access used for asynchronous readback of data from the GPU.
- **Stencil operation:** A computation pattern where each cell's new value depends on a fixed neighborhood of surrounding cells.
