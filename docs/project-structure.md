# ALKAHEST: Project Structure

**Version:** 0.1.0-draft
**Date:** 2026-02-13
**Status:** Draft
**Companions:** requirements.md, architecture.md, milestones.md

---

## 1. How to Use This Document

This document defines the crate layout, module hierarchy, file organization, and public API boundaries for Alkahest. Each module is annotated with the milestone that introduces it. **Do not create modules, files, or placeholder stubs for milestones that have not been started.** The structure grows incrementally as milestones are completed.

Milestone annotations use the format `[M0]`, `[M1]`, etc. A module annotated `[M5]` does not exist until Milestone 5 begins. If a module is extended by a later milestone, both are noted: `[M3, extended M6]`.

---

## 2. Workspace Layout

Alkahest is a Cargo workspace with multiple crates. The workspace root contains no source code — only workspace configuration, CI scripts, and project-level documentation.

```
alkahest/
├── Cargo.toml                  [M0] Workspace manifest
├── Cargo.lock                  [M0]
├── README.md                   [M0]
├── docs/
│   ├── requirements.md         [M0] (pre-existing)
│   ├── architecture.md         [M0] (pre-existing)
│   ├── milestones.md           [M0] (pre-existing)
│   ├── project-structure.md    [M0] (this document)
│   └── modding-guide.md        [M12]
├── crates/
│   ├── alkahest-core/          [M0] Core engine library
│   ├── alkahest-web/           [M0] WASM entry point and browser glue
│   ├── alkahest-sim/           [M2] Simulation pipeline
│   ├── alkahest-render/        [M1] Rendering pipeline
│   ├── alkahest-rules/         [M3] Rule engine, material definitions, interaction matrix
│   ├── alkahest-world/         [M5] Multi-chunk world management
│   ├── alkahest-persist/       [M8] Save/load serialization
│   ├── alkahest-audio/         [M13] Audio system (optional)
│   └── alkahest-bench/         [M11] Benchmark harness (not shipped to users)
├── shaders/
│   ├── render/                 [M1] Rendering shaders (WGSL)
│   └── sim/                    [M2] Simulation compute shaders (WGSL)
├── data/
│   ├── materials/              [M3] Base material definitions (RON)
│   ├── rules/                  [M3] Base interaction rules (RON)
│   └── mods/                   [M12] Example mod packs
├── tests/
│   ├── determinism/            [M2] Deterministic snapshot tests
│   ├── rules/                  [M3] Rule validation and reaction tests
│   ├── benchmarks/             [M11] Performance benchmark scenes
│   └── integration/            [M5] Multi-chunk integration tests
├── tools/
│   ├── reaction-catalog/       [M9] Interaction matrix visualization (standalone HTML)
│   └── id-migration/           [M12] Material ID remapping tool
├── web/
│   ├── index.html              [M0] Host page
│   ├── style.css               [M7] Minimal UI styling
│   └── worker.js               [M5] Web Worker bootstrap
└── ci/
    ├── build.sh                [M0]
    ├── test.sh                 [M0]
    └── bench.sh                [M11]
```

---

## 3. Crate Dependency Graph

Dependencies flow downward. No crate may depend on a crate above it or at the same level (no circular dependencies). `alkahest-web` is the root and depends on everything. `alkahest-core` is the leaf and has no internal dependencies.

```
alkahest-web        [M0]  Entry point, browser bindings, frame loop
 ├── alkahest-render  [M1]  Rendering pipeline
 │    └── alkahest-core
 ├── alkahest-sim     [M2]  Simulation pipeline
 │    ├── alkahest-rules [M3]
 │    │    └── alkahest-core
 │    └── alkahest-core
 ├── alkahest-world   [M5]  Chunk management, spatial data structures
 │    ├── alkahest-sim
 │    ├── alkahest-render
 │    └── alkahest-core
 ├── alkahest-persist [M8]  Save/load
 │    ├── alkahest-world
 │    ├── alkahest-rules
 │    └── alkahest-core
 ├── alkahest-audio   [M13] Audio (optional)
 │    ├── alkahest-world
 │    └── alkahest-core
 └── alkahest-core    [M0]  Shared types, math, constants
```

