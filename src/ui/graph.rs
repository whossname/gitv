use super::Message;
use crate::git::graph::{PositionedCommit, PositionedGraph, RefKind};
use iced::{
    Color, Element, Length, Point, Rectangle, Renderer, Size, Theme, mouse,
    widget::canvas::{self, Canvas, Frame, Geometry, Path, Stroke, Text},
};

const ROW_HEIGHT: f32 = 28.0;
const LANE_WIDTH: f32 = 20.0;
const NODE_RADIUS: f32 = 5.0;
const DIAMOND_SIZE: f32 = 6.0;
const LABEL_COL_WIDTH: f32 = 140.0;
const LABEL_PADDING: f32 = 6.0;
const LABEL_HEIGHT: f32 = 18.0;
const LABEL_CORNER: f32 = 3.0;
const LANE_LINE_WIDTH: f32 = 2.5;
const GRAPH_EDGE_WIDTH: f32 = 1.5;
const TRUNCATE_APPROX_CHAR_WIDTH: f32 = 7.0;

const LANE_COLOURS: &[Color] = &[
    Color {
        r: 0.0,
        g: 0.85,
        b: 0.8,
        a: 1.0,
    }, // teal
    Color {
        r: 0.2,
        g: 0.5,
        b: 1.0,
        a: 1.0,
    }, // blue
    Color {
        r: 0.6,
        g: 0.3,
        b: 1.0,
        a: 1.0,
    }, // purple
    Color {
        r: 1.0,
        g: 0.3,
        b: 0.7,
        a: 1.0,
    }, // pink
    Color {
        r: 0.5,
        g: 0.2,
        b: 1.0,
        a: 1.0,
    }, // violet
    Color {
        r: 0.1,
        g: 0.85,
        b: 0.4,
        a: 1.0,
    }, // green
    Color {
        r: 0.7,
        g: 1.0,
        b: 0.0,
        a: 1.0,
    }, // lime
    Color {
        r: 1.0,
        g: 0.6,
        b: 0.0,
        a: 1.0,
    }, // orange
    Color {
        r: 1.0,
        g: 0.2,
        b: 0.2,
        a: 1.0,
    }, // red
];

fn lane_colour(lane: usize) -> Color {
    LANE_COLOURS[lane % LANE_COLOURS.len()]
}

fn commit_point(row: usize, lane: usize) -> Point {
    Point {
        x: LABEL_COL_WIDTH + LANE_WIDTH * lane as f32 + LANE_WIDTH / 2.0,
        y: ROW_HEIGHT * row as f32 + ROW_HEIGHT / 2.0,
    }
}

// --- Ref label grouping ---

#[derive(Debug)]
enum LabelKind {
    LocalOnly,
    RemoteOnly,
    Both,
    Tag,
}

#[derive(Debug)]
struct GroupedLabel {
    name: String, // short display name (no origin/ prefix)
    kind: LabelKind,
}

/// Group refs on a commit into display labels.
/// - local + matching remote → one ◆ pill
/// - local only → ▲ pill
/// - remote only → ▼ pill (strip origin/ prefix)
/// - tags → # pill
/// Sorted: local/remote branches first (main/master priority, then by name length),
/// tags after. HEAD is dropped.
fn group_labels(refs: &[crate::git::graph::RefLabel]) -> Vec<GroupedLabel> {
    use std::collections::{HashMap, HashSet};

    let mut local: Vec<String> = Vec::new();
    let mut remotes: HashSet<String> = HashSet::new();
    let mut tags: Vec<String> = Vec::new();

    for r in refs {
        match r.kind {
            RefKind::Head => {}
            RefKind::Branch => local.push(r.name.clone()),
            RefKind::RemoteBranch => {
                // strip "origin/" prefix for matching
                let short = r.name.splitn(2, '/').nth(1).unwrap_or(&r.name).to_string();
                remotes.insert(short);
            }
            RefKind::Tag => tags.push(r.name.clone()),
        }
    }

    let mut grouped: Vec<GroupedLabel> = Vec::new();

    // Track which remotes have been matched
    let mut matched_remotes: HashSet<String> = HashSet::new();

    // Sort locals: main/master first, then by length
    let mut sorted_local = local.clone();
    sorted_local.sort_by_key(|n| {
        let priority = if n == "main" || n == "master" { 0 } else { 1 };
        (priority, n.len())
    });

    for name in sorted_local {
        if remotes.contains(&name) {
            matched_remotes.insert(name.clone());
            grouped.push(GroupedLabel {
                name,
                kind: LabelKind::Both,
            });
        } else {
            grouped.push(GroupedLabel {
                name,
                kind: LabelKind::LocalOnly,
            });
        }
    }

    // Unmatched remotes
    for name in &remotes {
        if !matched_remotes.contains(name) {
            grouped.push(GroupedLabel {
                name: name.clone(),
                kind: LabelKind::RemoteOnly,
            });
        }
    }

    // Tags
    let mut sorted_tags = tags;
    sorted_tags.sort_by_key(|n| n.len());
    for name in sorted_tags {
        grouped.push(GroupedLabel {
            name,
            kind: LabelKind::Tag,
        });
    }

    grouped
}

