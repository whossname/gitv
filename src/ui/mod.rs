// src/ui/mod.rs
use std::path::PathBuf;
use iced::{Element, Task, Theme};
use crate::git::graph::PositionedGraph;

pub fn run(path: PathBuf) -> iced::Result {
    iced::application("gitv", Gitv::update, Gitv::view)
        .theme(|_| Theme::Dark)
        .run_with(|| Gitv::new(path))
}

struct Gitv {
    graph: Option<PositionedGraph>,
    error: Option<String>,
}

#[derive(Debug, Clone)]
enum Message {
    GraphLoaded(Result<PositionedGraph, String>),
}

impl Gitv {
    fn new(path: PathBuf) -> (Self, Task<Message>) {
        let task = Task::perform(
            async move {
                crate::git::graph::load(path)
                    .map_err(|e| e.to_string())
            },
            Message::GraphLoaded,
        );

        (Self { graph: None, error: None }, task)
    }

    fn update(&mut self, message: Message) {
        match message {
            Message::GraphLoaded(Ok(graph)) => self.graph = Some(graph),
            Message::GraphLoaded(Err(e)) => self.error = Some(e),
        }
    }

    fn view(&self) -> Element<Message> {
        if let Some(err) = &self.error {
            iced::widget::text(format!("Error: {}", err)).into()
        } else if let Some(_graph) = &self.graph {
            iced::widget::text(format!("Graph loaded — renderer coming in step 3")).into()
        } else {
            iced::widget::text("Loading...").into()
        }
    }
}
