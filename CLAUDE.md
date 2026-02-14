# CLAUDE.md

## Project Overview

Alkahest is a browser-based 3D voxel cellular automata sandbox. Rust → WASM + WebGPU. The player interacts with a volumetric simulation of 500+ material types governed by data-driven interaction rules.

## Documentation

All project decisions are documented. Read before coding:

- `docs/requirements.md` — RFC 2119 requirements (the contract)
- `docs/architecture.md` — System design, data layouts, pipeline (the blueprint)
- `docs/milestones.md` — Build order, acceptance criteria (the work queue)
- `docs/project-structure.md` — Crate/module layout per milestone (the map)
- `docs/technical-constraints.md` — Platform gotchas, prohibited patterns (the minefield chart)
- `docs/test-strategy.md` — Test types, per-milestone test plans (the quality gate)
- `docs/agent-prompt.md` — Detailed per-milestone implementation guide

## Current Milestone

Check git log or the most recently modified milestone-tagged code to determine the current milestone. Only work on the current milestone. Do not create files, stubs, traits, or placeholders for future milestones. See `docs/project-structure.md` Section 12 for what NOT to create.

## Critical Rules — Violating These Breaks the Project

1. **Never read and write the same voxel buffer in one pass.** Double-buffer swap every tick. This is non-negotiable (C-SIM-1).
2. **Never hardcode material behavior in shaders.** All material logic is data-driven via the material property buffer and interaction matrix. No `if (material == SAND)` in WGSL (C-DESIGN-1).
3. **Never scaffold future milestones.** No `todo!()`, no `unimplemented!()`, no empty files, no placeholder traits. Every file contains functional, tested code for the current milestone.
4. **Never use `unwrap()` on fallible operations.** Use `expect("descriptive message")` for logically unreachable cases. Use `Result` propagation for runtime-fallible operations (C-RUST-5).
5. **Never allocate GPU resources per frame.** Buffers, pipelines, bind groups, and textures are created at init or load time and reused (C-PERF-2).
6. **Never iterate over individual voxels on the CPU at runtime.** All per-voxel work runs in GPU compute shaders. The CPU operates on chunks, not voxels (C-PERF-1).

## Build Commands

```bash
# Build WASM
wasm-pack build --release crates/alkahest-web --target web

# Lint
cargo clippy --workspace -- -D warnings

# Format check
cargo fmt --all -- --check

# Unit tests (CPU only, fast)
cargo test --workspace

# GPU tests (requires GPU, slower)
cargo test --workspace --features gpu_tests

# Full CI
ci/build.sh && ci/test.sh
```

## Workspace Structure

```
crates/
  alkahest-core/     Shared types, constants, math. No GPU deps. No wgpu.
  alkahest-web/      WASM entry point, browser glue, frame loop, UI, tools.
  alkahest-render/   Rendering pipeline. Owns render shaders and bind groups.
  alkahest-sim/      Simulation pipeline. Owns compute shaders and double buffer.
  alkahest-rules/    Rule engine. Parses RON, validates, compiles to GPU buffers.
  alkahest-world/    Multi-chunk world management. Chunk state machine, dispatch.
  alkahest-persist/  Save/load serialization. Runs on Web Worker, no wgpu refs.
  alkahest-audio/    Audio system (optional). Reads world state, writes no sim state.
  alkahest-bench/    Benchmarks. Native only, not shipped.
shaders/
  common/            Shared WGSL (types, coords, rng). Concatenated into other shaders.
  render/            Ray march, lighting, debug lines.
  sim/               Compute shaders for each simulation pass.
data/
  materials/         Material definitions (RON).
  rules/             Interaction rules (RON).
```

Crate dependency flows downward: `alkahest-web` → everything else → `alkahest-core`. No circular deps. See `docs/project-structure.md` Section 3.

## Crate API Rules

- **One primary public struct per crate** (`Renderer`, `SimPipeline`, `World`, etc.). Helper return types may also be pub. No ambient public functions.
- **GPU resources stay inside their owning crate.** Render crate owns render pipelines. Sim crate owns compute pipelines. They share `Buffer` handles across crate boundaries but never touch each other's pipelines.
- **alkahest-core has zero wgpu dependency.** Pure data types and math only.
- **alkahest-web orchestrates.** It calls into other crates in the correct order. Other crates do not call each other directly.
- **Shaders are not API.** Shader files are implementation details of the crate that embeds them. Shared WGSL lives in `shaders/common/` and is concatenated by the build script.

## Voxel Data Layout (8 bytes)

