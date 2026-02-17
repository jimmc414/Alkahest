# Emergent Recipes

These recipes emerge from the interaction rule system. They are not hardcoded — players discover them by combining materials and observing the results.

## 1. Furnace — Smelting Iron from Ore

**Goal:** Convert Iron Ore into usable Iron.

**Materials needed:**
- Firebrick (ID 186) — walls and floor
- Coal (ID 106) or Charcoal (ID 105) — fuel
- Iron Ore (ID 80) — raw material

**Setup:**
1. Build a chamber of Firebrick (3-wide, 3-tall box with open top)
2. Place Coal at the bottom as fuel
3. Place Iron Ore on top of the coal
4. Ignite with Fire (heat tool or place Fire adjacent to Coal)

**What happens:**
- Fire + Coal produces sustained heat (combustion rule: temp_delta +300)
- Firebrick insulates (low thermal_conductivity 0.15), keeping heat in the chamber
- At high temperature (min_temp 500), Iron Ore + Coal triggers smelting (synthesis rule)
- Iron Ore transforms into Iron, with Slag as byproduct
- Alternatively, ore first melts into Molten Iron (phase change), which cools into solid Iron

**Variations:**
- Use Coke instead of Coal for higher temperatures
- Smelt Copper Ore, Gold Ore, or Tin Ore with the same setup
- Mix molten metals to create alloys (Copper + Tin = Bronze)

---

## 2. Water Purification — Sand and Charcoal Filter

**Goal:** Demonstrate filtration using layered materials.

**Materials needed:**
- Sand (ID 2) — filter medium
- Charcoal (ID 105) — activated carbon layer
- Water (ID 3)
- Mud (ID 23) or Brine (ID 34) — "dirty" water source

**Setup:**
1. Build a vertical column (1-wide, 6+ tall)
2. Place a layer of Sand at the bottom (2 voxels thick)
3. Place a layer of Charcoal above the sand (2 voxels thick)
4. Place another layer of Sand on top (2 voxels thick)
5. Pour Mud or Brine from above

**What happens:**
- Mud passing through Sand slowly converts via biological rules (Mud + Sand = Clay over time)
- Brine interacts with charcoal — while full purification isn't modeled, the density-driven movement creates visible separation
- Heavier particulates settle through Sand layers (density displacement)
- The visual effect of material flowing through filter layers demonstrates the simulation's density-based movement

---

## 3. Explosive Device — Pressure Rupture Blast

**Goal:** Create a contained explosion that shatters its container.

**Materials needed:**
- Sealed-Metal (ID 13) — pressure vessel
- Gunpowder (ID 12) — explosive charge
- Fire (ID 5) or Ember (ID 132) — ignition source

**Setup:**
1. Build a sealed box of Sealed-Metal (structural_integrity: 60)
2. Fill the interior entirely with Gunpowder
3. Place a single Fire or Ember voxel adjacent to the Gunpowder

**What happens:**
- Fire + Gunpowder triggers explosion (probability 1.0, pressure_delta +60)
- Gunpowder converts to Air, releasing massive pressure wave
- Chain reaction: each Gunpowder voxel adjacent to Fire explodes in sequence
- Pressure accumulates inside the Sealed-Metal container
- When pressure exceeds structural_integrity (60), the container ruptures
- Sealed-Metal fragments scatter, and the pressure wave propagates outward

**Variations:**
- Use Glass (structural_integrity: 8) for an easier-to-break container
- Mix in Napalm for sustained fire after the blast
- Create chain detonations with multiple chambers connected by Gunpowder fuses

---

## 4. Volcanic Eruption — Lava Meets Water

**Goal:** Simulate a volcanic eruption with steam explosions.

**Materials needed:**
- Lava (ID 11) or Magma (ID 149) — underground heat source
- Stone (ID 1) — rock layer
- Water (ID 3) — ocean/lake above
- Sand (ID 2) — optional terrain

**Setup:**
1. Build a terrain: flat layer of Stone with a column of Sand on top
2. Place a large body of Water above the terrain
3. Create a vertical channel through the Stone
4. Fill the channel from below with Lava or Magma

