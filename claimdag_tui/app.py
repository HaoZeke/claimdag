"""WorkGraph pane: Textual Tree over claimdag work.json.

Mutations go through the claimdag CLI. This process does not reimplement
claim, complete, or unlink.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import socket
import subprocess
import sys
from pathlib import Path
from typing import Any

from textual.app import App, ComposeResult
from textual.binding import Binding
from textual.widgets import Footer, Header, Tree

from claimdag_tui.theme import CSS_FILES

ZERO = "0" * 32


def default_dir() -> Path:
    raw = os.environ.get("CLAIMDAG_DIR")
    if raw:
        return Path(raw)
    xdg = os.environ.get("XDG_RUNTIME_DIR", "/tmp")
    return Path(xdg) / "claimdag"


def default_actor() -> str:
    raw = (os.environ.get("CLAIMDAG_ACTOR") or "").strip().lower()
    if len(raw) == 32 and all(c in "0123456789abcdef" for c in raw):
        return raw
    ident = f"{os.environ.get('USER', 'actor')}@{socket.gethostname()}"
    return hashlib.sha256(ident.encode()).hexdigest()[:32]


def load_nodes(directory: Path) -> list[dict[str, Any]]:
    path = directory / "work.json"
    if not path.is_file():
        return []
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return []
    if not isinstance(data, dict):
        return []
    nodes = data.get("nodes") or []
    if not isinstance(nodes, list):
        return []
    return [n for n in nodes if isinstance(n, dict) and n.get("id")]


def node_id(node: dict[str, Any]) -> str:
    return str(node.get("id") or "")


def node_label(node: dict[str, Any]) -> str:
    nid = node_id(node)
    status = str(node.get("status") or "?")
    summary = str(node.get("summary") or "").strip() or nid[:8]
    deps = node.get("deps") or []
    extra = f"  deps={len(deps)}" if isinstance(deps, list) and deps else ""
    return f"{status}  {summary}{extra}"


def parent_tree(
    nodes: list[dict[str, Any]],
) -> tuple[dict[str, dict[str, Any]], dict[str, list[str]], list[str]]:
    by_id = {node_id(n): n for n in nodes if node_id(n)}
    children: dict[str, list[str]] = {k: [] for k in by_id}
    roots: list[str] = []
    for nid, node in by_id.items():
        parent = str(node.get("parent") or ZERO)
        if parent in by_id and parent not in {ZERO, nid}:
            children[parent].append(nid)
        else:
            roots.append(nid)
    for kids in children.values():
        kids.sort()
    roots.sort()
    return by_id, children, roots


def dump_forest(nodes: list[dict[str, Any]]) -> str:
    by_id, children, roots = parent_tree(nodes)
    if not roots:
        return "(empty)\n"
    lines: list[str] = []

    def walk(nid: str, prefix: str, seen: frozenset[str]) -> None:
        if nid in seen:
            lines.append(f"{prefix}{nid[:8]}  (cycle)")
            return
        node = by_id[nid]
        lines.append(f"{prefix}{node_label(node)}")
        nxt = seen | {nid}
        for kid in children.get(nid, []):
            walk(kid, prefix + "  ", nxt)

    for rid in roots:
        walk(rid, "", frozenset())
    return "\n".join(lines) + "\n"


class WorkGraphApp(App[None]):
    """Team DAG only. Not packset, not todos, not the vault."""

    CSS_PATH = CSS_FILES
    TITLE = "WorkGraph"
    BINDINGS = [
        Binding("c", "claim", "Claim"),
        Binding("d", "complete", "Done"),
        Binding("u", "unlink", "Unlink"),
        Binding("r", "refresh", "Refresh"),
        Binding("q", "quit", "Quit"),
    ]

    def __init__(self, directory: Path | None = None) -> None:
        super().__init__()
        self.directory = directory or default_dir()
        self.actor = default_actor()

    def compose(self) -> ComposeResult:
        self.sub_title = str(self.directory)
        yield Header()
        yield Tree("work")
        yield Footer()

    def on_mount(self) -> None:
        self.refresh_tree()

    def refresh_tree(self) -> None:
        tree = self.query_one(Tree)
        tree.clear()
        tree.root.expand()
        by_id, children, roots = parent_tree(load_nodes(self.directory))
        if not roots:
            tree.root.add_leaf("(empty)")
            return

        def add(parent_node: Any, nid: str, seen: frozenset[str]) -> None:
            if nid in seen:
                parent_node.add_leaf(f"{nid[:8]}  (cycle)")
                return
            label = node_label(by_id[nid])
            branch = parent_node.add(label, data=nid, expand=True)
            nxt = seen | {nid}
            for kid in children.get(nid, []):
                add(branch, kid, nxt)

        for rid in roots:
            add(tree.root, rid, frozenset())

    def _selected_id(self) -> str | None:
        tree = self.query_one(Tree)
        node = tree.cursor_node
        if node is None or node.data is None:
            return None
        return str(node.data)

    def _run_cli(self, args: list[str]) -> str:
        bin_name = os.environ.get("CLAIMDAG_BIN", "claimdag")
        exe = shutil.which(bin_name)
        if not exe:
            return f"{bin_name} not on PATH"
        cmd = [exe, "--dir", str(self.directory), *args]
        try:
            proc = subprocess.run(cmd, check=False, capture_output=True, text=True)
        except OSError as exc:
            return str(exc)
        if proc.returncode != 0:
            err = (proc.stderr or proc.stdout or "").strip()
            return err or f"exit {proc.returncode}"
        return ""

    def action_refresh(self) -> None:
        self.refresh_tree()

    def action_claim(self) -> None:
        nid = self._selected_id()
        if not nid:
            self.notify("select a node")
            return
        err = self._run_cli(["claim", nid, "--assignee", self.actor])
        self.notify(err or "claimed")
        self.refresh_tree()

    def action_complete(self) -> None:
        nid = self._selected_id()
        if not nid:
            self.notify("select a node")
            return
        err = self._run_cli(
            ["complete", nid, "--status", "done", "--actor", self.actor]
        )
        self.notify(err or "done")
        self.refresh_tree()

    def action_unlink(self) -> None:
        nid = self._selected_id()
        if not nid:
            self.notify("select a node")
            return
        nodes = {node_id(n): n for n in load_nodes(self.directory)}
        node = nodes.get(nid) or {}
        deps = node.get("deps") or []
        if not isinstance(deps, list) or not deps:
            self.notify("no edge")
            return
        err = self._run_cli(["unlink", str(deps[0]), nid, "--actor", self.actor])
        self.notify(err or "unlinked")
        self.refresh_tree()


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(prog="claimdag-tui")
    parser.add_argument(
        "--dir",
        type=Path,
        default=None,
        help="Directory that holds work.json (else CLAIMDAG_DIR or XDG_RUNTIME_DIR/claimdag)",
    )
    parser.add_argument(
        "--dump",
        action="store_true",
        help="Print the parent tree as text and exit",
    )
    args = parser.parse_args(argv)
    directory = args.dir or default_dir()
    if args.dump:
        sys.stdout.write(dump_forest(load_nodes(directory)))
        return 0
    WorkGraphApp(directory=directory).run()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
