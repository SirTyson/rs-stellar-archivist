//! Tests for path normalization and file URL handling

use super::utils::file_url_from_path;
use crate::storage::{from_url_with_config, ErrorClass, OpendalStore, Storage};
use crate::test_helpers::test_storage_config;
use normalize_path::NormalizePath;
use opendal::Buffer;
use rstest::rstest;
use tempfile::TempDir;

#[test]
fn test_filesystem_root_path_is_normalized() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let root = temp_dir.path().join("subdir").join("..");

    let store = OpendalStore::filesystem(&root, &test_storage_config())
        .expect("Failed to create filesystem store");
    let base = store
        .get_base_path()
        .expect("Expected filesystem store to expose base path");

    assert_eq!(base, root.normalize());
}

#[tokio::test]
async fn test_file_url_round_trip_base_path() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let url = file_url_from_path(temp_dir.path());

    let store = from_url_with_config(&url, &test_storage_config())
        .expect("Failed to create store from file URL");
    let base = store
        .get_base_path()
        .expect("Expected filesystem store to expose base path");

    assert_eq!(base, temp_dir.path());
}

#[cfg(windows)]
#[tokio::test]
async fn test_file_url_backslashes_are_accepted() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let raw = format!("file://{}", temp_dir.path().display());

    let store = from_url_with_config(&raw, &test_storage_config())
        .expect("Failed to create store from backslash file URL");
    let base = store
        .get_base_path()
        .expect("Expected filesystem store to expose base path");

    assert_eq!(base, temp_dir.path());
}

#[rstest]
#[case::empty("")]
#[case::absolute("/bucket/ab/cd/ef/file.xdr.gz")]
#[case::parent("bucket/../../outside/file.xdr.gz")]
#[case::current("bucket/./ab/file.xdr.gz")]
#[case::double_separator("bucket//ab/file.xdr.gz")]
#[case::backslash("bucket\\..\\outside\\file.xdr.gz")]
#[case::drive_prefix("C:/outside/file.xdr.gz")]
#[tokio::test]
async fn test_storage_rejects_invalid_object_paths(#[case] object: &str) {
    let temp_dir = TempDir::new().unwrap();
    let store = OpendalStore::filesystem(temp_dir.path(), &test_storage_config()).unwrap();

    let exists_error = store.exists(object).await.unwrap_err();
    assert_eq!(exists_error.class, ErrorClass::Fatal);
    assert!(exists_error.message.contains("Invalid archive object path"));

    let Err(read_error) = store.open_reader(object).await else {
        panic!("invalid object path opened for reading");
    };
    assert_eq!(read_error.class, ErrorClass::Fatal);
    assert!(read_error.message.contains("Invalid archive object path"));
}

#[rstest]
#[case::plain(false)]
#[case::atomic(true)]
#[tokio::test]
async fn test_filesystem_writes_cannot_escape_root(#[case] atomic_file_writes: bool) {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path().join("archive");
    let mut config = test_storage_config();
    config.atomic_file_writes = atomic_file_writes;
    let store = OpendalStore::filesystem(&root, &config).unwrap();
    let object = "bucket/../../outside/bucket-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.xdr.gz";

    let error = store
        .write(object, Buffer::from("archive controlled bytes"))
        .await
        .unwrap_err();
    assert_eq!(error.class, ErrorClass::Fatal);
    assert!(error.message.contains("Invalid archive object path"));
    assert!(!temp_dir.path().join("outside").exists());
}
