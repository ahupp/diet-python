#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import uuid
from dataclasses import dataclass
from pathlib import Path
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[1]
DIET_PYTHON_RUNTIME = "__dp__.py"
PROJECT_TESTS_DIR = "tests"
VENDOR_DIR = "vendor"
NON_VENDOR_FILESET = "~vendor"
WARLOC_COUNT_KEYS = ("file_count", "code_lines", "test_lines", "blank_lines", "comment_lines")
JJ_STAT_SUMMARY_RE = re.compile(
    r"^\d+\s+files?\s+changed"
    r"(?:,\s+(?P<insertions>\d+)\s+insertions?\(\+\))?"
    r"(?:,\s+(?P<deletions>\d+)\s+deletions?\(-\))?$"
)


@dataclass(frozen=True)
class CommitMetadata:
    commit_id: str
    change_id: str
    timestamp: str
    description: str


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Walk visible jj history, run `cargo warloc --jsonl` at each commit, "
            "and write one merged JSON object per commit."
        )
    )
    parser.add_argument(
        "output",
        help="Path to the output JSONL file",
    )
    parser.add_argument(
        "--revset",
        default="..",
        help=(
            "jj revset to walk, oldest to newest "
            "(default: '..', i.e. all visible commits except root())"
        ),
    )
    return parser.parse_args()


def run(
    cmd: list[str],
    *,
    cwd: Path,
    capture_output: bool = False,
) -> subprocess.CompletedProcess[str]:
    proc = subprocess.run(
        cmd,
        cwd=cwd,
        capture_output=capture_output,
        text=True,
    )
    if proc.returncode == 0:
        return proc

    detail_parts = [f"command failed ({proc.returncode}): {' '.join(cmd)}"]
    if proc.stdout:
        detail_parts.append(f"stdout:\n{proc.stdout}")
    if proc.stderr:
        detail_parts.append(f"stderr:\n{proc.stderr}")
    raise RuntimeError("\n".join(detail_parts))


def jj_cmd(*args: str, ignore_working_copy: bool = False) -> list[str]:
    cmd = ["jj", "--color=never", "--no-pager"]
    if ignore_working_copy:
        cmd.append("--ignore-working-copy")
    cmd.extend(args)
    return cmd


def list_commits(revset: str) -> list[CommitMetadata]:
    proc = run(
        jj_cmd(
            "log",
            "-r",
            revset,
            "--reversed",
            "--no-graph",
            "-T",
            'json(self) ++ "\\n"',
            ignore_working_copy=True,
        ),
        cwd=REPO_ROOT,
        capture_output=True,
    )
    commits: list[CommitMetadata] = []
    for raw_line in proc.stdout.splitlines():
        line = raw_line.strip()
        if not line:
            continue
        payload = json.loads(line)
        commits.append(
            CommitMetadata(
                commit_id=payload["commit_id"],
                change_id=payload["change_id"],
                timestamp=payload["author"]["timestamp"],
                description=payload["description"].rstrip("\n"),
            )
        )
    if not commits:
        raise RuntimeError(f"revset matched no commits: {revset}")
    return commits


def create_workspace() -> tuple[str, Path, Path]:
    workspace_parent = Path(tempfile.mkdtemp(prefix="warloc-history-"))
    workspace_root = workspace_parent / "workspace"
    workspace_name = f"warloc-history-{os.getpid()}-{uuid.uuid4().hex[:8]}"
    run(
        jj_cmd(
            "workspace",
            "add",
            "--name",
            workspace_name,
            "--sparse-patterns",
            "full",
            "--revision",
            "root()",
            str(workspace_root),
            ignore_working_copy=True,
        ),
        cwd=REPO_ROOT,
        capture_output=True,
    )
    return workspace_name, workspace_parent, workspace_root


def forget_workspace(workspace_name: str) -> None:
    run(
        jj_cmd(
            "workspace",
            "forget",
            workspace_name,
            ignore_working_copy=True,
        ),
        cwd=REPO_ROOT,
        capture_output=True,
    )


def update_stale_workspace(workspace_root: Path) -> None:
    run(
        jj_cmd("workspace", "update-stale"),
        cwd=workspace_root,
        capture_output=True,
    )


def restore_workspace_from_commit(workspace_root: Path, commit_id: str) -> None:
    run(
        jj_cmd("restore", "--from", commit_id),
        cwd=workspace_root,
        capture_output=True,
    )


def is_vendor_path(path: str) -> bool:
    normalized = path.removeprefix("./")
    return normalized == VENDOR_DIR or normalized.startswith(f"{VENDOR_DIR}/")