`alkahest-bench` depends on `alkahest-world`, `alkahest-sim`, and `alkahest-render`, but is never a dependency of anything else. It is excluded from the WASM build.

---

## 4. Crate Details

### 4.1 alkahest-core [M0]

Shared types, constants, and utility code used by all other crates. This crate has zero dependencies on wgpu or any GPU API — it is pure Rust data types and math.

```
alkahest-core/src/
├── lib.rs
├── types.rs            [M0] VoxelData, ChunkCoord, WorldCoord, MaterialId
│                             type aliases and newtypes
├── constants.rs        [M0] CHUNK_SIZE (32), VOXEL_BYTES (8),
│                             MAX_MATERIALS (65535), AMBIENT_TEMP (293.0)
├── math.rs             [M0] Fixed-point helpers, temperature quantization
│                             (f32 ↔ 12-bit), coordinate conversions
│                             (world ↔ chunk-local ↔ voxel index)
├── material.rs         [M3] MaterialDef struct (all properties from ARCH 6.2),
│                             MaterialTable (indexed by MaterialId),
│                             Phase enum, category tags
├── rule.rs             [M3] InteractionRule struct (inputs, conditions, outputs),
│                             RuleSet container, rule file schema types
├── direction.rs        [M2] Direction enum (26 neighbors), offset tables,
│                             face/edge/corner classification
└── error.rs            [M0] Shared error types (AlkahestError enum)
```

**Public API boundary:** Everything in this crate is pub. It is the shared vocabulary of the project. Changes to types here affect all downstream crates, so the types should be stable before downstream crates are built. In practice, `types.rs`, `constants.rs`, `math.rs`, `direction.rs`, and `error.rs` are defined in M0–M2 and rarely change. `material.rs` and `rule.rs` are added in M3 and stabilize after M3.

### 4.2 alkahest-web [M0]

The WASM entry point. This crate owns the browser event loop, the wgpu Device and Queue, the egui context, and the frame orchestration. It is the only crate that touches browser APIs (web-sys, wasm-bindgen).

```
alkahest-web/src/
├── lib.rs              [M0] wasm_bindgen entry point, #[wasm_bindgen(start)]
├── app.rs              [M0, extended M2/M5/M7] Application struct: holds all
│                             subsystem handles, runs the per-frame update sequence
│                             (input → chunk mgmt → sim → render → UI)
├── gpu.rs              [M0] WebGPU initialization: adapter request, device request,
│                             surface configuration, feature detection
├── input.rs            [M1, extended M7] Keyboard/mouse event capture,
│                             input state struct, keybinding map
├── camera.rs           [M1, extended M7] Free-orbit camera (M1), first-person
│                             camera (M7), camera mode switching
├── ui/
│   ├── mod.rs          [M0] egui integration, panel layout
│   ├── debug.rs        [M0, extended M2/M5] Debug panel: FPS, frame time,
│   │                         tick count, active voxels, chunk counts
│   ├── toolbar.rs      [M7] Tool palette: brush type, brush size, active tool
│   ├── browser.rs      [M7] Material browser: search, categories, selection
│   ├── hud.rs          [M7] In-game HUD: current tool, material, sim speed
│   ├── hover.rs        [M7] Voxel hover info panel
│   └── settings.rs     [M7] Settings menu: keybindings, display, audio toggle
├── tools/
│   ├── mod.rs          [M2] Tool trait, tool registry, active tool state
│   ├── place.rs        [M2, extended M7] Place voxel tool
│   │                         (M2: single voxel only; M7: all brush shapes)
│   ├── remove.rs       [M2, extended M7] Remove voxel tool
│   ├── heat.rs         [M4] Heat gun / freeze tool
│   ├── push.rs         [M7] Directional force tool
│   └── brush.rs        [M7] Brush shape definitions: single, cube, sphere
├── commands.rs         [M2, extended M4/M7] Player command buffer: encodes tool
│                             actions into GPU-uploadable command structs
└── worker.rs           [M5] Web Worker communication: SharedArrayBuffer setup,
                              postMessage protocol for chunk lifecycle events
```

