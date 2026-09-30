"""Check external documentation links weekly without blocking unrelated PRs."""
import subprocess
from tools import ROOT, ensure


def main():
    files = sorted(ROOT.glob('*.md')) + sorted((ROOT / 'crates').rglob('README.md')) + sorted((ROOT / 'crates/web/src/content/docs').rglob('*.mdx'))
    report = ROOT / 'target/links.md'
    report.parent.mkdir(parents=True, exist_ok=True)
    subprocess.run([ensure('lychee'), '--config', str(ROOT / '.lychee.toml'), '--format', 'markdown',
                    '--output', str(report), *map(str, files)], cwd=ROOT, check=True)


if __name__ == '__main__':
    main()
