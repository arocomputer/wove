"""Check generated site links, or verify production HTTPS and redirects."""
import argparse
from html.parser import HTMLParser
from pathlib import Path
import time
from urllib.error import HTTPError, URLError
from urllib.parse import unquote, urljoin, urlparse
from urllib.request import urlopen

ROOT = Path(__file__).resolve().parents[1]
ORIGIN = "https://wovetui.com"


class Page(HTMLParser):
    """Collect targets and references without executing site JavaScript."""
    def __init__(self):
        super().__init__()
        self.ids = set()
        self.refs = []

    def handle_starttag(self, tag, attrs):
        attributes = dict(attrs)
        if "id" in attributes:
            self.ids.add(attributes["id"])
        for key in ("href", "src"):
            if key in attributes:
                self.refs.append(attributes[key])


def links(directory):
    """Resolve local files and fragments exactly as links in a static site do."""
    pages = {}
    for path in directory.rglob("*.html"):
        parser = Page()
        parser.feed(path.read_text())
        pages[path] = parser
    if not pages:
        raise ValueError("no generated HTML; build the website first")
    failures = []
    for path, page in pages.items():
        relative = path.relative_to(directory).as_posix()
        base = "/" + (relative.removesuffix("index.html") if relative.endswith("/index.html") or relative == "index.html" else relative)
        for ref in page.refs:
            original = urlparse(ref)
            if original.scheme or original.netloc:
                continue
            destination = urlparse(urljoin(base, ref))
            target = directory / unquote(destination.path.lstrip("/"))
            if target.is_dir():
                target /= "index.html"
            if not target.is_file():
                failures.append(f"{relative}: missing file {ref}")
            elif destination.fragment and target in pages and unquote(destination.fragment) not in pages[target].ids:
                failures.append(f"{relative}: missing anchor {ref}")
    if failures:
        raise ValueError("\n".join(failures))
    print(f"checked links and anchors in {len(pages)} pages")


def production():
    """Check HTTPS, actual guide content, a real 404, and the canonical www redirect."""
    for path, marker in (("/", "Wove"), ("/docs/", "Introduction"), ("/docs/quickstart/", "Quickstart")):
        with urlopen(ORIGIN + path, timeout=15) as response:
            if response.status != 200 or marker not in response.read().decode():
                raise ValueError(f"production page failed: {path}")
            if not response.url.startswith(ORIGIN + "/"):
                raise ValueError(f"unexpected production origin: {response.url}")
    try:
        with urlopen(ORIGIN + "/__wove_missing_page__", timeout=15):
            raise ValueError("unknown URL must return HTTP 404")
    except HTTPError as error:
        if error.code != 404 or "Page not found" not in error.read().decode():
            raise ValueError("unknown URL did not return the Wove 404 page") from error
    with urlopen("https://www.wovetui.com/docs/", timeout=15) as response:
        if response.status != 200 or response.url != ORIGIN + "/docs/":
            raise ValueError("www did not redirect to the canonical HTTPS origin")
    print("production HTTPS, docs, 404, and www redirect passed")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("mode", choices=("links", "smoke"))
    args = parser.parse_args()
    if args.mode == "links":
        links(ROOT / "crates/web/dist")
    else:
        for attempt in range(6):
            try:
                production()
                return
            except (ValueError, URLError, TimeoutError) as error:
                if attempt == 5:
                    raise
                print(f"production not ready: {error}; retrying in 10 seconds", flush=True)
                time.sleep(10)


if __name__ == "__main__":
    main()
