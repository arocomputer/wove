//! Markdown structure without terminal styles or application presentation.
use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd};
use std::{cell::Cell, iter::Peekable, ops::Range};

/// Table column alignment declared by the source.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Alignment {
    None,
    Left,
    Center,
    Right,
}

/// Inline content retains semantic emphasis and destinations for any formatter.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Inline {
    Text(String),
    /// Raw HTML source, preserved literally for application-selected handling.
    Html(String),
    Code(String),
    Emphasis(Vec<Inline>),
    Strong(Vec<Inline>),
    Strikethrough(Vec<Inline>),
    Link {
        destination: String,
        content: Vec<Inline>,
    },
    Image {
        destination: String,
        content: Vec<Inline>,
    },
    SoftBreak,
    HardBreak,
}

/// One list item, including its source number and optional task state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ListItem {
    pub number: Option<u64>,
    pub checked: Option<bool>,
    pub blocks: Vec<Block>,
}

/// Document blocks retain nesting, code info strings, and semantic table content.
/// Parsing assigns no glyphs, colors, wrapping, or terminal control sequences.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Block {
    Paragraph(Vec<Inline>),
    /// Raw block HTML, kept separate from inline HTML and paragraph content.
    Html(String),
    Heading {
        level: u8,
        content: Vec<Inline>,
    },
    Code {
        info: String,
        source: String,
    },
    Quote(Vec<Block>),
    List {
        start: Option<u64>,
        tight: bool,
        items: Vec<ListItem>,
    },
    Table {
        alignment: Vec<Alignment>,
        header: Vec<Vec<Inline>>,
        rows: Vec<Vec<Vec<Inline>>>,
    },
    Rule,
}

/// Parse CommonMark with tables, task lists, and strikethrough. HTML is literal
/// text and footnotes remain ordinary source. Incomplete blocks are valid,
/// making this suitable for reparsing a streaming document. Source nested more
/// than 128 parser tags deep remains literal text to bound stack usage.
pub fn parse(source: &str) -> Vec<Block> {
    let options =
        Options::ENABLE_TABLES | Options::ENABLE_TASKLISTS | Options::ENABLE_STRIKETHROUGH;
    // The public document is recursive. Bound source nesting before building it,
    // so parsing, formatting, and dropping untrusted Markdown use bounded stacks.
    let too_deep = Cell::new(false);
    let mut events = Parser::new_ext(source, options)
        .into_offset_iter()
        .scan(0usize, |depth, (event, range)| {
            match event {
                Event::Start(_) => *depth += 1,
                Event::End(_) => *depth = depth.saturating_sub(1),
                _ => {}
            }
            if *depth > 128 {
                too_deep.set(true);
                None
            } else {
                Some((event, range))
            }
        })
        .peekable();
    let result = blocks(&mut events, source, None, &mut None, &mut false);
    if too_deep.get() {
        vec![Block::Paragraph(vec![Inline::Text(source.into())])]
    } else {
        result
    }
}

type Events<I> = Peekable<I>;

