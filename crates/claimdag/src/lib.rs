//! claimdag: CAS claim/complete on a DAG.
//!
//! The host process is the sole mutator. Snapshot is `work.json`.

mod graph;
mod id;

pub use graph::{
    WorkGraph, WorkKind, WorkLedgerEntry, WorkNode, WorkRole, WorkStatus, SNAP_FILE,
};
pub use id::{mint_work_id, WorkId};
