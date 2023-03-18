use super::AssetMetadata;
use crate::PacklerConfig;
use log::{debug, info, trace, warn};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use walkdir::WalkDir;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageProcessOutput {
    pub generated_at: u64,
    pub source_dir: PathBuf,
    pub files: Vec<AssetMetadata>,
}

pub fn process(config: &PacklerConfig) -> Result<Vec<AssetMetadata>, Box<dyn std::error::Error>> {
    let images_dir = config.source_image_dir();

    info!("IMG: Collecting all images metadata");
    let images: Vec<AssetMetadata> = WalkDir::new(&images_dir)
        .into_iter()
        .filter_map(|entry| {
            match entry {
                Ok(entry) => {
                    if entry.path().is_file() {
                        let relative_path = entry
                            .path()
                            .strip_prefix(&config.assets_source_dir)
                            .unwrap();

                        debug!(
                            "IMG: {} (relative: {})",
                            entry.path().display(),
                            relative_path.display()
                        );

                        let image_content = std::fs::read(entry.path()).unwrap();
                        let hash = seahash::hash(&image_content);

                        // file_stem() instead of file_prefix() otherwise we would
                        // lose a component if there are two '.' in the filename.
                        let hashed_name = format!(
                            "{}-{:x}.{}",
                            relative_path.file_stem().unwrap().to_string_lossy(),
                            hash,
                            relative_path.extension().unwrap().to_string_lossy()
                        );

                        Some(AssetMetadata {
                            source_path: entry.path().to_owned(),
                            logical_path: relative_path.to_owned(),
                            processed_relative_path: relative_path.with_file_name(hashed_name),
                            hash,
                        })
                    } else {
                        trace!("{} is not a file. Skip", entry.path().display());
                        None
                    }
                }
                Err(e) => {
                    warn!("Could not walk into images: {e}");
                    None
                }
            }
        })
        .collect();

    info!("IMG: Cleaning destination directory");
    clean_dist_dir(config);

    // Actual file copy
    for image in images.iter() {
        let dest_path = config.dist_dir.join(&image.processed_relative_path);

        if let Some(dir) = dest_path
            .parent() { std::fs::create_dir_all(dir).expect("Could not create final directory") }

        std::fs::copy(&image.source_path, &dest_path).unwrap();
    }

    Ok(images)
}

pub fn clean_dist_dir(cfg: &PacklerConfig) {
    let images_dir = cfg.dist_image_dir();

    if images_dir.exists() {
        std::fs::remove_dir_all(&images_dir)
            .unwrap_or_else(|_| panic!("Could not remove '{}'", images_dir.display()))
    }
}
