use crate::utils::ansi_to_rich::{ansi_to_spans, make_spans};
use anyhow::{Context, bail};
use async_stream::stream;
use duct::cmd;
use futures::Stream;
use iced::keyboard::key::Code::Comma;
use iced::widget::text::{Rich, Span};
use iced::widget::{
    TextInput, button, column, container, pick_list, progress_bar, rich_text, row, scrollable,
    text, text_input,
};
use iced::{Border, Color, Element, Font, Length, Padding, Task};
use iced_futures::core::Background;
use log::{debug, error};
use serde_json::json;
use ssh2_config::{ParseRule, SshConfig};
use std::borrow::Cow;
use std::cell::OnceCell;
use std::env;
use std::ffi::OsStr;
use std::fs::File;
use std::net::{IpAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::process::{Command, Stdio};
use std::task::Poll;
use crate::pages::nix_diff::cache::DiffCache;

#[derive(Debug, Clone)]
pub enum Message {
    StartDiff,
    IpAttrChanged(String),
    DiffResult(Option<String>),
    Error(String),
    DiffProgress(f32),
}

mod cache {
    use std::mem;
    use iced::advanced::text::Span;
    use iced::widget::rich_text;
    use iced::widget::text::Rich;
    use crate::pages::nix_diff::Message;
    use crate::utils::ansi_to_rich::{ansi_to_spans, make_spans};

    // Uh-uh.. No touching.
    // This is locked away because it's self-referential.
    pub struct DiffCache {
        raw: String,
        spans: Vec<Span<'static, Message>>,
    }

    impl DiffCache {
        pub fn new(diff: String) -> Self {
            unsafe {
                let static_diff: &'static str = mem::transmute(diff.as_str());
                let raw_spans = ansi_to_spans(&static_diff);
                let spans = make_spans(&raw_spans);

                Self { raw: diff, spans }
            }
        }

        pub fn spans(&self) -> &[Span<'static, Message>] {
            &self.spans
        }
    }
}

pub struct NixNodeDiffView {
    node_path: PathBuf,
    ip_attr: String,
    node_name: String,
    diff: Option<DiffCache>,
    loading_diff: bool,
    error: Option<String>,
    diff_progress: f32,
}

impl NixNodeDiffView {
    pub fn is_diffing(&self) -> bool {
        self.loading_diff
    }
}

impl NixNodeDiffView {
    pub fn new(cluster_path: PathBuf, ip_attr: String, node_name: String) -> Self {
        Self {
            node_path: cluster_path,
            ip_attr,
            node_name,
            diff: None,
            loading_diff: false,
            error: None,
            diff_progress: 0.0,
        }
    }
}

impl NixNodeDiffView {
    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::StartDiff => {
                if !self.loading_diff {
                    return self.run_diff_task();
                }
            }
            Message::IpAttrChanged(mut ip_attr) => {
                if ip_attr.is_empty() {
                    ip_attr = "config.base.primaryIP.address".to_owned();
                }
                self.ip_attr = ip_attr;
            }
            Message::DiffResult(diff) => {
                self.loading_diff = false;
                self.diff = diff.map(DiffCache::new);
            }
            Message::DiffProgress(progress) => {
                self.diff_progress = progress;
            }
            Message::Error(err) => {
                self.error = Some(err.to_string());
            }
        }

        Task::none()
    }

    pub fn view(&self) -> Element<Message> {
        let ip_attr_header = text("Node IP Address Attribute Location:");
        let ip_attr_input =
            text_input("Attribute Path", &self.ip_attr).on_input(Message::IpAttrChanged);

        let ip_attr_group = container(column![ip_attr_header, ip_attr_input])
            .padding(Padding::ZERO.bottom(5).top(5));

        let mut run_diff_btn = button("Run Diff");
        if !self.loading_diff {
            run_diff_btn = run_diff_btn.on_press(Message::StartDiff);
        }

        let progress_bar = progress_bar(0.0..=10.0, self.diff_progress).height(Length::Fixed(5.));

        let error_txt = text(self.error.as_deref().unwrap_or(""))
            .color(Color::new(1.0, 0.2, 0.2, 1.0))
            .width(Length::Fill)
            .center();

        let top =
            container(column![ip_attr_group, run_diff_btn, error_txt, progress_bar,].padding(50))
                .style(|theme| {
                    let mut style = container::rounded_box(theme);
                    style.background = None;
                    style
                });

        let diff_log = if let Some(diff) = &self.diff {
            let rich_diff = rich_text(diff.spans()).font(Font::MONOSPACE);
            container(scrollable(rich_diff))
                .padding(5)
                .style(container::dark)
                .width(Length::Fill)
                .height(Length::Fill)
        } else {
            container(column![])
                .padding(5)
                .style(container::dark)
                .width(Length::Fill)
                .height(Length::Fill)
        };

        let main = column![top, diff_log];
        container(main).into()
    }

    pub fn run_diff_task(&mut self) -> Task<Message> {
        self.loading_diff = true;

        let cluster_path = self.node_path.clone();
        let node_name = self.node_name.clone();
        let ip_attr = self.ip_attr.clone();

        Task::stream(run_diff(cluster_path, node_name, ip_attr)).then(|res| match res {
            Ok(msg) => Task::done(msg),
            Err(err) => {
                error!("Failed to diff: {err:?}");
                let err = err.to_string();
                Task::done(Message::DiffResult(None))
                    .chain(Task::done(Message::Error(err)))
                    .chain(Task::done(Message::DiffProgress(0.0)))
            }
        })
    }
}

