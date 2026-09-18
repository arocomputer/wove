# Content

Markdown, syntax-highlighted code, and line diffs produce `RichText` elements.
Applications choose styles and themes. Parsing is separate from rendering, so
unchanged documents do not need reparsing on every frame.

Markdown supports CommonMark text, headings, emphasis, lists, code, quotes, and
links with visible destinations. It currently uses linear block layout. HTML is
literal text; tables, clickable links, and image loading are not implemented.

`Highlighter` accepts a reusable Syntect syntax set and theme. `diff` accepts
base, addition, and deletion styles. Core does not depend on these parsers.
