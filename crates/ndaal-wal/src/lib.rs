//! Write-ahead log: format, group commit, reading, recovery.

// unsafe allowed (ARCHITECTURE.md section 3): aligned buffers for direct I/O.
// Every unsafe block carries a SAFETY comment.