fn ip_from_node(cluster_path: &PathBuf, node_name: &str, ip_attr: &str) -> anyhow::Result<IpAddr> {
    let ip_json = if cluster_path.ends_with("flake.nix") {
        let args = [
            "eval",
            &format!(".#nixosConfigurations.{node_name}.{ip_attr}"),
            "--json",
        ];
        run_nix_command_in_dir(&cluster_path, &args)
    } else {
        todo!("only flakes are supported right now")
    }?;

    let ip_str = serde_json::from_str::<String>(&ip_json)
        .with_context(|| format!("Couldn't parse JSON {ip_json:?}"))?;

    Ok(ip_str.parse()?)
}

pub async fn fetch_cluster_nodes(cluster_path: PathBuf) -> anyhow::Result<Vec<String>> {
    if !cluster_path.is_dir() {
        return fetch_nodes_from_file(&cluster_path);
    }

    let flake = cluster_path.join("flake.nix");
    if let Ok(nodes) = fetch_nodes_from_flake(&flake) {
        return Ok(nodes);
    }

    let hive = cluster_path.join("hive.nix");
    if let Ok(nodes) = fetch_nodes_from_legacy(&hive) {
        return Ok(nodes);
    }

    bail!("Couldn't get proper nodes from flake.nix or hive.nix");
}

