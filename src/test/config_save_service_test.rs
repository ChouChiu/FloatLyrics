use super::*;
use std::time::Duration;

#[test]
fn worker_serializes_changes_and_persists_the_latest_config() {
    let directory = tempfile::tempdir().unwrap();
    let config_file = directory.path().join("config.toml");
    let service = ConfigSaveService::new(config_file.clone()).unwrap();
    let (sender, receiver) = mpsc::channel();

    let mut first = AppConfig::default();
    first.window.width = 420;
    let first_sender = sender.clone();
    service.save(first, move |result| {
        first_sender.send((1, result)).unwrap();
    });

    let mut latest = AppConfig::default();
    latest.window.width = 520;
    service.save(latest.clone(), move |result| {
        sender.send((2, result)).unwrap();
    });

    let mut outcomes = [None, None];
    for _ in 0..2 {
        let (request, result) = receiver.recv_timeout(Duration::from_secs(3)).unwrap();
        outcomes[request - 1] = Some(result);
    }

    assert!(matches!(
        outcomes[0],
        Some(ConfigSaveResult::Saved | ConfigSaveResult::Superseded)
    ));
    assert_eq!(outcomes[1], Some(ConfigSaveResult::Saved));
    assert_eq!(AppConfig::load_or_default(&config_file).unwrap(), latest);
}

#[test]
fn dropping_service_flushes_the_queued_latest_config() {
    let directory = tempfile::tempdir().unwrap();
    let config_file = directory.path().join("config.toml");
    let service = ConfigSaveService::new(config_file.clone()).unwrap();
    let mut config = AppConfig::default();
    config.window.width = 480;

    service.save(config.clone(), |_| {});
    drop(service);

    assert_eq!(AppConfig::load_or_default(&config_file).unwrap(), config);
}

#[test]
fn validation_failure_is_reported_without_writing_invalid_config() {
    let directory = tempfile::tempdir().unwrap();
    let config_file = directory.path().join("config.toml");
    let service = ConfigSaveService::new(config_file.clone()).unwrap();
    let (sender, receiver) = mpsc::channel();
    let mut config = AppConfig::default();
    config.window.width = 1;

    service.save(config, move |result| sender.send(result).unwrap());

    let result = receiver.recv_timeout(Duration::from_secs(3)).unwrap();
    assert!(matches!(result, ConfigSaveResult::Failed(error) if error.contains("window.width")));
    assert!(!config_file.exists());
}