fn label_icon(kind: &LabelKind) -> &'static str {
    match kind {
        LabelKind::LocalOnly => "▲",
        LabelKind::RemoteOnly => "▼",
        LabelKind::Both => "◆",
        LabelKind::Tag => "#",
    }
}

fn label_colour(kind: &LabelKind, lane_col: Color) -> Color {
    match kind {
        LabelKind::Tag => Color {
            r: 1.0,
            g: 0.85,
            b: 0.0,
            a: 1.0,
        },
        _ => lane_col,
    }
}

/// Truncate a string to fit within `max_width` pixels, appending `…` if needed.
fn truncate_to_width(s: &str, max_width: f32) -> String {
    let max_chars = (max_width / TRUNCATE_APPROX_CHAR_WIDTH).floor() as usize;
    if s.len() <= max_chars {
        s.to_string()
    } else if max_chars > 1 {
        format!("{}…", &s[..max_chars.saturating_sub(1)])
    } else {
        "…".to_string()
    }
}

// --- Canvas ---

pub struct GraphCanvas<'a> {
    graph: &'a PositionedGraph,
    selected_oid: Option<String>,
}

impl<'a> GraphCanvas<'a> {
    pub fn new(graph: &'a PositionedGraph, selected_oid: Option<String>) -> Element<'a, Message> {
        let total_height = graph.commits.len() as f32 * ROW_HEIGHT;

        let canvas = Canvas::new(Self {
            graph,
            selected_oid,
        })
        .width(Length::Fill)
        .height(Length::Fixed(total_height));

        iced::widget::scrollable(canvas).into()
    }
}

impl canvas::Program<Message> for GraphCanvas<'_> {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        let total_height = self.graph.commits.len() as f32 * ROW_HEIGHT;

        // --- Selection highlight row ---
        if let Some(oid) = &self.selected_oid {
            if let Some(p) = self.graph.commits.iter().find(|c| &c.node.oid == oid) {
                let y = p.row as f32 * ROW_HEIGHT;
                frame.fill(
                    &Path::rectangle(
                        Point { x: 0.0, y },
                        Size {
                            width: bounds.width,
                            height: ROW_HEIGHT,
                        },
                    ),
                    Color {
                        r: 1.0,
                        g: 1.0,
                        b: 1.0,
                        a: 0.06,
                    },
                );
            }
        }

        // --- Row backgrounds ---
        for positioned in &self.graph.commits {
            let y = positioned.row as f32 * ROW_HEIGHT;
            let colour = lane_colour(positioned.lane);
            let is_selected = self.selected_oid.as_deref() == Some(&positioned.node.oid);

            // Lane colour tint, full width
            frame.fill(
                &Path::rectangle(
                    Point { x: 0.0, y },
                    Size {
                        width: bounds.width,
                        height: ROW_HEIGHT,
                    },
                ),
                Color {
                    r: colour.r,
                    g: colour.g,
                    b: colour.b,
                    a: 0.01,
                },
            );

            // Selection highlight on top
            if is_selected {
                frame.fill(
                    &Path::rectangle(
                        Point { x: 0.0, y },
                        Size {
                            width: bounds.width,
                            height: ROW_HEIGHT,
                        },
                    ),
                    Color {
                        r: 0.8,
                        g: 0.8,
                        b: 0.8,
                        a: 0.001,
                    },
                );
            }
        }