**Public API boundary:** This crate exposes nothing — it is the application root. All pub interfaces point inward (it calls into other crates).

### 4.3 alkahest-render [M1]

Owns the rendering pipeline: ray marching, lighting, transparency, and the render octree (when multi-chunk is introduced).

```
alkahest-render/src/
├── lib.rs
├── renderer.rs         [M1, extended M5/M10] Top-level Renderer struct.
│                             Owns all render pipelines, bind groups, and
│                             render-specific GPU buffers. Public methods:
│                             new(), resize(), render_frame(), set_clip_plane()
├── ray_march.rs        [M1, extended M5/M10] Ray march shader pipeline setup,
│                             bind group layout, uniform buffer management.
│                             M1: single-chunk DDA. M5: multi-chunk with octree
│                             traversal. M10: LOD integration.
├── lighting.rs         [M1, extended M10] Light buffer management.
│                             M1: single hardcoded light. M10: dynamic light
│                             extraction from emissive voxels, shadow ray config.
├── ao.rs               [M10] Ambient occlusion computation and buffer management
├── transparency.rs     [M10] Volumetric transparency compositing configuration
├── octree.rs           [M5, extended M10] Render-side sparse voxel octree.
│                             Built from chunk data. Incrementally updated when
│                             chunks change. Used by ray_march.rs for empty-space
│                             skipping and by LOD for distant rendering.
├── pick.rs             [M7] GPU pick buffer: writes hit voxel coords + material ID
│                             during ray march, read back for hover info
├── sky.rs              [M10] Procedural sky rendering
└── debug_lines.rs      [M1] Wireframe debug rendering (chunk boundaries, brush
                              preview ghost). Uses a simple line-list pipeline
                              separate from the ray marcher.
```

**Public API boundary:** The `Renderer` struct in `renderer.rs` is the only public interface. All other modules are `pub(crate)`. External code calls `renderer.new()`, `renderer.render_frame(voxel_buffer, camera, lights, clip_plane)`, etc. Internal details (octree, AO, transparency) are hidden.

### 4.4 alkahest-sim [M2]

Owns the simulation compute pipeline: all simulation passes, double buffering, and the deterministic test harness.

```
alkahest-sim/src/
├── lib.rs
├── pipeline.rs         [M2, extended M3/M4/M5/M6] Top-level SimPipeline struct.
│                             Owns all compute pipelines, the double buffer, and
│                             the command buffer. Public methods: new(),
│                             tick(), apply_commands(), get_current_buffer(),
│                             pause(), resume(), set_tick_rate()
│                             M2: gravity pass only. M3: adds reaction pass.
│                             M4: adds thermal pass. M5: multi-chunk dispatch.
│                             M6: adds pressure pass.
├── buffers.rs          [M2, extended M5] Double-buffer management.
│                             M2: single chunk pair. M5: chunk-pool allocator
│                             with slot management for multi-chunk buffers.
├── passes/
│   ├── mod.rs          [M2] Pass trait, pass ordering
│   ├── commands.rs     [M2] Pass 1: player command application
│   ├── movement.rs     [M2, extended M3] Pass 2: gravity + displacement.
│   │                         M2: hardcoded sand gravity. M3: density-driven,
│   │                         liquid flow, gas rise (uses material table).
│   ├── reactions.rs    [M3] Pass 3: interaction matrix evaluation,
│   │                         byproduct spawning, state transitions
│   ├── thermal.rs      [M4] Pass 4a: heat diffusion, entropy drain, convection bias
│   ├── pressure.rs     [M6] Pass 4b: pressure accumulation, diffusion, rupture
│   └── activity.rs     [M5] Pass 5: per-chunk dirty flag scan
├── conflict.rs         [M2] Checkerboard sub-pass scheduling, direction ordering
├── rng.rs              [M2] Deterministic per-voxel PRNG (coordinate + tick hash)
├── structural.rs       [M6] CPU-side structural integrity: bond evaluation,
│                             flood-fill disconnection detection, collapse flagging
│                             (runs async, not per-tick)
└── test_harness.rs     [M2] Deterministic snapshot test infrastructure:
                              init state → run N ticks → readback → compare.
                              Used by tests/ directory, not shipped to users.
```

