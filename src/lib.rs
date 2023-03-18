//! Configuration:
//! - dist folder: final folder where all the generated stuff will be placed.
//! - metadata.toml location. Do we put it in the dist folder? Probably no
//!   because we want it next to the server.
//! - offline mode (do not download external tools)
//!
use crate::{
    cli::build_parser,
    pipelines::assets::{build_assets, deploy_assets},
};
use clap::{Args, Parser};
use lazy_static::lazy_static;
use log::{debug, info, trace};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use pipelines::assets::clean_assets;
use std::{
    fmt::Display,
    path::{Path, PathBuf},
    str::FromStr,
    time::{Duration, Instant},
};

pub mod common;
pub mod pipelines;
pub mod tools;

pub const DEFAULT_SASS_VERSION: &str = "1.59.3";
pub const DEFAULT_OUTPUT_DIR: &str = "dist";
pub const DEFAULT_ASSETS_DIR: &str = "assets";
pub const DEFAULT_IMAGES_DIR: &str = "images";
pub const DEFAULT_SASS_DIR: &str = "css";
pub const DEFAULT_METADATA_FILENAME: &str = "assets.json";

/// Fetch the metadata of the crate.
pub fn cargo_metadata() -> &'static cargo_metadata::Metadata {
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

#[derive(Debug)]
pub struct PacklerParams {
    /// The SASS entry points. They will be compiled to CSS
    pub sass_entrypoints: Vec<PathBuf>,

    /// The names of the backend crate.
    pub backend_crate: Option<String>,

    /// The names of the frontend crates.
    pub frontend_crates: Vec<String>,

    /// The name of the bucket in which the assets and compiled frontends
    /// will be uploaded.
    pub static_bucket_name: Option<String>,
}

impl PacklerParams {
    pub fn new<P, E, C, S>(
        sass_entrypoints: E,
        frontend_crates: C,
        backend_crate: Option<S>,
        static_bucket_name: Option<S>,
    ) -> Self
    where
        P: Into<PathBuf> + Send + Clone,
        S: Into<String> + Send + Clone,
        E: IntoIterator<Item = P>,
        C: IntoIterator<Item = S>,
    {
        Self {
            sass_entrypoints: sass_entrypoints.into_iter().map(Into::into).collect(),
            backend_crate: backend_crate.map(Into::into),
            frontend_crates: frontend_crates.into_iter().map(Into::into).collect(),
            static_bucket_name: static_bucket_name.map(Into::into),
        }
    }
}

/// Entry point for the Packler configuration. The configuration covers all the
/// aspects of Packler at once.
#[derive(Debug, Clone)]
pub struct PacklerConfig {
    /// Directory where are located the assets we want to process (images,
    /// css/sass).
    ///
    /// Default: the `assets` directory at the root of the workspace
    pub assets_source_dir: PathBuf,

    /// The subdirectory of [`Self::assets_source_dir`] that contains
    /// the images that need to be processed.
    /// Default: [`DEFAULT_IMAGES_DIR`]
    pub images_dir_name: String,

    /// The subdirectory of [`Self::assets_source_dir`] that contains
    /// the images that need to be processed.
    /// Default: [`DEFAULT_SASS_DIR`]
    pub sass_dir_name: String,

    /// The Sass version to use
    /// Default [`DEFAULT_SASS_VERSION`]
    pub sass_version: String,

    /// The target folder where we put compiled items.
    ///
    /// Default: the target as found by [Metadata.target_directory()][1].
    ///
    /// [1]: https://docs.rs/cargo_metadata/latest/cargo_metadata/struct.Metadata.html#structfield.target_directory
    pub target: PathBuf,

    /// The final directory where all the processed assets and frontends will be
    /// stored. Typically, the content of this directory can be served by a
    /// dedicated HTTP server or sent to a CDN.
    ///
    /// Default: the [`DEFAULT_OUTPUT_DIR`] directory at the root of the workspace
    pub dist_dir: PathBuf,

    /// The name of the final Metadata file. This file will lie in the
    /// [`Self::dist_dir`].
    pub metadata_filename: String,
}

impl Default for PacklerConfig {
    fn default() -> Self {
        let target = cargo_metadata()
            .target_directory
            .clone()
            .into_std_path_buf();
        Self {
            assets_source_dir: PathBuf::from_str(DEFAULT_ASSETS_DIR).unwrap(),
            images_dir_name: DEFAULT_IMAGES_DIR.to_owned(),
            sass_dir_name: DEFAULT_SASS_DIR.to_owned(),
            sass_version: DEFAULT_SASS_VERSION.to_owned(),
            target,
            dist_dir: PathBuf::from_str(DEFAULT_OUTPUT_DIR).unwrap(),
            metadata_filename: DEFAULT_METADATA_FILENAME.to_owned(),
        }
    }
}

impl PacklerConfig {
    pub fn metadata_file(&self) -> PathBuf {
        self.dist_dir.join(&self.metadata_filename)
    }

    pub fn source_image_dir(&self) -> PathBuf {
        self.assets_source_dir.join(&self.images_dir_name)
    }

    pub fn dist_image_dir(&self) -> PathBuf {
        self.dist_dir.join(&self.images_dir_name)
    }

    pub fn source_sass_dir(&self) -> PathBuf {
        self.assets_source_dir.join(&self.sass_dir_name)
    }

    pub fn dist_sass_dir(&self) -> PathBuf {
        self.dist_dir.join(&self.sass_dir_name)
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

#[derive(Debug)]
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
            // FIXME: this might be improved for finer control in case of
            // invalid component name. Do we want to stop the whole execution
            // or try to gobe the error, display a warning and continue the
            // execution with the valid part?
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

    pub async fn start(&self) {
        // There are implicit dependencies between actions, for example to
        // deploy, we must build first.
        //
        // Is there also implicit deps between components? Something like 'you
        // must first build the assets before the frontend' ?
        //
        // Steps could be optimized later (eg. by fingerprinting the build
        // maybe?) Clean can be a simple delete of all the working dirs for the
        // component
        //
        // In any case, it might be useful to lay out the plan of execution to
        // the user (FIXME?)
        //
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
