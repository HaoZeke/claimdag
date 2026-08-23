//! claimdag command line over a directory that holds work.json.

use std::path::PathBuf;

use claimdag::{WorkGraph, WorkId, WorkKind, WorkRole, WorkStatus};
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "claimdag", version, about = "CAS claim and complete on a DAG")]
struct Cli {
    /// Directory that holds work.json.
    #[arg(long, default_value = ".")]
    dir: PathBuf,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    List,
    Get { id: String },
    Upsert {
        #[arg(long)]
        id: Option<String>,
        #[arg(long, default_value = "task")]
        kind: String,
        #[arg(long, default_value = "todo")]
        status: String,
        #[arg(long, default_value = "")]
        summary: String,
        #[arg(long, default_value = "00000000000000000000000000000000")]
        actor: String,
    },
    Claim {
        id: String,
        #[arg(long)]
        assignee: String,
        #[arg(long)]
        gen: Option<u64>,
    },
    Complete {
        id: String,
        #[arg(long, default_value = "done")]
        status: String,
        #[arg(long, default_value = "")]
        summary: String,
        #[arg(long, default_value = "00000000000000000000000000000000")]
        actor: String,
    },
    Link {
        parent: String,
        child: String,
        #[arg(long, default_value = "00000000000000000000000000000000")]
        actor: String,
    },
    Unlink {
        parent: String,
        child: String,
        #[arg(long, default_value = "00000000000000000000000000000000")]
        actor: String,
    },
}

fn parse_id(s: &str) -> Result<WorkId, String> {
    WorkId::from_hex(s).ok_or_else(|| format!("bad id {s}"))
}

fn main() {
    if let Err(e) = run() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let cli = Cli::parse();
    let mut g = WorkGraph::load_dir(&cli.dir);
    match cli.cmd {
        Cmd::List => {
            for n in g.list() {
                println!(
                    "{}  {}  {}  {}",
                    n.id.to_hex(),
                    n.status.as_str(),
                    n.kind.as_str(),
                    n.summary
                );
            }
        }
        Cmd::Get { id } => {
            let id = parse_id(&id)?;
            let n = g.get(id).ok_or("not found")?;
            println!(
                "{}  {}  {}  gen={}  {}",
                n.id.to_hex(),
                n.status.as_str(),
                n.kind.as_str(),
                n.cas_gen,
                n.summary
            );
        }
        Cmd::Upsert {
            id,
            kind,
            status,
            summary,
            actor,
        } => {
            let wid = match id {
                Some(s) => parse_id(&s)?,
                None => WorkId::ZERO,
            };
            let kind = WorkKind::parse_str(&kind).ok_or("bad kind")?;
            let status = WorkStatus::parse_str(&status).ok_or("bad status")?;
            let actor = parse_id(&actor)?;
            let out = g.upsert(wid, kind, status, WorkRole::Unset, WorkId::ZERO, actor, &summary)?;
            g.save_dir(&cli.dir)?;
            println!("{}", out.to_hex());
        }
        Cmd::Claim {
            id,
            assignee,
            gen,
        } => {
            let cas = g.claim(parse_id(&id)?, parse_id(&assignee)?, gen)?;
            g.save_dir(&cli.dir)?;
            println!("gen={cas}");
        }
        Cmd::Complete {
            id,
            status,
            summary,
            actor,
        } => {
            let status = WorkStatus::parse_str(&status).ok_or("bad status")?;
            g.complete(parse_id(&id)?, status, &summary, parse_id(&actor)?)?;
            g.save_dir(&cli.dir)?;
        }
        Cmd::Link {
            parent,
            child,
            actor,
        } => {
            g.link_dep(parse_id(&parent)?, parse_id(&child)?, parse_id(&actor)?)?;
            g.save_dir(&cli.dir)?;
        }
        Cmd::Unlink {
            parent,
            child,
            actor,
        } => {
            g.unlink_dep(parse_id(&parent)?, parse_id(&child)?, parse_id(&actor)?)?;
            g.save_dir(&cli.dir)?;
        }
    }
    Ok(())
}