**Public API boundary:** The `SimPipeline` struct in `pipeline.rs` is the only public interface. The `test_harness` module is `pub` but `#[cfg(test)]` gated (available for integration tests in the `tests/` directory but not in release builds). All pass modules are `pub(crate)`.

### 4.5 alkahest-rules [M3]

Owns material definitions, interaction rules, file parsing, validation, and GPU-side rule data compilation.

```
alkahest-rules/src/
├── lib.rs
├── loader.rs           [M3, extended M12] Parse RON files, deserialize material
│                             and rule definitions. M12: mod directory scanning,
│                             multi-file loading, load order.
├── validator.rs        [M3, extended M12] Validate loaded rules: property ranges,
│                             material ID uniqueness, infinite loop detection,
│                             CFL stability check for thermal conductivity (M4).
│                             M12: cross-mod conflict detection, ID range checks.
├── compiler.rs         [M3] Compile the loaded MaterialTable and RuleSet into
│                             GPU-uploadable buffers: material property buffer
│                             (indexed by MaterialId), interaction lookup texture
│                             (2D, sparse), packed rule buffer.
├── defaults.rs         [M9] Category-level default behaviors: "all metals
│                             conduct heat," "all organics are flammable."
│                             Applied during compilation before per-material
│                             overrides.
└── migration.rs        [M12] Material ID remapping between rule set versions.
                              Offline utility, not used at runtime.
```

**Public API boundary:** Public interface consists of: `load_rules(paths) → Result<(MaterialTable, RuleSet)>`, `validate(table, rules) → Result<Vec<Warning>>`, `compile_for_gpu(table, rules, device) → GpuRuleData`. The `GpuRuleData` struct (containing wgpu buffer handles) is passed to `SimPipeline::new()`.

### 4.6 alkahest-world [M5]

Owns the multi-chunk world: chunk hash map, chunk state machine, spatial queries, and chunk lifecycle orchestration.

```
alkahest-world/src/
├── lib.rs
├── world.rs            [M5] Top-level World struct. Owns the chunk map, the
│                             dispatch list builder, and the octree rebuild
│                             scheduler. Public methods: new(), update(),
│                             get_dispatch_list(), load_chunk(), unload_chunk(),
│                             get_chunk_state(), set_voxel(), get_voxel()
├── chunk.rs            [M5] Chunk struct: coordinates, state enum, GPU buffer
│                             slot reference, activity counter, neighbor links.
│                             ChunkState enum: Unloaded, Static, Active, Boundary.
├── chunk_map.rs        [M5] HashMap<ChunkCoord, Chunk> with spatial query helpers:
│                             get_neighbors_26(), chunks_in_radius(),
│                             chunks_in_box()
├── state_machine.rs    [M5] Chunk state transition logic: activation propagation,
│                             boundary promotion, sleep-after-N-ticks, wake-on-
│                             neighbor-activity. Consumes activity scan results.
├── dispatch.rs         [M5] Builds the per-frame dispatch list: which chunks to
│                             simulate, in what order, with what neighbor table.
│                             Outputs a DispatchList struct consumed by SimPipeline.
├── streaming.rs        [M5] Camera-distance-based chunk loading/unloading.
│                             Manages the load queue and unload queue.
└── terrain.rs          [M5] Procedural terrain generation for initial world
                              population. Simple noise-based heightmap producing
                              stone, sand, and water layers. Used for test scenes
                              and new-game world setup.
```

