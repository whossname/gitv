use iced::{Element, Length, Color};
use iced::widget::{column, row, text, button, container, scrollable, Space};
use crate::git::graph::PositionedCommit;
use super::Message;

// Stub file list — real implementation reads the commit tree diff via git2
const STUB_FILES: &[&str] = &[
    "src/main.rs",
    "src/ui/mod.rs",
    "Cargo.toml",
];

pub fn view<'a>(
    commit: Option<&'a PositionedCommit>,
    selected_file: Option<&'a str>,
) -> Element<'a, Message> {
    let Some(commit) = commit else {
        return container(
            text("Select a commit").style(|_| text::Style {
                color: Some(Color { r: 0.5, g: 0.5, b: 0.5, a: 1.0 }),
            })
        )
        .padding(16)
        .into();
    };

    let node = &commit.node;

    let header = column![
        text(&node.summary).size(14),
        Space::with_height(4),
        text(&node.author).size(12).style(|_| text::Style {
            color: Some(Color { r: 0.6, g: 0.6, b: 0.6, a: 1.0 }),
        }),
        text(format_timestamp(node.time)).size(11).style(|_| text::Style {
            color: Some(Color { r: 0.5, g: 0.5, b: 0.5, a: 1.0 }),
        }),
        Space::with_height(4),
        text(&node.oid[..8]).size(10).style(|_| text::Style {
            color: Some(Color { r: 0.4, g: 0.4, b: 0.4, a: 1.0 }),
        }),
    ]
    .spacing(2)
    .padding(12);

    // File list — stub for now
    let files = STUB_FILES.iter().fold(
        column![].spacing(0),
        |col, path| {
            let is_selected = selected_file == Some(path);
            let label = text(*path).size(12);
            let btn = button(label)
                .on_press(Message::FileSelected(path.to_string()))
                .width(Length::Fill)
                .style(move |theme, status| {
                    let mut style = button::Style::default();
                    if is_selected {
                        style.background = Some(iced::Background::Color(
                            Color { r: 0.2, g: 0.4, b: 0.8, a: 0.4 }
                        ));
                    }
                    style
                });
            col.push(btn)
        }
    );

    let content = column![
        header,
        container(
            text("Changed files").size(11).style(|_| text::Style {
                color: Some(Color { r: 0.5, g: 0.5, b: 0.5, a: 1.0 }),
            })
        ).padding([4, 12]),
        scrollable(files).height(Length::Fill),
    ];

    container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn format_timestamp(unix: i64) -> String {
    // Basic formatting without chrono dep — good enough for MVP
    let secs = unix;
    let days = secs / 86400;
    // Days since unix epoch — not human readable yet, swap in chrono later
    format!("t={}", days)
}
