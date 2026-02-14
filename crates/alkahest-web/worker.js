// worker.js — Web Worker stub for M5.
// Chunk management runs on the main thread for now (O(256) per frame, microseconds).
// Full Worker implementation deferred to when profiling shows it's needed.
//
// SharedArrayBuffer setup requires cross-origin isolation headers:
//   Cross-Origin-Opener-Policy: same-origin
//   Cross-Origin-Embedder-Policy: require-corp

"use strict";

self.onmessage = function(e) {
    // Stub: echo back any messages received.
    self.postMessage({ type: "ack", data: e.data });
};
