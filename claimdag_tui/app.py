"""WorkGraph pane: Textual Tree over claimdag work.json.

Mutations go through the claimdag CLI. This process does not reimplement
claim/complete/link.
"""

from __future__ import annotations

import json
import os
import shutil
import subprocess
from pathlib import Path

from textual.app import App, ComposeResult
from textual.binding import Binding
from textual.widgets import Footer, Header, Tree

ZERO = "0" * 32


def default_dir() -> Path:
    raw = os.environ.get("CLAIMDAG_DIR")
    if raw:
        return Path(raw)
    xdg = os.environ.get("XDG_RUNTIME_DIR", "/tmp")
    return Path(xdg) / "claimdag"


def load_nodes(directory: Path) -> list[dict]:
    path = directory / "work.json"
    if not path.is_file():
        return []
    data = json.loads(path.read_text())
    nodes = data.get("nodes") or []
    return [n for n in nodes if isinstance(n, dict)]


class WorkGraphApp(App[None]):
    """Team DAG only. Not packset, not todos, not the vault."""

    CSS_PATH = Path(__file__).with_name("storm.tcss")
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
        self.actor = os.environ.get("CLAIMDAG_ACTOR", ZERO)

    def compose(self) -> ComposeResult:
        yield Header()
        yield Tree("work")
        yield Footer()

    def on_mount(self) -> None:
        self.refresh_tree()

    def refresh_tree(self) -> None:
        tree = self.query_one(Tree)
        tree.clear()
        tree.root.expand()
        nodes = load_nodes(self.directory)
        by_id = {str(n.get("id") or ""): n for n in nodes}
        children: dict[str, list[str]] = {k: [] for k in by_id}
        roots: list[str] = []
        for nid, n in by_id.items():
            parent = str(n.get("parent") or ZERO)
            if parent in by_id and parent != ZERO:
                children[parent].append(nid)
            else:
                roots.append(nid)
        if not roots:
            tree.root.add_leaf("(empty)")
            return
        def add(parent_node, nid: str) -> None:
            n = by_id[nid]
            label = f"{n.get('status', '?')}  {n.get('summary') or nid[:8]}"
            branch = parent_node.add(label, data=nid, expand=True)
            for kid in children.get(nid, []):
                add(branch, kid)
        for rid in roots:
            add(tree.root, rid)

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
        err = (proc.stderr or proc.stdout or "").strip()
        if proc.returncode != 0:
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
        nodes = {str(n.get("id") or ""): n for n in load_nodes(self.directory)}
        n = nodes.get(nid) or {}
        deps = n.get("deps") or []
        if not deps:
            self.notify("no edge")
            return
        err = self._run_cli(["unlink", str(deps[0]), nid, "--actor", self.actor])
        self.notify(err or "unlinked")
        self.refresh_tree()


def main() -> None:
    WorkGraphApp().run()


if __name__ == "__main__":
    main()