**What happens:**
- Lava rises through the channel (density 2800, displaces lighter materials above)
- Lava + Stone at high temp: Stone begins to melt (phase_change_temp 1500 → Lava)
- This creates a chain reaction — melting Stone widens the channel
- When Lava reaches the Water layer: explosive steam generation
  - Lava + Water → Stone + Steam (displacement rule, temp_delta: -200)
  - Magma + Water → Volcanic Rock + Steam (with pressure_delta)
- Steam rapidly expands upward (low density gas)
- The contact zone creates a cycle: Lava cools to Stone/Volcanic Rock, new Lava rises
- Sand melts into Glass at the lava contact zone (min_temp 800)

**Variations:**
- Use Magma (denser, hotter) for a more violent eruption
- Add Sulfur near the vent for toxic gas generation
- Place Ice above for a subglacial eruption effect

---

## 5. Alchemical Transmutation — Lead into Gold

**Goal:** Use exotic materials to transmute base metals into precious ones.

**Materials needed:**
- Philosopher's Stone (ID 210) — the catalyst
- Iron (ID 50) or Lead (ID 56) — base metal
- Fire (ID 5) or Lava (ID 11) — heat source

**Setup:**
1. Place a crucible of Firebrick or Dragon Scale (fireproof container)
2. Place the base metal (Iron or Lead) inside
3. Place the Philosopher's Stone adjacent to the metal
4. Heat the arrangement with Fire or Lava (min_temp: 400 required)

**What happens:**
- At sufficient temperature, Philosopher's Stone + Iron/Lead triggers transmutation
- The base metal transforms into Gold (probability 0.08–0.1 per tick)
- The Philosopher's Stone is NOT consumed (output_a remains Philosopher's Stone)
- This means a single Philosopher's Stone can transmute unlimited metal over time
- The low probability makes large-scale transmutation slow but steady

**Variations:**
- Use Transmutation Catalyst (ID 231) instead — works but requires higher temp (min_temp 600) and lower probability
- Philosopher's Mercury (ID 246) works as a liquid catalyst
- Transmutation Catalyst + Bronze → Mythril (exotic alloy, min_temp 700)
- Transmutation Catalyst + Steel → Adamantine (exotic alloy, min_temp 800)
- Transmutation Catalyst + Gold → Orichalcum (exotic alloy, min_temp 900)

---

## 6. Simple Circuit — Power to LED

**Goal:** Build a basic electrical circuit that lights up an LED Crystal.

**Materials needed:**
- Power Source (ID 556) — constant charge emitter
- Copper Wire (ID 550) — conductor
- Resistor Paste (ID 551) — current limiter (optional but recommended)
- LED Crystal (ID 558) — visual indicator
- Ground (ID 557) — charge sink

**Setup:**
1. Place a Power Source voxel
2. Extend a line of Copper Wire from the Power Source (3-5 voxels)
3. Place a Resistor Paste voxel in the middle of the wire
4. Place an LED Crystal after the resistor
5. Continue Copper Wire from the LED to a Ground voxel

**What happens:**
- Power Source emits charge (charge_emission: 255) into adjacent conductors
- Charge propagates through Copper Wire (electrical_conductivity: 0.95)
- Resistor Paste attenuates current and generates heat (electrical_resistance: 0.80)
- LED Crystal receives charge and emits light proportional to charge level (emission: 0.2 base, increases with charge)
- Ground absorbs remaining charge, completing the circuit
- Without the Ground, charge builds up and may trigger overload rules

**Variations:**
- Use Fuse Wire (ID 559) instead of a resistor for a circuit that burns out under excess current
- Add Insulator Coat (ID 552) around the wire to prevent charge leaking to adjacent materials
- Place multiple LED Crystals in parallel for a light display

---

## 7. AND Gate Logic — Signal Sand Circuit

**Goal:** Build a logic gate using Signal Sand that only activates when two inputs are powered.

**Materials needed:**
- Power Source (ID 556) x2 — two independent inputs
- Copper Wire (ID 550) — signal paths
- Signal Sand (ID 553) — AND gate (requires 2+ charged neighbors to conduct)
- LED Crystal (ID 558) — output indicator
- Ground (ID 557) — charge sink
- Insulator Coat (ID 552) — prevent crosstalk

