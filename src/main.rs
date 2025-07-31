use iced::{Element, Theme};
use std::net::IpAddr;
use iced_futures::Subscription;
use crate::pages::nix_diff::NixDiffHostPage;
use crate::pages::ping::PingPage;

mod pages;

#[derive(Debug)]
pub enum MainMessage {
    PingPage(pages::ping::Message),
    NixDiffPage(pages::nix_diff::Message),
}

#[derive(Default)]
pub struct CheckITApp {
    ping_page: PingPage,
    nix_diff_page: NixDiffHostPage,
}

pub struct PingProc {
    target: IpAddr,
}

impl CheckITApp {
    fn update(&mut self, msg: MainMessage) {
        match msg {
            MainMessage::PingPage(msg) => self.ping_page.update(msg),
            MainMessage::NixDiffPage(msg) => self.nix_diff_page.update(msg),
        }
    }
    
    fn view(&self) -> Element<MainMessage> {
        // self.ping_page.view().map(|m| MainMessage::PingPage(m))
        self.nix_diff_page.view().map(|m| MainMessage::NixDiffPage(m))
    }
    
    fn subscription(&self) -> Subscription<MainMessage> {
        // self.ping_page.subscription()
        //     .map(|m| MainMessage::PingPage(m))
        Subscription::none()
    }
}

fn main() -> iced::Result {
    iced::application("CheckIT", CheckITApp::update, CheckITApp::view)
        .theme(|_| Theme::CatppuccinMocha)
        .subscription(CheckITApp::subscription)
        .run()
}