**Public API boundary:** The `World` struct in `world.rs` is the primary interface. `chunk_map.rs` is `pub(crate)`. The `DispatchList` type returned by `dispatch.rs` is pub because `SimPipeline` consumes it. `terrain.rs` is pub for use by `alkahest-web` when creating a new game.

### 4.7 alkahest-persist [M8]

Owns save/load serialization, compression, and file I/O.

```
alkahest-persist/src/
├── lib.rs
├── format.rs           [M8] Save file format constants: magic number, version,
│                             header struct, chunk table entry struct
├── save.rs             [M8] Serialization: iterate loaded chunks, compress each
│                             with LZ4, write header + chunk table + data blocks.
│                             Designed to run on a Web Worker (no wgpu references,
│                             operates on raw voxel byte slices).
├── load.rs             [M8] Deserialization: read header, validate, decompress
│                             chunks, return a Vec of (ChunkCoord, voxel_data).
│                             The caller (alkahest-web) uploads to GPU.
├── compress.rs         [M8] LZ4 compression/decompression wrappers.
│                             Handles the single-material-fill special case.
├── compat.rs           [M8] Rule set hash comparison, version compatibility
│                             checks, warning generation for mismatched saves.
└── subregion.rs        [M8] Subregion export: given a bounding box, filter
                              the chunk set and produce a save file containing
                              only the selected chunks.
```

**Public API boundary:** Public interface: `save(chunks, rule_hash, tick, camera) → Vec<u8>`, `load(bytes) → Result<SaveData>`, `export_subregion(chunks, bbox, ...) → Vec<u8>`. The `SaveData` struct contains the header metadata plus a `Vec<(ChunkCoord, Vec<u8>)>` of decompressed chunk data.

### 4.8 alkahest-audio [M13] (Optional)

Owns procedural audio generation and spatial mixing.

```
alkahest-audio/src/
├── lib.rs
├── scanner.rs          [M13] Reads active chunk data, identifies acoustic events
│                              (fire density, water flow, rupture events), outputs
│                              a list of AudioSource { position, type, intensity }
├── generators.rs       [M13] Procedural audio synthesis per sound type:
│                              crackle, hiss, flow, rumble, boom.
│                              Wraps Web Audio API oscillators and noise nodes.
├── mixer.rs            [M13] Spatial attenuation, panning, priority-based
│                              source limiting, master volume
└── bridge.rs           [M13] web-sys bindings to Web Audio API (AudioContext,
                               GainNode, PannerNode, etc.)
```

**Public API boundary:** Public interface: `AudioSystem::new()`, `AudioSystem::update(camera, audio_sources)`, `AudioSystem::set_enabled(bool)`. The scanner is called by `alkahest-web`; the rest is internal.

### 4.9 alkahest-bench [M11]

Benchmark harness. Excluded from the WASM build (native-only). Not shipped to users.

```
alkahest-bench/src/
├── lib.rs
├── scenes.rs           [M11] Standardized benchmark scenes at various voxel counts.
│                              Each scene defines initial voxel state, camera, and
│                              expected active voxel count.
├── runner.rs           [M11] Runs a scene for N ticks, records per-frame timing
│                              for each simulation pass and the renderer.
└── report.rs           [M11] Outputs timing results as JSON and markdown table.
                               Compares against a baseline file and flags regressions.
```

---

## 5. Shader File Organization

Shaders live in a top-level `shaders/` directory, not inside any crate. They are loaded at build time (embedded via `include_str!` or a build script) or at runtime during development (for hot-reload iteration). The split between `render/` and `sim/` mirrors the crate split between `alkahest-render` and `alkahest-sim`.

