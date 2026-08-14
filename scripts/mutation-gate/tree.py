"""Scratch copies of the tracked tree, and the digest that proves it untouched.

Copying is a plain byte copy of the paths `git ls-files` names, never
`git archive` or a checkout: an export applies the eol filters, and a CRLF
rewrite would silently change what the mutated self-test reads.
"""

import hashlib
import pathlib
import shutil
import subprocess


def _git(*args, cwd):
    result = subprocess.run(
        ["git", *args], cwd=str(cwd), capture_output=True, text=True, check=True
    )
    return result.stdout


def tracked_paths(root):
    out = _git("ls-files", "-z", cwd=root)
    return sorted(part for part in out.split("\0") if part)


def digest(root):
    """A single hash over every tracked path, its bytes, and any dirty state."""
    total = hashlib.sha256()
    for rel in tracked_paths(root):
        source = pathlib.Path(root) / rel
        total.update(rel.encode("utf-8"))
        total.update(b"\0")
        if source.is_file():
            total.update(hashlib.sha256(source.read_bytes()).digest())
        else:
            total.update(b"<absent>")
    total.update(_git("status", "--porcelain", cwd=root).encode("utf-8"))
    return total.hexdigest()


class Scratch:
    """One copy of the tree, restored file-by-file between mutations."""

    def __init__(self, root, destination):
        self.source = pathlib.Path(root)
        self.root = pathlib.Path(destination)
        self._paths = tracked_paths(root)

    def populate(self):
        for rel in self._paths:
            source = self.source / rel
            if not source.is_file():
                continue
            target = self.root / rel
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(source, target)
        return len(self._paths)

    def restore(self, rel):
        source = self.source / rel
        target = self.root / rel
        if not source.is_file():
            raise SystemExit(f"{rel} is not a tracked file")
        shutil.copyfile(source, target)

    # newline="" on both halves: universal-newline translation would rewrite a
    # CRLF working copy to LF on the way through a mutation.
    def read(self, rel):
        target = self.root / rel
        if not target.is_file():
            raise SystemExit(f"{rel} is not present in the scratch tree")
        with target.open(encoding="utf-8", newline="") as handle:
            return handle.read()

    def write(self, rel, text):
        with (self.root / rel).open("w", encoding="utf-8", newline="") as handle:
            handle.write(text)
