# ALKAHEST: 3D Volumetric Cellular Automata Sandbox

## Requirements Specification

**Version:** 1.0.0
**Date:** 2026-02-16
**Status:** Fulfilled

---

## 1. Terminology

The key words "MUST", "MUST NOT", "REQUIRED", "SHALL", "SHALL NOT", "SHOULD", "SHOULD NOT", "RECOMMENDED", "MAY", and "OPTIONAL" in this document are to be interpreted as described in [RFC 2119](https://www.ietf.org/rfc/rfc2119.txt).

---

## 2. Project Overview

Alkahest is a browser-based 3D voxel sandbox built on cellular automata principles. Players interact with a volumetric simulation of 500+ material types governed by physically-grounded interaction rules. The game targets WebGPU-capable browsers and is authored primarily in Rust (compiled to WASM) with WGSL compute shaders executing the simulation on the GPU.

---

## 3. Platform and Runtime Requirements

### 3.1 Target Platform

3.1.1. The application MUST run in modern desktop browsers that support the WebGPU API.

3.1.2. The application MUST be compiled to WebAssembly (WASM) from a Rust source codebase.

3.1.3. The application SHOULD support Chrome, Edge, and Firefox on Windows, macOS, and Linux. Safari support is OPTIONAL.

3.1.4. The application MUST NOT require any browser plugins, extensions, or native installations.

3.1.5. The application MAY provide a native desktop build (Windows, macOS, Linux) using the same Rust codebase with a Vulkan or Metal backend, but this is OPTIONAL and SHALL NOT take priority over the browser target.

### 3.2 Performance Targets

3.2.1. The simulation MUST sustain a minimum of 60 frames per second with 1,000,000 (one million) active voxels on a mid-range discrete GPU (e.g., NVIDIA RTX 4060 or equivalent).

3.2.2. The simulation SHOULD sustain 60 FPS with up to 3,000,000 (three million) active voxels on a high-end discrete GPU (e.g., NVIDIA RTX 4090 or equivalent).

3.2.3. The simulation MUST gracefully degrade when voxel counts exceed hardware capabilities. The application MUST NOT crash or hang; it SHOULD reduce simulation tick rate before dropping frames.

3.2.4. GPU compute shaders MUST execute the core simulation loop. The CPU MUST NOT perform per-voxel simulation logic at runtime.

3.2.5. Memory usage MUST NOT exceed 4 GB of system RAM for the WASM module under normal gameplay conditions.

3.2.6. The initial application payload (WASM + assets) SHOULD NOT exceed 50 MB compressed.

---

## 4. Simulation Engine

### 4.1 Voxel Representation

4.1.1. The simulation world MUST be composed of discrete cubic voxels on a uniform 3D grid.

4.1.2. Each voxel MUST store, at minimum, the following state: material type ID (16-bit unsigned integer), temperature (16-bit float), velocity vector (3x8-bit or 3x16-bit fixed-point), pressure (8-bit unsigned integer), and a general-purpose flags/metadata field (8-bit).

4.1.3. The voxel data layout MUST be optimized for GPU cache coherency. Struct-of-arrays (SoA) layout is RECOMMENDED over array-of-structs (AoS).

4.1.4. Voxel state MUST be double-buffered: the simulation reads from buffer A and writes to buffer B, then swaps. The simulation MUST NOT read and write to the same buffer in a single tick to avoid race conditions.

### 4.2 Spatial Data Structure

4.2.1. The world MUST use a chunked sparse voxel octree (or equivalent spatial acceleration structure) to avoid allocating memory for empty regions.

4.2.2. Chunks MUST be fixed-size cubic regions. A chunk size of 32x32x32 or 64x64x64 voxels is RECOMMENDED.

4.2.3. Only chunks containing at least one active (non-empty, non-static) voxel SHOULD be dispatched to the GPU for simulation. Fully static or empty chunks MUST be skipped.

4.2.4. The engine MUST support dynamic chunk loading and unloading as the player moves through the world or as simulation activity spreads.

4.2.5. The world SHOULD support a minimum explorable volume of 1024x512x1024 voxels (approximately 512 million potential cells), with only the active subset resident in memory.

### 4.3 Cellular Automata Rules

4.3.1. Each simulation tick, every active voxel MUST evaluate its state against its 26 immediate 3D neighbors (the Moore neighborhood in 3D).

4.3.2. The simulation MUST support the following fundamental interaction categories:
  - (a) **State transitions:** A voxel changing its material type (e.g., ice → water at temperature threshold).
  - (b) **Movement:** A voxel swapping position with a neighbor based on density, gravity, and velocity (e.g., sand falling, gas rising).
  - (c) **Reactions:** Two adjacent voxels producing one or more byproduct voxels (e.g., fire + wood → ash + smoke).
  - (d) **Field propagation:** Temperature, pressure, and electrical charge spreading across neighboring voxels per tick.

4.3.3. Interaction rules MUST be data-driven. Rules SHALL be defined in a declarative format (e.g., JSON, RON, or TOML) and loaded at startup. Rules MUST NOT be hard-coded in shader source.

4.3.4. The rule engine MUST support conditional logic based on: temperature thresholds, pressure thresholds, neighbor material types, neighbor counts (density checks), random probability (for stochastic behaviors), and the voxel's current flags/metadata.

4.3.5. The interaction rule set MUST support a minimum of 500 distinct material types.

4.3.6. The interaction matrix (material A + material B → outcome) MUST support at least 10,000 explicitly defined pairwise rules. Undefined pairs SHOULD default to no interaction.

4.3.7. The rule evaluation order MUST be deterministic for a given simulation state. The simulation MUST produce identical results given identical initial conditions and identical rule sets (reproducibility requirement).

### 4.4 Physics Subsystems

#### 4.4.1 Gravity and Movement

4.4.1.1. The simulation MUST implement gravity as a per-tick downward velocity applied to non-static voxels, scaled by material density.

4.4.1.2. Liquids MUST flow laterally to equalize pressure when vertical movement is blocked.

4.4.1.3. Gases MUST rise based on density differential with surrounding materials and MUST diffuse laterally over time.

4.4.1.4. Granular materials (sand, powder) MUST exhibit avalanche behavior: they SHALL slide diagonally downward when not supported laterally, with a configurable angle of repose per material.

#### 4.4.2 Thermal System

4.4.2.1. The simulation MUST implement heat conduction between adjacent voxels. Heat transfer rate MUST vary by material (thermal conductivity property).

4.4.2.2. Materials MUST define temperature thresholds for state transitions (e.g., melting point, boiling point, ignition point).

4.4.2.3. Heat SHOULD dissipate over time in the absence of a heat source (entropy). The dissipation rate is RECOMMENDED to be configurable globally.

4.4.2.4. Convection SHOULD be approximated by biasing gas and liquid movement upward when heated and downward when cooled.

#### 4.4.3 Pressure System

4.4.3.1. The simulation MUST track pressure as a per-voxel scalar.

4.4.3.2. Enclosed volumes of gas or liquid MUST accumulate pressure when additional material is added or when heated.

4.4.3.3. When pressure exceeds a material's structural integrity threshold, the containing voxels MUST fracture or be displaced (explosion/rupture behavior).

4.4.3.4. Pressure waves (blast propagation) SHOULD propagate outward from a rupture point at a configurable speed, applying force to surrounding voxels.

#### 4.4.4 Structural Integrity

4.4.4.1. Solid materials MUST support a bonding system: adjacent solid voxels of compatible types SHALL form structural bonds.

4.4.4.2. Structural bonds MUST have a tensile strength property. When gravitational or pressure forces on a connected group exceed aggregate bond strength, the structure MUST fracture.

4.4.4.3. Bond strength SHOULD be reduced by temperature (thermal weakening) and by chemical reactions (corrosion).

4.4.4.4. The structural solver MUST NOT require full rigid-body physics. A simplified connected-component analysis with aggregate force thresholds is RECOMMENDED.

#### 4.4.5 Electrical System (IMPLEMENTED)

4.4.5.1. The simulation MAY implement an electrical conductivity property per material.

4.4.5.2. If implemented, electrical charge MUST propagate through conductive materials at a rate proportional to conductivity.

4.4.5.3. If implemented, the system MUST support at minimum: signal propagation, resistance-based attenuation, and short-circuit / overload heating.

---

## 5. Material System

### 5.1 Material Properties

5.1.1. Each material type MUST define the following base properties:
  - (a) **Phase:** solid, liquid, gas, powder, or plasma.
  - (b) **Density:** Determines gravity behavior and displacement priority.
  - (c) **Thermal conductivity:** Rate of heat transfer.
  - (d) **Melting point, boiling point, ignition point:** Temperature thresholds for state transitions.
  - (e) **Structural integrity:** Bond strength for solids; viscosity for liquids.
  - (f) **Flammability:** Whether and how the material combusts.
  - (g) **Color and emission:** Base visual appearance and whether the voxel emits light.

5.1.2. Materials SHOULD additionally support OPTIONAL properties: electrical conductivity, toxicity, radioactivity (decay rate), magnetism, solubility, and opacity.

5.1.3. All material definitions MUST be stored in external data files, not compiled into the engine binary.

### 5.2 Material Categories

5.2.1. The game MUST ship with a minimum of 500 distinct material types.

5.2.2. Materials MUST be organized into the following categories at minimum: naturals (stone, water, sand, soil, air, etc.), metals (iron, copper, gold, alloys, etc.), organics (wood, plant matter, oils, biological tissue), energy (fire, plasma, electrical arc, radiation), synthetics (polymers, ceramics, composites), and exotic/fictional (materials with properties not found in nature, for gameplay purposes).

5.2.3. The game SHOULD include at least 50 materials in each category.

### 5.3 Logic and Signal Materials (OPTIONAL)

5.3.1. The material set MAY include materials that function as discrete logic components: signal conductors, transistor-analog materials (output depends on two inputs), and toggle/latch materials that retain state.

5.3.2. If implemented, these materials MUST operate purely through the existing cellular automata rules. They MUST NOT use a separate logic simulation system.

---

## 6. Rendering

### 6.1 Visual Pipeline

6.1.1. The renderer MUST use the WebGPU API for all GPU operations (both compute and rendering).

6.1.2. Voxels MUST be rendered using a GPU-accelerated technique suitable for large voxel counts. Ray marching, ray casting against the sparse octree, or GPU-instanced cube rendering are RECOMMENDED. Traditional polygon mesh generation per-voxel MUST NOT be used.

6.1.3. The renderer MUST support dynamic lighting from voxel light sources (e.g., fire, lava, glowing materials). A minimum of 64 simultaneous dynamic light sources MUST be supported.

6.1.4. The renderer SHOULD support ambient occlusion (screen-space or voxel-based).

6.1.5. The renderer SHOULD support a global illumination approximation (e.g., voxel cone tracing or light propagation volumes). Full path tracing is OPTIONAL.

6.1.6. The renderer MUST support transparent and semi-transparent materials (water, glass, gas) with correct depth sorting or order-independent transparency.

### 6.2 Camera and Viewport

6.2.1. The game MUST support a free-orbit 3D camera with zoom, pan, and rotation.

6.2.2. The game MUST support a cross-section/slice view that reveals the interior of the simulation volume along any axis.

6.2.3. The game SHOULD support a first-person camera mode for exploring the simulation from within.

---

## 7. Player Interaction

### 7.1 Core Tools

7.1.1. The player MUST be able to place voxels of any unlocked material type into the world.

7.1.2. The player MUST be able to remove (destroy) voxels from the world.

7.1.3. The player MUST be able to select a brush size for placement and removal. Minimum supported brush shapes: single voxel, cube, and sphere. A cylinder brush is RECOMMENDED.

7.1.4. The player MUST be able to pause, resume, and single-step the simulation.

7.1.5. The player MUST be able to adjust the simulation tick rate (slow motion).

7.1.6. The player SHOULD be able to "paint" temperature changes onto existing voxels (heat gun / freeze tool).

7.1.7. The player SHOULD be able to apply directional force to voxels (wind / push tool).

### 7.2 Material Browser

7.2.1. The game MUST provide a searchable, categorized material browser.

7.2.2. Each material MUST display its name, category, phase, and a brief description of its key behaviors.

7.2.3. The material browser SHOULD display a live preview of the selected material's behavior (e.g., a small simulation vignette).

### 7.3 Save and Load

7.3.1. The player MUST be able to save the complete simulation state to a file.

7.3.2. The player MUST be able to load a previously saved simulation state.

7.3.3. Save files MUST include: all voxel data, the full rule set version/hash, simulation tick count, and camera state.

7.3.4. Save files SHOULD use a compressed binary format. Uncompressed save size MUST NOT exceed 500 MB for a maximally populated world.

7.3.5. The game SHOULD support auto-save at a configurable interval.

### 7.4 Sharing

7.4.1. The game SHOULD support exporting a save file or a defined subregion of the world as a shareable file.

7.4.2. The game MAY support a community gallery or sharing platform. This is OPTIONAL for initial release.

---

## 8. User Interface

8.1.1. The UI MUST NOT obstruct the simulation viewport. Controls MUST be collapsible or positioned along screen edges.

8.1.2. The UI MUST display current FPS, active voxel count, and simulation tick rate.

8.1.3. The UI MUST provide keyboard shortcuts for all core tools (place, remove, pause, step, camera modes).

8.1.4. The UI SHOULD display a real-time info panel when hovering over a voxel, showing: material name, temperature, pressure, and velocity.

8.1.5. The UI MUST be usable with mouse and keyboard. Gamepad support is OPTIONAL. Touch input is OPTIONAL.

---

## 9. Audio (OPTIONAL)

9.1.1. The game MAY implement procedural audio driven by simulation state (e.g., crackling from fire voxels, hissing from steam, rumbling from structural collapse).

9.1.2. If implemented, audio SHOULD be spatialized relative to the camera position.

9.1.3. Audio MUST NOT be a blocking dependency for any milestone. It MAY be deferred to post-launch.

---

## 10. Modding and Extensibility

10.1.1. Because interaction rules are data-driven (see 4.3.3), the game MUST support loading custom material definitions and rule sets from user-provided data files.

10.1.2. The game SHOULD provide documentation for the material and rule schema sufficient for community modders to create new materials.

10.1.3. Custom materials and rules MUST be validated on load. Malformed or conflicting rules MUST be rejected with a clear error message and MUST NOT crash the application.

10.1.4. The game SHOULD support loading multiple mod packs simultaneously, with a defined load order for conflict resolution.

---

## 11. AI-Assisted Development Process

This section documents requirements for the development workflow, not the runtime application.

11.1.1. Material interaction rules SHOULD be initially generated using LLM assistance, then reviewed, balanced, and approved by a human designer before inclusion.

11.1.2. Compute shader code MAY be drafted using LLM assistance but MUST pass automated correctness tests and performance benchmarks before integration.

11.1.3. All AI-generated code and data MUST be committed to version control with the same review standards as human-authored code. The origin (AI-assisted vs. human-authored) SHOULD be noted in commit messages.

11.1.4. A test harness MUST exist that can load a rule set, run a deterministic simulation for N ticks, and compare the output against a known-good snapshot (regression testing for rule changes).

11.1.5. A profiling pipeline MUST exist that benchmarks shader performance per commit. Performance regressions exceeding 10% MUST block merge.

---

## 12. Non-Functional Requirements

### 12.1 Code Quality

12.1.1. The Rust codebase MUST compile with zero warnings under `#[deny(warnings)]`.

12.1.2. All public APIs MUST include documentation comments.

12.1.3. The project MUST include a CI pipeline that runs linting (`clippy`), formatting (`rustfmt`), unit tests, and integration tests on every pull request.

### 12.2 Licensing

12.2.1. All third-party dependencies MUST be compatible with the project's chosen license.

12.2.2. The project SHOULD prefer permissively licensed dependencies (MIT, Apache 2.0, BSD).

### 12.3 Accessibility

12.3.1. The game SHOULD support colorblind-friendly rendering modes (configurable palette or material shape indicators).

12.3.2. The UI SHOULD support text scaling.

12.3.3. The game MUST NOT rely solely on color to convey critical gameplay information.

---

## Appendix A: Glossary

- **Active voxel:** A voxel that is not empty and not in a fully static resting state; requires simulation each tick.
- **Chunk:** A fixed-size cubic subdivision of the world grid, used as the unit of memory allocation and GPU dispatch.
- **Interaction rule:** A data-driven definition of what occurs when two specific material types are adjacent under specified conditions.
- **Moore neighborhood (3D):** The 26 cells surrounding a central cell in a 3D grid (6 face-adjacent + 12 edge-adjacent + 8 corner-adjacent).
- **SoA (Struct of Arrays):** A data layout where each property is stored in a separate contiguous array, improving GPU cache performance.
- **Sparse voxel octree:** A hierarchical spatial data structure that only allocates memory for occupied regions of the world.
- **Tick:** A single discrete simulation step in which all active voxels evaluate their rules and update state.
- **WGSL:** WebGPU Shading Language, the shader language used by the WebGPU API.

---

## Appendix B: Reference Games

- **Noita** (Nolla Games, 2019): 2D falling sand with ~50 materials. Benchmark for emergent material interactions in gameplay.
- **Powder Toy** (Community, 2008–present): 2D particle simulator with ~180 elements including electrical and logic components. Benchmark for element count and community modding.
- **Teardown** (Tuxedo Labs, 2022): Voxel-based 3D destructible environments with ray-traced rendering. Benchmark for 3D voxel rendering quality.
- **Minecraft** (Mojang, 2011): Block-based 3D sandbox. Benchmark for world scale, modding ecosystem, and player creative freedom.

---

## Appendix C: Requirement Fulfillment Matrix

| Requirement | Description | Milestone |
|-------------|-------------|-----------|
| 3.1.1 | WebGPU browser target | M0 |
| 3.1.2 | Rust/WASM compilation | M0 |
| 3.1.3 | Chrome, Edge, Firefox support | M0 |
| 3.1.4 | No browser plugins | M0 |
| 3.2.1 | 1M voxels at 60 FPS (RTX 4060) | M11 |
| 3.2.2 | 3M voxels at 60 FPS (RTX 4090) | M11 |
| 3.2.3 | Graceful degradation | M11 |
| 3.2.4 | GPU compute simulation | M2 |
| 3.2.5 | Memory under 4 GB | M11 |
| 3.2.6 | Payload under 50 MB | M11 |
| 4.1.1 | Voxel grid | M1 |
| 4.1.2 | Voxel state fields | M1 |
| 4.1.3 | GPU cache-coherent layout | M1 |
| 4.1.4 | Double buffering | M2 |
| 4.2.1 | Chunked sparse structure | M5 |
| 4.2.2 | Fixed-size chunks (32x32x32) | M5 |
| 4.2.3 | Skip static/empty chunks | M5 |
| 4.2.4 | Dynamic chunk loading | M5 |
| 4.2.5 | 1024x512x1024 world volume | M5 |
| 4.3.1 | 26-neighbor evaluation | M2 |
| 4.3.2a | State transitions | M3 |
| 4.3.2b | Movement (density-driven) | M2, M3 |
| 4.3.2c | Reactions (byproducts) | M3 |
| 4.3.2d | Field propagation | M4, M15 |
| 4.3.3 | Data-driven rules (RON) | M3 |
| 4.3.4 | Conditional rule logic | M3 |
| 4.3.5 | 500+ material types | M14 |
| 4.3.6 | 10,000+ pairwise rules | M14 |
| 4.3.7 | Deterministic simulation | M2 |
| 4.4.1 | Gravity and movement | M2, M3 |
| 4.4.2 | Thermal system | M4 |
| 4.4.3 | Pressure system | M6 |
| 4.4.4 | Structural integrity | M6 |
| 4.4.5 | Electrical system | M15 |
| 5.1.1 | Material base properties | M3 |
| 5.1.2 | Optional material properties | M9, M15 |
| 5.1.3 | External data files | M3 |
| 5.2.1 | 500+ material types | M14 |
| 5.2.2 | Material categories | M9, M14 |
| 5.2.3 | 50+ per category | M14 |
| 5.3.1-2 | Logic/signal materials | M15 |
| 6.1.1 | WebGPU rendering | M1 |
| 6.1.2 | Ray marching | M1 |
| 6.1.3 | 64 dynamic lights | M10 |
| 6.1.4 | Ambient occlusion | M10 |
| 6.1.6 | Transparency | M10 |
| 6.2.1 | Free-orbit camera | M1 |
| 6.2.2 | Cross-section view | M7 |
| 6.2.3 | First-person camera | M7 |
| 7.1.1-7 | Player tools | M7 |
| 7.2.1-3 | Material browser | M7 |
| 7.3.1-5 | Save/load | M8 |
| 7.4.1 | Subregion export | M8 |
| 8.1.1-5 | UI requirements | M7 |
| 9.1.1-3 | Audio | M13 |
| 10.1.1-4 | Modding support | M12 |
| 11.1.1-5 | AI-assisted development | M9, M14 |
| 12.1.1-3 | Code quality / CI | M0 |