```
shaders/
├── common/
│   ├── types.wgsl          [M1] Shared struct definitions: VoxelData, MaterialProps,
│   │                             CameraUniforms. Included by other shaders.
│   ├── coords.wgsl         [M1] Coordinate conversion functions: world ↔ chunk ↔ local,
│   │                             linear index ↔ 3D position within chunk.
│   └── rng.wgsl            [M2] Deterministic hash-based PRNG for compute shaders.
├── render/
│   ├── ray_march.wgsl      [M1, extended M5/M10] Primary visibility ray marcher.
│   │                             M1: single-chunk DDA. M5: multi-chunk octree traversal.
│   │                             M10: LOD termination, volumetric transparency.
│   ├── lighting.wgsl       [M1, extended M10] Direct lighting + shadow rays.
│   │                             M1: single light. M10: multi-light loop, AO term.
│   ├── sky.wgsl            [M10] Procedural sky / background.
│   ├── composite.wgsl      [M10] Final compositing: tone mapping, gamma correction.
│   ├── debug_lines.wgsl    [M1] Wireframe line rendering (vertex + fragment).
│   └── pick.wgsl           [M7] Write hit voxel info to pick buffer during ray march.
├── sim/
│   ├── commands.wgsl       [M2] Pass 1: apply player commands to voxel buffer.
│   ├── movement.wgsl       [M2, extended M3] Pass 2: gravity, density displacement,
│   │                             liquid flow, gas rise. Contains sub-pass logic
│   │                             for checkerboard conflict resolution.
│   ├── reactions.wgsl      [M3] Pass 3: interaction matrix lookup, byproduct spawning.
│   ├── thermal.wgsl        [M4] Pass 4a: heat diffusion stencil, entropy drain.
│   ├── pressure.wgsl       [M6] Pass 4b: pressure diffusion, rupture detection.
│   └── activity.wgsl       [M5] Pass 5: per-chunk dirty flag reduction.
```

**Shared code between shaders:** WGSL does not have a native `#include` mechanism. The build pipeline concatenates `common/*.wgsl` files as a preamble to each shader that needs them. A build script in the workspace root handles this concatenation and outputs the final shader strings for embedding. This is simple but effective — avoid over-engineering a shader module system.

---

## 6. Data File Organization

```
data/
├── materials/
│   ├── _schema.ron         [M3] Schema documentation / example (not loaded by engine)
│   ├── naturals.ron        [M3] Stone, sand, water, air, ice, etc.
│   ├── metals.ron          [M3, extended M9] Iron, copper, gold, etc.
│   ├── organics.ron        [M3, extended M9] Wood, plant matter, oil, etc.
│   ├── energy.ron          [M3, extended M9] Fire, smoke, steam, plasma, etc.
│   ├── synthetics.ron      [M9] Polymers, ceramics, composites
│   ├── exotic.ron          [M9] Gameplay-only fictional materials
│   └── electrical.ron      [M15] Conductive, resistive, logic materials (optional)
├── rules/
│   ├── _schema.ron         [M3] Schema documentation / example
│   ├── combustion.ron      [M3, extended M9] Fire/fuel interactions
│   ├── phase_change.ron    [M3, extended M4/M9] Melting, boiling, condensation
│   ├── displacement.ron    [M3, extended M9] Density-based material displacement
│   ├── dissolution.ron     [M9] Acid, solvent interactions
│   ├── biological.ron      [M9] Organic growth, decay
│   ├── electrical.ron      [M15] Conductivity, short-circuit interactions (optional)
│   └── structural.ron      [M6] Bond strength overrides, corrosion rules
└── mods/
    └── example-mod/        [M12]
        ├── mod.ron          Mod metadata: name, version, load order hint
        ├── materials/
        │   └── crystals.ron Custom material definitions
        └── rules/
            └── crystal_interactions.ron
```

Material and rule files are split by category for human readability, but the engine loads all files in the `materials/` and `rules/` directories. File names are conventions, not load-order-significant.

---

## 7. Test Organization

