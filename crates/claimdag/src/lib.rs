//! claimdag: CAS claim/complete on a DAG.
//!
//! The host process is the sole mutator. Snapshot is unpacked Cap'n
//! `work.bin` (mmap). `work.json` is load-only v0.

mod claimdag_capnp;
mod graph;
mod id;
mod snap;

pub use graph::{WorkGraph, WorkKind, WorkLedgerEntry, WorkNode, WorkRole, WorkStatus, SNAP_FILE};
pub use id::{mint_work_id, WorkId};
pub use snap::{SNAP_BIN, SNAP_JSON_V0};