```
Material ID:  16 bits (u16)    — index into material table, 0 = air
Temperature:  12 bits (u12)    — quantized 0–8000 K at ~2 K resolution
Velocity X:    8 bits (i8)     — voxels/tick, signed
Velocity Y:    8 bits (i8)
Velocity Z:    8 bits (i8)
Pressure:      6 bits (u6)     — 0–63 abstract units
Flags:         6 bits (u6)     — bit 0: active, bit 1: updated, bit 2: bonded
Total:        64 bits = 8 bytes
```

Packed into two u32 values on the GPU. Pack/unpack functions must exist in both Rust (`alkahest-core/math.rs`) and WGSL (`shaders/common/types.wgsl`) and must produce identical results. Temperature threshold comparisons happen in integer space, not float (C-GPU-11).

## Simulation Pass Order

Every tick executes in this exact order. Do not reorder.

1. **Commands** — Apply player tool actions from the command buffer
2. **Movement** — Gravity, density displacement, liquid flow, gas rise (directional sub-passes with checkerboard conflict resolution)
3. **Reactions** — Interaction matrix evaluation, byproduct spawning, state transitions
4. **Thermal** — Heat diffusion, entropy drain, convection bias
5. **Pressure** — Pressure accumulation, diffusion, rupture detection
6. **Activity Scan** — Per-chunk dirty flag for chunk sleep/wake

Each pass reads from the current buffer and writes to the next buffer. Swap after all passes complete.

## Shader Conventions

- Every shader file starts with a comment: what pass it implements, what buffers it reads/writes, workgroup size and rationale.
- Every loop has a bounded max iteration count. No `while(true)`.
- `workgroupBarrier()` must be reached by every thread in the workgroup. Never place inside conditional branches. Never early-return before a barrier (C-WGSL-4, C-WGSL-5).
- Use i32 for coordinate math, convert to u32 only for final buffer index after bounds checking (C-WGSL-6).
- No recursion (C-GPU-6). Use iterative loops with fixed-size manual stacks.
- No storage buffer pointers as function parameters (C-WGSL-1). Access storage globals directly, copy to local vars for helper functions.
- Shared constants (CHUNK_SIZE, workgroup dims, quantization ranges) are injected by the build script from `alkahest-core/constants.rs`. Never duplicate constants between Rust and WGSL.

## Testing

- Every milestone's acceptance criteria must map to at least one automated test.
- Deterministic snapshot tests are the primary correctness mechanism. Same initial state + same tick count = byte-identical output, every run.
- Snapshot regeneration requires a justification comment. No bulk "update all snapshots."
- No `sleep()` or time-based waits in tests. Use explicit tick counts and synchronous readback.
- Test names describe what they verify: `test_fire_wood_combustion`, not `test_3`.
- Any test failure blocks merge. No "known flaky" exceptions.
- Performance regression >10% blocks merge (update baseline with justification if intentional).

## Git Conventions

- Prefix commits with milestone: `M2: implement double-buffer management`
- One coherent change per commit. Not one monolithic commit per milestone.
- AI-assisted code noted in commit messages: `M9: add metal material definitions (AI-generated, human-reviewed)`
- Snapshot regeneration commits explain why: `M4: regenerate thermal tests — diffusion rate tuned from 0.08 to 0.06`

## Common Mistakes to Avoid

- **Creating files for future milestones.** If it's not in the current milestone's column in `docs/project-structure.md` Section 8, don't create it.
- **Using `std::time` in WASM.** Use `performance.now()` via web-sys, wrapped in a utility function.
- **Forgetting cross-origin isolation headers.** SharedArrayBuffer requires `Cross-Origin-Opener-Policy: same-origin` and `Cross-Origin-Embedder-Policy: require-corp` on the server.
- **Blocking the frame loop on `mapAsync`.** GPU readback is async. Fire on frame N, process results on frame N+1 or N+2. Never await in the render loop.
- **Writing movement logic that checks material IDs instead of density.** Movement is density-driven. The shader reads density from the material property buffer and compares.
- **Putting temperature comparisons in float space.** Threshold comparisons are integer comparisons against pre-quantized values in the rule buffer.
- **Tight coupling between simulation passes.** Passes communicate only through the voxel state buffer. No side-channel buffers between passes (C-DESIGN-2).
- **Premature abstraction.** No traits with one implementation. No generics that aren't generic over anything yet. Concrete code first, abstractions when a second implementation demands them (C-DESIGN-4).

## When in Doubt

Read `docs/technical-constraints.md` Appendix A for the current milestone's constraint list. Read the relevant sections of `docs/architecture.md`. If a design question isn't answered by the documentation, flag it rather than guessing — a wrong assumption in the simulation pipeline propagates to every later milestone.