```
tests/
├── determinism/
│   ├── single_chunk.rs     [M2] Single-chunk deterministic snapshot tests
│   │                             (sand fall, pile formation, avalanche)
│   ├── reactions.rs        [M3] Reaction snapshot tests
│   │                             (combustion, extinguishing, density displacement)
│   ├── thermal.rs          [M4] Thermal diffusion snapshot tests
│   │                             (equilibrium, melting, convection)
│   ├── pressure.rs         [M6] Pressure snapshot tests
│   │                             (accumulation, rupture, blast propagation)
│   └── multi_chunk.rs      [M5] Cross-chunk boundary snapshot tests
├── rules/
│   ├── validation.rs       [M3] Rule file parsing and validation tests
│   │                             (valid files, malformed files, infinite loops)
│   ├── balancing.rs        [M9] Automated degenerate behavior detection
│   │                             (self-replication, runaway temperature, fuel exhaustion)
│   └── mod_loading.rs      [M12] Mod loading, conflict resolution, rejection tests
├── integration/
│   ├── save_load.rs        [M8] Save/load round-trip correctness
│   ├── chunk_lifecycle.rs  [M5] Chunk state transitions, activation propagation
│   └── structural.rs       [M6] Structural collapse scenarios
└── benchmarks/
    ├── scenes/             [M11] Benchmark scene definitions (RON or Rust)
    └── baselines/          [M11] Known-good timing baselines (JSON)
```

Test files live in the workspace-level `tests/` directory (integration tests) rather than inside individual crates. This is intentional: most tests exercise multiple crates working together (sim + rules + world). Unit tests within individual crates use the standard `#[cfg(test)] mod tests` pattern inside source files.

---

## 8. Module Introduction by Milestone

This table lists every module and the milestone that creates it. Use this as the canonical reference for what to build when.

