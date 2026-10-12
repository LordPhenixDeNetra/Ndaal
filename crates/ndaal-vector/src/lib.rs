//! HNSW, quantization, segments, filters, payload indexes, disk index.

// unsafe allowed (ARCHITECTURE.md section 3): SIMD distance kernels.
// Every unsafe block carries a SAFETY comment.
