use brain_services::runtime::*;
use brain_services::worker::*;
use std::fs;
use tempfile::tempdir;

#[tokio::test]
async fn test_local_filesystem_artifact_store_staging_and_publishing() {
    let dir = tempdir().unwrap();
    let store = LocalFilesystemArtifactStore::new(dir.path().to_path_buf());

    let task_id = TaskId::new();
    let file_path = dir.path().join("output.txt");
    fs::write(&file_path, "sample output").unwrap();

    let pub_ref = store
        .publish_artifact(task_id, ArtifactKind::Output, &file_path)
        .await
        .unwrap();

    assert!(pub_ref.starts_with("artifact://"));

    let staged_path = store.stage_input(&pub_ref).await.unwrap();
    assert!(staged_path.exists());
    assert_eq!(fs::read_to_string(staged_path).unwrap(), "sample output");
}
