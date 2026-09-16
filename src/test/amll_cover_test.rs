use super::*;

use std::path::PathBuf;

fn file(path: &str) -> Cover {
    Cover::File(PathBuf::from(path))
}

#[test]
fn decodes_absolute_local_file_urls() {
    assert_eq!(
        cover_source("file:///home/listener/Music/cover.jpg"),
        file("/home/listener/Music/cover.jpg")
    );
}

#[test]
fn decodes_percent_escapes_in_local_paths() {
    assert_eq!(
        cover_source("file:///home/listener/My%20Music/%E6%AD%8C/cover.jpg"),
        Cover::File(PathBuf::from("/home/listener/My Music/歌/cover.jpg"))
    );
}

#[test]
fn accepts_a_localhost_host_and_rejects_remote_hosts() {
    assert!(matches!(
        cover_source("file://localhost/home/listener/cover.jpg"),
        Cover::File(_)
    ));
    assert!(matches!(
        cover_source("file://media-server/cover.jpg"),
        Cover::Uri(_)
    ));
}

#[test]
fn keeps_remote_urls_and_unusual_values_verbatim() {
    for url in [
        "https://i.example.test/cover.jpg",
        "data:image/png;base64,AAAA",
        "file://",
        "file:///home/listener/%ZZ.jpg",
        "  https://i.example.test/cover.jpg  ",
    ] {
        let trimmed = url.trim();
        match cover_source(url) {
            Cover::Uri(value) => assert_eq!(value, trimmed),
            Cover::File(_) => panic!("{url} should not be read as a local file"),
        }
    }
}

#[test]
fn keeps_urls_without_a_path_untouched() {
    assert!(matches!(cover_source("file://localhost"), Cover::Uri(_)));
}
