#![allow(dead_code)]
use std::hash::Hash;
use std::io::{BufRead, BufReader};
use std::net::IpAddr;
use std::process::{Child, Command, Stdio};
use std::str::FromStr;
use futures::StreamExt;
use iced::{Color, Element, Length, Padding};
use iced::widget::{button, container, row, scrollable, text, text_input, Column, Space, TextEditor};
use iced::widget::text_editor::Content;
use iced_futures::{subscription, BoxStream, Subscription};
use iced_futures::subscription::{EventStream, Hasher};
use crate::PingProc;

impl subscription::Recipe for PingProc {
    type Output = Message;

    fn hash(&self, state: &mut Hasher) {
        self.target.hash(state);
    }

    fn stream(self: Box<Self>, _input: EventStream) -> BoxStream<Self::Output> {
        let mut cmd = match Command::new("ping")
            .arg(self.target.to_string())
            .stdout(Stdio::piped())
            .spawn()
        {
            Ok(cmd) => cmd,
            Err(e) => {
                return futures::stream::once(futures::future::ready(Message::CheckIpError(
                    e.to_string(),
                )))
                    .boxed();
            }
        };

        let output = cmd.stdout.take().expect("Output is piped");
        let log_buffer = BufReader::new(output);
        let log_stream = futures::stream::iter(log_buffer.lines());

        futures::stream::once(futures::future::ready(Message::ActivePing(Some(cmd))))
            .chain(log_stream.map(|l| Message::AddLogContent(l.expect("Invalid IO"))))
            .boxed()
    }
}

#[derive(Debug)]
pub enum Message {
    CheckIp,
    UpdateIP(String),
    AddLogContent(String),
    Kill,
    CheckIpError(String),
    ActivePing(Option<Child>),
}

impl Clone for Message {
    fn clone(&self) -> Self {
        match self {
            Message::CheckIp => Message::CheckIp,
            Message::UpdateIP(ip) => Message::UpdateIP(ip.clone()),
            Message::AddLogContent(content) => Message::AddLogContent(content.clone()),
            Message::Kill => Message::Kill,
            Message::CheckIpError(err) => Message::CheckIpError(err.to_string()),
            Message::ActivePing(_) => Message::ActivePing(None),
        }
    }
}

#[derive(Default)]
pub struct PingPage {
    ip_input: String,
    log_lines: String,
    log_content: Content,
    target: Option<IpAddr>,
    ping_error: Option<String>,
    active_ping: Option<Child>,
}

impl PingPage {
    pub fn update(&mut self, message: Message) {
        match message {
            Message::CheckIp => {
                let target = match IpAddr::from_str(&self.ip_input) {
                    Ok(ip) => {
                        self.ping_error = None;
                        ip
                    }
                    Err(e) => {
                        self.ping_error = Some(e.to_string());
                        return;
                    }
                };

                self.active_ping = None;
                self.target = Some(target);
            }
            Message::ActivePing(child) => {
                self.active_ping = child;
            }
            Message::CheckIpError(err) => self.ping_error = Some(err),
            Message::UpdateIP(new_ip) => {
                self.ip_input = new_ip;
            }
            Message::AddLogContent(content) => {
                self.log_lines.push_str(&content);
                self.log_lines.push('\n');
                self.log_content = Content::with_text(&self.log_lines);
            }
            Message::Kill => {
                self.target = None;
                self.active_ping = None;
            }
        }
    }

    pub fn view(&self) -> Element<Message> {
        let ping_header = text("Ping Scan").width(Length::Fill).center();
        let ip_input = text_input("IP Address", &self.ip_input).on_input(Message::UpdateIP);
        let check_error = self.ping_error.as_ref().map(|err| {
            text(err)
                .width(Length::Fill)
                .center()
                .color(Color::new(0.7, 0.3, 0.3, 1.0))
        });
        let mut check_btn = button(text("Ping").center()).width(Length::FillPortion(1));
        let mut kill_btn = button(text("Kill").center()).width(Length::FillPortion(1));
        if self.active_ping.is_none() {
            check_btn = check_btn.on_press(Message::CheckIp);
        } else {
            kill_btn = kill_btn.on_press(Message::Kill);
        }

        let buttons = row![
            check_btn,
            Space::with_width(Length::FillPortion(2)),
            kill_btn
        ]
            .width(Length::Fill)
            .padding(Padding::ZERO.top(5.));

        let left = Column::new()
            .push(ping_header)
            .push(ip_input)
            .push_maybe(check_error)
            .push(buttons)
            .padding(5.);

        let terminal_header = text("Log Output").width(Length::Fill).center();
        let log = container(scrollable(TextEditor::new(&self.log_content)));

        let right = iced::widget::column![terminal_header, log].padding(Padding::new(5.));

        container(row![left, right]).into()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let Some(log_stream) = &self.target else {
            return Subscription::none();
        };

        subscription::from_recipe(PingProc {
            target: log_stream.clone(),
        })
    }
}