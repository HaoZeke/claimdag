//! Claim/complete DAG. Host process is the sole mutator.
//!
//! Identity is [`WorkId`] (xxh3-128). Closed enums for kind/status/role.
//! One open text field: summary. Snapshot is work.json next to the host
//! state dir.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::Path;

use crate::id::{mint_work_id, WorkId};

/// Snapshot format written by this crate. Loader also accepts unversioned v0.
pub const FORMAT_V1: &str = "claimdag/v1";

/// Snapshot filename under the host state dir.
pub const SNAP_FILE: &str = "work.json";

const MAX_WORK_NODES: usize = 4096;
const MAX_LEDGER: usize = 8192;

/// Work node status (CAS claim/complete).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkStatus {
    Todo,
    Ready,
    Claimed,
    Running,
    Blocked,
    Done,
    Failed,
    Cancelled,
}

impl WorkStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Todo => "todo",
            Self::Ready => "ready",
            Self::Claimed => "claimed",
            Self::Running => "running",
            Self::Blocked => "blocked",
            Self::Done => "done",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    pub fn parse_str(s: &str) -> Option<Self> {
        match s {
            "todo" => Some(Self::Todo),
            "ready" => Some(Self::Ready),
            "claimed" => Some(Self::Claimed),
            "running" => Some(Self::Running),
            "blocked" => Some(Self::Blocked),
            "done" => Some(Self::Done),
            "failed" => Some(Self::Failed),
            "cancelled" => Some(Self::Cancelled),
            _ => None,
        }
    }

    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Done | Self::Failed | Self::Cancelled)
    }


}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkRole {
    Unset,
    Explore,
    Architect,
    Implementor,
    Verifier,
    Orchestrator,
    General,
}

impl WorkRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Unset => "unset",
            Self::Explore => "explore",
            Self::Architect => "architect",
            Self::Implementor => "implementor",
            Self::Verifier => "verifier",
            Self::Orchestrator => "orchestrator",
            Self::General => "general",
        }
    }

    pub fn parse_str(s: &str) -> Option<Self> {
        match s {
            "unset" => Some(Self::Unset),
            "explore" => Some(Self::Explore),
            "architect" => Some(Self::Architect),
            "implementor" => Some(Self::Implementor),
            "verifier" => Some(Self::Verifier),
            "orchestrator" => Some(Self::Orchestrator),
            "general" => Some(Self::General),
            _ => None,
        }
    }


}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkKind {
    Unset,
    Goal,
    Step,
    Task,
    Molecule,
}

impl WorkKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Unset => "unset",
            Self::Goal => "goal",
            Self::Step => "step",
            Self::Task => "task",
            Self::Molecule => "molecule",
        }
    }

    pub fn parse_str(s: &str) -> Option<Self> {
        match s {
            "unset" => Some(Self::Unset),
            "goal" => Some(Self::Goal),
            "step" => Some(Self::Step),
            "task" => Some(Self::Task),
            "molecule" => Some(Self::Molecule),
            _ => None,
        }
    }


}

#[derive(Debug, Clone)]
pub struct WorkNode {
    pub id: WorkId,
    pub kind: WorkKind,
    pub status: WorkStatus,
    pub role: WorkRole,
    pub assignee: WorkId,
    pub parent: WorkId,
    pub deps: Vec<WorkId>,
    pub cas_gen: u64,
    pub created_unix: u64,
    pub updated_unix: u64,
    pub finished_unix: u64,
    /// Sole open-content field (writer-owned length).
    pub summary: String,
}


#[derive(Debug, Clone)]
pub struct WorkLedgerEntry {
    pub seq: u64,
    pub ts_unix: u64,
    pub work_id: WorkId,
    pub actor: WorkId,
    /// Closed op tag (static str interned as &'static str in practice).
    pub op: &'static str,
}

#[derive(Debug)]
pub struct WorkGraph {
    nodes: HashMap<WorkId, WorkNode>,
    ledger: VecDeque<WorkLedgerEntry>,
    next_seq: u64,
    mint_seq: u64,
}

