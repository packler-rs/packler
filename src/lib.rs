use crate::{
    cli::build_parser,
    pipelines::assets::{build_assets, deploy_assets},
};
use clap::{Args, Parser};
pub use config::{PacklerConfig, PacklerParams};
use lazy_static::lazy_static;
use log::{debug, info, trace};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use pipelines::assets::clean_assets;
use std::{
    fmt::Display,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

pub mod common;
pub mod config;
pub mod pipelines;
pub mod tools;

/// Fetch the metadata of the crate.
pub(crate) fn cargo_metadata() -> &'static cargo_metadata::Metadata {
    lazy_static! {
        static ref METADATA: cargo_metadata::Metadata = cargo_metadata::MetadataCommand::new()
            .exec()
            .expect("cannot get crate's metadata");
    }

    &METADATA
}

#[derive(Debug)]
enum Error {
    /// The given component does not exist.
    UnknownComponent(String),
}

impl std::error::Error for Error {}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownComponent(component) => {
                write!(f, "Component '{component}' does not exist")
            }
        }
    }
}

#[derive(Args, Debug)]
pub struct BuildOpts {
    pub watch: bool,
}

#[derive(Parser, Debug)]
pub enum Action {
    Build(BuildOpts),
    Clean,
    Deploy,
    Unknown,
}

#[derive(Debug, Clone)]
pub enum Component {
    Backend,
    /// Assets are the things that should be served but that are not code per
    /// se. This includes images, CSS
    Assets,
    Frontend(String),
}

impl Component {
    fn new<S: AsRef<str>>(value: S) -> Result<Self, Error> {
        match value.as_ref().to_lowercase().as_str() {
            "backend" => Ok(Component::Backend),
            "assets" => Ok(Component::Assets),
            "frontend" => Ok(Component::Frontend("FIXME".to_owned())),
            unknown => Err(Error::UnknownComponent(unknown.to_owned())),
        }
    }
}

pub fn path_to_watch(
    params: &PacklerParams,
    config: &PacklerConfig,
    component: &Component,
) -> Option<PathBuf> {
    match component {
        Component::Backend => params.backend_crate.as_ref().and_then(|crate_name| {
            // Pretty basic by default.
            // We watch the directory where the Cargo.toml file lies.
            cargo_metadata()
                .workspace_packages()
                .into_iter()
                .find(|p| &p.name == crate_name)
                .and_then(|p| p.manifest_path.parent())
                .map(|p| p.to_owned().into_std_path_buf())
        }),
        Component::Assets => Some(config.assets_source_dir.clone()),
        Component::Frontend(_) => None, // FIXME
    }
}

pub struct Run {
    pub params: PacklerParams,
    pub config: PacklerConfig,
    pub action: Action,
    pub components: Vec<Component>,
}

impl Run {
    /// the `buildable_components` param lists all the possible components that
    /// can be built.
    pub fn new(params: PacklerParams, config: PacklerConfig) -> Self {
        debug!("Start Manual arg parsing");

        let clap = build_parser();
        let parsed = clap.get_matches();

        let raw_components: Vec<String> = parsed
            .get_many::<String>("components")
            .unwrap_or_default()
            .cloned()
            .collect();

        let action = match parsed.subcommand() {
            Some(("build", args)) => {
                // Option Watch
                let watch = args.get_flag("watch");
                Action::Build(BuildOpts { watch })
            }
            Some(("clean", _args)) => Action::Clean,
            Some(("deploy", _args)) => Action::Deploy,
            Some((cmd_name, _args)) => {
                debug!("Action {cmd_name} is unkown");
                Action::Unknown
            }
            None => {
                debug!("No subcommand was provided");
                Action::Unknown
            }
        };

        let buildable_components = vec![Component::Backend, Component::Assets];

        let components = if raw_components.is_empty() {
            buildable_components
        } else {
            raw_components
                .iter()
                .filter_map(|name| Component::new(name).ok())
                .collect()
        };

        Self {
            config,
            params,
            action,
            components,
        }
    }

