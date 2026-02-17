# Alkahest Modding Guide

## Overview

Alkahest supports loading external material and rule packs (mods) that extend the base game with new materials and interactions. Mods are defined as directories containing a manifest file and RON data files.

## Mod Format

A mod is a directory with this structure:

```
my-mod/
  mod.ron                         # Manifest (required)
  materials/
    my_materials.ron              # Material definitions (one or more files)
  rules/
    my_rules.ron                  # Interaction rules (one or more files)
```

### Manifest (mod.ron)

Every mod must have a `mod.ron` file at its root:

```ron
(
    name: "My Mod",
    version: "1.0.0",
    author: "Your Name",
    description: "A short description of what this mod adds",
    load_order_hint: 100,
)
```

| Field | Type | Description |
|-------|------|-------------|
| `name` | String | Human-readable mod name |
| `version` | String | Semantic version (e.g., "1.0.0") |
| `author` | String | Mod author |
| `description` | String | Brief description |
| `load_order_hint` | u32 | Lower values load first. Base game is implicitly 0 |

## Material Definition Schema

Materials are defined in RON files as arrays of material structs:

```ron
[
    (
        id: 10001,                          // u16, MUST be >= 10000 for mods
        name: "My Material",                // String
        phase: Solid,                       // Gas, Liquid, Solid, or Powder
        density: 3000.0,                    // f32, abstract units
        color: (0.5, 0.5, 1.0),           // (f32, f32, f32) RGB 0.0-1.0
        emission: 0.5,                      // f32, 0.0 = none, 5.0 = bright glow
        flammability: 0.0,                  // f32, 0.0-1.0
        ignition_temp: 0.0,                 // f32, Kelvin (max 8000)
        decay_rate: 0,                      // u32, per-tick temp decrement
        decay_threshold: 0,                 // u32, temp below which decay occurs
        decay_product: 0,                   // u16, material ID to decay into
        viscosity: 0.0,                     // f32, 0.0 = free flow, 1.0 = no flow
        thermal_conductivity: 0.3,          // f32, 0.0-1.0
        phase_change_temp: 0.0,             // f32, Kelvin (0 = no phase change)
        phase_change_product: 0,            // u16, material ID after phase change
        structural_integrity: 30.0,         // f32, 0.0-63.0
        opacity: Some(0.5),                 // Option<f32>, None = derive from phase
        absorption_rate: 0.0,               // f32, depth-dependent darkening
    ),
]
```

**Required fields:** `id`, `name`, `phase`, `density`, `color`

All other fields have defaults (0, 0.0, or None) via `#[serde(default)]`.

### Property Constraints

| Property | Range | Notes |
|----------|-------|-------|
| `id` | >= 10000 | Mod IDs must be in the mod range |
| `ignition_temp` | 0-8000 K | Exceeding 8000 is rejected |
| `thermal_conductivity` | 0.0-1.0 | Must satisfy CFL stability |
| `structural_integrity` | 0.0-63.0 | 6-bit quantized |
| `decay_threshold` | 0-4095 | 12-bit quantized |

## Rule Definition Schema

Rules define pairwise interactions between materials:

```ron
[
    (
        name: "Fire+MyCrystal melting",     // String, human-readable
        input_a: 5,                          // u16, first input material ID
        input_b: 10001,                      // u16, second input material ID
        output_a: 5,                         // u16, what input_a becomes
        output_b: 10020,                     // u16, what input_b becomes
        probability: 0.3,                    // f32, 0.0-1.0 per tick
        temp_delta: 100,                     // i32, temperature change (quantized)
        min_temp: 800,                       // u32, minimum temp for reaction (0 = any)
        max_temp: 0,                         // u32, maximum temp for reaction (0 = any)
        pressure_delta: 0,                   // i32, pressure change
        min_charge: 0,                       // u32, minimum charge for reaction (0 = any)
        max_charge: 0,                       // u32, maximum charge for reaction (0 = any)
    ),
]
```

**Required fields:** `name`, `input_a`, `input_b`, `output_a`, `output_b`, `probability`

### Electrical Rule Fields

The `min_charge` and `max_charge` fields gate reactions on electrical charge level. This enables charge-dependent behavior:

- `min_charge: 100` — reaction only fires when the voxel's charge is >= 100 (e.g., overload/short-circuit rules)
- `max_charge: 10` — reaction only fires when charge is <= 10 (e.g., Toggle-ite deactivation when power is removed)
- Both set to 0 (default) — reaction is not charge-gated

### Rule Semantics

- When voxel A (`input_a`) is adjacent to voxel B (`input_b`), A becomes `output_a` and B becomes `output_b`
- The compiler creates bidirectional GPU entries automatically
- `temp_delta > 0` with no material transform is rejected (energy conservation)
- Overlapping A<->B cycles with overlapping temp ranges are rejected (infinite loops)
- Rules reference material IDs. You can reference both base game IDs (0-559) and your mod IDs (10000+)

## ID Allocation

**Mod materials MUST use IDs >= 10000.** IDs below 10000 are reserved for the base game (currently 0-559, with 561 base materials).