        // --- Edges ---
        for edge in &self.graph.edges {
            let from = commit_point(edge.from_row, edge.from_lane);
            let to = commit_point(edge.to_row, edge.to_lane);
            let colour = lane_colour(edge.from_lane);

            let path = Path::new(|b| {
                b.move_to(from);
                if edge.from_lane == edge.to_lane {
                    b.line_to(to);
                } else {
                    let mid_y = from.y + ROW_HEIGHT / 2.0;
                    b.line_to(Point {
                        x: from.x,
                        y: mid_y,
                    });
                    b.line_to(Point { x: to.x, y: mid_y });
                    b.line_to(to);
                }
            });

            frame.stroke(
                &path,
                Stroke::default()
                    .with_color(colour)
                    .with_width(GRAPH_EDGE_WIDTH),
            );
        }

        // --- Nodes + labels ---
        for positioned in &self.graph.commits {
            let centre = commit_point(positioned.row, positioned.lane);
            let colour = lane_colour(positioned.lane);
            let is_selected = self.selected_oid.as_deref() == Some(&positioned.node.oid);
            let is_merge = positioned.node.parent_oids.len() > 1;

            if is_merge {
                draw_diamond(&mut frame, centre, colour, is_selected);
            } else {
                draw_circle(&mut frame, centre, colour, is_selected);
            }

            // --- Ref labels left of graph ---
            let labels = group_labels(&positioned.node.refs);

            if !labels.is_empty() {
                // Available width for label pills: LABEL_COL_WIDTH minus a small right margin
                // We fit as many as possible, show +N for the rest
                let right_edge = LABEL_COL_WIDTH - 4.0;
                let mut x_cursor = 2.0f32;
                let y_mid = centre.y;

                let mut shown = 0usize;

                // First pass: count how many fit, reserving space for +N badge if needed
                let mut fits: Vec<(String, &GroupedLabel)> = Vec::new();
                for (i, label) in labels.iter().enumerate() {
                    let icon = label_icon(&label.kind);
                    let display = format!("{} {}", icon, label.name);
                    let remaining = right_edge - x_cursor;
                    // reserve room for +N badge if not the last label
                    let reserve = if i < labels.len() - 1 { 28.0 } else { 0.0 };
                    let pill_width = (display.len() as f32 * TRUNCATE_APPROX_CHAR_WIDTH)
                        .min(remaining - reserve - LABEL_PADDING * 2.0);

                    if pill_width < 14.0 {
                        break; // not enough room even for one char
                    }

                    let truncated =
                        truncate_to_width(&format!("{} {}", icon, label.name), pill_width);
                    fits.push((truncated, label));
                    x_cursor += pill_width + LABEL_PADDING * 2.0 + 3.0;
                    shown += 1;
                }

                let overflow = labels.len() - shown;

                // Second pass: draw
                let mut draw_x = 2.0f32;
                for (text_str, label) in &fits {
                    let col = label_colour(&label.kind, colour);
                    let pill_w =
                        text_str.len() as f32 * TRUNCATE_APPROX_CHAR_WIDTH + LABEL_PADDING * 2.0;

                    // pill background
                    draw_pill(
                        &mut frame,
                        Point {
                            x: draw_x,
                            y: y_mid - LABEL_HEIGHT / 2.0,
                        },
                        pill_w,
                        LABEL_HEIGHT,
                        Color {
                            r: col.r,
                            g: col.g,
                            b: col.b,
                            a: 0.15,
                        },
                        col,
                    );

                    // pill text
                    frame.fill_text(Text {
                        content: text_str.clone(),
                        position: Point {
                            x: draw_x + LABEL_PADDING,
                            y: y_mid - 7.0,
                        },
                        color: col,
                        size: iced::Pixels(11.0),
                        ..Text::default()
                    });

                    draw_x += pill_w + 3.0;
                }

                // +N badge
                if overflow > 0 {
                    let badge = format!("+{}", overflow);
                    let badge_w =
                        badge.len() as f32 * TRUNCATE_APPROX_CHAR_WIDTH + LABEL_PADDING * 2.0;
                    let badge_col = Color {
                        r: 0.6,
                        g: 0.6,
                        b: 0.6,
                        a: 1.0,
                    };
                    draw_pill(
                        &mut frame,
                        Point {
                            x: draw_x,
                            y: y_mid - LABEL_HEIGHT / 2.0,
                        },
                        badge_w,
                        LABEL_HEIGHT,
                        Color {
                            r: 0.3,
                            g: 0.3,
                            b: 0.3,
                            a: 0.4,
                        },
                        badge_col,
                    );
                    frame.fill_text(Text {
                        content: badge,
                        position: Point {
                            x: draw_x + LABEL_PADDING,
                            y: y_mid - 7.0,
                        },
                        color: badge_col,
                        size: iced::Pixels(11.0),
                        ..Text::default()
                    });
                }
            }

            // --- Commit summary (right of graph) ---
            let graph_right = LABEL_COL_WIDTH + self.graph.lane_count as f32 * LANE_WIDTH + 8.0;
            let summary_max_w = bounds.width - graph_right - 8.0;
            let summary = truncate_to_width(&positioned.node.summary, summary_max_w);

            frame.fill_text(Text {
                content: summary,
                position: Point {
                    x: graph_right,
                    y: centre.y - 7.0,
                },
                color: Color {
                    r: 0.85,
                    g: 0.85,
                    b: 0.85,
                    a: 1.0,
                },
                size: iced::Pixels(12.0),
                ..Text::default()
            });
        }