    /// Starth the Run. This will spawn an async runtime so the user does not
    /// need to provide it.
    pub fn start(&self) {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                println!("Hello world");
                self.start_async().await;
            });
    }

    /// Start the Run when you are already in an async context.
    async fn start_async(&self) {
        match &self.action {
            Action::Build(opts) => {
                for component in &self.components {
                    match component {
                        Component::Assets => {
                            let action = || async {
                                info!("Building assets");
                                build_assets(&self.params, &self.config).await;
                            };

                            action().await;

                            if opts.watch {
                                info!("Setting up Watcher");

                                let mut latest_run = Instant::now();
                                let debounce = Duration::from_secs(2);

                                let to_watch =
                                    path_to_watch(&self.params, &self.config, component).unwrap();

                                let (tx, rx) = std::sync::mpsc::channel();
                                let mut watcher =
                                    RecommendedWatcher::new(tx, notify::Config::default()).unwrap();

                                info!("Start to watch: {to_watch:?}, dir? {}", to_watch.is_dir());

                                watcher
                                    .watch(Path::new(&to_watch), RecursiveMode::Recursive)
                                    .unwrap();

                                while let Ok(res) = rx.recv() {
                                    match res {
                                        Ok(event) => {
                                            if latest_run.elapsed() > debounce {
                                                // The debounce here is quite gross as it is not scoped.
                                                let changed = event
                                                    .paths
                                                    .iter()
                                                    .map(|p| format!("{p:?}"))
                                                    .collect::<Vec<String>>()
                                                    .join(", ");
                                                info!("Modified File '{changed}'. Reload");
                                                action().await;
                                                latest_run = Instant::now();
                                            } else {
                                                // Ignore event.
                                                trace!("Debounce on '{event:?}'.")
                                            }
                                        }
                                        Err(e) => println!("watch error: {:?}", e),
                                    }
                                }
                            }
                        }
                        Component::Backend => {
                            unimplemented!("Backend build is not implemented yet")
                        }
                        Component::Frontend(_) => {
                            unimplemented!("Frontend build is not implemented yet")
                        }
                    }
                }
            }
            Action::Clean => {
                for component in &self.components {
                    match component {
                        Component::Assets => {
                            info!("Cleaning assets");
                            clean_assets(&self.config);
                        }
                        Component::Backend => {
                            unimplemented!("Backend clean is not implemented yet")
                        }
                        Component::Frontend(_) => {
                            unimplemented!("Frontend clean is not implemented yet")
                        }
                    }
                }
            }
            Action::Deploy => {
                for component in &self.components {
                    match component {
                        Component::Assets => {
                            info!("Deploying assets");
                            deploy_assets(&self.params, &self.config).await;
                        }
                        Component::Backend => {
                            unimplemented!("Backend deploy is not implemented yet")
                        }
                        Component::Frontend(_) => {
                            unimplemented!("Frontend deploy is not implemented yet")
                        }
                    }
                }
            }
            Action::Unknown => unimplemented!("This action is not implemented yet."),
        }
    }
}

pub mod cli {
    use clap::{Arg, ArgAction, Command};

    pub fn build_parser() -> Command {
        Command::new("xtask")
            .about("Packler xtasks")
            .arg(
                Arg::new("components")
                    .short('c')
                    .long("components")
                    .action(ArgAction::Append)
                    .global(true)
                    .help("List the components to build. Eg., -c frontend -c backend"),
            )
            .arg_required_else_help(true)
            .subcommand_required(true)
            .subcommand(
                Command::new("build").about("Build").arg(
                    Arg::new("watch")
                        .short('w')
                        .long("watch")
                        .action(ArgAction::SetTrue)
                        .help("Automatically rebuild the component(s) if their source changes"),
                ),
            )
            .subcommand(Command::new("clean").about("Clean "))
            .subcommand(Command::new("deploy").about("Deploy"))
    }
}
