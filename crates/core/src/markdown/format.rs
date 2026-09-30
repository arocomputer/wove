//! Plain block formatting over semantic Markdown. Applications can replace it.
use super::{Alignment, Block, Inline, Options, Palette};
use crate::{
    render::{columns, fit},
    text::{wrap, Span, Wrap},
    Style,
};
use std::sync::Arc;

/// Format a container and join blocks, preserving tight-list spacing.
pub(super) fn blocks(
    document: &[Block],
    palette: Palette,
    options: Options,
    tight: bool,
) -> Vec<Span> {
    let mut result = Vec::new();
    for block in document {
        if !result.is_empty() {
            result.push(Span::new(if tight { "\n" } else { "\n\n" }, palette.text));
        }
        let content = match block {
            Block::Paragraph(content) => lines(
                inlines(content, palette.text, palette, options),
                options.width,
            ),
            Block::Heading { level, content } => {
                let style = palette
                    .headings
                    .get(usize::from(level.saturating_sub(1)))
                    .copied()
                    .flatten()
                    .unwrap_or(palette.heading);
                lines(inlines(content, style, palette, options), options.width)
            }
            Block::Code { source, .. } => lines(
                vec![Span::new(source.trim_end_matches('\n'), palette.code)],
                options.width,
            ),
            Block::Rule => vec![Span::new(
                "─".repeat(usize::from(
                    options.rule_width.min(options.width.unwrap_or(u16::MAX)),
                )),
                palette.rule,
            )],
            Block::Quote(children) => {
                let inner = blocks(children, palette, narrower(options, 2), tight);
                prefix(inner, "│ ", "│ ", palette.quote)
            }
            Block::List {
                start,
                tight,
                items,
            } => {
                let mut output = Vec::new();
                for (index, item) in items.iter().enumerate() {
                    if index > 0 {
                        output.push(Span::new(if *tight { "\n" } else { "\n\n" }, palette.text));
                    }
                    let marker = match item.checked {
                        Some(true) => "[x] ".into(),
                        Some(false) => "[ ] ".into(),
                        None => match start.map(|n| n.saturating_add(index as u64)) {
                            Some(n) => format!("{n}. "),
                            None => "• ".into(),
                        },
                    };
                    let width = columns(&marker);
                    let content = blocks(&item.blocks, palette, narrower(options, width), *tight);
                    output.extend(prefix(content, &marker, &" ".repeat(width), palette.marker));
                }
                output
            }
            Block::Table {
                alignment,
                header,
                rows,
            } => table(alignment, header, rows, palette, options),
        };
        result.extend(content);
    }
    trim_rows(&mut result);
    result
}

/// Restrict descendants to the cells left after their display-only prefix.
fn narrower(mut options: Options, prefix: usize) -> Options {
    options.width = options.width.map(|w| {
        w.saturating_sub(prefix.min(usize::from(u16::MAX)) as u16)
            .max(1)
    });
    options
}

/// Apply inline emphasis without losing the enclosing block's attributes.
pub(super) fn inlines(
    content: &[Inline],
    style: Style,
    palette: Palette,
    options: Options,
) -> Vec<Span> {
    let mut result = Vec::new();
    append_inline(&mut result, content, style, palette, options, None);
    result
}