        vec![frame.into_geometry()]
    }

    fn update(
        &self,
        _state: &mut Self::State,
        event: canvas::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> (iced::event::Status, Option<Message>) {
        if let canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) = event {
            if let Some(pos) = cursor.position_in(bounds) {
                let row = (pos.y / ROW_HEIGHT) as usize;
                if let Some(commit) = self.graph.commits.get(row) {
                    return (
                        iced::event::Status::Captured,
                        Some(Message::CommitSelected(commit.node.oid.clone())),
                    );
                }
            }
        }
        (iced::event::Status::Ignored, None)
    }
}

// --- Drawing helpers ---

fn draw_circle(frame: &mut Frame, centre: Point, colour: Color, selected: bool) {
    let circle = Path::circle(centre, NODE_RADIUS);
    frame.fill(&circle, colour);
    if selected {
        let ring = Path::circle(centre, NODE_RADIUS + 2.5);
        frame.stroke(
            &ring,
            Stroke::default().with_color(Color::WHITE).with_width(1.5),
        );
    }
}

fn draw_diamond(frame: &mut Frame, centre: Point, colour: Color, selected: bool) {
    let s = DIAMOND_SIZE;
    let diamond = Path::new(|b| {
        b.move_to(Point {
            x: centre.x,
            y: centre.y - s,
        });
        b.line_to(Point {
            x: centre.x + s,
            y: centre.y,
        });
        b.line_to(Point {
            x: centre.x,
            y: centre.y + s,
        });
        b.line_to(Point {
            x: centre.x - s,
            y: centre.y,
        });
        b.close();
    });
    frame.fill(&diamond, colour);
    if selected {
        frame.stroke(
            &diamond,
            Stroke::default().with_color(Color::WHITE).with_width(1.5),
        );
    }
}

fn draw_pill(
    frame: &mut Frame,
    top_left: Point,
    width: f32,
    height: f32,
    fill: Color,
    border: Color,
) {
    let rect = Path::rounded_rectangle(top_left, Size { width, height }, LABEL_CORNER.into());
    frame.fill(&rect, fill);
    frame.stroke(&rect, Stroke::default().with_color(border).with_width(1.0));
}