impl Default for WorkGraph {
    fn default() -> Self {
        Self {
            nodes: HashMap::new(),
            ledger: VecDeque::new(),
            next_seq: 0,
            mint_seq: 0,
        }
    }
}

impl WorkGraph {
    pub fn now() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }

    /// Mint a new work id (xxh3 family). Deterministic from seq + salt.
    pub fn mint_id(&mut self, kind: WorkKind, parent: WorkId, summary: &str) -> WorkId {
        self.mint_seq = self.mint_seq.saturating_add(1);
        let salt = Self::now().to_le_bytes();
        let role = match kind {
            WorkKind::Goal => "work-goal",
            WorkKind::Step => "work-step",
            WorkKind::Task => "work-task",
            WorkKind::Molecule => "work-molecule",
            WorkKind::Unset => "work",
        };
        mint_work_id(
            parent,
            WorkId::ZERO,
            role,
            summary,
            self.mint_seq,
            &salt,
        )
    }

    pub fn get(&self, id: WorkId) -> Option<&WorkNode> {
        self.nodes.get(&id)
    }

    pub fn list(&self) -> Vec<&WorkNode> {
        let mut v: Vec<_> = self.nodes.values().collect();
        v.sort_by(|a, b| {
            b.updated_unix
                .cmp(&a.updated_unix)
                .then_with(|| a.id.hi.cmp(&b.id.hi).then(a.id.lo.cmp(&b.id.lo)))
        });
        v
    }

    fn push_ledger(&mut self, work_id: WorkId, actor: WorkId, op: &'static str) {
        self.next_seq = self.next_seq.saturating_add(1);
        self.ledger.push_back(WorkLedgerEntry {
            seq: self.next_seq,
            ts_unix: Self::now(),
            work_id,
            actor,
            op,
        });
        while self.ledger.len() > MAX_LEDGER {
            self.ledger.pop_front();
        }
    }

    fn prune(&mut self) {
        if self.nodes.len() <= MAX_WORK_NODES {
            return;
        }
        let overflow = self.nodes.len() - MAX_WORK_NODES;
        let mut terminal: Vec<(WorkId, u64)> = self
            .nodes
            .iter()
            .filter(|(_, n)| n.status.is_terminal())
            .map(|(k, n)| (*k, n.finished_unix.max(n.updated_unix)))
            .collect();
        terminal.sort_by_key(|(_, t)| *t);
        for (id, _) in terminal.into_iter().take(overflow) {
            self.nodes.remove(&id);
        }
    }

    pub fn upsert(
        &mut self,
        mut id: WorkId,
        kind: WorkKind,
        status: WorkStatus,
        role: WorkRole,
        parent: WorkId,
        actor: WorkId,
        summary: &str,
    ) -> Result<WorkId, String> {
        if id.is_zero() {
            id = self.mint_id(kind, parent, summary);
        }
        let now = Self::now();
        let entry = self.nodes.entry(id).or_insert_with(|| WorkNode {
            id,
            kind,
            status: WorkStatus::Todo,
            role,
            assignee: WorkId::ZERO,
            parent,
            deps: Vec::new(),
            // Start at 1 so wire expectedGen=0 means "ignore", not "assert gen 0".
            cas_gen: 1,
            created_unix: now,
            updated_unix: now,
            finished_unix: 0,
            summary: String::new(),
        });
        if entry.status.is_terminal() {
            // Sticky terminal: reject reopen/status forge (client sees error).
            return Err("upsert: node is terminal".into());
        }
        // Lifecycle authority: claim/complete own Claimed/Running/terminal.
        // Upsert only metadata + Todo|Ready|Blocked.
        match status {
            WorkStatus::Todo | WorkStatus::Ready | WorkStatus::Blocked => {
                entry.status = status;
            }
            WorkStatus::Claimed | WorkStatus::Running | WorkStatus::Done
            | WorkStatus::Failed | WorkStatus::Cancelled => {
                return Err(
                    "upsert: status claim/run/terminal requires claimWork or completeWork"
                        .into(),
                );
            }
        }
        if kind != WorkKind::Unset {
            entry.kind = kind;
        }
        if role != WorkRole::Unset {
            entry.role = role;
        }
        if !parent.is_zero() {
            entry.parent = parent;
        }
        if !summary.is_empty() {
            entry.summary = summary.to_string();
        }
        entry.updated_unix = now;
        let wid = entry.id;
        self.push_ledger(wid, actor, "upsert");
        self.prune();
        Ok(wid)
    }

    pub fn link_dep(
        &mut self,
        parent: WorkId,
        child: WorkId,
        actor: WorkId,
    ) -> Result<(), String> {
        if parent.is_zero() || child.is_zero() {
            return Err("link: zero id".into());
        }
        if parent == child {
            return Err("link: self-dep".into());
        }
        if !self.nodes.contains_key(&parent) {
            return Err("link: parent missing".into());
        }
        if !self.nodes.contains_key(&child) {
            return Err("link: child missing".into());
        }
        if self.would_cycle(parent, child) {
            return Err("link: would create cycle".into());
        }
        let node = self.nodes.get_mut(&child).expect("child present");
        if !node.deps.iter().any(|d| *d == parent) {
            node.deps.push(parent);
            node.updated_unix = Self::now();
        }
        self.recompute_ready(child);
        self.push_ledger(child, actor, "link");
        Ok(())
    }

    /// Drop a boolean hard dep. Missing edge is Ok.
    pub fn unlink_dep(
        &mut self,
        parent: WorkId,
        child: WorkId,
        actor: WorkId,
    ) -> Result<(), String> {
        if parent.is_zero() || child.is_zero() {
            return Err("unlink: zero id".into());
        }
        let node = self
            .nodes
            .get_mut(&child)
            .ok_or_else(|| "unlink: child missing".to_string())?;
        let before = node.deps.len();
        node.deps.retain(|d| *d != parent);
        if node.deps.len() != before {
            node.updated_unix = Self::now();
        }
        self.recompute_ready(child);
        self.push_ledger(child, actor, "unlink");
        Ok(())
    }

    fn would_cycle(&self, parent: WorkId, child: WorkId) -> bool {
        let mut stack = vec![parent];
        let mut seen = HashSet::new();
        while let Some(id) = stack.pop() {
            if id == child {
                return true;
            }
            if !seen.insert(id) {
                continue;
            }
            if let Some(n) = self.nodes.get(&id) {
                stack.extend(n.deps.iter().copied());
            }
        }
        false
    }

    fn deps_satisfied(&self, node: &WorkNode) -> bool {
        node.deps.iter().all(|d| {
            self.nodes
                .get(d)
                .map(|p| p.status == WorkStatus::Done)
                .unwrap_or(false)
        })
    }

    fn recompute_ready(&mut self, id: WorkId) {
        let ready = {
            let Some(n) = self.nodes.get(&id) else {
                return;
            };
            matches!(
                n.status,
                WorkStatus::Todo | WorkStatus::Blocked | WorkStatus::Ready
            ) && self.deps_satisfied(n)
        };
        if let Some(n) = self.nodes.get_mut(&id) {
            if ready && matches!(n.status, WorkStatus::Todo | WorkStatus::Blocked) {
                n.status = WorkStatus::Ready;
                n.updated_unix = Self::now();
            } else if !ready && n.status == WorkStatus::Ready {
                n.status = WorkStatus::Todo;
                n.updated_unix = Self::now();
            }
        }
    }

    fn promote_dependents(&mut self, done_id: WorkId) {
        let children: Vec<WorkId> = self
            .nodes
            .values()
            .filter(|n| n.deps.iter().any(|d| *d == done_id))
            .map(|n| n.id)
            .collect();
        for c in children {
            self.recompute_ready(c);
        }
    }

    pub fn claim(
        &mut self,
        id: WorkId,
        assignee: WorkId,
        expected_gen: Option<u64>,
    ) -> Result<u64, String> {
        if id.is_zero() {
            return Err("claim: zero id".into());
        }
        if assignee.is_zero() {
            return Err("claim: zero assignee".into());
        }
        let deps: Vec<WorkId> = {
            let node = self
                .nodes
                .get(&id)
                .ok_or_else(|| "claim: not found".to_string())?;
            if let Some(g) = expected_gen {
                if node.cas_gen != g {
                    return Err("claim: gen mismatch".into());
                }
            }
            if !matches!(node.status, WorkStatus::Ready | WorkStatus::Todo) {
                return Err(format!("claim: status {}", node.status.as_str()));
            }
            node.deps.clone()
        };
        let deps_satisfied = deps.iter().all(|d| {
            self.nodes
                .get(d)
                .map(|p| p.status == WorkStatus::Done)
                .unwrap_or(false)
        });
        if !deps_satisfied {
            return Err("claim: deps unsatisfied".into());
        }
        // Occupancy is a graph predicate: one Claimed|Running node per assignee.
        // Same mutex as the gen bump (caller holds work.lock()). Skip `id`.
        let mut busy: Vec<WorkId> = self
            .nodes
            .values()
            .filter(|n| {
                n.id != id
                    && n.assignee == assignee
                    && matches!(n.status, WorkStatus::Claimed | WorkStatus::Running)
            })
            .map(|n| n.id)
            .collect();
        if !busy.is_empty() {
            busy.sort_by(|a, b| a.hi.cmp(&b.hi).then(a.lo.cmp(&b.lo)));
            let listed = busy
                .iter()
                .map(|held| held.to_hex())
                .collect::<Vec<_>>()
                .join(" ");
            return Err(format!("claim: assignee busy {listed}"));
        }
        let now = Self::now();
        let node = self.nodes.get_mut(&id).unwrap();
        node.status = WorkStatus::Claimed;
        node.assignee = assignee;
        node.cas_gen = node.cas_gen.saturating_add(1);
        node.updated_unix = now;
        let cas_gen = node.cas_gen;
        self.push_ledger(id, assignee, "claim");
        Ok(cas_gen)
    }

    pub fn set_running(&mut self, id: WorkId, actor: WorkId) -> Result<(), String> {
        let node = self
            .nodes
            .get_mut(&id)
            .ok_or_else(|| "running: not found".to_string())?;
        if !matches!(node.status, WorkStatus::Claimed | WorkStatus::Running) {
            return Err(format!("running: status {}", node.status.as_str()));
        }
        node.status = WorkStatus::Running;
        node.updated_unix = Self::now();
        self.push_ledger(id, actor, "running");
        Ok(())
    }

    pub fn complete(
        &mut self,
        id: WorkId,
        status: WorkStatus,
        summary: &str,
        actor: WorkId,
    ) -> Result<(), String> {
        if !status.is_terminal() {
            return Err("complete: status not terminal".into());
        }
        if id.is_zero() {
            return Err("complete: zero id".into());
        }
        let now = Self::now();
        let node = self
            .nodes
            .get_mut(&id)
            .ok_or_else(|| "complete: not found".to_string())?;
        if node.status.is_terminal() {
            if !summary.is_empty() {
                node.summary = summary.to_string();
            }
            return Ok(());
        }
        // If claimed/running, only the assignee (or zero actor = CLI escape) may complete.
        if matches!(node.status, WorkStatus::Claimed | WorkStatus::Running)
            && !node.assignee.is_zero()
            && !actor.is_zero()
            && node.assignee != actor
        {
            return Err("complete: not assignee".into());
        }
        node.status = status;
        if !summary.is_empty() {
            node.summary = summary.to_string();
        }
        node.finished_unix = now;
        node.updated_unix = now;
        self.push_ledger(id, actor, "complete");
        self.promote_dependents(id);
        Ok(())
    }

    pub fn verify(&self) -> Result<(), String> {
        for (id, n) in &self.nodes {
            for d in &n.deps {
                if !self.nodes.contains_key(d) {
                    return Err(format!("verify: {} deps missing {}", id.to_hex(), d.to_hex()));
                }
            }
            let mut stack = n.deps.clone();
            let mut seen = HashSet::new();
            while let Some(cur) = stack.pop() {
                if cur == *id {
                    return Err(format!("verify: cycle involving {}", id.to_hex()));
                }
                if !seen.insert(cur) {
                    continue;
                }
                if let Some(nn) = self.nodes.get(&cur) {
                    stack.extend(nn.deps.iter().copied());
                }
            }
        }
        Ok(())
    }

    /// Load `$dir/work.json`, or an empty graph when the file is missing.
    pub fn load_dir(dir: &Path) -> Self {
        let path = dir.join(SNAP_FILE);
        match std::fs::read(&path) {
            Ok(bytes) => match Self::from_snap_bytes(&bytes) {
                Ok(g) => g,
                Err(_) => Self::default(),
            },
            Err(_) => Self::default(),
        }
    }

    /// Atomic replace of `$dir/work.json`.
    pub fn save_dir(&self, dir: &Path) -> Result<(), String> {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        let path = dir.join(SNAP_FILE);
        let tmp = dir.join("work.json.tmp");
        let bytes = self.to_snap_bytes()?;
        std::fs::write(&tmp, &bytes).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, &path).map_err(|e| e.to_string())?;
        Ok(())
    }

    fn to_snap_bytes(&self) -> Result<Vec<u8>, String> {
        let snap = WorkSnap {
            format: Some(FORMAT_V1.to_string()),
            next_seq: self.next_seq,
            mint_seq: self.mint_seq,
            nodes: self
                .list()
                .into_iter()
                .map(WorkNodeSnap::from_node)
                .collect(),
        };
        serde_json::to_vec_pretty(&snap).map_err(|e| e.to_string())
    }

    fn from_snap_bytes(bytes: &[u8]) -> Result<Self, String> {
        let snap: WorkSnap = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
        let mut g = Self {
            nodes: HashMap::new(),
            ledger: VecDeque::new(),
            next_seq: snap.next_seq,
            mint_seq: snap.mint_seq,
        };
        for n in snap.nodes {
            let node = n.into_node()?;
            g.nodes.insert(node.id, node);
        }
        Ok(g)
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
struct WorkSnap {
    #[serde(default)]
    format: Option<String>,
    next_seq: u64,
    mint_seq: u64,
    nodes: Vec<WorkNodeSnap>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct WorkNodeSnap {
    id: String,
    kind: String,
    status: String,
    role: String,
    assignee: String,
    parent: String,
    deps: Vec<String>,
    cas_gen: u64,
    created_unix: u64,
    updated_unix: u64,
    finished_unix: u64,
    summary: String,
}

impl WorkNodeSnap {
    fn from_node(n: &WorkNode) -> Self {
        Self {
            id: n.id.to_hex(),
            kind: n.kind.as_str().into(),
            status: n.status.as_str().into(),
            role: n.role.as_str().into(),
            assignee: n.assignee.to_hex(),
            parent: n.parent.to_hex(),
            deps: n.deps.iter().map(|d| d.to_hex()).collect(),
            cas_gen: n.cas_gen,
            created_unix: n.created_unix,
            updated_unix: n.updated_unix,
            finished_unix: n.finished_unix,
            summary: n.summary.clone(),
        }
    }

    fn into_node(self) -> Result<WorkNode, String> {
        let id = WorkId::from_hex(&self.id).ok_or("snap: bad id")?;
        let mut deps = Vec::with_capacity(self.deps.len());
        for d in self.deps {
            deps.push(WorkId::from_hex(&d).ok_or("snap: bad dep")?);
        }
        Ok(WorkNode {
            id,
            kind: WorkKind::parse_str(&self.kind).ok_or("snap: bad kind")?,
            status: WorkStatus::parse_str(&self.status).ok_or("snap: bad status")?,
            role: WorkRole::parse_str(&self.role).ok_or("snap: bad role")?,
            assignee: WorkId::from_hex(&self.assignee).ok_or("snap: bad assignee")?,
            parent: WorkId::from_hex(&self.parent).ok_or("snap: bad parent")?,
            deps,
            cas_gen: self.cas_gen,
            created_unix: self.created_unix,
            updated_unix: self.updated_unix,
            finished_unix: self.finished_unix,
            summary: self.summary,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(n: u64) -> WorkId {
        WorkId { hi: n, lo: n.wrapping_mul(3) }
    }

    #[test]
    fn claim_cas_and_deps() {
        let mut g = WorkGraph::default();
        let a = id(1);
        let b = id(2);
        g.upsert(
            a,
            WorkKind::Step,
            WorkStatus::Ready,
            WorkRole::Explore,
            WorkId::ZERO,
            id(9),
            "A",
        )
        .unwrap();
        g.upsert(
            b,
            WorkKind::Step,
            WorkStatus::Todo,
            WorkRole::Implementor,
            WorkId::ZERO,
            id(9),
            "B",
        )
        .unwrap();
        g.link_dep(a, b, id(9)).unwrap();
        assert!(g.claim(b, id(10), None).is_err());
        g.complete(a, WorkStatus::Done, "ok", id(10)).unwrap();
        assert_eq!(g.get(b).unwrap().status, WorkStatus::Ready);
        assert_eq!(g.get(b).unwrap().cas_gen, 1);
        assert_eq!(
            g.claim(b, id(10), Some(99)).unwrap_err(),
            "claim: gen mismatch"
        );
        let cas_gen = g.claim(b, id(10), None).unwrap();
        assert_eq!(
            g.claim(b, id(11), Some(1)).unwrap_err(),
            "claim: gen mismatch"
        );
        assert!(g.claim(b, id(11), Some(cas_gen)).is_err());
        g.set_running(b, id(10)).unwrap();
        g.complete(b, WorkStatus::Done, "done", id(10)).unwrap();
        assert!(g.verify().is_ok());
    }

    fn upsert_ready(g: &mut WorkGraph, node: WorkId, summary: &str) {
        g.upsert(
            node,
            WorkKind::Task,
            WorkStatus::Ready,
            WorkRole::Unset,
            WorkId::ZERO,
            id(9),
            summary,
        )
        .unwrap();
    }

    #[test]
    fn claim_rejects_assignee_already_holding_live_node() {
        let mut g = WorkGraph::default();
        let a = id(1);
        let b = id(2);
        let x = id(20);
        let y = id(21);
        upsert_ready(&mut g, a, "A");
        upsert_ready(&mut g, b, "B");
        assert_eq!(g.get(a).unwrap().cas_gen, 1);
        assert_eq!(g.get(b).unwrap().cas_gen, 1);
        g.claim(a, x, None).unwrap();
        assert_eq!(g.get(a).unwrap().status, WorkStatus::Claimed);
        let err = g.claim(b, x, None).unwrap_err();
        assert!(
            err.starts_with("claim: assignee busy"),
            "expected busy, got {err}"
        );
        assert!(
            err.contains(&a.to_hex()),
            "busy error must list held id {}; got {err}",
            a.to_hex()
        );
        let b_after = g.get(b).unwrap();
        assert_eq!(b_after.status, WorkStatus::Ready);
        assert_eq!(b_after.cas_gen, 1);
        assert!(b_after.assignee.is_zero());
        g.claim(b, y, None).unwrap();
        assert_eq!(g.get(b).unwrap().status, WorkStatus::Claimed);
        assert_eq!(g.get(b).unwrap().assignee, y);
        g.set_running(a, x).unwrap();
        let c = id(3);
        upsert_ready(&mut g, c, "C");
        let running_err = g.claim(c, x, None).unwrap_err();
        assert!(
            running_err.starts_with("claim: assignee busy"),
            "Running still occupies; got {running_err}"
        );
        assert!(running_err.contains(&a.to_hex()), "{running_err}");
        g.complete(a, WorkStatus::Done, "ok", x).unwrap();
        g.claim(c, x, None).unwrap();
        assert_eq!(g.get(c).unwrap().status, WorkStatus::Claimed);
        assert_eq!(g.get(c).unwrap().assignee, x);
    }

    #[test]
    fn claim_concurrent_same_assignee_one_live() {
        use std::sync::{Arc, Mutex};
        let mut g = WorkGraph::default();
        let a = id(1);
        let b = id(2);
        let x = id(20);
        upsert_ready(&mut g, a, "A");
        upsert_ready(&mut g, b, "B");
        let g = Arc::new(Mutex::new(g));
        let g_a = Arc::clone(&g);
        let g_b = Arc::clone(&g);
        let t_a = std::thread::spawn(move || g_a.lock().unwrap().claim(a, x, None));
        let t_b = std::thread::spawn(move || g_b.lock().unwrap().claim(b, x, None));
        let r_a = t_a.join().expect("claim A thread");
        let r_b = t_b.join().expect("claim B thread");
        assert_eq!(
            u8::from(r_a.is_ok()) + u8::from(r_b.is_ok()),
            1,
            "exactly one of X-on-A / X-on-B succeeds; A={r_a:?} B={r_b:?}"
        );
        let busy = if r_a.is_err() {
            r_a.unwrap_err()
        } else {
            r_b.unwrap_err()
        };
        assert!(
            busy.starts_with("claim: assignee busy"),
            "loser must be occupancy; got {busy}"
        );
        let g = g.lock().unwrap();
        let live: Vec<_> = g
            .list()
            .into_iter()
            .filter(|n| {
                n.assignee == x && matches!(n.status, WorkStatus::Claimed | WorkStatus::Running)
            })
            .collect();
        assert_eq!(live.len(), 1, "exactly one Claimed|Running for X");
    }

    #[test]
    fn reject_cycle() {
        let mut g = WorkGraph::default();
        let a = id(1);
        let b = id(2);
        g.upsert(a, WorkKind::Task, WorkStatus::Todo, WorkRole::Unset, WorkId::ZERO, id(9), "")
            .unwrap();
        g.upsert(b, WorkKind::Task, WorkStatus::Todo, WorkRole::Unset, WorkId::ZERO, id(9), "")
            .unwrap();
        g.link_dep(a, b, id(9)).unwrap();
        assert!(g.link_dep(b, a, id(9)).is_err());
    }

    #[test]
    fn mint_nonzero() {
        let mut g = WorkGraph::default();
        let id = g
            .upsert(
                WorkId::ZERO,
                WorkKind::Goal,
                WorkStatus::Ready,
                WorkRole::Orchestrator,
                WorkId::ZERO,
                WorkId::ZERO,
                "mint me",
            )
            .unwrap();
        assert!(!id.is_zero());
    }

    #[test]
    fn upsert_rejects_claim_status() {
        let mut g = WorkGraph::default();
        let a = id(7);
        g.upsert(
            a,
            WorkKind::Task,
            WorkStatus::Ready,
            WorkRole::Unset,
            WorkId::ZERO,
            id(9),
            "x",
        )
        .unwrap();
        assert!(g
            .upsert(
                a,
                WorkKind::Task,
                WorkStatus::Claimed,
                WorkRole::Unset,
                WorkId::ZERO,
                id(9),
                "",
            )
            .is_err());
        // Initial cas_gen is 1 (wire 0 = ignore).
        assert_eq!(g.get(a).unwrap().cas_gen, 1);
    }

    #[test]
    fn snap_roundtrip_keeps_done_node() {
        let dir = std::env::temp_dir().join(format!("claimdag-work-snap-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut g = WorkGraph::default();
        let a = id(42);
        g.upsert(
            a,
            WorkKind::Task,
            WorkStatus::Ready,
            WorkRole::Implementor,
            WorkId::ZERO,
            id(9),
            "persist me",
        )
        .unwrap();
        g.complete(a, WorkStatus::Done, "done on disk", id(9))
            .unwrap();
        g.save_dir(&dir).unwrap();
        let loaded = WorkGraph::load_dir(&dir);
        let n = loaded.get(a).expect("done node reloads");
        assert_eq!(n.status, WorkStatus::Done);
        assert_eq!(n.kind, WorkKind::Task);
        assert_eq!(n.summary, "done on disk");
        assert!(n.finished_unix > 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_dir_missing_is_empty() {
        let dir = std::env::temp_dir().join(format!("claimdag-work-missing-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let g = WorkGraph::load_dir(&dir);
        assert!(g.list().is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn snap_writer_marks_v1_and_reads_v0() {
        let dir = std::env::temp_dir().join(format!("claimdag-work-v1-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut g = WorkGraph::default();
        let a = id(7);
        upsert_ready(&mut g, a, "v1");
        g.save_dir(&dir).unwrap();
        let raw = std::fs::read_to_string(dir.join(SNAP_FILE)).unwrap();
        assert!(raw.contains("claimdag/v1"), "{raw}");
        let v0 = raw.replacen("\"format\": \"claimdag/v1\",\n", "", 1);
        std::fs::write(dir.join(SNAP_FILE), v0).unwrap();
        let loaded = WorkGraph::load_dir(&dir);
        assert_eq!(loaded.get(a).unwrap().summary, "v1");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn unlink_drops_edge_and_readies() {
        let mut g = WorkGraph::default();
        let a = id(1);
        let b = id(2);
        upsert_ready(&mut g, a, "A");
        g.upsert(
            b,
            WorkKind::Task,
            WorkStatus::Todo,
            WorkRole::Unset,
            WorkId::ZERO,
            id(9),
            "B",
        )
        .unwrap();
        g.link_dep(a, b, id(9)).unwrap();
        assert_eq!(g.get(b).unwrap().status, WorkStatus::Todo);
        g.unlink_dep(a, b, id(9)).unwrap();
        assert!(g.get(b).unwrap().deps.is_empty());
        assert_eq!(g.get(b).unwrap().status, WorkStatus::Ready);
        // Missing edge is Ok; recompute_ready stays Ready.
        g.unlink_dep(a, b, id(9)).unwrap();
        assert!(g.get(b).unwrap().deps.is_empty());
        assert_eq!(g.get(b).unwrap().status, WorkStatus::Ready);
    }

    #[test]
    fn unlink_missing_edge_is_ok() {
        let mut g = WorkGraph::default();
        let a = id(1);
        let b = id(2);
        upsert_ready(&mut g, a, "A");
        g.upsert(
            b,
            WorkKind::Task,
            WorkStatus::Todo,
            WorkRole::Unset,
            WorkId::ZERO,
            id(9),
            "B",
        )
        .unwrap();
        g.unlink_dep(a, b, id(9)).unwrap();
        assert!(g.get(b).unwrap().deps.is_empty());
        assert_eq!(g.get(b).unwrap().status, WorkStatus::Ready);
    }

    #[test]
    fn sticky_terminal_rejects_reopen() {
        let mut g = WorkGraph::default();
        let a = id(1);
        upsert_ready(&mut g, a, "A");
        g.complete(a, WorkStatus::Done, "done", id(9)).unwrap();
        assert_eq!(g.get(a).unwrap().status, WorkStatus::Done);
        let err = g
            .upsert(
                a,
                WorkKind::Task,
                WorkStatus::Ready,
                WorkRole::Unset,
                WorkId::ZERO,
                id(9),
                "reopen",
            )
            .unwrap_err();
        assert_eq!(err, "upsert: node is terminal");
        let after_upsert = g.get(a).unwrap();
        assert_eq!(after_upsert.status, WorkStatus::Done);
        assert_eq!(after_upsert.summary, "done");
        assert_eq!(g.claim(a, id(10), None).unwrap_err(), "claim: status done");
        g.complete(a, WorkStatus::Failed, "still done", id(9))
            .unwrap();
        let after_complete = g.get(a).unwrap();
        assert_eq!(after_complete.status, WorkStatus::Done);
        assert_eq!(after_complete.summary, "still done");

        let b = id(2);
        upsert_ready(&mut g, b, "B");
        g.complete(b, WorkStatus::Cancelled, "nope", id(9)).unwrap();
        assert_eq!(
            g.upsert(
                b,
                WorkKind::Task,
                WorkStatus::Todo,
                WorkRole::Unset,
                WorkId::ZERO,
                id(9),
                "",
            )
            .unwrap_err(),
            "upsert: node is terminal"
        );
        assert_eq!(g.get(b).unwrap().status, WorkStatus::Cancelled);
        assert_eq!(
            g.claim(b, id(10), None).unwrap_err(),
            "claim: status cancelled"
        );
    }
}
