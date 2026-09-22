#!/usr/bin/env python3
"""Check staged whitespace, conflict markers, and Rust formatting without modifying files."""
import difflib
import re
import subprocess
import sys
from pathlib import PurePosixPath


def git(*args):
    """Read Git objects, retaining filenames and blob bytes exactly."""
    return subprocess.check_output(['git', *args])


def check():
    """Inspect the staged index, ignoring unstaged worktree edits."""
    subprocess.run(['git', 'diff', '--check', '--cached'], check=True)
    paths = git('diff', '--name-only', '-z', '--diff-filter=ACMR', '--cached').split(b'\0')
    failed = False
    for raw in paths:
        if not raw.endswith(b'.rs'):
            continue
        path = raw.decode('utf-8', errors='surrogateescape')
        # Use the nearest staged manifest, including workspace-inherited editions.
        edition = None
        for parent in PurePosixPath(path).parents:
            manifest = ':' + str(parent / 'Cargo.toml')
            result = subprocess.run(['git', 'show', manifest], capture_output=True)
            if result.returncode == 0:
                match = re.search(rb'^edition\s*=\s*"(\d+)"', result.stdout, re.MULTILINE)
                if match:
                    edition = match.group(1).decode()
                    break
        if edition is None:
            raise RuntimeError(f'Cannot determine Rust edition for {path}')
        print(f'Checking staged Rust: {path}', flush=True)
        source = git('show', ':' + path)
        result = subprocess.run(['rustfmt', '--edition', edition,
                                 '--config', 'skip_children=true'], input=source, stdout=subprocess.PIPE)
        # rustfmt's stdin check mode can report differences with a zero exit status.
        if result.returncode != 0 or result.stdout != source:
            failed = True
            if result.returncode == 0:
                sys.stderr.writelines(difflib.unified_diff(
                    source.decode().splitlines(keepends=True),
                    result.stdout.decode().splitlines(keepends=True),
                    fromfile=path, tofile=path + ' (formatted)'))
    if failed:
        print('Format the reported files, review the changes, and stage them again.', file=sys.stderr)
    return int(failed)


if __name__ == '__main__':
    if len(sys.argv) > 1:
        sys.exit('usage: check.py')
    try:
        sys.exit(check())
    except (subprocess.CalledProcessError, RuntimeError, FileNotFoundError) as error:
        sys.exit(str(error))
