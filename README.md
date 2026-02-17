# Alkahest

**A 3D voxel sandbox where everything reacts.**

Build a stone furnace. Fill it with ore and fuel. Seal it shut. Watch pressure build until the walls crack or the metal melts first, depending on what you built the walls from.

Pour water down a mountainside and watch it carve channels through sand, pool in cavities, and flash to steam when it hits the lava below. Set fire to a wooden bridge and watch the supports weaken, buckle, and collapse under the weight of the stone they were holding.

Alkahest simulates over 500 materials in a fully destructible 3D world, not with scripted animations, but with real physics running on every voxel. Heat conducts. Pressure accumulates. Structures bear load. Chemistry cascades. Every material has properties. Every interaction has consequences. Every experiment teaches you something new.

There are no quests. No objectives. No tutorials that hold your hand. There's a world made of matter, and the matter follows rules, and the rules compose in ways nobody has fully explored.

What happens when you pour acid into a pressurized glass chamber full of gunpowder?

Who knows? That's the point.

---

## What This Is

A browser-based (WebGPU) 3D sandbox where players interact with a simulation of 500+ materials with physically-grounded interactions — building, experimenting, and discovering emergent chemistry in a destructible 3D world. The closest comparisons are **Noita**, **Powder Toy**, **Teardown**, and **Minecraft**, but nothing quite like this exists yet.

The simulation runs entirely on the GPU via compute shaders. Every voxel is 8 bytes. Every interaction is data-driven. Every frame, millions of voxels evaluate their neighbors and follow the rules of a hand-tuned (and AI-assisted) interaction matrix spanning 10,000+ pairwise rules.

## Tech Stack

| Component | Technology |
|---|---|
| Language | Rust → WebAssembly |
| GPU API | WebGPU (via wgpu) |
| Shaders | WGSL |
| Data format | RON |
| UI | egui |
| Target | Modern browsers (Chrome, Firefox, Edge) |

## Features

- **561 materials** across 8 categories (naturals, metals, organics, energy, synthetics, exotic, explosives, electrical) with **11,995 interaction rules**
- **7-pass GPU simulation pipeline:** Commands, Movement, Reactions, Thermal, Electrical, Pressure, Activity Scan — all running in compute shaders with double-buffered state
- **Electrical system:** Charge propagation, resistance heating, logic gates (Signal Sand AND gate, Toggle-ite memory latch), short-circuit cascades
- **Rendering:** Ray-marched voxels with ambient occlusion, volumetric transparency, LOD for distant chunks, sky dome, and 64 simultaneous dynamic lights with shadow rays
- **Procedural audio:** Spatialized sound for fire, water, steam, explosions, and structural collapse driven by simulation state
- **Save/load:** LZ4-compressed binary format with auto-save, subregion export, and Web Worker serialization
- **Modding:** Load custom material and rule packs from external RON files with validation, conflict resolution, and multi-mod support
- **Full sandbox tools:** Brush shapes (single/cube/sphere), material browser, heat/freeze tools, wind gun, cross-section view, first-person camera, simulation speed control

## Build Instructions

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

## Project Documentation

| Document | Purpose |
|---|---|
| [Requirements](docs/requirements.md) | RFC 2119 specification — what the game must do |
| [Architecture](docs/architecture.md) | System design, data layouts, GPU pipeline |
| [Milestones](docs/milestones.md) | Phased build plan with acceptance criteria |
| [Project Structure](docs/project-structure.md) | Crate and module layout, annotated by milestone |
| [Technical Constraints](docs/technical-constraints.md) | Platform limitations and prohibited patterns |
| [Test Strategy](docs/test-strategy.md) | Testing approach per milestone and subsystem |
| [Agent Prompt](docs/agent-prompt.md) | Implementation guide for AI-assisted development |
| [Modding Guide](docs/modding-guide.md) | How to create custom material and rule packs |
| [Recipes](docs/recipes.md) | Emergent recipes discovered through material interactions |

## Status

All 16 milestones (M0-M15) complete. 561 materials, 11,995 interaction rules, 7-pass simulation pipeline. See [milestones.md](docs/milestones.md) for the full development history.

## License

TBD
