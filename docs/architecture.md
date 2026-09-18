# Architecture and contracts

The initial workspace has one library crate. Modules separate pure rendering
from terminal ownership; separate crates will be introduced only when there
is a dependency or release reason.

| Module | Responsibility |
| --- | --- |
| layout | Rectangles, fixed tracks, weighted tracks, bounded placement |
| buffer | Styled cell storage, grapheme ownership, clipped writes |
| widgets | Composition over the buffer, with application-owned state |
| terminal | Optional event loop, changed-cell output, terminal restoration |

## Drawing

Coordinates use terminal columns and rows. Text is segmented into extended
grapheme clusters. A wide grapheme owns continuation cells and is never split
at the right edge. Overwriting any part clears its whole old footprint, which
can clear a cell just outside a write region if a wide glyph straddles it.
Control characters and standalone zero-width graphemes are skipped. Text
widgets interpret newlines; raw buffer writes are single-line operations.
Widths follow unicode-width and may differ from a terminal's Unicode version.

Widgets receive a shared frame and a clipping rectangle. Custom widgets must
honor that rectangle; the trait is a contract, not a sandbox. Public buffer
writes always clip to frame bounds. Text and list styles cover written glyphs,
not an entire row. Applications control unused space through their composition.

Frames are rebuilt from application state. The renderer emits changed lead
cells and necessary blanks, omitting wide continuation cells. It commits its
shadow frame only after successful output. Dimensions changing invalidate the
comparison. The initial renderer clones the completed frame; measure before
adding more complex buffer reuse or damage tracking.

## Terminal lifecycle

Terminal sessions require interactive stdin and stdout. They reject an existing
raw session and competing weft sessions. Sessions use the alternate screen,
bracketed paste, hidden cursor, and disabled line wrapping. Drop restores a
normal shell configuration. Previously customized cursor or wrap modes are not
queried or restored exactly. Avoid nesting with other terminal owners.

Normal errors and panic unwinding run cleanup. Aborts and uncatchable signals
do not. The library does not install a global panic hook or signal handler.
`run` reserves Ctrl-C for exit. Custom event loops own that policy themselves.

## Independence

No application owns weft's API. Application settings, themes, persistence,
networking, tasks, and domain-specific models stay in consumers. Examples must
exercise different kinds of applications. e is a possible future consumer;
compatibility with its current implementation is not a library contract.
