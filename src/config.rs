use std::{path::PathBuf, str::FromStr};

use crate::pipelines::assets::bucket::AssetsBucketParams;

pub const DEFAULT_SASS_VERSION: &str = "1.59.3";
pub const DEFAULT_OUTPUT_DIR: &str = "dist";
pub const DEFAULT_ASSETS_DIR: &str = "assets";
pub const DEFAULT_IMAGES_DIR: &str = "images";
pub const DEFAULT_SASS_DIR: &str = "css";
pub const DEFAULT_METADATA_FILENAME: &str = "assets.json";

pub struct PacklerParams {
    /// The SASS entry points. They will be compiled to CSS.
    pub sass_entrypoints: Vec<PathBuf>,

    /// The names of the backend crate.
    pub backend_crate: Option<String>,

    /// The names of the frontend crates.
    pub frontend_crates: Vec<String>,

    /// Optional
    pub bucket_asset: Option<AssetsBucketParams>,
}

impl PacklerParams {
    pub fn new<P, E, C, S>(
        sass_entrypoints: E,
        frontend_crates: C,
        backend_crate: Option<S>,
        static_bucket_name: Option<S>,
    ) -> Self
    where
        P: Into<PathBuf>,
        S: Into<String>,
        E: IntoIterator<Item = P>,
        C: IntoIterator<Item = S>,
    {
        Self {
            sass_entrypoints: sass_entrypoints.into_iter().map(Into::into).collect(),
            backend_crate: backend_crate.map(Into::into),
            frontend_crates: frontend_crates.into_iter().map(Into::into).collect(),
            bucket_asset: , //static_bucket_name.map(Into::into),
        }
    }
}

/// The configuration is editable by the user but Packler aims to provide
/// sensible defaults.
#[derive(Clone)]
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
        let target = crate::cargo_metadata()
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