**Setup:**
1. Place two Power Sources separated by 5+ voxels
2. Run a Copper Wire path from each Power Source toward a central point
3. At the junction, place Signal Sand voxels (3-4 in a cluster)
4. Surround the Signal Sand with Insulator Coat on non-connected sides
5. Run Copper Wire from the Signal Sand output to an LED Crystal, then to Ground

**What happens:**
- Signal Sand has activation_threshold: 2, meaning it only conducts when receiving charge from 2+ adjacent charged neighbors
- With both Power Sources active: both wire paths deliver charge to the Signal Sand → Signal Sand conducts → LED lights up
- With only one Power Source active: only one neighbor is charged → Signal Sand does not conduct → LED stays dark
- This implements a physical AND gate using automata rules, not a separate logic system

**Variations:**
- Chain multiple AND gates to build more complex logic
- Use Toggle-ite (ID 554/555) as memory cells to store intermediate results
- Replace one Power Source with a Spark (temporary pulse) for edge-triggered behavior

---

## 8. Short Circuit Explosion — Electrical Overload

**Goal:** Create a destructive short circuit that causes a fire and chain reaction.

**Materials needed:**
- Power Source (ID 556) — charge emitter
- Copper Wire (ID 550) — conductor
- Ground (ID 557) — charge sink
- Wood (ID 8) — combustible surroundings
- Gunpowder (ID 12) — optional amplifier

**Setup:**
1. Build a structure of Wood (walls and floor)
2. Place a Power Source inside
3. Run Copper Wire directly to a Ground voxel with minimal wire length (1-2 voxels)
4. Optionally, surround the short circuit point with Gunpowder

**What happens:**
- The short path between Power Source and Ground creates extreme charge concentration
- "Wire+Ground short circuit" rule fires at min_charge 230: Wire becomes Molten Copper (ID 72) and Spark (ID 131), temp_delta +350
- The heat ignites adjacent Wood via "Wire arcs to Wood" (min_charge 150)
- Sparks scatter and may ignite more Wood or Gunpowder
- If Gunpowder is present, the heat triggers combustion → pressure builds → container ruptures
- Chain reaction: melting wire → sparks → fire → pressure → explosion

**Variations:**
- Use Fuse Wire (ID 559) in the circuit — it melts at lower temperature (phase_change_temp: 600K), acting as a safety cutoff
- Submerge the circuit in Water — "Water shorts Wire" rule converts Wire to Molten Copper and Water to Steam (min_charge 100)
- Build a pressure vessel around the short circuit for a contained electrical explosion

---

## 9. Resistance Heater — Electrical Furnace

**Goal:** Use electrical resistance to generate heat for smelting, without fire.

**Materials needed:**
- Power Source (ID 556) — charge emitter
- Copper Wire (ID 550) — conductor
- Resistor Paste (ID 551) — heating element
- Firebrick (ID 186) — insulated chamber walls
- Iron Ore (ID 80) — material to smelt
- Ground (ID 557) — charge sink

**Setup:**
1. Build a chamber of Firebrick (3-wide, 3-tall box with open top)
2. Line the bottom with Resistor Paste (3+ voxels)
3. Connect Copper Wire from a Power Source to one end of the Resistor Paste
4. Connect the other end of the Resistor Paste via Copper Wire to Ground
5. Place Iron Ore on top of the Resistor Paste

**What happens:**
- Charge flows through the circuit: Power Source → Wire → Resistor Paste → Wire → Ground
- Resistor Paste has high electrical_resistance (0.80), generating significant Joule heating
- The heat from resistance accumulates in the Firebrick-insulated chamber
- At sufficient temperature (min_temp 500), Iron Ore begins smelting via synthesis rules
- "Resistor melts Tin" and "Resistor melts Lead" rules demonstrate electrical heating of metals directly

**Variations:**
- Increase heating by using longer Resistor Paste chains
- Add Water adjacent to the resistor for electrical steam generation ("Resistor boils Water" rule, min_charge 60)
- Smelt lower-melting-point metals first: Tin (ID 55) melts via "Resistor melts Tin" at min_temp 200, Lead (ID 56) at min_temp 250