/// Extend spans with nested attributes and the nearest enclosing destination.
fn append_inline(
    result: &mut Vec<Span>,
    content: &[Inline],
    style: Style,
    palette: Palette,
    options: Options,
    link: Option<Arc<str>>,
) {
    for inline in content {
        let mut next = style;
        match inline {
            Inline::Text(text) => result.push(Span {
                text: text.clone(),
                style,
                link: link.clone(),
            }),
            Inline::SoftBreak | Inline::HardBreak => result.push(Span {
                text: if matches!(inline, Inline::SoftBreak) {
                    " "
                } else {
                    "\n"
                }
                .into(),
                style,
                link: link.clone(),
            }),
            Inline::Code(text) => result.push(Span {
                text: text.clone(),
                style: role(style, palette.code),
                link: link.clone(),
            }),
            Inline::Emphasis(children) => {
                next.italic = true;
                append_inline(result, children, next, palette, options, link.clone());
            }
            Inline::Strong(children) => {
                next.bold = true;
                append_inline(result, children, next, palette, options, link.clone());
            }
            Inline::Strikethrough(children) => {
                next.strikethrough = true;
                append_inline(result, children, next, palette, options, link.clone());
            }
            Inline::Link {
                destination,
                content,
            }
            | Inline::Image {
                destination,
                content,
            } => {
                let at = result.len();
                let next = role(style, palette.link);
                append_inline(
                    result,
                    content,
                    next,
                    palette,
                    options,
                    Some(Arc::from(destination.as_str())),
                );
                let shown: String = result[at..].iter().map(|s| s.text.as_str()).collect();
                if options.link_destinations && shown != *destination {
                    result.push(Span {
                        text: format!(" ({destination})"),
                        style: next,
                        link: link.clone(),
                    });
                }
            }
        }
    }
}

/// A role supplies its colors and adds attributes to enclosing emphasis.
fn role(base: Style, role: Style) -> Style {
    Style {
        bold: base.bold || role.bold,
        dim: base.dim || role.dim,
        italic: base.italic || role.italic,
        underline: base.underline || role.underline,
        strikethrough: base.strikethrough || role.strikethrough,
        reverse: base.reverse || role.reverse,
        ..role
    }
}

/// Break spans at shared grapheme boundaries, retaining styles and link metadata.
fn lines(spans: Vec<Span>, width: Option<u16>) -> Vec<Span> {
    let Some(width) = width else {
        return spans;
    };
    let text: String = spans.iter().map(|s| s.text.as_str()).collect();
    let ends = span_ends(&spans);
    let mut result = Vec::new();
    let mut base = 0;
    for (line_index, line) in text.split('\n').enumerate() {
        for (row_index, range) in wrap(line, usize::from(width), Wrap::Word)
            .into_iter()
            .enumerate()
        {
            if line_index > 0 || row_index > 0 {
                result.push(Span::new("\n", Style::default()));
            }
            result.extend(slice(&spans, &ends, base + range.start..base + range.end));
        }
        base += line.len() + 1;
    }
    result
}

/// Cumulative byte ends allow display-row extraction without rescanning earlier spans.
fn span_ends(spans: &[Span]) -> Vec<usize> {
    let mut end = 0;
    spans
        .iter()
        .map(|span| {
            end += span.text.len();
            end
        })
        .collect()
}

/// Copy a byte range at layout-selected grapheme boundaries, retaining metadata.
fn slice(spans: &[Span], ends: &[usize], range: std::ops::Range<usize>) -> Vec<Span> {
    let first = ends.partition_point(|end| *end <= range.start);
    let mut result = Vec::new();
    for index in first..spans.len() {
        let start = if index == 0 { 0 } else { ends[index - 1] };
        if start >= range.end {
            break;
        }
        let span = &spans[index];
        let a = range.start.max(start) - start;
        let b = range.end.min(ends[index]) - start;
        if a < b {
            result.push(Span {
                text: span.text[a..b].into(),
                style: span.style,
                link: span.link.clone(),
            });
        }
    }
    result
}

/// Prefix every physical line, preserving hyperlinks and styles inside it.
fn prefix(spans: Vec<Span>, first: &str, rest: &str, style: Style) -> Vec<Span> {
    let mut result = vec![Span::new(first, style)];
    for span in spans {
        for (i, text) in span.text.split('\n').enumerate() {
            if i > 0 {
                result.push(Span::new(format!("\n{rest}"), style));
            }
            if !text.is_empty() {
                result.push(Span {
                    text: text.into(),
                    style: span.style,
                    link: span.link.clone(),
                });
            }
        }
    }
    result
}

