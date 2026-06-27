use std::path::PathBuf;
use iced::{Element, Task, Theme, Length};
use iced::widget::{row, container, vertical_rule};
use crate::git::graph::PositionedGraph;

mod graph;
mod detail;
mod diff;

pub use graph::GraphCanvas;

pub fn run(path: PathBuf) -> iced::Result {
    iced::application("gitv", Gitv::update, Gitv::view)
        .theme(|_| Theme::Dark)
        .run_with(|| Gitv::new(path))
}

#[derive(Debug, Clone)]
pub enum LeftPanel {
    Graph,
    Diff { commit_oid: String, file_path: String },
}

pub struct Gitv {
    graph: Option<PositionedGraph>,
    error: Option<String>,
    selected_oid: Option<String>,
    selected_file: Option<String>,
    left_panel: LeftPanel,
    divider_ratio: f32, // 0.0-1.0, default 0.8
}

#[derive(Debug, Clone)]
pub enum Message {
    GraphLoaded(Result<PositionedGraph, String>),
    CommitSelected(String),
    FileSelected(String),
    BackToGraph,
    DividerMoved(f32),
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

        (
            Self {
                graph: None,
                error: None,
                selected_oid: None,
                selected_file: None,
                left_panel: LeftPanel::Graph,
                divider_ratio: 0.8,
            },
            task,
        )
    }

    fn update(&mut self, message: Message) {
        match message {
            Message::GraphLoaded(Ok(graph)) => self.graph = Some(graph),
            Message::GraphLoaded(Err(e)) => self.error = Some(e),

            Message::CommitSelected(oid) => {
                self.selected_oid = Some(oid);
                self.selected_file = None;
                self.left_panel = LeftPanel::Graph;
            }

            Message::FileSelected(path) => {
                self.selected_file = Some(path.clone());
                if let Some(oid) = &self.selected_oid {
                    self.left_panel = LeftPanel::Diff {
                        commit_oid: oid.clone(),
                        file_path: path,
                    };
                }
            }

            Message::BackToGraph => {
                self.left_panel = LeftPanel::Graph;
                self.selected_file = None;
            }

            Message::DividerMoved(ratio) => {
                self.divider_ratio = ratio.clamp(0.2, 0.9);
            }
        }
    }

    fn view(&self) -> Element<Message> {
        if let Some(err) = &self.error {
            return iced::widget::text(format!("Error: {}", err)).into();
        }

        let left: Element<Message> = match &self.left_panel {
            LeftPanel::Graph => {
                match &self.graph {
                    Some(graph) => graph::GraphCanvas::new(graph, self.selected_oid.clone())
                        .into(),
                    None => iced::widget::text("Loading...").into(),
                }
            }
            LeftPanel::Diff { commit_oid, file_path } => {
                diff::view(commit_oid, file_path)
            }
        };

        let right: Element<Message> = match &self.graph {
            Some(graph) => {
                let commit = self.selected_oid.as_ref().and_then(|oid| {
                    graph.commits.iter().find(|c| &c.node.oid == oid)
                });
                detail::view(commit, self.selected_file.as_deref())
            }
            None => iced::widget::text("").into(),
        };

        let left_container = container(left)
            .width(Length::FillPortion((self.divider_ratio * 100.0) as u16))
            .height(Length::Fill);

        let right_container = container(right)
            .width(Length::FillPortion(((1.0 - self.divider_ratio) * 100.0) as u16))
            .height(Length::Fill);

        row![
            left_container,
            vertical_rule(1),
            right_container,
        ]
        .into()
    }
}