| Milestone | New Crates | New Modules (in existing crates) | New Shaders | New Data Files |
|---|---|---|---|---|
| M0 | alkahest-core, alkahest-web | core: types, constants, math, error. web: lib, app, gpu, ui/mod, ui/debug | — | — |
| M1 | alkahest-render | render: renderer, ray_march, lighting, debug_lines. web: input, camera | common/types, common/coords, render/ray_march, render/lighting, render/debug_lines | — |
| M2 | alkahest-sim | sim: pipeline, buffers, passes/mod, passes/commands, passes/movement, conflict, rng, test_harness. core: direction. web: tools/mod, tools/place, tools/remove, commands | common/rng, sim/commands, sim/movement | — |
| M3 | alkahest-rules | rules: loader, validator, compiler. core: material, rule. sim: passes/reactions. sim/passes/movement extended. | sim/reactions. sim/movement extended. | materials/*.ron (initial 10), rules/*.ron (initial 15+), _schema files |
| M4 | — | sim: passes/thermal. web: tools/heat. rules/validator extended (CFL check). | sim/thermal | rules/phase_change.ron extended |
| M5 | alkahest-world | world: world, chunk, chunk_map, state_machine, dispatch, streaming, terrain. sim: passes/activity, buffers extended. render: octree. web: worker. | sim/activity. render/ray_march extended. | — |
| M6 | — | sim: passes/pressure, structural. | sim/pressure | rules/structural.ron, new materials in materials/*.ron |
| M7 | — | web: ui/toolbar, ui/browser, ui/hud, ui/hover, ui/settings, tools/brush, tools/push. web/input extended, web/camera extended. render: pick. | render/pick | — |
| M8 | alkahest-persist | persist: format, save, load, compress, compat, subregion | — | — |
| M9 | — | rules: defaults. | — | materials/*.ron expanded, rules/*.ron expanded |
| M10 | — | render: ao, transparency, sky. render/ray_march extended, render/lighting extended. | render/sky, render/composite. Others extended. | — |
| M11 | alkahest-bench | bench: scenes, runner, report | — | — |
| M12 | — | rules: loader extended, validator extended, migration. | — | mods/example-mod/*, docs/modding-guide.md |
| M13 | alkahest-audio | audio: scanner, generators, mixer, bridge | — | — |
| M14 | — | — (content expansion, no new modules) | — | materials/*.ron expanded, rules/*.ron expanded |
| M15 | — | Modules in sim and rules for electrical. | — | materials/electrical.ron, rules/electrical.ron |

---

## 9. API Boundary Rules

These rules govern how crates communicate. They are enforced by code review and by the Cargo dependency graph (a crate cannot call into a crate it doesn't depend on).

**Rule 1: One public struct per crate.** Each engine crate exposes a single primary struct as its public API (`Renderer`, `SimPipeline`, `World`, etc.). Helper types returned by that struct's methods (e.g., `DispatchList`, `SaveData`) are also public, but there should be no ambient public functions or module-level exports. This keeps the API surface small and auditable.

**Rule 2: GPU resources stay inside their owning crate.** `alkahest-render` owns render pipelines and bind groups. `alkahest-sim` owns compute pipelines and the double buffer. The two crates share voxel data by passing wgpu `Buffer` handles across crate boundaries (the buffer is created by one crate and bound by the other), but neither crate directly creates or modifies the other's pipelines.

**Rule 3: alkahest-web orchestrates, other crates execute.** The frame loop in `alkahest-web` calls methods on `World`, `SimPipeline`, and `Renderer` in the correct order. The engine crates do not call each other directly except through data dependencies (e.g., `SimPipeline::tick()` takes a `&DispatchList` produced by `World::get_dispatch_list()`). This prevents hidden coupling between subsystems.

**Rule 4: No wgpu in alkahest-core.** The core crate is pure data types. This ensures it compiles fast, has no GPU dependencies, and can be used in offline tools (reaction catalog, ID migration, test harness setup).

**Rule 5: Shaders are not API.** Shader source files are implementation details of the crate that uses them. `alkahest-render` embeds render shaders; `alkahest-sim` embeds sim shaders. No shader file is shared between crates. Shared WGSL code (type definitions, coordinate helpers) lives in `shaders/common/` and is concatenated into both render and sim shaders by the build script, but the concatenation is a build-time concern, not a runtime dependency between crates.

---

## 10. File Naming Conventions

**Rust source:** Snake case. One primary type per file, file named after the type. `chunk_map.rs` contains `ChunkMap`. `ray_march.rs` contains ray march pipeline setup. Exceptions: `mod.rs` for module roots, `lib.rs` for crate roots.

**Shaders:** Snake case, `.wgsl` extension. Named after the simulation pass or render stage they implement. `movement.wgsl`, `ray_march.wgsl`.

**Data files:** Snake case, `.ron` extension. Named after the material category or rule category they contain. `naturals.ron`, `combustion.ron`.

**Tests:** Snake case, `.rs` extension. Named after the subsystem they test. `single_chunk.rs`, `save_load.rs`.

---

## 11. Build Outputs

```
target/
└── wasm32-unknown-unknown/
    └── release/
        ├── alkahest_web_bg.wasm    The WASM binary (~5-15 MB compressed)
        ├── alkahest_web.js         wasm-bindgen JavaScript glue
        └── alkahest_web.d.ts       TypeScript type definitions
```

The `web/index.html` file loads `alkahest_web.js`, which initializes the WASM module. Build artifacts are placed in a `dist/` directory by the CI script for deployment. The `dist/` directory contains: `index.html`, `style.css`, the WASM binary, the JS glue, and the `data/` directory (materials and rules, for runtime loading).

Data files are loaded at runtime via fetch (not embedded in WASM) so that mods can be added without recompiling. The base data files are served alongside the WASM binary.

---

## 12. What NOT to Create

At any given milestone, the following should not exist in the repository:

- Empty stub files for future modules ("TODO: implement in M7").
- Trait definitions for systems that don't exist yet ("pub trait AudioSystem" before M13).
- Placeholder crates with only a `lib.rs` containing `// coming soon`.
- Abstract interfaces designed to accommodate hypothetical future requirements. Write concrete code for the current milestone; refactor when the next milestone reveals the actual abstraction needed.
- `mod.rs` files that re-export modules from future milestones.

The project structure grows by accretion. Each milestone adds exactly what it needs. If a later milestone requires restructuring an earlier module (e.g., splitting a file that grew too large), that restructuring happens as part of the later milestone, not preemptively.
