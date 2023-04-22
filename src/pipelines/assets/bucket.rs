use super::AssetsOutput;
use crate::PacklerConfig;
use aws_config::SdkConfig;
use aws_sdk_s3::{
    config::Region,
    primitives::ByteStream,
    types::{CorsConfiguration, CorsRule, ObjectCannedAcl},
    Client,
};
use log::{debug, warn};

#[derive(Debug)]
pub struct AssetsBucketParams {
    pub bucket_name: String,

    /// Eg., "fr-par"
    pub bucket_region: String,

    /// Eg., "https://s3.fr-par.scw.cloud"
    pub bucket_endpoint_url: String,

    /// Allowed origin will be use to set the CORS rules
    pub allowed_origins: Vec<String>,
}

pub struct AssetBucket {
    client: Client,
    bucket_name: String,
    cors_config: CorsConfiguration,
}

impl AssetBucket {
    /// This will fetch the credentials from the environment.
    pub async fn new(config: &AssetsBucketParams) -> Self {
        let aws_config = aws_config::load_from_env().await;
        Self::with_aws_config(&aws_config, &config)
    }

    pub fn with_aws_config(aws_config: &SdkConfig, config: &AssetsBucketParams) -> Self {
        let s3_config = aws_sdk_s3::config::Builder::from(aws_config)
            .region(Region::new(config.bucket_region.clone()))
            .endpoint_url(&config.bucket_endpoint_url)
            .build();
        Self {
            client: aws_sdk_s3::Client::from_conf(s3_config),
            bucket_name: config.bucket_name.clone(),
            cors_config: CorsConfiguration::builder()
                .cors_rules(
                    CorsRule::builder()
                        .set_allowed_origins(Some(config.allowed_origins.clone()))
                        .allowed_headers("*")
                        .allowed_methods("GET")
                        .allowed_methods("HEAD")
                        .expose_headers("Etag")
                        .max_age_seconds(360)
                        .build(),
                )
                .build(),
        }
    }

    pub async fn send_cors(&self) {
        let res = self
            .client
            .put_bucket_cors()
            .bucket(&self.bucket_name)
            .cors_configuration(self.cors_config.clone())
            .send()
            .await;

        match res {
            Ok(_) => {}
            Err(e) => warn!("could not set CORS: {e}"),
        }
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
    pub async fn send_assets(&self, cfg: &PacklerConfig, metadata: &AssetsOutput) {
        for item in metadata.iter() {
            // We always reupload everything.
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

            let upload = self
                .client
                .put_object()
                .key(object_name)
                .bucket(&self.bucket_name)
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
}
