// File utilities for Whisper API
//
// This module contains utility functions for file operations used in the Whisper API.
// It handles creating unique file paths, cleaning up temporary files, and other file-related tasks.

use log::{error, info, debug};
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
/// Returns an IO error if directory creation fails or if paths are invalid
pub fn generate_unique_job_paths(
    base_dir: &str,
    prefix: &str,
    extension: &str,
) -> io::Result<JobPaths> {
    // Validate base directory
    let base_path = Path::new(base_dir);
    if !base_path.exists() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("Base directory does not exist: {}", base_dir)
        ));
    }
    
    if !base_path.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("Base path is not a directory: {}", base_dir)
        ));
    }

    // Validate prefix and extension
    if prefix.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Prefix cannot be empty"
        ));
    }
    
    if extension.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Extension cannot be empty"
        ));
    }

    let uuid = Uuid::new_v4();
    let id = uuid.to_string();
    let folder_name = id.clone();
    let filename = format!("{}_{}.{}", prefix, uuid, extension);

    // Create full directory path
    let folder = base_path.join(&folder_name);

    // Create the directory with proper error handling
    fs::create_dir_all(&folder).map_err(|e| {
        error!("Failed to create directory {}: {}", folder.display(), e);
        e
    })?;

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
    // Validate file path
    if file_path.exists() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("File already exists: {}", file_path.display())
        ));
    }
    
    // Validate parent directory exists and is writable
    if let Some(parent) = file_path.parent() {
        if !parent.exists() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("Parent directory does not exist: {}", parent.display())
            ));
        }
        
        // Check if parent directory is writable by attempting to create a test file
        let test_file = parent.join(".write_test");
        if let Err(e) = File::create(&test_file) {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("Parent directory is not writable: {} ({})", parent.display(), e)
            ));
        }
        // Clean up test file
        let _ = fs::remove_file(&test_file);
    }
    
    // Validate data is not empty
    if data.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Cannot write empty data to file"
        ));
    }

    // Create file with explicit permissions and error handling
    let mut file = File::create(file_path).map_err(|e| {
        error!("Failed to create file {}: {}", file_path.display(), e);
        e
    })?;
    
    // Write data with error handling
    file.write_all(data).map_err(|e| {
        error!("Failed to write data to file {}: {}", file_path.display(), e);
        // Attempt to clean up the file if write fails
        let _ = fs::remove_file(file_path);
        e
    })?;
    
    info!("Successfully saved file: {}", file_path.display());
    Ok(())
}

/// Clean up a folder and its contents
///
/// # Arguments
///
/// * `folder_path` - Path to the folder to clean up
///
/// Clean up a folder and its contents with result tracking
///
/// # Arguments
///
/// * `folder_path` - Path to the folder to clean up
///
/// # Returns
///
/// * `bool` - true if cleanup was successful, false otherwise
pub fn cleanup_folder_with_result(folder_path: &Path) -> bool {
    // Validate folder path before attempting cleanup
    if !folder_path.exists() {
        debug!("Cleanup folder does not exist: {}", folder_path.display());
        return true; // Not an error if it doesn't exist
    }
    
    if !folder_path.is_dir() {
        error!("Cleanup path is not a directory: {}", folder_path.display());
        return false;
    }
    
    // Additional safety check - don't delete system directories
    if folder_path == Path::new("/") || folder_path == Path::new("C:\\") {
        error!("Refusing to delete system root directory: {}", folder_path.display());
        return false;
    }
    
    // Attempt cleanup
    if let Err(e) = fs::remove_dir_all(folder_path) {
        error!("Failed to clean up folder {}: {}", folder_path.display(), e);
        false
    } else {
        info!("Successfully cleaned up folder: {}", folder_path.display());
        true
    }
}

// This function was removed as it's not currently used
// If needed in the future, uncomment and use it
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
