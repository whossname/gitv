use iced::{Element, Length, Color};
use iced::widget::{column, row, text, button, container, scrollable, Space};
use super::Message;

pub fn view<'a>(commit_oid: &'a str, file_path: &'a str) -> Element<'a, Message> {
    let back_button = button(text("← Back to graph"))
        .on_press(Message::BackToGraph);

    let header = row![
        Space::with_width(Length::Fill),
        back_button,
    ]
    .padding(8);

    let body = container(
        text(format!("Diff: {} @ {}", file_path, &commit_oid[..8]))
            .size(12)
            .style(|_| text::Style {
                color: Some(Color { r: 0.6, g: 0.6, b: 0.6, a: 1.0 }),
            })
    )
    .padding(16);

    column![header, body]
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}
