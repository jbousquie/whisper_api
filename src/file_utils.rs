// File utilities for Whisper API
//
// This module contains utility functions for file operations used in the Whisper API.
// It handles creating unique file paths, cleaning up temporary files, and other file-related tasks.

use log::{error, info};
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::Path;
use uuid::Uuid;

use crate::config::JobPaths;

/// Generate a unique filename with UUID and create a subfolder for it
///
/// # Arguments
///
/// * `base_dir` - Base directory for temporary files
/// * `prefix` - Prefix for the filename
/// * `extension` - File extension
///
/// # Returns
///
/// * `JobPaths` containing folder path, file path, and UUID
///
/// # Errors
///
/// Returns an IO error if directory creation fails
pub fn generate_unique_job_paths(
    base_dir: &str,
    prefix: &str,
    extension: &str,
) -> io::Result<JobPaths> {
    let uuid = Uuid::new_v4();
    let id = uuid.to_string();
    let folder_name = id.clone();
    let filename = format!("{}_{}.{}", prefix, uuid, extension);

    // Create full directory path
    let folder = Path::new(base_dir).join(&folder_name);

    // Create the directory
    fs::create_dir_all(&folder)?;

    // Full file path inside the new directory
    let audio_file = folder.join(&filename);

    Ok(JobPaths {
        folder,
        audio_file,
        id,
    })
}

/// Save uploaded file data to the filesystem
///
/// # Arguments
///
/// * `data` - Bytes to write to the file
/// * `file_path` - Path where the file should be saved
///
/// # Returns
///
/// * `Result<(), io::Error>` - Ok if successful, Err with the error otherwise
pub fn save_file_data(data: &[u8], file_path: &Path) -> io::Result<()> {
    let mut file = File::create(file_path)?;
    file.write_all(data)?;
    Ok(())
}

/// Clean up a folder and its contents
///
/// # Arguments
///
/// * `folder_path` - Path to the folder to clean up
///
/// This function logs errors but doesn't return them to the caller
pub fn cleanup_folder(folder_path: &Path) {
    if let Err(e) = fs::remove_dir_all(folder_path) {
        error!("Failed to clean up folder {}: {}", folder_path.display(), e);
    } else {
        info!("Successfully cleaned up folder: {}", folder_path.display());
    }
}

// This function was removed as it's not currently used
/*
/// Check if a file exists
///
/// # Arguments
///
/// * `path` - Path to check
///
/// # Returns
///
/// * `bool` - true if file exists, false otherwise
pub fn file_exists(path: &Path) -> bool {
    path.exists() && path.is_file()
}
*/

/// Reads a text file into a string
///
/// # Arguments
///
/// * `path` - Path to the file to read
///
/// # Returns
///
/// * `Result<String, io::Error>` - Contents of the file or an error
pub fn read_text_file(path: &Path) -> io::Result<String> {
    fs::read_to_string(path)
}
