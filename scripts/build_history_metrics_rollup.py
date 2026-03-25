#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import math
from dataclasses import dataclass
from datetime import datetime
from html import escape
from pathlib import Path
from typing import Any
from zoneinfo import ZoneInfo


REPO_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_CODEX_ROOT = Path.home() / ".codex"
DEFAULT_CODEX_CWD_PREFIXES = [
    str(REPO_ROOT),
    str(REPO_ROOT.parent / "diet-python"),
]
DEFAULT_TIMEZONE = "America/Los_Angeles"
DEFAULT_HTML_OUTPUT = REPO_ROOT / "web" / "history_metrics.html"
DEFAULT_HTML_TEMPLATE = REPO_ROOT / "web" / "history_metrics_template.html"
SVG_WIDTH = 1200
SVG_HEIGHT = 360
SVG_MARGIN_LEFT = 78
SVG_MARGIN_RIGHT = 24
SVG_MARGIN_TOP = 78
SVG_MARGIN_BOTTOM = 52
SVG_PLOT_WIDTH = SVG_WIDTH - SVG_MARGIN_LEFT - SVG_MARGIN_RIGHT
SVG_PLOT_HEIGHT = SVG_HEIGHT - SVG_MARGIN_TOP - SVG_MARGIN_BOTTOM
COLOR_CODE = "#7fd4ff"
COLOR_TESTS = "#ffd166"
COLOR_CHURN = "#ff8c69"
COLOR_TOKENS_IN = "#5bd6a0"
COLOR_TOKENS_OUT = "#ff6f91"


@dataclass(frozen=True)
class DailyTokenTotals:
    date: str
    input_tokens: int
    output_tokens: int


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Read per-commit history metrics JSONL, build a daily rollup, and emit "
            "a static HTML report plus SVG chart assets."
        )
    )
    parser.add_argument(
        "history_jsonl",
        help="Path to the per-commit history JSONL produced by collect_warloc_history.py",
    )
    parser.add_argument(
        "daily_output",
        help="Path to the daily rollup JSONL file",
    )
    parser.add_argument(
        "--html-output",
        "--web-output",
        dest="html_output",
        default=str(DEFAULT_HTML_OUTPUT),
        help=f"Path to the static HTML report (default: {DEFAULT_HTML_OUTPUT})",
    )
    parser.add_argument(
        "--html-template",
        default=str(DEFAULT_HTML_TEMPLATE),
        help=f"Path to the static HTML template (default: {DEFAULT_HTML_TEMPLATE})",
    )
    parser.add_argument(
        "--codex-root",
        default=str(DEFAULT_CODEX_ROOT),
        help=f"Root directory for Codex logs (default: {DEFAULT_CODEX_ROOT})",
    )
    parser.add_argument(
        "--timezone",
        default=DEFAULT_TIMEZONE,
        help=f"IANA timezone name for daily grouping (default: {DEFAULT_TIMEZONE})",
    )
    parser.add_argument(
        "--codex-cwd-prefix",
        action="append",
        dest="codex_cwd_prefixes",
        help="Only count Codex sessions whose cwd starts with this path; repeat to add multiple prefixes",
    )
    args = parser.parse_args()
    if args.codex_cwd_prefixes is None:
        args.codex_cwd_prefixes = list(DEFAULT_CODEX_CWD_PREFIXES)
    return args


def parse_timestamp(value: str) -> datetime:
    return datetime.fromisoformat(value.replace("Z", "+00:00"))


def local_day(value: str, timezone: ZoneInfo) -> str:
    return parse_timestamp(value).astimezone(timezone).date().isoformat()


def load_jsonl(path: Path) -> list[dict[str, Any]]:
    records: list[dict[str, Any]] = []
    with path.open("r", encoding="utf-8") as fh:
        for raw_line in fh:
            line = raw_line.strip()
            if not line:
                continue
            payload = json.loads(line)
            if not isinstance(payload, dict):
                raise RuntimeError(f"expected JSON object in {path}, got {type(payload)!r}")
            records.append(payload)
    return records


