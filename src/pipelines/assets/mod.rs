use crate::{pipelines::assets::bucket::AssetBucket, PacklerConfig, PacklerParams};
use log::{error, info, warn};
use serde::{Deserialize, Serialize};
use std::{fs::File, io::Write, path::PathBuf};

pub mod bucket;
pub mod images;
pub mod sass;

pub async fn deploy_assets(params: &PacklerParams, cfg: &PacklerConfig) {
    info!("building assets");
    let Ok(metadata) = build_assets_inner(params, cfg).await else {
        error!("Could not build assets.");
        return
    };

    info!("uploading assets");
    let Some(bucket_params) = &params.assets_bucket else {
        error!("Cannot deploy assets: bucket parameters were not provided");
        return;
    };

    let bucket = AssetBucket::new(bucket_params).await;
    bucket.send_assets(&cfg, &metadata).await;

    info!("writing metadata file");
    write_metadata_file(cfg, &metadata);

    info!("setting CORS config on assets bucket");
    bucket.send_cors().await;
}

pub fn write_metadata_file(config: &PacklerConfig, metadata: &AssetsOutput) {
    let content = serde_json::to_string_pretty(&metadata)
        .map_err(Error::CannotSerializeMetadataFile)
        .unwrap();

    let out_path = config.metadata_file();

    if out_path.exists() {
        std::fs::remove_file(&out_path).unwrap();
    }

    File::create(out_path)
        .and_then(|mut f| f.write_all(content.as_bytes()))
        .map_err(Error::CannotWriteMetadataFile)
        .unwrap()
}

pub fn clean_assets(cfg: &PacklerConfig) {
    images::clean_dist_dir(cfg);
    sass::clean_dist_dir(cfg);
}

pub async fn build_assets(params: &PacklerParams, cfg: &PacklerConfig) {
    info!("building assets");
    let Ok(metadata) = build_assets_inner(params, cfg).await else {
        error!("Could not build assets.");
        return
    };

    info!("writing metadata file");
    write_metadata_file(cfg, &metadata);
}

pub async fn build_assets_inner(
    params: &PacklerParams,
    cfg: &PacklerConfig,
) -> Result<AssetsOutput, Error> {
    let processed_images = match images::process(cfg) {
        Ok(images) => images,
        Err(e) => {
            warn!("Could not process images: {e}");
            Vec::default()
        }
    };

    let processed_sass = match sass::process(cfg, &params.sass_entrypoints).await {
        Ok(sass) => sass,
        Err(e) => {
            warn!("Could not process SASS files: {e}");
            Vec::default()
        }
    };

    let output = AssetsOutput {
        images: processed_images,
        sass: processed_sass,
    };

    Ok(output)
}

#[derive(Serialize, Deserialize)]
pub struct AssetsOutput {
    pub images: Vec<AssetMetadata>,
    pub sass: Vec<AssetMetadata>,
}

impl AssetsOutput {
    pub fn iter(&self) -> impl Iterator<Item = &'_ AssetMetadata> {
        self.images.iter().chain(self.sass.iter())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetMetadata {
    pub source_path: PathBuf,
    pub logical_path: PathBuf,
    pub processed_relative_path: PathBuf,

    #[serde(skip)]
    pub hash: u64,
}

#[derive(Debug)]
pub enum Error {
    EntryPointDoesNotExist(String),
    CannotSerializeMetadataFile(serde_json::Error),
    CannotWriteMetadataFile(std::io::Error),
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::CannotSerializeMetadataFile(source) => Some(source),
            Self::CannotWriteMetadataFile(source) => Some(source),
            _ => None,
        }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::EntryPointDoesNotExist(entrypoint) => {
                write!(f, "Entrypoint '{entrypoint}' does not exist")
            }
            Error::CannotSerializeMetadataFile(source) => {
                write!(f, "Could not serialize json metadata output: '{source}'")
            }
            Error::CannotWriteMetadataFile(source) => write!(f, "Cannot write file: '{source}'"),
        }
    }
}
