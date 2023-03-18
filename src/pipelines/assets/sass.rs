//! Shamelessly stolen from Trunk:
//! https://github.com/thedodd/trunk/blob/master/src/pipelines/sass.rs
//!
//! The flow is as followed:
//!
//! - clean the intermediate folder & dist/css folder
//! - generate output for each entrypoint
//! - copy all the output to the dist/css.
//!

use crate::common::{self};
use crate::pipelines::assets::{AssetMetadata, Error};
use crate::tools::{self, Application};
use crate::PacklerConfig;
use futures_util::future::join_all;
use log::{error, info, warn};
use serde::{Deserialize, Serialize};
use std::iter::Iterator;
use std::path::{Path, PathBuf};

pub fn clean_dist_dir(cfg: &PacklerConfig) {
    let sass_dir = cfg.dist_sass_dir();

    if sass_dir.exists() {
        std::fs::remove_dir_all(&sass_dir)
            .unwrap_or_else(|_| panic!("Could not remove {}", sass_dir.display()))
    }
}

pub async fn process<E, P>(
    config: &PacklerConfig,
    entry_points: E,
) -> Result<Vec<AssetMetadata>, Box<dyn std::error::Error>>
where
    P: AsRef<Path> + Send + Clone,
    E: IntoIterator<Item = P>,
{
    let sass_cfg = SassRun {
        config: config.clone(),
    };
    sass_cfg.start(entry_points).await
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SassOutput {
    pub generated_at: u64,
    pub files: Vec<AssetMetadata>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SassEntrypointOutput {
    pub original_file_name: PathBuf,
    pub output_file_name: PathBuf,
    pub hash: String,
}

pub struct SassRun {
    config: PacklerConfig,
}

impl SassRun {
    pub fn intermediate_dir(&self) -> PathBuf {
        // FIXME: hardcoded path
        self.config.target.join("packler").join("sass")
    }

    pub fn clean_intermediate_folder(&self) {
        let dir = self.intermediate_dir();

        if dir.exists() {
            match std::fs::remove_dir_all(&dir) {
                Ok(()) => info!("SASS: Intermediate folder cleared"),
                Err(e) => warn!("SASS: Could not remove intermediate folder: {e}"),
            }
        }
    }

    /// Spawn the pipeline for this asset type.
    pub async fn start<P: AsRef<Path> + Send + Clone, E: IntoIterator<Item = P>>(
        self,
        entrypoints: E,
    ) -> Result<Vec<AssetMetadata>, Box<dyn std::error::Error>> {
        info!("SASS: Start SASS Pipeline");

        let sass = tools::get(Application::Sass, Some(&self.config.sass_version)).await?;

        self.clean_intermediate_folder();
        clean_dist_dir(&self.config);

        let futures = entrypoints
            .into_iter()
            .map(|entry| self.run(&sass, entry, false));
        let results = join_all(futures).await;

        // Only copy to final dist if all files are OK.

        let files = results
            .iter()
            .filter_map(|r| match r {
                Ok(output) => Some(output.clone()),
                Err(_e) => None,
            })
            .collect();

        // FIXME: Return the errors as well!
        Ok(files)
    }

    pub async fn run<P: AsRef<Path> + Send>(
        &self,
        sass_path: &PathBuf,
        entrypoint: P,
        compress: bool,
    ) -> Result<AssetMetadata, Box<dyn std::error::Error>> {
        let style = if compress { "compressed" } else { "expanded" };

        let original_path = self.config.source_sass_dir().join(&entrypoint);

        if !original_path.exists() {
            error!(
                "Entrypoint '{}' does not exist.",
                &entrypoint.as_ref().display()
            );
            return Err(Box::new(Error::EntryPointDoesNotExist(
                entrypoint.as_ref().display().to_string(),
            )));
        }

        let path_str = original_path.display().to_string();
        let entrypoint_filestem = original_path.file_stem().unwrap().to_string_lossy();

        let mut prehash_file_path = self.intermediate_dir();
        prehash_file_path.push(&entrypoint);
        prehash_file_path.set_extension("css");

        let args = &[
            "--no-source-map",
            "-s",
            style,
            &path_str,
            &prehash_file_path.display().to_string(),
        ];

        // SASS Compile
        log::info!("SASS: compiling sass/scss (into {prehash_file_path:?})");
        common::run_command(Application::Sass.name(), sass_path, args).await?;

        // Hash Content
        log::info!("SASS: hashing file content");
        let css = tokio::fs::read_to_string(&prehash_file_path).await?;
        let hash = seahash::hash(css.as_bytes());

        // Copy to intermediate dir
        let final_file_name = format!("{entrypoint_filestem}-{hash:x}.css");
        let mut final_file_path = self.config.dist_sass_dir();
        final_file_path.push(&entrypoint);
        final_file_path.set_file_name(&final_file_name);

        log::info!("SASS: moving file to final destination '{final_file_path:?}");

        if let Some(dir) = final_file_path.parent() {
            std::fs::create_dir_all(dir).expect("Could not create final directory")
        }

        // Using fs::rename with SELinux would _not_ set the right label on the
        // new file. It would stay `unlabeled_t`. This is annoying if we want to
        // serve those files from a container for example (it would need the
        // `container_file_t` label.)
        // Doing the copy+remove circumvents the issue ¯\_(ツ)_/¯
        std::fs::copy(&prehash_file_path, &final_file_path)
            .expect("error copying the compiled SASS file");

        std::fs::remove_file(&prehash_file_path)
            .expect("error deleting the intermediate SASS file");

        let metadata = AssetMetadata {
            source_path: original_path.clone(),
            logical_path: original_path
                .strip_prefix(&self.config.assets_source_dir)
                .unwrap()
                .into(),
            processed_relative_path: final_file_path
                .strip_prefix(&self.config.dist_dir)
                .unwrap()
                .into(),
            hash,
        };

        Ok(metadata)
    }
}
