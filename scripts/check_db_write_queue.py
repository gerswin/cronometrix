#!/usr/bin/env python3
"""Reject raw libSQL writer identifiers outside reviewed infrastructure.

This is intentionally a conservative lexical boundary, not a complete Rust
parser. Outside comments, literals, test-only cfg items, and the exact
allowlist, every ``execute``, ``execute_batch``, or ``transaction`` identifier
is rejected. That also covers turbofish calls, UFCS/method references, and
identifiers passed to macros. Token concatenation performed inside an external
or procedural macro cannot be inferred here and remains outside this check.
"""

from __future__ import annotations

import argparse
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable, Sequence


ALLOWLIST = frozenset(
    {
        "backend/src/db/mod.rs",
        "backend/src/db/write_queue.rs",
        "backend/src/bin/seed_e2e.rs",
        "backend/src/test_reset/mod.rs",
    }
)
RAW_WRITE_METHODS = frozenset({"execute", "execute_batch", "transaction"})


@dataclass(frozen=True)
class Token:
    text: str
    start: int
    end: int
    line: int


@dataclass(frozen=True)
class Violation:
    path: Path
    line: int
    method: str


def _skip_quoted(source: str, start: int, quote: str) -> int:
    index = start + 1
    while index < len(source):
        if source[index] == "\\":
            index += 2
            continue
        if source[index] == quote:
            return index + 1
        index += 1
    return len(source)


def _char_literal_end(source: str, start: int) -> int | None:
    """Return the end of one Rust char literal, or None for a lifetime/label."""

    index = start + 1
    if index >= len(source) or source[index] in {"'", "\n", "\r"}:
        return None
    if source[index] == "\\":
        index += 1
        if index >= len(source):
            return None
        if source[index] == "x":
            index += 3  # `x` plus exactly two hexadecimal digits.
        elif source[index] == "u" and source.startswith("u{", index):
            close = source.find("}", index + 2)
            if close < 0:
                return None
            index = close + 1
        else:
            index += 1
    else:
        index += 1
    if index < len(source) and source[index] == "'":
        return index + 1
    return None


def _raw_string_end(source: str, start: int) -> int | None:
    index = start
    if source.startswith("br", index) or source.startswith("cr", index):
        index += 1
    if index >= len(source) or source[index] != "r":
        return None
    index += 1
    hashes = 0
    while index < len(source) and source[index] == "#":
        hashes += 1
        index += 1
    if index >= len(source) or source[index] != '"':
        return None
    terminator = '"' + ("#" * hashes)
    end = source.find(terminator, index + 1)
    return len(source) if end < 0 else end + len(terminator)


def _tokens(source: str) -> list[Token]:
    """Lex enough Rust to distinguish code from comments and string literals."""

    tokens: list[Token] = []
    index = 0
    line = 1
    while index < len(source):
        char = source[index]
        if char == "\n":
            line += 1
            index += 1
            continue
        if char.isspace():
            index += 1
            continue
        if source.startswith("//", index):
            end = source.find("\n", index + 2)
            index = len(source) if end < 0 else end
            continue
        if source.startswith("/*", index):
            depth = 1
            cursor = index + 2
            while cursor < len(source) and depth:
                if source.startswith("/*", cursor):
                    depth += 1
                    cursor += 2
                elif source.startswith("*/", cursor):
                    depth -= 1
                    cursor += 2
                else:
                    if source[cursor] == "\n":
                        line += 1
                    cursor += 1
            index = cursor
            continue

        raw_end = _raw_string_end(source, index)
        if raw_end is not None:
            line += source.count("\n", index, raw_end)
            index = raw_end
            continue
        if char == '"' or (
            char in {"b", "c"}
            and index + 1 < len(source)
            and source[index + 1] == '"'
        ):
            quote_start = index if char == '"' else index + 1
            end = _skip_quoted(source, quote_start, '"')
            line += source.count("\n", index, end)
            index = end
            continue
        if char == "'":
            end = _char_literal_end(source, index)
            if end is not None:
                line += source.count("\n", index, end)
                index = end
                continue

        if char.isalpha() or char == "_":
            end = index + 1
            while end < len(source) and (source[end].isalnum() or source[end] == "_"):
                end += 1
            tokens.append(Token(source[index:end], index, end, line))
            index = end
            continue

        tokens.append(Token(char, index, index + 1, line))
        index += 1
    return tokens


class _CfgExpression:
    """Evaluate whether a cfg expression can be true while `test` is false."""

    def __init__(self, tokens: Sequence[Token]) -> None:
        self.tokens = tokens
        self.index = 0

    def parse(self) -> tuple[bool, bool]:
        if self.index >= len(self.tokens):
            return True, True
        name = self.tokens[self.index].text
        self.index += 1
        if self.index < len(self.tokens) and self.tokens[self.index].text == "(":
            self.index += 1
            children: list[tuple[bool, bool]] = []
            while self.index < len(self.tokens) and self.tokens[self.index].text != ")":
                children.append(self.parse())
                while (
                    self.index < len(self.tokens)
                    and self.tokens[self.index].text not in {",", ")"}
                ):
                    self.index += 1
                if self.index < len(self.tokens) and self.tokens[self.index].text == ",":
                    self.index += 1
            if self.index < len(self.tokens) and self.tokens[self.index].text == ")":
                self.index += 1
            if name == "all":
                return all(child[0] for child in children), any(
                    child[1] for child in children
                )
            if name == "any":
                return any(child[0] for child in children), all(
                    child[1] for child in children
                )
            if name == "not" and len(children) == 1:
                return children[0][1], children[0][0]
            return True, True
        while self.index < len(self.tokens) and self.tokens[self.index].text not in {
            ",",
            ")",
        }:
            self.index += 1
        return (False, True) if name == "test" else (True, True)


