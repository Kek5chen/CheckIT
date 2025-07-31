use crate::pages::nix_diff::{NixNodeDiffView, fetch_cluster_nodes};
use iced::widget::{button, column, container, pick_list, row, text, text_input};
use iced::{Element, Length, Padding, Task};
use iced_aw::selection_list;
use log::error;
use std::env::current_exe;
use std::path::PathBuf;
use std::thread::current;

#[derive(Debug, Clone)]
pub enum Message {
    IpAttrChanged(String),
    ClusterPathChanged(String),
    PickClusterDir,
    StartUpdateClusterInfo,
    UpdateClusterInfo(Option<Vec<String>>),
    NodeNameChange(usize, String),
    Error(String),
    NodeDiffMessage(usize, super::nix_diff::Message),
    DiffAll,
}

pub struct NixClusterView {
    ip_attr: String,
    cluster_path: PathBuf,
    all_cluster_nodes: Vec<String>,
    node_diff_views: Vec<NixNodeDiffView>,
    loading_cluster: bool,
    error: Option<String>,
    current_node: Option<usize>,
}

impl Default for NixClusterView {
    fn default() -> Self {
        Self {
            ip_attr: "config.base.primaryIP.address".to_owned(),
            cluster_path: PathBuf::new(),
            all_cluster_nodes: Vec::new(),
            node_diff_views: Vec::new(),
            loading_cluster: false,
            error: None,
            current_node: None,
        }
    }
}

impl NixClusterView {
    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::PickClusterDir => {
                if let Some(cluster_dir) = rfd::FileDialog::new()
                    .set_directory(&self.cluster_path)
                    .pick_file()
                {
                    self.cluster_path = cluster_dir;
                    return self.start_cluster_info_update();
                };
            }
            Message::ClusterPathChanged(path) => {
                self.cluster_path = PathBuf::from(path);
            }
            Message::StartUpdateClusterInfo => {
                return self.start_cluster_info_update();
            }
            Message::UpdateClusterInfo(nodes) => {
                self.loading_cluster = false;
                if let Some(nodes) = nodes {
                    self.all_cluster_nodes = nodes;
                    self.node_diff_views = self
                        .all_cluster_nodes
                        .iter()
                        .map(|node| {
                            NixNodeDiffView::new(
                                self.cluster_path.clone(),
                                self.ip_attr.clone(),
                                node.clone(),
                            )
                        })
                        .collect();
                    if self.all_cluster_nodes.is_empty() {
                        self.current_node = None;
                    } else {
                        self.current_node = Some(0);
                    }
                }
            }
            Message::IpAttrChanged(changed) => self.ip_attr = changed,
            Message::NodeNameChange(idx, _) => self.current_node = Some(idx),
            Message::Error(err) => self.error = Some(err.clone()),
            Message::NodeDiffMessage(idx, msg) => {
                if let Some(view) = self.node_diff_views.get_mut(idx) {
                    return view
                        .update(msg)
                        .map(move |msg| Message::NodeDiffMessage(idx, msg));
                }
            }
            Message::DiffAll => {
                let diff_tasks = self
                    .node_diff_views
                    .iter_mut()
                    .enumerate()
                    .map(|(i, view)| (i, view.update(super::nix_diff::Message::StartDiff)))
                    .map(|(idx, task)| task.map(move |msg| Message::NodeDiffMessage(idx, msg)));

                return Task::batch(diff_tasks);
            }
        }
        Task::none()
    }

    pub fn view(&self) -> Element<Message> {
        let settings_header = text("Cluster Settings").width(Length::Fill).center();

        let cluster_dir_header = text("Nix Hive Location:");

        let base_dir = self.cluster_path.as_os_str().to_string_lossy();
        let mut cluster_dir_input = text_input("Cluster Directory", base_dir.as_ref());
        let mut cluster_dir_pick_btn = button("Browse");
        if !self.loading_cluster {
            cluster_dir_input = cluster_dir_input
                .on_input(Message::ClusterPathChanged)
                .on_submit(Message::StartUpdateClusterInfo);
            cluster_dir_pick_btn = cluster_dir_pick_btn.on_press(Message::PickClusterDir);
        }
        let cluster_dir_picker = row![cluster_dir_input, cluster_dir_pick_btn];

        let cluster_dir_group =
            iced::widget::column![settings_header, cluster_dir_header, cluster_dir_picker];

        let ip_attr_header = text("Node IP Address Attribute Location:");
        let ip_attr_input =
            text_input("Attribute Path", &self.ip_attr).on_input(Message::IpAttrChanged);

        let ip_attr_group = container(iced::widget::column![ip_attr_header, ip_attr_input])
            .padding(Padding::ZERO.bottom(5).top(5));

        let node_name_header = text("Nodes").width(Length::Fill).center();
        let node_diff_all = container(button("Diff All").on_press(Message::DiffAll))
            .padding(Padding::ZERO.bottom(5).top(5));
        let currently_diffing = self
            .node_diff_views
            .iter()
            .filter(|node| node.is_diffing())
            .count();
        let total = self.node_diff_views.len();
        let node_diff_count = if currently_diffing > 0 {
            Some(text!("In Diff: {currently_diffing}/{total}").center())
        } else {
            None
        };
        let node_name_picker = selection_list(&self.all_cluster_nodes[..], Message::NodeNameChange);
        let diff_all_row = row![node_diff_all].push_maybe(node_diff_count);

        let node_name_group =
            container(column![node_name_header, diff_all_row, node_name_picker]).padding(5);

        let mut settings_and_node = column![cluster_dir_group, ip_attr_group]
            .width(Length::FillPortion(3))
            .padding(5);
        if let Some(idx) = self.current_node {
            let current_node = self
                .node_diff_views
                .get(idx)
                .map(|n| n.view().map(move |msg| Message::NodeDiffMessage(idx, msg)));
            let node_view = current_node.map(|node| {
                let node_header = text(format!("Node Diff View {}", self.all_cluster_nodes[idx]))
                    .width(Length::Fill)
                    .center();
                column![node_header, node]
            });
            settings_and_node = settings_and_node.push_maybe(node_view);
        }

        row![node_name_group, settings_and_node].into()
    }

    pub fn start_cluster_info_update(&mut self) -> Task<Message> {
        self.loading_cluster = true;
        self.all_cluster_nodes.clear();
        self.node_diff_views.clear();

        let cluster_path = self.cluster_path.clone();

        Task::future(fetch_cluster_nodes(cluster_path)).then(|res| match res {
            Ok(nodes) => Task::done(Message::UpdateClusterInfo(Some(nodes))),
            Err(err) => {
                error!("Couldn't update cluster nodes {err:?}");
                let err = err.to_string();
                Task::done(Message::UpdateClusterInfo(None)).chain(Task::done(Message::Error(err)))
            }
        })
    }
}
