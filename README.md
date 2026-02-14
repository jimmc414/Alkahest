# Alkahest

**A 3D voxel sandbox where everything reacts.**

Build a stone furnace. Fill it with ore and fuel. Seal it shut. Watch pressure build until the walls crack — or the metal melts first, depending on what you built the walls from.

Pour water down a mountainside and watch it carve channels through sand, pool in cavities, and flash to steam when it hits the lava below. Set fire to a wooden bridge and watch the supports weaken, buckle, and collapse under the weight of the stone they were holding.

Alkahest simulates over 500 materials in a fully destructible 3D world — not with scripted animations, but with real physics running on every voxel. Heat conducts. Pressure accumulates. Structures bear load. Chemistry cascades. Every material has properties. Every interaction has consequences. Every experiment teaches you something new.

There are no quests. No objectives. No tutorials that hold your hand. There's a world made of matter, and the matter follows rules, and the rules compose in ways nobody — including us — has fully explored.

What happens when you pour acid into a pressurized glass chamber full of gunpowder?

We don't know either. That's the point.

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

## Status

Under construction. See [milestones.md](docs/milestones.md) for current progress.

## License

TBD