/// Remove only final empty display rows, leaving whitespace within code intact.
fn trim_rows(spans: &mut Vec<Span>) {
    while let Some(last) = spans.last_mut() {
        last.text.truncate(last.text.trim_end_matches('\n').len());
        if !last.text.is_empty() {
            break;
        }
        spans.pop();
    }
}

/// Render a table with aligned columns. Column allocation uses display cells;
/// overlong cells are clipped at grapheme boundaries instead of wrapping a grid.
fn table(
    alignment: &[Alignment],
    header: &[Vec<Inline>],
    rows: &[Vec<Vec<Inline>>],
    palette: Palette,
    options: Options,
) -> Vec<Span> {
    let data: Vec<Vec<Vec<Span>>> = std::iter::once(header)
        .chain(rows.iter().map(Vec::as_slice))
        .enumerate()
        .map(|(index, row)| {
            row.iter()
                .map(|cell| {
                    inlines(
                        cell,
                        if index == 0 {
                            palette.table_header
                        } else {
                            palette.text
                        },
                        palette,
                        options,
                    )
                })
                .collect()
        })
        .collect();
    let count = alignment.len().max(header.len());
    let mut widths = vec![1; count];
    for row in &data {
        for (i, cell) in row.iter().enumerate().take(count) {
            widths[i] = widths[i].max(columns(
                &cell.iter().map(|s| s.text.as_str()).collect::<String>(),
            ));
        }
    }
    if let Some(width) = options.width {
        let budget = usize::from(width)
            .saturating_sub(count.saturating_sub(1) * 3)
            .max(count);
        if widths.iter().sum::<usize>() > budget {
            // Cap large columns together instead of removing one cell at a time
            // from potentially enormous source text.
            let (mut low, mut high) = (1, widths.iter().copied().max().unwrap_or(1));
            while low < high {
                let cap = low + (high - low).div_ceil(2);
                if widths.iter().map(|w| (*w).min(cap)).sum::<usize>() <= budget {
                    low = cap;
                } else {
                    high = cap - 1;
                }
            }
            let mut spare = budget - widths.iter().map(|w| (*w).min(low)).sum::<usize>();
            for column in &mut widths {
                let extra = usize::from(*column > low && spare > 0);
                *column = (*column).min(low) + extra;
                spare -= extra;
            }
        }
    }
    let mut result = Vec::new();
    for (row_index, row) in data.iter().enumerate() {
        if row_index > 0 {
            result.push(Span::new("\n", palette.text));
        }
        for (i, width) in widths.iter().copied().enumerate() {
            if i > 0 {
                result.push(Span::new(" | ", palette.table_border));
            }
            let cell = row.get(i).map(Vec::as_slice).unwrap_or(&[]);
            let text: String = cell.iter().map(|span| span.text.as_str()).collect();
            let fitted = fit(&text, width);
            let clipped = slice(cell, &span_ends(cell), 0..fitted.len());
            let remaining = width.saturating_sub(columns(fitted));
            let left = match alignment.get(i) {
                Some(Alignment::Right) => remaining,
                Some(Alignment::Center) => remaining / 2,
                _ => 0,
            };
            result.push(Span::new(" ".repeat(left), palette.text));
            result.extend(clipped);
            result.push(Span::new(" ".repeat(remaining - left), palette.text));
        }
        if row_index == 0 {
            result.push(Span::new("\n", palette.table_border));
            for (i, width) in widths.iter().copied().enumerate() {
                if i > 0 {
                    result.push(Span::new("-+-", palette.table_border));
                }
                result.push(Span::new("─".repeat(width), palette.table_border));
            }
        }
    }
    if let Some(width) = options.width {
        let text: String = result.iter().map(|s| s.text.as_str()).collect();
        let ends = span_ends(&result);
        let mut clipped = Vec::new();
        let mut base = 0;
        for (index, row) in text.split('\n').enumerate() {
            if index > 0 {
                clipped.push(Span::new("\n", palette.text));
            }
            clipped.extend(slice(
                &result,
                &ends,
                base..base + fit(row, usize::from(width)).len(),
            ));
            base += row.len() + 1;
        }
        return clipped;
    }
    result
}