def _matching(tokens: Sequence[Token], start: int, opener: str, closer: str) -> int:
    depth = 0
    for index in range(start, len(tokens)):
        if tokens[index].text == opener:
            depth += 1
        elif tokens[index].text == closer:
            depth -= 1
            if depth == 0:
                return index
    return len(tokens) - 1


def _is_test_only_cfg(attribute: Sequence[Token]) -> bool:
    # attribute includes `# [ ... ]`; only cfg(...), never cfg_attr(...), gates it.
    inner = list(attribute[2:-1])
    if len(inner) < 3 or inner[0].text != "cfg" or inner[1].text != "(":
        return False
    close = _matching(inner, 1, "(", ")")
    can_be_true_without_test, _ = _CfgExpression(inner[2:close]).parse()
    return not can_be_true_without_test


def _item_end(tokens: Sequence[Token], start: int) -> int:
    parens = 0
    brackets = 0
    index = start
    while index < len(tokens):
        text = tokens[index].text
        if text == "(":
            parens += 1
        elif text == ")":
            parens = max(0, parens - 1)
        elif text == "[":
            brackets += 1
        elif text == "]":
            brackets = max(0, brackets - 1)
        elif text == ";" and parens == 0 and brackets == 0:
            return index
        elif text == "{" and parens == 0 and brackets == 0:
            return _matching(tokens, index, "{", "}")
        index += 1
    return len(tokens) - 1


def _test_only_ranges(tokens: Sequence[Token]) -> list[tuple[int, int]]:
    ranges: list[tuple[int, int]] = []
    index = 0
    while index + 1 < len(tokens):
        if tokens[index].text != "#" or tokens[index + 1].text != "[":
            index += 1
            continue
        attribute_end = _matching(tokens, index + 1, "[", "]")
        if _is_test_only_cfg(tokens[index : attribute_end + 1]):
            item_start = attribute_end + 1
            item_end = _item_end(tokens, item_start)
            if item_start < len(tokens):
                ranges.append((tokens[index].start, tokens[item_end].end))
            index = item_end + 1
        else:
            index = attribute_end + 1
    return ranges


def _inside(position: int, ranges: Sequence[tuple[int, int]]) -> bool:
    return any(start <= position < end for start, end in ranges)


def _relative_path(path: Path, repo_root: Path) -> Path:
    resolved = path.resolve()
    try:
        return resolved.relative_to(repo_root.resolve())
    except ValueError:
        return resolved


def _scan_file(path: Path, repo_root: Path) -> list[Violation]:
    relative = _relative_path(path, repo_root)
    if relative.as_posix() in ALLOWLIST:
        return []
    source = path.read_text(encoding="utf-8")
    tokens = _tokens(source)
    ignored = _test_only_ranges(tokens)
    violations: list[Violation] = []
    for token in tokens:
        if token.text not in RAW_WRITE_METHODS or _inside(token.start, ignored):
            continue
        violations.append(Violation(relative, token.line, token.text))
    return violations


def _rust_files(target: Path) -> Iterable[Path]:
    if target.is_file():
        if target.suffix == ".rs":
            yield target
        return
    yield from sorted(target.rglob("*.rs"), key=lambda path: path.as_posix())


def scan_path(target: Path | str, repo_root: Path | str) -> list[Violation]:
    target_path = Path(target)
    root_path = Path(repo_root)
    violations: list[Violation] = []
    for path in _rust_files(target_path):
        violations.extend(_scan_file(path, root_path))
    return violations


def _parser() -> argparse.ArgumentParser:
    script_repo_root = Path(__file__).resolve().parents[1]
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--repo-root",
        type=Path,
        default=script_repo_root,
        help="repository root used for exact allowlist matching",
    )
    parser.add_argument(
        "target",
        nargs="?",
        type=Path,
        default=Path("backend/src"),
        help="Rust source file or directory (default: backend/src)",
    )
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    args = _parser().parse_args(argv)
    target = args.target if args.target.is_absolute() else args.repo_root / args.target
    if not target.exists():
        print(f"DB write queue boundary: target does not exist: {target}", file=sys.stderr)
        return 2
    violations = scan_path(target, args.repo_root)
    for violation in violations:
        print(
            f"{violation.path.as_posix()}:{violation.line}: forbidden raw write "
            f"identifier '{violation.method}' "
            "bypasses state.db_write; use DbWriteQueue"
        )
    if violations:
        print(f"DB write queue boundary: FAIL ({len(violations)} violations)")
        return 1
    print("DB write queue boundary: PASS (0 violations)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
