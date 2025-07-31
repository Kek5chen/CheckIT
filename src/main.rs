use iced::{Element, Task, Theme};
use std::net::IpAddr;
use iced_futures::Subscription;
use crate::pages::nix_cluster::NixClusterView;
use crate::pages::ping::PingPage;

mod pages;
pub mod utils;

#[derive(Debug)]
pub enum MainMessage {
    PingView(pages::ping::Message),
    NixClusterView(pages::nix_cluster::Message),
}

#[derive(Default)]
pub struct CheckITApp {
    ping_page: PingPage,
    nix_cluster: NixClusterView,
}

pub struct PingProc {
    target: IpAddr,
}

impl CheckITApp {
    fn update(&mut self, msg: MainMessage) -> Task<MainMessage> {
        match msg {
            MainMessage::PingView(msg) => {
                self.ping_page.update(msg);
                Task::none()
            },
            MainMessage::NixClusterView(msg) => self.nix_cluster.update(msg).map(MainMessage::NixClusterView),
        }
    }
    
    fn view(&self) -> Element<MainMessage> {
        // self.ping_page.view().map(|m| MainMessage::PingPage(m))
        self.nix_cluster.view().map(|m| MainMessage::NixClusterView(m))
    }
    
    fn subscription(&self) -> Subscription<MainMessage> {
        // self.ping_page.subscription()
        //     .map(|m| MainMessage::PingPage(m))
        Subscription::none()
    }
}

fn main() -> iced::Result {
    env_logger::init();

    iced::application("CheckIT", CheckITApp::update, CheckITApp::view)
        .theme(|_| Theme::CatppuccinMocha)
        .subscription(CheckITApp::subscription)
        .run()
}