def write_jsonl(path: Path, records: list[dict[str, Any]]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", encoding="utf-8") as fh:
        for record in records:
            fh.write(json.dumps(record, sort_keys=True))
            fh.write("\n")


def build_daily_rollup(commit_records: list[dict[str, Any]], timezone: ZoneInfo) -> list[dict[str, Any]]:
    sorted_records = sorted(commit_records, key=lambda record: parse_timestamp(record["timestamp"]))
    per_day: dict[str, dict[str, Any]] = {}
    for record in sorted_records:
        date = local_day(record["timestamp"], timezone)
        day_rollup = per_day.setdefault(
            date,
            {
                "date": date,
                "code_lines": 0,
                "tests_python_total_lines": 0,
                "daily_churn": 0,
            },
        )
        day_rollup["code_lines"] = int(record["code_lines"])
        day_rollup["tests_python_total_lines"] = int(record["tests_python_total_lines"])
        day_rollup["daily_churn"] += int(record["lines_changed"])
    return [per_day[date] for date in sorted(per_day)]


def normalize_cwd_prefixes(cwd_prefixes: list[str]) -> list[str]:
    normalized: list[str] = []
    seen: set[str] = set()
    for prefix in cwd_prefixes:
        normalized_prefix = str(Path(prefix).expanduser().resolve())
        if normalized_prefix in seen:
            continue
        seen.add(normalized_prefix)
        normalized.append(normalized_prefix)
    return normalized


def session_cwd_matches_prefixes(session_cwd: str, repo_cwd_prefixes: list[str]) -> bool:
    if not repo_cwd_prefixes:
        return True
    for prefix in repo_cwd_prefixes:
        if session_cwd == prefix or session_cwd.startswith(f"{prefix}/"):
            return True
    return False


def iter_token_events(session_path: Path, repo_cwd_prefixes: list[str]) -> list[tuple[str, int, int]]:
    cwd_matches = not repo_cwd_prefixes
    previous_input = 0
    previous_output = 0
    token_events: list[tuple[str, int, int]] = []
    with session_path.open("r", encoding="utf-8") as fh:
        for raw_line in fh:
            line = raw_line.strip()
            if not line:
                continue
            payload = json.loads(line)
            if not isinstance(payload, dict):
                continue
            record_type = payload.get("type")
            if record_type == "session_meta":
                session_meta = payload.get("payload", {})
                if not isinstance(session_meta, dict):
                    continue
                session_cwd = session_meta.get("cwd")
                cwd_matches = isinstance(session_cwd, str) and session_cwd_matches_prefixes(
                    session_cwd, repo_cwd_prefixes
                )
                continue
            if not cwd_matches or record_type != "event_msg":
                continue
            event_payload = payload.get("payload", {})
            if not isinstance(event_payload, dict) or event_payload.get("type") != "token_count":
                continue
            info = event_payload.get("info", {})
            if not isinstance(info, dict):
                continue
            usage = info.get("total_token_usage", {})
            if not isinstance(usage, dict):
                continue
            input_tokens = usage.get("input_tokens")
            output_tokens = usage.get("output_tokens")
            timestamp = payload.get("timestamp")
            if not isinstance(input_tokens, int) or not isinstance(output_tokens, int) or not isinstance(timestamp, str):
                continue
            delta_input = input_tokens - previous_input if input_tokens >= previous_input else input_tokens
            delta_output = output_tokens - previous_output if output_tokens >= previous_output else output_tokens
            previous_input = input_tokens
            previous_output = output_tokens
            token_events.append((timestamp, delta_input, delta_output))
    return token_events


def collect_daily_tokens(codex_root: Path, timezone: ZoneInfo, repo_cwd_prefixes: list[str]) -> list[dict[str, Any]]:
    sessions_root = codex_root / "sessions"
    if not sessions_root.is_dir():
        return []

    normalized_prefixes = normalize_cwd_prefixes(repo_cwd_prefixes)
    totals: dict[str, DailyTokenTotals] = {}
    for session_path in sorted(sessions_root.rglob("*.jsonl")):
        for timestamp, delta_input, delta_output in iter_token_events(session_path, normalized_prefixes):
            date = local_day(timestamp, timezone)
            current = totals.get(date)
            if current is None:
                totals[date] = DailyTokenTotals(
                    date=date,
                    input_tokens=delta_input,
                    output_tokens=delta_output,
                )
                continue
            totals[date] = DailyTokenTotals(
                date=date,
                input_tokens=current.input_tokens + delta_input,
                output_tokens=current.output_tokens + delta_output,
            )
    return [
        {
            "date": totals[date].date,
            "input_tokens": totals[date].input_tokens,
            "output_tokens": totals[date].output_tokens,
        }
        for date in sorted(totals)
    ]


def format_number(value: int) -> str:
    return f"{value:,}"


def x_for_index(index: int, count: int) -> float:
    if count <= 1:
        return SVG_MARGIN_LEFT + SVG_PLOT_WIDTH / 2
    return SVG_MARGIN_LEFT + (SVG_PLOT_WIDTH * index) / (count - 1)


def y_for_value(value: float, domain_max: float) -> float:
    return SVG_MARGIN_TOP + SVG_PLOT_HEIGHT - (value / domain_max) * SVG_PLOT_HEIGHT


def label_stride(count: int) -> int:
    return max(1, math.ceil(count / 6))


def svg_style() -> str:
    return """
<style>
.frame { fill: #09131d; }
.title { fill: #edf3fb; font: 800 24px Manrope, 'Segoe UI', sans-serif; }
.subtitle { fill: #a4b7cc; font: 14px Manrope, 'Segoe UI', sans-serif; }
.legend { fill: #a4b7cc; font: 13px Manrope, 'Segoe UI', sans-serif; }
.grid-line { stroke: rgba(186, 208, 233, 0.14); stroke-width: 1; }
.domain { stroke: rgba(186, 208, 233, 0.24); stroke-width: 1; }
.axis-label { fill: #6e8398; font: 11px 'IBM Plex Mono', monospace; }
.empty { fill: #a4b7cc; font: 15px Manrope, 'Segoe UI', sans-serif; }
</style>
""".strip()


def render_empty_chart_svg(title: str, subtitle: str, message: str) -> str:
    return "\n".join(
        [
            f'<svg xmlns="http://www.w3.org/2000/svg" width="{SVG_WIDTH}" height="{SVG_HEIGHT}" viewBox="0 0 {SVG_WIDTH} {SVG_HEIGHT}">',
            svg_style(),
            f'<rect class="frame" x="0" y="0" width="{SVG_WIDTH}" height="{SVG_HEIGHT}" rx="18" />',
            f'<text class="title" x="28" y="34">{escape(title)}</text>',
            f'<text class="subtitle" x="28" y="58">{escape(subtitle)}</text>',
            f'<text class="empty" x="{SVG_WIDTH / 2:.1f}" y="{SVG_HEIGHT / 2:.1f}" text-anchor="middle">{escape(message)}</text>',
            "</svg>",
        ]
    )


def render_legend(entries: list[tuple[str, str]]) -> list[str]:
    legend_parts: list[str] = []
    x = SVG_WIDTH - 230
    y = 30
    for index, (label, color) in enumerate(entries):
        entry_y = y + index * 20
        legend_parts.append(f'<circle cx="{x}" cy="{entry_y}" r="5" fill="{color}" />')
        legend_parts.append(f'<text class="legend" x="{x + 14}" y="{entry_y + 4}">{escape(label)}</text>')
    return legend_parts


def render_line_chart_svg(
    *,
    title: str,
    subtitle: str,
    labels: list[str],
    series: list[dict[str, Any]],
) -> str:
    if not labels or not series or all(not entry["values"] for entry in series):
        return render_empty_chart_svg(title, subtitle, "No data available.")

    all_values = [value for entry in series for value in entry["values"]]
    y_max = max(all_values) if all_values else 0
    domain_max = 1.0 if y_max <= 0 else y_max * 1.08
    parts = [
        f'<svg xmlns="http://www.w3.org/2000/svg" width="{SVG_WIDTH}" height="{SVG_HEIGHT}" viewBox="0 0 {SVG_WIDTH} {SVG_HEIGHT}">',
        svg_style(),
        f'<rect class="frame" x="0" y="0" width="{SVG_WIDTH}" height="{SVG_HEIGHT}" rx="18" />',
        f'<text class="title" x="28" y="34">{escape(title)}</text>',
        f'<text class="subtitle" x="28" y="58">{escape(subtitle)}</text>',
    ]
    parts.extend(render_legend([(entry["name"], entry["color"]) for entry in series]))

    for tick in range(5):
        value = domain_max * tick / 4
        y = y_for_value(value, domain_max)
        parts.append(
            f'<line class="grid-line" x1="{SVG_MARGIN_LEFT}" x2="{SVG_WIDTH - SVG_MARGIN_RIGHT}" y1="{y:.1f}" y2="{y:.1f}" />'
        )
        parts.append(
            f'<text class="axis-label" x="{SVG_MARGIN_LEFT - 10}" y="{y + 4:.1f}" text-anchor="end">{escape(format_number(round(value)))}</text>'
        )

    parts.append(
        f'<line class="domain" x1="{SVG_MARGIN_LEFT}" x2="{SVG_WIDTH - SVG_MARGIN_RIGHT}" y1="{SVG_HEIGHT - SVG_MARGIN_BOTTOM}" y2="{SVG_HEIGHT - SVG_MARGIN_BOTTOM}" />'
    )

    stride = label_stride(len(labels))
    for index, label in enumerate(labels):
        if index % stride != 0 and index != len(labels) - 1:
            continue
        x = x_for_index(index, len(labels))
        anchor = "end" if index == len(labels) - 1 else "middle"
        parts.append(
            f'<text class="axis-label" x="{x:.1f}" y="{SVG_HEIGHT - 18}" text-anchor="{anchor}">{escape(label)}</text>'
        )

    baseline = SVG_HEIGHT - SVG_MARGIN_BOTTOM
    for entry in series:
        points = " ".join(
            f"{x_for_index(index, len(labels)):.1f},{y_for_value(value, domain_max):.1f}"
            for index, value in enumerate(entry["values"])
        )
        if not points:
            continue
        first_x = x_for_index(0, len(labels))
        last_x = x_for_index(len(labels) - 1, len(labels))
        area_points = f"{first_x:.1f},{baseline} {points} {last_x:.1f},{baseline}"
        parts.append(f'<polygon points="{area_points}" fill="{entry["color"]}" opacity="0.1" />')
        parts.append(
            f'<polyline points="{points}" fill="none" stroke="{entry["color"]}" stroke-width="3" stroke-linecap="round" stroke-linejoin="round" />'
        )

    parts.append("</svg>")
    return "\n".join(parts)


def render_bar_chart_svg(*, title: str, subtitle: str, labels: list[str], values: list[int], color: str) -> str:
    if not labels or not values:
        return render_empty_chart_svg(title, subtitle, "No data available.")

    y_max = max(values) if values else 0
    domain_max = 1.0 if y_max <= 0 else y_max * 1.08
    bar_width = SVG_PLOT_WIDTH / max(len(values), 1)
    parts = [
        f'<svg xmlns="http://www.w3.org/2000/svg" width="{SVG_WIDTH}" height="{SVG_HEIGHT}" viewBox="0 0 {SVG_WIDTH} {SVG_HEIGHT}">',
        svg_style(),
        f'<rect class="frame" x="0" y="0" width="{SVG_WIDTH}" height="{SVG_HEIGHT}" rx="18" />',
        f'<text class="title" x="28" y="34">{escape(title)}</text>',
        f'<text class="subtitle" x="28" y="58">{escape(subtitle)}</text>',
    ]
    parts.extend(render_legend([("Lines changed", color)]))

    for tick in range(5):
        value = domain_max * tick / 4
        y = y_for_value(value, domain_max)
        parts.append(
            f'<line class="grid-line" x1="{SVG_MARGIN_LEFT}" x2="{SVG_WIDTH - SVG_MARGIN_RIGHT}" y1="{y:.1f}" y2="{y:.1f}" />'
        )
        parts.append(
            f'<text class="axis-label" x="{SVG_MARGIN_LEFT - 10}" y="{y + 4:.1f}" text-anchor="end">{escape(format_number(round(value)))}</text>'
        )

    parts.append(
        f'<line class="domain" x1="{SVG_MARGIN_LEFT}" x2="{SVG_WIDTH - SVG_MARGIN_RIGHT}" y1="{SVG_HEIGHT - SVG_MARGIN_BOTTOM}" y2="{SVG_HEIGHT - SVG_MARGIN_BOTTOM}" />'
    )

    stride = label_stride(len(labels))
    for index, label in enumerate(labels):
        if index % stride != 0 and index != len(labels) - 1:
            continue
        x = SVG_MARGIN_LEFT + index * bar_width + bar_width / 2
        anchor = "end" if index == len(labels) - 1 else "middle"
        parts.append(
            f'<text class="axis-label" x="{x:.1f}" y="{SVG_HEIGHT - 18}" text-anchor="{anchor}">{escape(label)}</text>'
        )

    baseline = SVG_HEIGHT - SVG_MARGIN_BOTTOM
    for index, value in enumerate(values):
        x = SVG_MARGIN_LEFT + index * bar_width + bar_width * 0.16
        y = y_for_value(value, domain_max)
        parts.append(
            f'<rect x="{x:.1f}" y="{y:.1f}" width="{max(2.0, bar_width * 0.68):.1f}" height="{max(0.0, baseline - y):.1f}" rx="4" fill="{color}" opacity="0.82" />'
        )

    parts.append("</svg>")
    return "\n".join(parts)


def build_summary_replacements(
    *,
    generated_at: str,
    timezone_name: str,
    history_path: Path,
    codex_root: Path,
    repo_cwd_prefixes: list[str],
    daily_rollup: list[dict[str, Any]],
    daily_tokens: list[dict[str, Any]],
    loc_chart_name: str,
    churn_chart_name: str,
    tokens_chart_name: str,
) -> dict[str, str]:
    latest_rollup = daily_rollup[-1] if daily_rollup else None
    total_churn = sum(int(item["daily_churn"]) for item in daily_rollup)
    total_input_tokens = sum(int(item["input_tokens"]) for item in daily_tokens)
    total_output_tokens = sum(int(item["output_tokens"]) for item in daily_tokens)
    token_scope = ", ".join(repo_cwd_prefixes)
    return {
        "__GENERATED_AT__": escape(generated_at),
        "__TIMEZONE__": escape(timezone_name),
        "__HISTORY_JSONL__": escape(str(history_path)),
        "__CODEX_ROOT__": escape(str(codex_root)),
        "__CODEX_CWD_PREFIXES__": escape(token_scope),
        "__SUMMARY_CODE__": format_number(int(latest_rollup["code_lines"])) if latest_rollup else "-",
        "__SUMMARY_CODE_NOTE__": escape(f"As of {latest_rollup['date']}") if latest_rollup else "No daily LOC records.",
        "__SUMMARY_TESTS__": format_number(int(latest_rollup["tests_python_total_lines"])) if latest_rollup else "-",
        "__SUMMARY_TESTS_NOTE__": escape(f"As of {latest_rollup['date']}") if latest_rollup else "No daily test records.",
        "__SUMMARY_CHURN__": format_number(total_churn),
        "__SUMMARY_CHURN_NOTE__": escape(f"{len(daily_rollup)} daily buckets in {timezone_name}") if daily_rollup else "No daily churn records.",
        "__SUMMARY_TOKENS__": f"{format_number(total_input_tokens)} / {format_number(total_output_tokens)}",
        "__SUMMARY_TOKENS_NOTE__": "Input / output tokens" if daily_tokens else "No token usage records.",
        "__LOC_CHART__": escape(loc_chart_name),
        "__CHURN_CHART__": escape(churn_chart_name),
        "__TOKENS_CHART__": escape(tokens_chart_name),
    }


def render_html_from_template(template_text: str, replacements: dict[str, str]) -> str:
    rendered = template_text
    for needle, replacement in replacements.items():
        rendered = rendered.replace(needle, replacement)
    return rendered


def write_static_report(
    *,
    html_output_path: Path,
    template_path: Path,
    generated_at: str,
    timezone_name: str,
    history_path: Path,
    codex_root: Path,
    repo_cwd_prefixes: list[str],
    daily_rollup: list[dict[str, Any]],
    daily_tokens: list[dict[str, Any]],
) -> None:
    html_output_path.parent.mkdir(parents=True, exist_ok=True)
    loc_chart_path = html_output_path.with_name(f"{html_output_path.stem}_loc.svg")
    churn_chart_path = html_output_path.with_name(f"{html_output_path.stem}_churn.svg")
    tokens_chart_path = html_output_path.with_name(f"{html_output_path.stem}_tokens.svg")

    loc_chart_path.write_text(
        render_line_chart_svg(
            title="End-of-Day LOC",
            subtitle="Repository code LOC plus top-level __dp__.py, and total Python LOC under tests/.",
            labels=[entry["date"] for entry in daily_rollup],
            series=[
                {
                    "name": "Code LOC",
                    "color": COLOR_CODE,
                    "values": [int(entry["code_lines"]) for entry in daily_rollup],
                },
                {
                    "name": "Test LOC",
                    "color": COLOR_TESTS,
                    "values": [int(entry["tests_python_total_lines"]) for entry in daily_rollup],
                },
            ],
        )
        + "\n",
        encoding="utf-8",
    )
    churn_chart_path.write_text(
        render_bar_chart_svg(
            title="Daily Churn",
            subtitle="Sum of insertions and deletions across commits that landed on each day.",
            labels=[entry["date"] for entry in daily_rollup],
            values=[int(entry["daily_churn"]) for entry in daily_rollup],
            color=COLOR_CHURN,
        )
        + "\n",
        encoding="utf-8",
    )
    tokens_chart_path.write_text(
        render_line_chart_svg(
            title="Daily Codex Token Usage",
            subtitle="Repo-local Codex input and output token totals aggregated by day.",
            labels=[entry["date"] for entry in daily_tokens],
            series=[
                {
                    "name": "Input tokens",
                    "color": COLOR_TOKENS_IN,
                    "values": [int(entry["input_tokens"]) for entry in daily_tokens],
                },
                {
                    "name": "Output tokens",
                    "color": COLOR_TOKENS_OUT,
                    "values": [int(entry["output_tokens"]) for entry in daily_tokens],
                },
            ],
        )
        + "\n",
        encoding="utf-8",
    )

    template_text = template_path.read_text(encoding="utf-8")
    html_output_path.write_text(
        render_html_from_template(
            template_text,
            build_summary_replacements(
                generated_at=generated_at,
                timezone_name=timezone_name,
                history_path=history_path,
                codex_root=codex_root,
                repo_cwd_prefixes=repo_cwd_prefixes,
                daily_rollup=daily_rollup,
                daily_tokens=daily_tokens,
                loc_chart_name=loc_chart_path.name,
                churn_chart_name=churn_chart_path.name,
                tokens_chart_name=tokens_chart_path.name,
            ),
        )
        + "\n",
        encoding="utf-8",
    )


def main() -> int:
    args = parse_args()
    history_path = Path(args.history_jsonl).resolve()
    daily_output_path = Path(args.daily_output).resolve()
    html_output_path = Path(args.html_output).resolve()
    template_path = Path(args.html_template).resolve()
    codex_root = Path(args.codex_root).expanduser().resolve()
    timezone = ZoneInfo(args.timezone)
    codex_cwd_prefixes = normalize_cwd_prefixes(args.codex_cwd_prefixes)
    commit_records = load_jsonl(history_path)
    daily_rollup = build_daily_rollup(commit_records, timezone)
    daily_tokens = collect_daily_tokens(codex_root, timezone, codex_cwd_prefixes)
    generated_at = datetime.now().astimezone().isoformat()
    write_jsonl(daily_output_path, daily_rollup)
    write_static_report(
        html_output_path=html_output_path,
        template_path=template_path,
        generated_at=generated_at,
        timezone_name=args.timezone,
        history_path=history_path,
        codex_root=codex_root,
        repo_cwd_prefixes=codex_cwd_prefixes,
        daily_rollup=daily_rollup,
        daily_tokens=daily_tokens,
    )
    print(f"wrote {len(daily_rollup)} daily rollup rows to {daily_output_path}")
    print(f"wrote static history report to {html_output_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
