# ALKAHEST: Technical Constraints

**Version:** 1.0.0
**Date:** 2026-02-16
**Status:** Complete
**Companions:** requirements.md, architecture.md, milestones.md, project-structure.md

---

## 1. How to Use This Document

This document lists technical constraints, platform limitations, and prohibited patterns that affect Alkahest development. Each constraint is annotated with the milestone where it first becomes relevant. Constraints are things the agent must know before writing code — they prevent wasted effort on approaches that will fail.

Constraints are organized by category, not by milestone. To find all constraints relevant to a specific milestone, use the milestone index in Appendix A.

Severity levels:

- **HARD:** Violating this constraint produces code that will not compile, will crash at runtime, or will fail silently in ways that are extremely difficult to debug. These are non-negotiable.
- **PERF:** Violating this constraint produces code that works but will not meet performance targets. The code will need to be rewritten.
- **COMPAT:** Violating this constraint produces code that works on some browsers or GPUs but fails on others. The code will need to be rewritten for cross-platform support.
- **DESIGN:** Violating this constraint produces code that works now but creates structural problems for later milestones. The code will need to be refactored.

---

## 2. WebGPU Constraints

### C-GPU-1: No WebGPU on page load [M0] — HARD

WebGPU adapter and device acquisition is asynchronous. The `navigator.gpu.requestAdapter()` and `adapter.requestDevice()` calls return Promises. You cannot issue any GPU commands until both resolve. The application must handle the case where WebGPU is entirely unavailable (the browser doesn't support it or no suitable adapter exists) with a user-visible error message, not a silent failure or console-only error.

### C-GPU-2: Maximum buffer size varies by implementation [M2, critical at M5] — COMPAT

The WebGPU spec guarantees a minimum `maxBufferSize` of 256 MB, but some implementations (particularly on integrated GPUs) may report lower limits. Query `device.limits.maxBufferSize` at startup and use it to calculate the maximum number of chunk slots that fit in a single storage buffer. Do not assume 256 MB is available. At M5, when allocating the chunk pool buffer, this limit determines the pool size.

### C-GPU-3: maxStorageBuffersPerShaderStage is 8 [M2, critical at M5] — HARD

The default WebGPU limit for storage buffers bound to a single shader stage is 8. A simulation compute shader that binds the read buffer, write buffer, material table, rule lookup texture, rule data buffer, command buffer, activity flags buffer, and a neighbor chunk table is already at 8. If you need more, you must either request a higher limit via `requiredLimits` at device creation (not guaranteed to be granted) or pack multiple data structures into a single buffer with offset-based access. Plan bind group layouts carefully — hitting this limit mid-development requires restructuring all shader bindings.

### C-GPU-4: Storage buffer binding alignment is 256 bytes [M5] — HARD

When binding a sub-range of a storage buffer (e.g., binding a specific chunk's data within a large pool buffer), the offset must be a multiple of `minStorageBufferOffsetAlignment`, which is 256 bytes. Since each chunk is 256 KB (a multiple of 256), this is satisfied naturally for chunk-aligned access. But if you attempt to bind at an arbitrary voxel offset within a chunk, it will fail. All dynamic buffer offsets in bind groups must respect this alignment.

### C-GPU-5: Compute shader workgroup size limits [M2] — HARD

`maxComputeWorkgroupSizeX/Y/Z` defaults to 256 per dimension, and `maxComputeInvocationsPerWorkgroup` defaults to 256 total. The architecture specifies 8×8×4 = 256 threads per workgroup, which exactly hits this limit. Do not increase any dimension without checking that the total stays at or below 256 (or requesting higher limits). A workgroup of 8×8×8 = 512 will fail on default-limit devices.

### C-GPU-6: No recursion in WGSL [M1] — HARD

WGSL does not support recursive function calls. The ray march DDA and octree traversal must be implemented as iterative loops, not recursive descent. This also applies to any tree traversal (octree, flood-fill). Use explicit loop-with-stack patterns (a fixed-size array as a manual stack) if tree traversal depth is bounded.

### C-GPU-7: No dynamic indexing into arrays of textures or samplers [M3] — HARD

WGSL does not allow indexing into a binding array of textures with a runtime variable (e.g., `textures[material_id]`). If you plan to use a 2D texture for the interaction lookup matrix, it must be a single texture sampled with computed UV coordinates, not an array of textures indexed by material ID. Alternatively, use a storage buffer with manual indexing, which does support runtime indices.

### C-GPU-8: mapAsync stalls the pipeline if waited on synchronously [M5] — PERF

`GPUBuffer.mapAsync()` returns a Promise. If you `await` it in the render loop, you introduce a GPU pipeline stall (the CPU waits for the GPU to finish all prior work before the map can complete). The activity scan readback must use a multi-frame approach: call `mapAsync()` on frame N, check if the Promise resolved on frame N+1 or N+2, and process the results when available. Never block the frame loop waiting for a map operation.

### C-GPU-9: Shader compilation is synchronous and slow [M0] — PERF

`device.createComputePipeline()` and `device.createRenderPipeline()` are synchronous in most WebGPU implementations and can take 50–500ms per pipeline for complex shaders. Use `createComputePipelineAsync()` and `createRenderPipelineAsync()` (which return Promises) and compile all pipelines during the loading screen. Lazy pipeline creation during gameplay will cause frame hitches.

### C-GPU-10: No printf or console.log from shaders [M2] — HARD

WGSL has no debugging output mechanism. You cannot print values from a compute shader. Debugging simulation shaders requires writing suspect values to a debug buffer, reading them back on the CPU, and logging there. Budget time for building a debug buffer utility at M2 — you will need it throughout the project. A small dedicated storage buffer (e.g., 4 KB) that any shader can write diagnostic values into, read back each frame, is worth the binding slot cost.

### C-GPU-11: Floating-point determinism is not guaranteed across GPUs [M2] — COMPAT

The WebGPU spec does not mandate IEEE 754 compliance for all floating-point operations. Different GPUs may produce different results for the same f16 or f32 computation, particularly for transcendental functions (sin, cos, exp) and fused multiply-add. The simulation's determinism guarantee (REQ 4.3.7) means: avoid floating-point in state-critical logic wherever possible. Use integer and fixed-point arithmetic for movement decisions, reaction triggers, and pressure updates. The 12-bit quantized temperature field is stored as an integer; perform threshold comparisons as integer comparisons, not float comparisons. Temperature diffusion math is the one place where float arithmetic is unavoidable — accept that exact thermal values may differ across GPUs while ensuring the same qualitative behavior (same materials melt/boil at the same thresholds because thresholds are integer comparisons).

---

## 3. WGSL Language Constraints

### C-WGSL-1: No pointers to storage buffers in functions [M2] — HARD

WGSL functions cannot accept pointers to storage or uniform address spaces as parameters (only `function` and `private` address spaces). This means you cannot write a helper function like `fn get_voxel(buf: ptr<storage, array<u32>>, index: u32) -> VoxelData`. Instead, access storage buffers directly via their global binding variables within the function body. This leads to less modular shader code than you'd write in Rust — accept it, do not fight the language.

Workaround: copy data from storage into a local (`var<function>`) variable, pass that by value or pointer-to-function to helpers, then write results back to storage. This is the idiomatic WGSL pattern.

### C-WGSL-2: Array sizes must be compile-time constants [M2] — HARD

`var<function> arr: array<u32, N>` requires `N` to be a constant expression. You cannot create a local array sized by a uniform value. The manual stack for octree traversal (C-GPU-6) must be sized at compile time. Choose a conservative maximum depth (e.g., 16 for a 32³ chunk octree) and hardcode it. If the octree can be deeper at M5 (multi-chunk), increase the constant.

### C-WGSL-3: No bitcast between differently-sized types [M1] — HARD

WGSL `bitcast` only works between types of the same size (e.g., `bitcast<f32>(u32_val)` is valid, `bitcast<f32>(u16_val)` is not). The 8-byte voxel data is packed into two `u32` values. Extracting the 12-bit temperature field requires manual bit shifting and masking, not bitcast. Write explicit pack/unpack functions for the voxel layout and use them consistently across all shaders.

### C-WGSL-4: workgroupBarrier() placement restrictions [M2] — HARD

`workgroupBarrier()` must be called uniformly — every thread in the workgroup must reach the same barrier call. It cannot be placed inside a conditional branch where some threads might skip it. This is relevant for the halo-loading pattern: the load phase (all threads participate) must complete with a barrier before any thread begins the computation phase. Do not put the barrier inside an `if (is_halo_thread)` block.

### C-WGSL-5: No early return before workgroupBarrier [M2] — HARD

Closely related to C-WGSL-4: if any thread in the workgroup does an early `return` before reaching a `workgroupBarrier()`, the barrier will hang (the returned thread will never reach it, and the other threads will wait forever). Structure compute shaders so that all threads reach every barrier. Threads with "nothing to do" (e.g., threads outside the chunk boundary in a non-power-of-2 dispatch) should still reach barriers and simply not write results.

### C-WGSL-6: Integer overflow wraps silently [M2] — HARD

WGSL unsigned integer arithmetic wraps on overflow without error. If the voxel index calculation overflows a u32, you'll read/write the wrong memory location with no error. Validate index calculations, especially at chunk boundaries where coordinates may go negative (use i32 for signed coordinate math, convert to u32 only for the final buffer index after bounds checking).

### C-WGSL-7: No switch fallthrough [M3] — HARD

WGSL `switch` statements do not fall through between cases (unlike C). Each case is an independent block. This is actually safer than C-style switch, but if you're translating logic from C/C++ shader code, be aware that fallthrough patterns must be restructured.

### C-WGSL-8: f16 requires enable directive and may not be available [M1] — COMPAT

Using `f16` in WGSL requires `enable f16;` at the top of the shader and requires the `"shader-f16"` feature to be enabled on the device. Not all GPUs support this. If the voxel layout uses f16 for temperature (architecture.md specifies 12-bit quantized integer instead, but if you're tempted to use native f16), you must feature-detect it at startup. The safer path is to pack temperature as a 12-bit unsigned integer in a u32 and do the quantization math manually, which works on all hardware.

### C-WGSL-9: Subgroup operations are not universally available [M5] — COMPAT

Subgroup operations (`subgroupAdd`, `subgroupBroadcast`, etc.) would be useful for the activity scan reduction (summing dirty flags across a workgroup). They require `enable subgroups;` and the `"subgroups"` feature on the device. Write the activity scan using `workgroupBarrier` and shared memory reduction first (works everywhere). Add a subgroup-optimized path later behind a feature check. Do not write the initial implementation assuming subgroups are available.

---

## 4. Rust / WASM Constraints

### C-RUST-1: WASM is single-threaded by default [M0] — HARD

The main WASM module runs on the browser's main thread. There is no `std::thread` in WASM. Concurrency comes from Web Workers, which are separate WASM instances communicating via `postMessage` or `SharedArrayBuffer`. The simulation pipeline and renderer run on the GPU (not affected by this), but CPU-side work (chunk management, structural flood-fill, save/load) must be carefully partitioned. At M5, the Web Worker for chunk management and at M8, the Web Worker for save/load serialization, are the solutions.

### C-RUST-2: SharedArrayBuffer requires cross-origin isolation [M5] — HARD

Using `SharedArrayBuffer` (needed for zero-copy data sharing between the main thread and Web Workers) requires the page to be served with `Cross-Origin-Opener-Policy: same-origin` and `Cross-Origin-Embedder-Policy: require-corp` HTTP headers. This must be configured on the web server / hosting platform. Without these headers, `SharedArrayBuffer` construction will throw a runtime error. Test this in the deployment environment, not just in localhost dev servers (many dev servers set these headers automatically, masking the problem).

### C-RUST-3: wasm-bindgen closure leaks [M0] — PERF

When passing Rust closures to JavaScript (e.g., for `requestAnimationFrame` callbacks or event listeners), the `Closure` must be kept alive for the duration of its use. If the `Closure` is dropped, the JavaScript callback becomes a dangling pointer. The common pattern is to leak the closure intentionally (`closure.forget()`) or store it in a long-lived struct. Do not create a new `Closure` per frame — create it once during initialization and reuse it.

### C-RUST-4: wgpu version pinning [M0] — COMPAT

The `wgpu` crate's WebGPU backend is under active development. Different versions may have different browser compatibility, bugs, or API changes. Pin the `wgpu` version in `Cargo.toml` (exact version, not semver range) and do not update it without testing on all target browsers. Document the pinned version and the browsers it was tested against. At time of writing, test against wgpu 24.x or later for stable WebGPU backend support, but verify the latest stable release before starting M0.

### C-RUST-5: Panic = abort in WASM [M0] — HARD

Rust panics in WASM default to `abort` (no unwinding). An unwrap() on None or an out-of-bounds array access will crash the entire WASM module with a cryptic JavaScript error. Use `expect()` with descriptive messages instead of `unwrap()`. For GPU operations that can fail (buffer mapping, pipeline creation), use proper error handling with `Result`, not unwrap. Configure `[profile.release] panic = "abort"` explicitly to make this behavior clear in the workspace Cargo.toml.

### C-RUST-6: No std::time in WASM [M0] — HARD

`std::time::Instant` and `std::time::Duration` are not available in WASM. Use `web_sys::window().unwrap().performance().unwrap().now()` (returns milliseconds as f64) for timing. Wrap this in a utility function in `alkahest-core` so all crates use the same time source. The `instant` crate is an alternative that polyfills `Instant` for WASM, but adds a dependency.

### C-RUST-7: WASM memory is limited to 4 GB [M5] — HARD

WASM uses 32-bit memory addresses, limiting the module to 4 GB of linear memory. The voxel data lives on the GPU, not in WASM memory, so this is not the primary constraint — but CPU-side data structures (chunk map, save/load buffers, octree metadata) share this 4 GB. At M5, with hundreds of chunks tracked on the CPU, monitor WASM memory usage. The `wasm32-unknown-unknown` target does not support memory64 as of early 2026.

### C-RUST-8: serde deserialization can be slow in WASM [M3] — PERF

Deserializing large RON files (the material and rule definitions) using `serde` in WASM is slower than native due to WASM's limited integer instruction set and lack of SIMD for string parsing. For the initial 10 materials and 15 rules (M3), this is negligible. At M9 (200 materials) and M14 (500 materials, 10,000 rules), parse time may reach hundreds of milliseconds. Profile rule loading at M9 and consider switching to a pre-compiled binary format for the material table if parse time exceeds 500ms.

---

## 5. Simulation Constraints

### C-SIM-1: Never read and write the same voxel buffer in one pass [M2] — HARD

This is the fundamental invariant of the double-buffer architecture. If a compute shader reads from and writes to the same buffer, voxels processed later in the dispatch will see partially-updated state from voxels processed earlier. This breaks determinism and produces order-dependent artifacts (e.g., sand falling faster on the left side of the screen than the right). Every simulation pass must read from the "current" buffer and write to the "next" buffer. No exceptions.

### C-SIM-2: Sub-pass ordering must be identical every tick [M2] — HARD

The directional sub-pass schedule for movement (down, down-left, down-right, lateral, etc.) must execute in the same order every tick. If the order varies (e.g., randomized per-tick to "improve fairness"), determinism breaks. The specific order can be chosen during M2 development, but once chosen, it must be fixed and documented.

### C-SIM-3: Reaction pass must run after movement pass [M3] — HARD

If reactions run before movement, newly-spawned byproduct voxels may be immediately displaced by the movement pass, producing unexpected "teleporting" byproducts. The pass ordering (ARCH 5.2) is: commands → movement → reactions → field propagation → activity scan. Do not reorder passes.

### C-SIM-4: Stochastic rules must use deterministic PRNG [M2] — HARD

Every use of randomness in the simulation (fire spread probability, particle jitter, etc.) must use the per-voxel deterministic PRNG seeded from world coordinates + tick number. Never use a GPU-provided random number generator or a wall-clock-seeded PRNG. Never use `atomicAdd` on a shared counter as an entropy source. The PRNG function must be pure: `hash(x, y, z, tick) → u32`.

### C-SIM-5: Temperature diffusion rate must respect CFL condition [M4] — HARD

The discrete heat equation is numerically unstable if the diffusion rate is too high relative to the thermal conductivity. Specifically, for a 26-neighbor 3D stencil, the stability condition is approximately: `diffusion_rate * max_conductivity * 26 < 1.0`. If violated, temperatures will oscillate wildly (alternating hot/cold each tick). The rule validator must check this at load time and reject or clamp conductivity values that would violate the condition.

### C-SIM-6: Movement swap must be atomic at the sub-pass level [M2] — HARD

When two voxels swap positions (e.g., sand falling into an empty cell), both the read and write must happen in the same sub-pass dispatch. You cannot have one sub-pass move voxel A out and a second sub-pass move the empty into A's old position — other voxels might move into the "empty" before the second step. Each swap is a single operation: write A's data to B's position and B's data (empty) to A's position in the same shader invocation.

### C-SIM-7: Chunk boundary voxels require neighbor chunk data [M5] — HARD

A voxel at position (0, y, z) within a chunk needs to read position (31, y, z) from the chunk to its left. If that neighbor chunk is Unloaded, the simulation must treat the missing neighbor as a static solid wall (or air, depending on design choice — but the choice must be consistent). Never read from an unallocated buffer region. The dispatch system must guarantee that every chunk in the dispatch list has its neighbor table populated, with a sentinel value for unloaded neighbors.

### C-SIM-8: Activity scan false negatives are forbidden, false positives are acceptable [M5] — DESIGN

If the activity scan reports a chunk as inactive when it actually changed, the chunk will be removed from the dispatch list and its active reactions will freeze. This is a visible, hard-to-debug gameplay bug. If the activity scan reports a chunk as active when it didn't change, the chunk gets an extra few frames of simulation dispatch, which is harmless (it costs a bit of performance but produces no visible error). Design the scan conservatively: mark a chunk as active if any voxel might have changed, even if you're not certain.

---

## 6. Rendering Constraints

### C-RENDER-1: Ray march loop must have a bounded iteration count [M1] — HARD

WGSL does not guarantee that infinite loops will be detected and terminated. A ray march `while(true)` loop that fails to terminate (e.g., due to a floating-point precision issue causing the ray to never advance) will hang the GPU, potentially crashing the browser tab. Always use a `for` loop with a maximum iteration count (e.g., `for (var i = 0u; i < 512u; i++)` for a 32³ chunk). If the maximum is reached, output the sky color and move on.

### C-RENDER-2: Avoid divergent branching in the ray march inner loop [M1] — PERF

GPUs execute threads in lockstep warps/wavefronts (typically 32 or 64 threads). If threads within a warp take different branches in the inner DDA loop (e.g., some advance in X, some in Y), both branches execute serially. This is inherent to ray marching (different rays hit different geometry) and cannot be fully eliminated, but avoid adding unnecessary branching inside the DDA step. Keep the inner loop tight: advance, sample, test, repeat.

### C-RENDER-3: Don't rebuild the octree every frame [M5] — PERF

The render octree (ARCH 4.4) must be incrementally updated, not rebuilt from scratch each frame. A full rebuild of a multi-chunk octree is O(total voxel count) and will blow the frame budget. When the activity scan marks chunks as dirty, update only those chunks' octree nodes. At M5, implement the incremental update path from the start — do not write a full-rebuild path "for simplicity" and plan to optimize later, because the optimization requires a fundamentally different data structure layout.

### C-RENDER-4: Shadow rays are expensive — budget them [M1, critical at M10] — PERF

Each shadow ray is nearly as expensive as the primary ray (full DDA traversal). With 64 dynamic lights (REQ 6.1.3), naively tracing 64 shadow rays per pixel is 64x the cost of primary visibility. At M1, you have one light and this isn't a problem. At M10, you must implement shadow ray budgeting: only trace shadow rays for the N closest/brightest lights per pixel (N ≤ 8 is a reasonable starting point), or use shadow maps for distant lights.

### C-RENDER-5: Transparent voxel rendering order matters [M10] — HARD

For correct volumetric transparency (ARCH 7.3), the ray must accumulate opacity in front-to-back order. The DDA naturally visits voxels in ray order, so this is satisfied for the primary ray. But if you implement any deferred rendering passes that process voxels out of ray order, the transparency compositing will produce incorrect colors. Keep transparency in the ray march pass, not in a deferred pass.

---

## 7. Data Format Constraints

### C-DATA-1: Material IDs must be stable across saves [M3, critical at M8] — HARD

Save files store voxel data with material IDs, not material names. If the material definition files are reordered or new materials are inserted with IDs that shift existing materials, saved worlds will load with the wrong materials. Assign material IDs explicitly in the definition files (do not auto-assign based on file parse order). Reserve ID ranges by category: 0 = air, 1–999 = naturals, 1000–1999 = metals, 2000–2999 = organics, etc. Document the ID allocation scheme and treat it as a stable API.

### C-DATA-2: RON parsing is strict about trailing commas [M3] — HARD

RON (Rusty Object Notation) does not allow trailing commas in structs or lists in the default configuration. The `serde` RON deserializer will reject `[1, 2, 3,]`. Either configure the deserializer to accept trailing commas (`ron::Options::default().with_trailing_comma()`) or ensure all data files omit them. Pick one approach at M3 and enforce it project-wide.

### C-DATA-3: Rule files must not create energy from nothing [M3] — DESIGN

Every reaction rule that produces heat must consume something (fuel, a material transformation, etc.). A rule that says "material A + material B → material A + material B + 100 heat" is a perpetual energy source that will cause runaway temperature in any region containing A and B. The rule validator (M3) must check for energy conservation violations: if a rule produces temperature delta without consuming or transforming at least one input material, reject it.

### C-DATA-4: Material property ranges must match the voxel bit layout [M3] — HARD

The voxel layout quantizes temperature to 12 bits (0–4095) and pressure to 6 bits (0–63). Material definitions that specify a melting point of 50,000 K or a structural integrity of 200 cannot be represented. The rule validator must reject materials with properties that exceed the representable range. The quantization mapping (e.g., 0–4095 → 0–8000 K) must be defined once in `alkahest-core/constants.rs` and used consistently by the validator, the compiler, and the shader unpack functions.

---

## 8. Browser and Deployment Constraints

### C-BROWSER-1: Cross-origin isolation affects all resource loading [M5] — HARD

The cross-origin isolation headers required for `SharedArrayBuffer` (C-RUST-2) also prevent loading resources (scripts, images, fonts) from third-party CDNs unless those resources include CORS headers. If you're loading any external resources (e.g., fonts for the UI), verify they serve `Access-Control-Allow-Origin` headers. Self-hosting all resources is the safest approach.

### C-BROWSER-2: IndexedDB has per-origin storage limits [M8] — COMPAT

Browser-imposed storage limits for IndexedDB vary: Chrome allows up to 80% of disk space, Firefox allows up to 2 GB by default before prompting the user. Large save files (REQ 7.3.4 allows up to 500 MB) may hit these limits. Implement save-to-file (File System Access API) as the primary save mechanism and IndexedDB as a fallback for auto-saves. Detect and report storage quota errors gracefully.

### C-BROWSER-3: File System Access API is Chrome-only [M8] — COMPAT

The File System Access API (`showSaveFilePicker`, `showOpenFilePicker`) is supported in Chrome and Edge but not in Firefox or Safari as of early 2026. For Firefox, fall back to creating a Blob URL and triggering a download link. For loading, use a file `<input>` element. The save/load UI must abstract over these two paths.

### C-BROWSER-4: WebGPU error reporting varies by browser [M0] — COMPAT

Chrome surfaces WebGPU validation errors in the JavaScript console with detailed messages. Firefox's error messages may be less detailed. During development, test error paths (invalid buffer sizes, bind group mismatches, out-of-bounds dispatches) in both browsers to ensure errors are debuggable. Enable the `wgpu` crate's validation layer in debug builds (`wgpu::InstanceFlags::VALIDATION`), which adds a Rust-side validation layer on top of the browser's.

### C-BROWSER-5: requestAnimationFrame throttling [M0] — PERF

Browsers throttle `requestAnimationFrame` to the display refresh rate (typically 60 Hz) and may reduce it further when the tab is not visible (to as low as 1 Hz or 0 Hz). The simulation loop must handle variable frame intervals gracefully. When the tab is backgrounded, the simulation should effectively pause (do not try to "catch up" with accumulated ticks when the tab returns to foreground — this would cause a massive burst of simulation dispatches). Track wall-clock delta per frame and skip simulation ticks if the delta exceeds a threshold (e.g., 100ms).

### C-BROWSER-6: WASM binary size affects load time [M0, ongoing] — PERF

Every crate dependency increases the WASM binary size. Aggressive dependency management is necessary: avoid pulling in crates with large transitive dependency trees. Use `wasm-opt -Oz` in the CI build pipeline to optimize binary size. Monitor the compressed WASM size at each milestone. The REQ 3.2.6 target is 50 MB for the total payload (WASM + assets). The WASM binary alone should stay under 10 MB compressed. The data files (materials, rules) are loaded at runtime and count against the 50 MB total but not the WASM binary size.

---

## 9. egui Constraints

### C-EGUI-1: egui repaints the entire UI every frame [M0] — PERF

egui is an immediate-mode UI library: every frame, you rebuild the entire UI tree from scratch. This is simple to code but means the UI code runs on the CPU every frame. For a debug panel with 5 text labels, this is negligible. At M7, the material browser may display hundreds of entries with search filtering. If the browser UI becomes a performance bottleneck (profile it), implement virtual scrolling (only build UI elements for visible rows) or move the browser to a separate egui window that updates less frequently.

### C-EGUI-2: egui-wgpu integration requires a specific render pass order [M0] — HARD

The egui render pass must run after the main scene render pass and write to the same surface texture. The egui-wgpu integration expects to be given the surface texture's `TextureView` and a `CommandEncoder`. Do not interleave egui rendering with the voxel ray march pass. The frame sequence is: (1) simulation dispatch, (2) ray march render pass, (3) egui render pass, (4) submit command buffer, (5) present surface.

### C-EGUI-3: egui text rendering may appear blurry at non-integer scale factors [M0] — COMPAT

On high-DPI displays, egui text may appear blurry if the scale factor is not accounted for. Pass `window.devicePixelRatio` to egui's `pixels_per_point` setting. If the canvas CSS size and the actual canvas backing size don't match, the entire scene (including UI) will be blurry. Set the canvas element's `width` and `height` attributes to `CSS_width * devicePixelRatio` and `CSS_height * devicePixelRatio`.

---

## 10. Performance Anti-Patterns

These are patterns that produce correct code but will not meet performance targets. Avoid them from the start.

### C-PERF-1: CPU-side per-voxel iteration [M2] — PERF

Never iterate over individual voxels on the CPU at runtime. Not for debugging, not for "just this one case," not "temporarily." The CPU-side code should only operate on chunks (activating, deactivating, dispatching) and on aggregate data (activity flags, chunk metadata). If you find yourself writing a Rust loop that touches individual voxel data during the frame loop, it belongs in a compute shader.

Exception: the structural flood-fill (M6) does read individual voxels on the CPU, but it runs asynchronously on a Web Worker and is triggered by specific events, not per-frame.

### C-PERF-2: Allocating GPU buffers per frame [M1] — PERF

Creating wgpu `Buffer`, `BindGroup`, `Texture`, or `Pipeline` objects is expensive. Allocate all GPU resources during initialization or level loading. Reuse buffers across frames by writing new data into existing buffers (`queue.write_buffer()`). If you need a variable number of something per frame (e.g., the light list), allocate a fixed-capacity buffer and write the actual count as a uniform.

### C-PERF-3: Excessive bind group switching [M3, critical at M5] — PERF

Changing bind groups between dispatches has a measurable cost on some GPUs. When dispatching multiple chunks for the same simulation pass, use the same bind group layout and switch only the dynamic buffer offsets (if using dynamic offset bind groups) rather than creating and binding a new bind group per chunk. This requires designing the bind group layout to support dynamic offsets from the start (M2).

### C-PERF-4: Over-dispatching inactive chunks [M5] — PERF

Dispatching a compute shader over a chunk that has no active voxels wastes GPU time proportional to chunk size × number of passes. The chunk state machine exists to prevent this. If the activity scan or state machine has a bug that keeps chunks in the Active state permanently, performance will degrade linearly with world size even when nothing is happening. Add a debug mode that visualizes chunk states (color-code chunks by state in the wireframe debug view) to catch this during development.

### C-PERF-5: String formatting in the render loop [M0] — PERF

Rust's `format!()` macro allocates a heap String. In WASM, heap allocation is more expensive than native. Calling `format!()` every frame for debug text is measurable at high frame rates. For values that update every frame (FPS counter, tick count), use a pre-allocated `String` and write into it with `write!()`, or use egui's built-in number formatting which avoids allocation.

---

## 11. Design Anti-Patterns

These are structural decisions that will cause problems in later milestones. Avoid them even if they seem simpler in the current milestone.

### C-DESIGN-1: Hardcoding material behavior in shaders [M2, critical at M3] — DESIGN

At M2, you'll be tempted to write `if (material == SAND) { fall_down(); }` in the movement shader. This works for M2 but makes M3 (data-driven materials) a rewrite instead of an extension. At M2, even though there are only two materials (sand and stone), read the material's density from a material properties buffer and make movement decisions based on density comparisons, not material ID comparisons. This is slightly more work at M2 but saves a complete shader rewrite at M3.

### C-DESIGN-2: Tight coupling between simulation passes [M3, critical at M4/M6] — DESIGN

Each simulation pass should communicate with other passes only through the voxel state buffer. Do not create side-channel buffers between passes (e.g., "the reaction pass writes a list of newly-created fire voxels for the thermal pass to read"). The voxel buffer is the single source of truth. The thermal pass discovers fire voxels by reading the voxel buffer, not by reading a side channel from the reaction pass. Side channels create ordering dependencies that make it difficult to add or reorder passes.

### C-DESIGN-3: Embedding configuration in code [M0] — DESIGN

Constants like chunk size, workgroup size, max lights, temperature quantization range, and CFL stability limits must be defined in one place (`alkahest-core/constants.rs`) and propagated to shaders via uniform buffers or `#define`-like substitution in the shader concatenation build script. Do not have a Rust file that says `const CHUNK_SIZE: u32 = 32;` and a shader that separately says `const CHUNK_SIZE = 32u;` — these will inevitably diverge. The build script should inject shared constants into the shader preamble.

### C-DESIGN-4: Premature abstraction [M0, ongoing] — DESIGN

Do not create traits, generics, or plugin systems for things that currently have exactly one implementation. At M1, there is one renderer (ray march). Do not create a `trait Renderer` with `RayMarchRenderer` implementing it "in case we add a rasterizer later." At M2, there is one movement strategy (checkerboard sub-passes). Do not create a `trait ConflictResolver`. Write concrete implementations. If M10 or a later milestone actually requires an alternative, introduce the abstraction at that point with the benefit of knowing what both implementations need.

---

## Appendix A: Constraints by Milestone

This index lists which constraints are relevant at each milestone. Read the listed constraints before starting the milestone.

**M0 (Toolchain):** C-GPU-1, C-GPU-9, C-RUST-1, C-RUST-3, C-RUST-4, C-RUST-5, C-RUST-6, C-BROWSER-4, C-BROWSER-5, C-BROWSER-6, C-EGUI-1, C-EGUI-2, C-EGUI-3, C-PERF-2, C-PERF-5, C-DESIGN-3, C-DESIGN-4

**M1 (Static Rendering):** C-GPU-6, C-WGSL-3, C-WGSL-8, C-RENDER-1, C-RENDER-2, C-RENDER-4, C-PERF-2, C-DESIGN-1

**M2 (Gravity):** C-GPU-3, C-GPU-5, C-GPU-10, C-GPU-11, C-WGSL-1, C-WGSL-2, C-WGSL-4, C-WGSL-5, C-WGSL-6, C-SIM-1, C-SIM-2, C-SIM-4, C-SIM-6, C-PERF-1, C-PERF-3, C-DESIGN-1, C-DESIGN-2

**M3 (Multi-Material):** C-GPU-7, C-WGSL-7, C-SIM-3, C-DATA-1, C-DATA-2, C-DATA-3, C-DATA-4, C-RUST-8, C-DESIGN-2

**M4 (Thermal):** C-SIM-5

**M5 (Multi-Chunk):** C-GPU-2, C-GPU-4, C-GPU-8, C-WGSL-9, C-RUST-2, C-RUST-7, C-SIM-7, C-SIM-8, C-RENDER-3, C-BROWSER-1, C-PERF-3, C-PERF-4

**M6 (Pressure + Structural):** (no new constraints; uses C-SIM-1 through C-SIM-8 and C-DESIGN-2)

**M7 (Player Tools + UI):** C-EGUI-1 (re-evaluate browser performance)

**M8 (Save/Load):** C-DATA-1, C-BROWSER-2, C-BROWSER-3

**M9 (200+ Materials):** C-RUST-8 (re-evaluate parse time)

**M10 (Rendering Polish):** C-RENDER-4, C-RENDER-5

**M11 (Performance):** All C-PERF constraints revisited

**M12 (Modding):** C-DATA-1, C-DATA-2, C-DATA-3, C-DATA-4

**M13 (Audio):** (no new constraints)

**M14 (500+ Materials):** C-RUST-8 (re-evaluate parse time), C-GPU-3 (re-evaluate buffer limits)

**M15 (Electrical):** C-GPU-3 (resolved: electrical pass uses a separate bind group at @group(1) to avoid exceeding the 8-binding limit), C-DATA-4 (resolved: charge is stored in a separate 128 KB/chunk buffer rather than expanding the 8-byte voxel layout), C-SIM-5 (extended: electrical CFL stability validated alongside thermal)
