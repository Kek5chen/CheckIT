use iced::Element;
use iced::widget::{button, text_input, column};

#[derive(Debug, Clone)]
pub enum Message {
    NodeNameChange(String),
    RunDiff,
}

#[derive(Default)]
pub struct NixDiffHostPage {
    node_name: String,
}

impl NixDiffHostPage {
    pub fn update(&mut self, message: Message) {
        match message {
            Message::NodeNameChange(name) => self.node_name = name,
            Message::RunDiff => self.run_diff(),
        }
    }

    pub fn run_diff(&self) {

    }

    pub fn view(&self) -> Element<Message> {
        let input = text_input("Nix Node Name", &self.node_name).on_input(Message::NodeNameChange);
        let run_diff_btn = button("Run Hot Diff").on_press(Message::RunDiff);

        column![input, run_diff_btn].into()
    }
}