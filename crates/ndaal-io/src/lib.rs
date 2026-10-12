//! Time, disk, network and randomness abstractions with real and simulated implementations.

// unsafe allowed (ARCHITECTURE.md section 3): system calls, aligned buffers and io_uring.
// Every unsafe block carries a SAFETY comment.