/// Consume one container, synthesizing paragraphs for tight list text.
fn blocks<'a, I>(
    events: &mut Events<I>,
    source: &str,
    end: Option<TagEnd>,
    checked: &mut Option<bool>,
    paragraphs: &mut bool,
) -> Vec<Block>
where
    I: Iterator<Item = (Event<'a>, Range<usize>)>,
{
    let mut result = Vec::new();
    let mut pending = Vec::new();
    while let Some((event, _)) = events.next() {
        if matches!(&event, Event::End(tag) if Some(*tag) == end) {
            break;
        }
        let block = match event {
            Event::Start(Tag::Paragraph) => {
                *paragraphs = true;
                Some(Block::Paragraph(read_inlines(
                    events,
                    TagEnd::Paragraph,
                    checked,
                )))
            }
            Event::Start(Tag::Heading { level, .. }) => Some(Block::Heading {
                level: level as u8,
                content: inlines(events, TagEnd::Heading(level)),
            }),
            Event::Start(Tag::CodeBlock(kind)) => {
                let info = match kind {
                    CodeBlockKind::Fenced(info) => info.into_string(),
                    _ => String::new(),
                };
                let mut text = String::new();
                for (event, _) in events.by_ref() {
                    match event {
                        Event::End(TagEnd::CodeBlock) => break,
                        Event::Text(value) => text.push_str(&value),
                        _ => {}
                    }
                }
                Some(Block::Code { info, source: text })
            }
            Event::Start(Tag::BlockQuote(kind)) => Some(Block::Quote(blocks(
                events,
                source,
                Some(TagEnd::BlockQuote(kind)),
                &mut None,
                &mut false,
            ))),
            Event::Start(Tag::List(start)) => Some(list(events, source, start)),
            Event::Start(Tag::Table(alignment)) => Some(table(events, alignment)),
            Event::Rule => Some(Block::Rule),
            Event::Html(html) => Some(Block::Html(html.into_string())),
            Event::TaskListMarker(value) => {
                *checked = Some(value);
                None
            }
            other => {
                if let Some(value) = inline(other, events) {
                    pending.push(value);
                }
                None
            }
        };
        if let Some(block) = block {
            if !pending.is_empty() {
                result.push(Block::Paragraph(std::mem::take(&mut pending)));
            }
            result.push(block);
        }
    }
    if !pending.is_empty() {
        result.push(Block::Paragraph(pending));
    }
    result
}

/// Read list items without inventing source numbers. Loose lists retain their spacing.
fn list<'a, I>(events: &mut Events<I>, source: &str, start: Option<u64>) -> Block
where
    I: Iterator<Item = (Event<'a>, Range<usize>)>,
{
    let mut items = Vec::new();
    let mut tight = true;
    while let Some((event, range)) = events.next() {
        match event {
            Event::Start(Tag::Item) => {
                let number = start.and_then(|_| {
                    source[range.start..]
                        .split(|c: char| !c.is_ascii_digit())
                        .next()?
                        .parse()
                        .ok()
                });
                let mut checked = None;
                let mut paragraphs = false;
                let content = blocks(
                    events,
                    source,
                    Some(TagEnd::Item),
                    &mut checked,
                    &mut paragraphs,
                );
                // Only direct paragraphs mark the outer list loose; a quote or
                // nested list may have paragraphs with independent spacing.
                tight &= !paragraphs;
                items.push(ListItem {
                    number,
                    checked,
                    blocks: content,
                });
            }
            Event::End(TagEnd::List(_)) => break,
            _ => {}
        }
    }
    Block::List {
        start,
        tight,
        items,
    }
}

/// Keep table cells as inline content so links and emphasis survive custom layout.
fn table<'a, I>(events: &mut Events<I>, alignment: Vec<pulldown_cmark::Alignment>) -> Block
where
    I: Iterator<Item = (Event<'a>, Range<usize>)>,
{
    let alignment = alignment
        .into_iter()
        .map(|a| match a {
            pulldown_cmark::Alignment::None => Alignment::None,
            pulldown_cmark::Alignment::Left => Alignment::Left,
            pulldown_cmark::Alignment::Center => Alignment::Center,
            pulldown_cmark::Alignment::Right => Alignment::Right,
        })
        .collect();
    let mut header = Vec::new();
    let mut rows = Vec::new();
    let mut row = Vec::new();
    while let Some((event, _)) = events.next() {
        match event {
            Event::Start(Tag::TableCell) => row.push(inlines(events, TagEnd::TableCell)),
            Event::End(TagEnd::TableHead) => header = std::mem::take(&mut row),
            Event::End(TagEnd::TableRow) => rows.push(std::mem::take(&mut row)),
            Event::End(TagEnd::Table) => break,
            _ => {}
        }
    }
    Block::Table {
        alignment,
        header,
        rows,
    }
}

/// Consume nested inline content up to its matching end event.
fn inlines<'a, I>(events: &mut Events<I>, end: TagEnd) -> Vec<Inline>
where
    I: Iterator<Item = (Event<'a>, Range<usize>)>,
{
    read_inlines(events, end, &mut None)
}

/// Task state belongs to the surrounding item, even in a loose paragraph.
fn read_inlines<'a, I>(
    events: &mut Events<I>,
    end: TagEnd,
    checked: &mut Option<bool>,
) -> Vec<Inline>
where
    I: Iterator<Item = (Event<'a>, Range<usize>)>,
{
    let mut result = Vec::new();
    while let Some((event, _)) = events.next() {
        if let Event::TaskListMarker(value) = event {
            *checked = Some(value);
            continue;
        }
        if matches!(event, Event::End(tag) if tag == end) {
            break;
        }
        if let Some(value) = inline(event, events) {
            result.push(value);
        }
    }
    result
}

/// Translate one inline event, recursively consuming only its own children.
fn inline<'a, I>(event: Event<'a>, events: &mut Events<I>) -> Option<Inline>
where
    I: Iterator<Item = (Event<'a>, Range<usize>)>,
{
    Some(match event {
        Event::Text(text) => Inline::Text(text.into_string()),
        Event::Html(text) | Event::InlineHtml(text) => Inline::Html(text.into_string()),
        Event::Code(text) => Inline::Code(text.into_string()),
        Event::SoftBreak => Inline::SoftBreak,
        Event::HardBreak => Inline::HardBreak,
        Event::Start(Tag::Emphasis) => Inline::Emphasis(inlines(events, TagEnd::Emphasis)),
        Event::Start(Tag::Strong) => Inline::Strong(inlines(events, TagEnd::Strong)),
        Event::Start(Tag::Strikethrough) => {
            Inline::Strikethrough(inlines(events, TagEnd::Strikethrough))
        }
        Event::Start(Tag::Link { dest_url, .. }) => Inline::Link {
            destination: dest_url.into_string(),
            content: inlines(events, TagEnd::Link),
        },
        Event::Start(Tag::Image { dest_url, .. }) => Inline::Image {
            destination: dest_url.into_string(),
            content: inlines(events, TagEnd::Image),
        },
        _ => return None,
    })
}
