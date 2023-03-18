use crate::{pipelines::assets::bucket::send_cors, PacklerConfig, PacklerParams};
use aws_sdk_s3::{model::ObjectCannedAcl, types::ByteStream};
use log::{debug, error, info, warn};
use serde::{Deserialize, Serialize};
use std::{fs::File, io::Write, path::PathBuf};

use self::bucket::init_s3_client;

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
    upload_assets(params, cfg, &metadata).await;

    info!("writing metadata file");
    write_metadata_file(cfg, &metadata);

    if let Some(bucket) = &params.static_bucket_name {
        info!("Setting CORS config on bucket: '{bucket}'");
        let client = bucket::init_s3_client().await;
        send_cors(&client, bucket).await;
    }
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

/// Uploads all the assets listed in `metadata`.
///
/// Beware that this is an additive process, we will upload the assets _without_
/// removing the old ones. You can (and should) remove the old ones in a
/// different step.
///
/// It is designed this way so you can serve multiple version of the assets at
/// the same time (e.g., you have a rollout deploy and different versions of the
/// app might be running at the same time).
///
pub async fn upload_assets(params: &PacklerParams, cfg: &PacklerConfig, metadata: &AssetsOutput) {
    let Some(bucket_name) = &params.static_bucket_name else {
        error!("There are no specified bucket in which we want to deploy");
        return
    };

    let client = init_s3_client().await;

    for item in metadata.iter() {
        // FIXME: improvement: do not re-upload a file if it's already there.
        // we have a checksum in the filename so that shouldn't be a problem.
        let src = cfg.dist_dir.join(&item.processed_relative_path);
        let object_name = item.processed_relative_path.to_string_lossy();
        let mime_type = mime_guess::from_path(&src)
            .first_raw()
            .expect("could not get content type");

        debug!(
            "Uploading '{}' to: '{}' (content-type: '{}'))",
            src.display(),
            object_name,
            mime_type
        );

        let stream = ByteStream::from_path(&src)
            .await
            .expect("Could not open file to upload");

        let upload = client
            .put_object()
            .key(object_name)
            .bucket(bucket_name)
            .acl(ObjectCannedAcl::PublicRead)
            .content_type(mime_type)
            .body(stream)
            .send();

        match upload.await {
            Ok(_resp) => debug!("Asset Uploaded"),
            Err(err) => warn!("Could not upload {}: {err:?}", src.display()),
        }
    }
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
