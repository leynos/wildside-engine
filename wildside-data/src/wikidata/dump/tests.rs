use super::*;
use rstest::{fixture, rstest};
use std::{fs, io::Write};
use tempfile::TempDir;
use wikidata_rust::{Entity, Lang, WikiId};

#[derive(Debug)]
struct StubSource {
    base_url: String,
    manifest: Vec<u8>,
    archive: Vec<u8>,
}

impl DumpSource for StubSource {
    fn base_url(&self) -> &str {
        &self.base_url
    }

    fn fetch_status(&self) -> Result<Vec<u8>, TransportError> {
        Ok(self.manifest.clone())
    }

    fn download_archive(&self, _url: &str, sink: &mut dyn Write) -> Result<u64, TransportError> {
        sink.write_all(&self.archive)
            .map_err(|source| TransportError::Network {
                url: "stub".to_owned(),
                source,
            })?;
        let length = u64::try_from(self.archive.len()).expect("archive length should fit in u64");
        Ok(length)
    }
}

#[fixture]
fn base_url() -> String {
    "https://example.org".to_owned()
}

#[fixture]
fn manifest() -> Vec<u8> {
    let json = r#"{
        "jobs": {
            "json": {
                "status": "done",
                "files": {
                    "wikidatawiki-20240909-all.json.bz2": {
                        "url": "/wikidatawiki/entities/20240909/wikidatawiki-20240909-all.json.bz2",
                        "size": 5,
                        "sha1": "abc123"
                    }
                }
            }
        }
    }"#;
    json.as_bytes().to_vec()
}

#[fixture]
fn archive() -> Vec<u8> {
    b"hello".to_vec()
}

#[rstest]
fn parses_manifest(base_url: String, mut manifest: Vec<u8>) {
    let descriptor = select_dump(&mut manifest, &base_url).expect("manifest should parse");
    assert_eq!(descriptor.file_name, "wikidatawiki-20240909-all.json.bz2");
    assert_eq!(descriptor.size, Some(5));
    assert_eq!(
        descriptor.url,
        "https://example.org/wikidatawiki/entities/20240909/wikidatawiki-20240909-all.json.bz2",
    );
    assert_eq!(descriptor.sha1.as_deref(), Some("abc123"));
}

#[rstest]
fn download_pipeline_writes_file(base_url: String, manifest: Vec<u8>, archive: Vec<u8>) {
    let temp_dir = TempDir::new().expect("failed to create temporary directory");
    let output = temp_dir.path().join("dump.json.bz2");
    let source = StubSource {
        base_url,
        manifest,
        archive: archive.clone(),
    };
    let report = download_latest_dump(&source, &output, None).expect("download should succeed");
    let expected_len = u64::try_from(archive.len()).expect("archive length should fit in u64");
    assert_eq!(report.bytes_written, expected_len);
    let contents = fs::read(&output).expect("dump file should be readable");
    assert_eq!(contents, archive);
}

#[rstest]
fn errors_when_manifest_missing_dump(base_url: String) {
    let json = r#"{"jobs": {"json": {"status": "failed", "files": {}}}}"#;
    let mut manifest = json.as_bytes().to_vec();
    let outcome = select_dump(&mut manifest, &base_url);
    assert!(matches!(outcome, Err(WikidataDumpError::MissingDump)));
}

#[rstest]
fn logs_downloads(base_url: String, manifest: Vec<u8>, archive: Vec<u8>) {
    let temp_dir = TempDir::new().expect("failed to create temporary directory");
    let output = temp_dir.path().join("dump.json.bz2");
    let log_path = temp_dir.path().join("downloads.sqlite");
    let log = DownloadLog::initialise(&log_path).expect("log initialisation should succeed");
    let source = StubSource {
        base_url,
        manifest,
        archive,
    };
    let report =
        download_latest_dump(&source, &output, Some(&log)).expect("download should succeed");
    assert!(log.path().exists(), "log file should be created");
    let count: i64 = log
        .connection()
        .query_row("SELECT COUNT(*) FROM downloads", [], |row| row.get(0))
        .expect("failed to query downloads table");
    assert_eq!(count, 1);
    assert_eq!(report.output_path, output);
}

#[rstest]
fn parses_sample_entity() {
    let payload = r#"{
        "entities": {
            "Q42": {
                "type": "item",
                "id": "Q42",
                "labels": {
                    "en": { "language": "en", "value": "Douglas Adams" }
                },
                "descriptions": {},
                "aliases": {},
                "claims": {},
                "sitelinks": {}
            }
        }
    }"#;
    let mut bytes = payload.as_bytes().to_vec();
    let value: serde_json::Value =
        simd_json::serde::from_slice(&mut bytes).expect("failed to parse sample entity JSON");
    let entity = Entity::from_json(value).expect("failed to construct entity from JSON");
    let id = match &entity.id {
        WikiId::EntityId(qid) => qid.0,
        other => panic!("expected an entity ID, got {other:?}"),
    };
    assert_eq!(id, 42);
    let english = Lang("en".to_owned());
    let label = entity.labels.get(&english).map(String::as_str);
    assert_eq!(label, Some("Douglas Adams"));
}
