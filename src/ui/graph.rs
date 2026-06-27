use super::Message;
use crate::git::graph::{PositionedCommit, PositionedGraph};
use iced::{
    Color, Element, Length, Point, Rectangle, Renderer, Size, Theme, mouse,
    widget::canvas::{self, Canvas, Frame, Geometry, Path, Stroke, Text},
};

const ROW_HEIGHT: f32 = 28.0;
const LANE_WIDTH: f32 = 20.0;
const NODE_RADIUS: f32 = 5.0;
const DIAMOND_SIZE: f32 = 6.0;
const BRANCH_HIGHLIGHT_ALPHA: f32 = 0.07;

// Lane colour palette: teal, blue, purple, pink, violet, green, lime, orange, red
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

fn lane_highlight(lane: usize) -> Color {
    let c = lane_colour(lane);
    Color {
        r: c.r,
        g: c.g,
        b: c.b,
        a: BRANCH_HIGHLIGHT_ALPHA,
    }
}

// commit centre point given row/lane
fn commit_point(row: usize, lane: usize) -> Point {
    Point {
        x: LANE_WIDTH * lane as f32 + LANE_WIDTH / 2.0,
        y: ROW_HEIGHT * row as f32 + ROW_HEIGHT / 2.0,
    }
}

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
        let total_height = self.graph.commits.len() as f32 * ROW_HEIGHT;
        let graph_width = self.graph.lane_count as f32 * LANE_WIDTH;

        let mut frame = Frame::new(renderer, bounds.size());

        // --- Branch highlight bands ---
        // For each lane, draw a faint vertical band behind all rows
        // where that lane is active (has commits or passes through).
        // Simple approach: highlight behind every row for that lane's column.
        for lane in 0..self.graph.lane_count {
            let x = lane as f32 * LANE_WIDTH;
            let highlight = Path::rectangle(
                Point { x, y: 0.0 },
                Size {
                    width: LANE_WIDTH,
                    height: total_height,
                },
            );
            frame.fill(&highlight, lane_highlight(lane));
        }

        // --- Edges ---
        for edge in &self.graph.edges {
            let from = commit_point(edge.from_row, edge.from_lane);
            let to = commit_point(edge.to_row, edge.to_lane);
            let colour = lane_colour(edge.from_lane);

            let path = Path::new(|b| {
                b.move_to(from);
                if edge.from_lane == edge.to_lane {
                    // straight vertical
                    b.line_to(to);
                } else {
                    // diagonal: go down half a row then angle across
                    let mid_y = from.y + ROW_HEIGHT / 2.0;
                    b.line_to(Point {
                        x: from.x,
                        y: mid_y,
                    });
                    b.line_to(Point { x: to.x, y: mid_y });
                    b.line_to(to);
                }
            });

            frame.stroke(&path, Stroke::default().with_color(colour).with_width(1.5));
        }

        // --- Nodes ---
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

            // --- Ref labels (branch/tag/HEAD) ---
            let label_x = graph_width + 4.0;
            let mut label_offset = 0.0;
            for ref_label in &positioned.node.refs {
                let label_colour = match ref_label.kind {
                    crate::git::graph::RefKind::Tag => Color {
                        r: 1.0,
                        g: 0.85,
                        b: 0.0,
                        a: 1.0,
                    },
                    crate::git::graph::RefKind::Head => Color {
                        r: 0.3,
                        g: 1.0,
                        b: 0.5,
                        a: 1.0,
                    },
                    _ => colour,
                };

                frame.fill_text(Text {
                    content: ref_label.name.clone(),
                    position: Point {
                        x: label_x + label_offset,
                        y: centre.y - 7.0,
                    },
                    color: label_colour,
                    size: iced::Pixels(11.0),
                    ..Text::default()
                });

                // rough offset per label — proper measurement needs font metrics
                label_offset += ref_label.name.len() as f32 * 7.0 + 8.0;
            }

            // --- Commit summary ---
            let summary_x = label_x
                + label_offset
                + if positioned.node.refs.is_empty() {
                    0.0
                } else {
                    4.0
                };
            frame.fill_text(Text {
                content: positioned.node.summary.clone(),
                position: Point {
                    x: summary_x,
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

        // --- Selection highlight ---
        if let Some(oid) = &self.selected_oid {
            if let Some(positioned) = self.graph.commits.iter().find(|c| &c.node.oid == oid) {
                let y = positioned.row as f32 * ROW_HEIGHT;
                let highlight = Path::rectangle(
                    Point { x: 0.0, y },
                    Size {
                        width: bounds.width,
                        height: ROW_HEIGHT,
                    },
                );
                frame.fill(
                    &highlight,
                    Color {
                        r: 1.0,
                        g: 1.0,
                        b: 1.0,
                        a: 0.06,
                    },
                );
            }
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
        }); // top
        b.line_to(Point {
            x: centre.x + s,
            y: centre.y,
        }); // right
        b.line_to(Point {
            x: centre.x,
            y: centre.y + s,
        }); // bottom
        b.line_to(Point {
            x: centre.x - s,
            y: centre.y,
        }); // left
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