def warloc_total_from_by_file_jsonl(output: str) -> dict[str, Any]:
    totals: dict[str, Any] = {"scope": "total"}
    for key in WARLOC_COUNT_KEYS:
        totals[key] = 0

    saw_file_record = False
    for raw_line in output.splitlines():
        line = raw_line.strip()
        if not line:
            continue
        payload = json.loads(line)
        if not isinstance(payload, dict):
            raise RuntimeError(f"expected JSON object from `cargo warloc --jsonl --by-file`, got {type(payload)!r}")
        scope = payload.get("scope")
        if scope == "total":
            continue
        if scope != "file":
            raise RuntimeError(f"expected file-scoped JSON object from `cargo warloc --jsonl --by-file`, got {payload!r}")
        saw_file_record = True
        file_path = payload.get("file")
        if not isinstance(file_path, str):
            raise RuntimeError(f"expected string file path from `cargo warloc --jsonl --by-file`, got {file_path!r}")
        if is_vendor_path(file_path):
            continue
        for key in WARLOC_COUNT_KEYS:
            value = payload.get(key)
            if not isinstance(value, int):
                raise RuntimeError(f"expected integer {key} from `cargo warloc --jsonl --by-file`, got {value!r}")
            totals[key] += value

    if not saw_file_record:
        raise RuntimeError("expected at least one file-scoped JSONL line from `cargo warloc --jsonl --by-file`")
    return totals


def run_warloc(workspace_root: Path) -> dict[str, Any]:
    proc = run(
        ["cargo", "warloc", "--jsonl", "--by-file"],
        cwd=workspace_root,
        capture_output=True,
    )
    return warloc_total_from_by_file_jsonl(proc.stdout)


def count_lines(path: Path) -> int:
    if not path.is_file():
        return 0
    with path.open("r", encoding="utf-8", errors="surrogateescape") as fh:
        return sum(1 for _ in fh)


def count_python_lines_under(root: Path) -> int:
    if not root.is_dir():
        return 0
    return sum(count_lines(path) for path in sorted(root.rglob("*.py")) if path.is_file())


def parse_lines_changed_from_stat(stat_output: str) -> int:
    for raw_line in reversed(stat_output.splitlines()):
        line = raw_line.strip()
        if not line:
            continue
        match = JJ_STAT_SUMMARY_RE.match(line)
        if match is None:
            continue
        insertions = int(match.group("insertions") or 0)
        deletions = int(match.group("deletions") or 0)
        return insertions + deletions
    raise RuntimeError(f"failed to parse jj stat summary from output:\n{stat_output}")


def lines_changed_for_commit(commit_id: str) -> int:
    proc = run(
        jj_cmd("diff", "-r", commit_id, "--stat", NON_VENDOR_FILESET, ignore_working_copy=True),
        cwd=REPO_ROOT,
        capture_output=True,
    )
    return parse_lines_changed_from_stat(proc.stdout)


def collect_commit_record(commit: CommitMetadata, workspace_root: Path) -> dict[str, Any]:
    warloc = run_warloc(workspace_root)
    warloc_code_lines = warloc.get("code_lines")
    if not isinstance(warloc_code_lines, int):
        raise RuntimeError(f"expected integer code_lines from warloc, got {warloc_code_lines!r}")
    runtime_lines = count_lines(workspace_root / DIET_PYTHON_RUNTIME)
    tests_python_total_lines = count_python_lines_under(workspace_root / PROJECT_TESTS_DIR)
    return {
        "commit_id": commit.commit_id,
        "change_id": commit.change_id,
        "timestamp": commit.timestamp,
        "commit_description": commit.description,
        **warloc,
        "warloc_code_lines": warloc_code_lines,
        "dp_runtime_lines": runtime_lines,
        "code_lines": warloc_code_lines + runtime_lines,
        "tests_python_total_lines": tests_python_total_lines,
        "lines_changed": lines_changed_for_commit(commit.commit_id),
    }


def collect_history(output_path: Path, revset: str) -> None:
    commits = list_commits(revset)
    output_path.parent.mkdir(parents=True, exist_ok=True)

    tmp_output_fd, tmp_output_name = tempfile.mkstemp(
        prefix=f"{output_path.name}.",
        suffix=".tmp",
        dir=output_path.parent,
    )
    os.close(tmp_output_fd)
    tmp_output_path = Path(tmp_output_name)

    workspace_name: str | None = None
    workspace_parent: Path | None = None
    workspace_root: Path | None = None
    try:
        workspace_name, workspace_parent, workspace_root = create_workspace()
        update_stale_workspace(workspace_root)
        with tmp_output_path.open("w", encoding="utf-8") as fh:
            total = len(commits)
            for index, commit in enumerate(commits, start=1):
                print(
                    f"[{index}/{total}] {commit.commit_id[:12]} {commit.change_id}",
                    file=sys.stderr,
                )
                restore_workspace_from_commit(workspace_root, commit.commit_id)
                merged = collect_commit_record(commit, workspace_root)
                fh.write(json.dumps(merged, sort_keys=True))
                fh.write("\n")
        os.replace(tmp_output_path, output_path)
        print(f"wrote {len(commits)} JSONL records to {output_path}", file=sys.stderr)
    finally:
        if tmp_output_path.exists():
            tmp_output_path.unlink()
        if workspace_name is not None:
            try:
                forget_workspace(workspace_name)
            except Exception as exc:
                print(f"warning: failed to forget temp workspace {workspace_name}: {exc}", file=sys.stderr)
        if workspace_parent is not None:
            shutil.rmtree(workspace_parent, ignore_errors=True)


def main() -> int:
    args = parse_args()
    output_path = Path(args.output).resolve()
    collect_history(output_path, args.revset)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
