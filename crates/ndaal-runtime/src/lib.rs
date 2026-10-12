//! Thread-per-core runtime: shards, cross-core messaging, pools, admission.

// unsafe allowed (ARCHITECTURE.md section 3): thread pinning and lock-free cross-core queues.
// Every unsafe block carries a SAFETY comment.