pub fn run_diff(
    cluster_path: PathBuf,
    node_name: String,
    ip_attr: String,
) -> impl Stream<Item = anyhow::Result<Message>> {
    stream! {
        yield Ok(Message::DiffProgress(0.0));

        let ip = ip_from_node(&cluster_path, &node_name, &ip_attr)
            .with_context(|| "Couldn't find IP Address of Node {node_name}")?;
        yield Ok(Message::DiffProgress(1.0));

        let cluster_path = cluster_path
            .parent()
            .context("Couldn't get cluster directory")?;
        yield Ok(Message::DiffProgress(2.0));

        let new_drv: PathBuf = cmd!(
            "nix",
            "build",
            format!(".#nixosConfigurations.{node_name}.config.system.build.toplevel"),
            "--print-out-paths"
        )
        .dir(&cluster_path)
        .read()
        .context("Couldn't build local node")?
        .into();
        yield Ok(Message::DiffProgress(3.0));

        let ip_str = ip.to_string();
        let ssh_config = SshConfig::parse_default_file(ParseRule::STRICT)?;
        yield Ok(Message::DiffProgress(4.0));

        let params = ssh_config.query(&ip_str);
        let addr = params
            .bind_address
            .and_then(|addr| addr.parse().ok())
            .unwrap_or_else(|| ip.clone());
        let port = params.port.unwrap_or(22);
        let username = params.user.unwrap_or_else(whoami::username).to_string();

        let connection = TcpStream::connect((addr, port))?;
        yield Ok(Message::DiffProgress(5.0));

        let mut session = ssh2::Session::new().expect("Couldn't create ssh session");
        session.set_tcp_stream(connection);
        session.handshake()?;
        yield Ok(Message::DiffProgress(6.0));

        session.userauth_agent(&username)?;
        yield Ok(Message::DiffProgress(7.0));

        let sftp = session.sftp()?;
        yield Ok(Message::DiffProgress(8.0));

        let system_drv = sftp.realpath(Path::new("/nix/var/nix/profiles/system/system"))?;
        yield Ok(Message::DiffProgress(9.0));

        debug!("Copying {system_drv:?} from host");

        drop(session);
        drop(sftp);

        cmd!("nix-copy-closure", "--from", ip_str, &system_drv)
            .run()
            .context("Couldn't download system closure")?;
        yield Ok(Message::DiffProgress(10.0));

        debug!("Diffing: {system_drv:?} against {new_drv:?}");

        let diff_out = cmd!("nvd", "--color", "always", "diff", system_drv, new_drv)
            .read()
            .context("Couldn't diff the two derivations")?;

        yield Ok(Message::DiffResult(Some(diff_out)));
    }
}

fn is_nix_file(path: &Path) -> bool {
    path.extension() == Some(OsStr::new("nix"))
}

fn fetch_nodes_from_file(path: &Path) -> anyhow::Result<Vec<String>> {
    if path.ends_with("flake.nix") {
        fetch_nodes_from_flake(path)
    } else if is_nix_file(path) {
        fetch_nodes_from_legacy(path)
    } else {
        bail!("Not a nix file. Cannot fetch nodes");
    }
}

fn fetch_nodes_from_legacy(hive: &Path) -> anyhow::Result<Vec<String>> {
    const LEGACY_ARGS: &[&str] = &[
        "eval",
        "-E",
        r#"builtins.attrNames (builtins.removeAttrs (import ./hive.nix { nixpkgs = <nixpkgs>; }) ["defaults" "meta"])"#,
        "--json",
        "--impure",
    ];
    nodes_from_nix_command(hive, &LEGACY_ARGS)
}

pub fn fetch_nodes_from_flake(flake: &Path) -> anyhow::Result<Vec<String>> {
    const FLAKE_ARGS: &[&str] = &[
        "eval",
        r#".#nixosConfigurations"#,
        "--json",
        "--apply",
        "builtins.attrNames",
    ];
    nodes_from_nix_command(flake, &FLAKE_ARGS)
}

fn run_nix_command_in_dir(file_path: &Path, args: &[&str]) -> anyhow::Result<String> {
    if !file_path.is_file() {
        bail!("Nix Cluster path is not a file");
    }

    let parent = file_path
        .parent()
        .context("Cluster path file didn't have a parent folder.")?;

    cmd("nix", args)
        .dir(parent)
        .read()
        .context("Failed to run nix command")
}

fn nodes_from_nix_command(file_path: &Path, args: &[&str]) -> anyhow::Result<Vec<String>> {
    let output = run_nix_command_in_dir(file_path, args)?;
    nodes_from_json(&output)
}

fn nodes_from_json(json_output: &str) -> anyhow::Result<Vec<String>> {
    let json = serde_json::from_str::<serde_json::Value>(json_output)
        .with_context(|| format!("Couldn't parse json from nix output: {json_output}"))?;

    json.as_array()
        .map(|nodes| {
            nodes
                .iter()
                .filter_map(|node| node.as_str().map(ToOwned::to_owned))
                .collect()
        })
        .context("Invalid command output. Expected array of nodes.")
}