The mod loader automatically remaps your IDs from the 10000+ range to contiguous internal IDs (starting after the base game's max ID, currently ~560). This keeps the GPU lookup table compact. The mapping is:

- **External IDs** (in your RON files): 10000+ — stable, used for save compatibility
- **Internal IDs** (at runtime): contiguous after base — used for GPU lookup table

You do not need to worry about internal IDs. Always use 10000+ in your data files, and cross-references between your mod's materials will be remapped automatically.

## Conflict Resolution

When multiple mods define rules for the same `(input_a, input_b)` pair:

- **Last-loaded wins:** The mod loaded later overrides the earlier rule
- **Warnings are logged:** Each conflict generates a warning like:
  `Mod 'Crystal Pack': rule 'Fire+Quartz heating' overrides base rule 'Fire+Quartz base'`
- **Load order** is determined by `load_order_hint` in the manifest. Lower values load first

Material ID conflicts between mods are prevented by the remapping system — each mod's IDs are independently remapped to non-overlapping internal ranges.

## Tutorial: Creating a Simple Mod

### 1. Create the directory structure

```
my-first-mod/
  mod.ron
  materials/
    my_materials.ron
  rules/
    my_rules.ron
```

### 2. Write the manifest

```ron
// mod.ron
(
    name: "My First Mod",
    version: "0.1.0",
    author: "Your Name",
    description: "Adds a glowing crystal material",
    load_order_hint: 100,
)
```

### 3. Define a material

```ron
// materials/my_materials.ron
[
    (
        id: 10001,
        name: "Glow Crystal",
        phase: Solid,
        density: 3000.0,
        color: (0.2, 0.8, 1.0),
        emission: 2.0,
        thermal_conductivity: 0.4,
        structural_integrity: 40.0,
        opacity: Some(0.4),
    ),
]
```

### 4. Define an interaction rule

```ron
// rules/my_rules.ron
[
    // Lava melts Glow Crystal into air (simple destruction)
    (
        name: "Lava+Glow Crystal melting",
        input_a: 11,          // Lava (base game ID)
        input_b: 10001,       // Glow Crystal (our mod ID)
        output_a: 11,         // Lava remains
        output_b: 0,          // Crystal becomes Air
        probability: 0.5,
        temp_delta: 50,
        min_temp: 800,
    ),
]
```

### 5. Test

The mod will be validated at load time. Common issues:

- **ID below 10000:** All mod material IDs must be >= 10000
- **Unknown material reference:** A rule references an ID that doesn't exist in base or mod
- **Energy from nothing:** A rule has `temp_delta > 0` but doesn't transform any material
- **Thermal conductivity out of range:** Must be 0.0-1.0
- **Ignition temp too high:** Maximum is 8000K

## Example: Crystal Pack

The included example mod (`data/mods/example-mod/`) demonstrates:

- 25 crystal and gemstone materials (IDs 10001-10025)
- 52 interaction rules covering:
  - Lava melting crystals into Molten Crystal
  - Fire heating crystals (thermal absorption)
  - Molten Crystal cooling into Crystal Dust
  - Acid dissolving crystals
  - Crystal growth from Crystal Dust + Water
  - Cross-crystal interactions (Ruby+Sapphire resonance, Diamond+Ruby fusion)
  - Crystal + base material interactions (sintering, vitrification)

## Base Game Material ID Reference

Common base game IDs for use in mod rules (base game range: 0-559):

| ID | Material | Phase | Category |
|----|----------|-------|----------|
| 0 | Air | Gas | Natural |
| 1 | Stone | Solid | Natural |
| 2 | Sand | Powder | Natural |
| 3 | Water | Liquid | Natural |
| 4 | Oil | Liquid | Natural |
| 5 | Fire | Gas | Energy |
| 6 | Smoke | Gas | Energy |
| 7 | Steam | Gas | Energy |
| 8 | Wood | Solid | Organic |
| 9 | Ash | Powder | Natural |
| 11 | Lava | Liquid | Energy |
| 12 | Gunpowder | Powder | Explosive |
| 13 | Sealed-Metal | Solid | Explosive |
| 14 | Glass | Solid | Synthetic |
| 50 | Iron | Solid | Metal |
| 51 | Copper | Solid | Metal |
| 54 | Aluminum | Solid | Metal |
| 55 | Tin | Solid | Metal |
| 56 | Lead | Solid | Metal |
| 80 | Iron Ore | Solid | Metal |
| 131 | Spark | Gas | Energy |
| 132 | Ember | Gas | Energy |
| 166 | Acid | Liquid | Natural |
| 550 | Copper Wire | Solid | Electrical |
| 551 | Resistor Paste | Solid | Electrical |
| 552 | Insulator Coat | Solid | Electrical |
| 553 | Signal Sand | Powder | Electrical |
| 554 | Toggle-ite Off | Solid | Electrical |
| 555 | Toggle-ite On | Solid | Electrical |
| 556 | Power Source | Solid | Electrical |
| 557 | Ground | Solid | Electrical |
| 558 | LED Crystal | Solid | Electrical |
| 559 | Fuse Wire | Solid | Electrical |
