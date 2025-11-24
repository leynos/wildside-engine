//! Test helpers for composing ingest CLI datasets and layered overrides.

use super::*;
use base64::{Engine as _, engine::general_purpose};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8};
use tempfile::TempDir;

pub(super) fn open_ambient_path(path: &Utf8Path) -> (fs_utf8::Dir, &str) {
    let parent = path.parent().unwrap_or_else(|| Utf8Path::new("."));
    let file_name = path.file_name().expect("target should include a file name");
    let dir =
        fs_utf8::Dir::open_ambient_dir(parent, ambient_authority()).expect("open ambient dir");
    (dir, file_name)
}

pub(super) fn write_utf8(path: &Utf8Path, contents: impl AsRef<[u8]>) {
    let (dir, file_name) = open_ambient_path(path);
    dir.write(file_name, contents.as_ref()).expect("write file");
}

pub(super) fn read_utf8(path: &Utf8Path) -> String {
    let (dir, file_name) = open_ambient_path(path);
    dir.read_to_string(file_name).expect("read file")
}

#[derive(Debug, Clone, Default)]
pub(super) struct LayerOverrides {
    pub(super) osm_pbf: Option<Utf8PathBuf>,
    pub(super) wikidata_dump: Option<Utf8PathBuf>,
    pub(super) output_dir: Option<Utf8PathBuf>,
}

#[derive(Debug)]
pub(super) struct DatasetFiles {
    _dir: TempDir,
    cli_osm: Utf8PathBuf,
    cli_wikidata: Utf8PathBuf,
    config_osm: Utf8PathBuf,
    config_wikidata: Utf8PathBuf,
    env_wikidata: Utf8PathBuf,
}

impl DatasetFiles {
    pub(super) fn new() -> Self {
        let dir = TempDir::new().expect("tempdir");
        let cli_osm =
            Utf8PathBuf::from_path_buf(dir.path().join("cli.osm.pbf")).expect("utf-8 path");
        let cli_wikidata = Utf8PathBuf::from_path_buf(dir.path().join("cli.wikidata.json.bz2"))
            .expect("utf-8 path");
        let config_osm =
            Utf8PathBuf::from_path_buf(dir.path().join("config.osm.pbf")).expect("utf-8 path");
        let config_wikidata =
            Utf8PathBuf::from_path_buf(dir.path().join("config.wikidata.json.bz2"))
                .expect("utf-8 path");
        let env_wikidata = Utf8PathBuf::from_path_buf(dir.path().join("env.wikidata.json.bz2"))
            .expect("utf-8 path");
        for path in [
            &cli_osm,
            &cli_wikidata,
            &config_osm,
            &config_wikidata,
            &env_wikidata,
        ] {
            write_utf8(path, b"dataset contents");
        }
        Self {
            _dir: dir,
            cli_osm,
            cli_wikidata,
            config_osm,
            config_wikidata,
            env_wikidata,
        }
    }

    pub(super) fn osm(&self) -> &Utf8Path {
        &self.cli_osm
    }

    pub(super) fn wikidata(&self) -> &Utf8Path {
        &self.cli_wikidata
    }

    pub(super) fn config_osm(&self) -> &Utf8Path {
        &self.config_osm
    }

    pub(super) fn config_wikidata(&self) -> &Utf8Path {
        &self.config_wikidata
    }

    pub(super) fn env_wikidata(&self) -> &Utf8Path {
        &self.env_wikidata
    }
}

pub(super) fn merge_layers(
    mut cli_args: IngestArgs,
    file_layer: Option<LayerOverrides>,
    env_layer: Option<LayerOverrides>,
) -> Result<IngestConfig, CliError> {
    merge_field(
        &mut cli_args.osm_pbf,
        extract_field(&env_layer, |layer| &layer.osm_pbf),
        extract_field(&file_layer, |layer| &layer.osm_pbf),
    );
    merge_field(
        &mut cli_args.wikidata_dump,
        extract_field(&env_layer, |layer| &layer.wikidata_dump),
        extract_field(&file_layer, |layer| &layer.wikidata_dump),
    );
    merge_field(
        &mut cli_args.output_dir,
        extract_field(&env_layer, |layer| &layer.output_dir),
        extract_field(&file_layer, |layer| &layer.output_dir),
    );
    resolve_ingest_config(cli_args)
}

fn merge_field<T: Clone>(target: &mut Option<T>, env_value: Option<T>, file_value: Option<T>) {
    if target.is_none()
        && let Some(value) = env_value.or(file_value)
    {
        *target = Some(value);
    }
}

fn extract_field<T: Clone>(
    layer: &Option<LayerOverrides>,
    accessor: fn(&LayerOverrides) -> &Option<T>,
) -> Option<T> {
    layer.as_ref().and_then(|entry| accessor(entry).clone())
}

pub(super) fn fixtures_dir() -> Utf8PathBuf {
    Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../wildside-data/tests/fixtures")
}

pub(super) fn decode_pbf_fixture(dest_dir: &Utf8Path, stem: &str) -> Utf8PathBuf {
    let encoded_path = fixtures_dir().join(format!("{stem}.osm.pbf.b64"));
    let encoded = read_utf8(&encoded_path);
    let cleaned: String = encoded
        .chars()
        .filter(|ch| !ch.is_ascii_whitespace())
        .collect();
    let decoded = general_purpose::STANDARD
        .decode(cleaned.as_bytes())
        .expect("decode base64 fixture");
    let output_path = dest_dir.join(format!("{stem}.osm.pbf"));
    write_utf8(&output_path, decoded);
    output_path
}

pub(super) fn write_wikidata_dump(dir: &Utf8Path) -> Utf8PathBuf {
    let dump_path = dir.join("wikidata.json");
    let payload = r#"[
{"id":"Q64","claims":{"P1435":[{"mainsnak":{"snaktype":"value","datavalue":{"type":"wikibase-entityid","value":{"id":"Q9259"}}}}]}},
{"id":"Q42","claims":{}}
]"#;
    write_utf8(&dump_path, payload);
    dump_path
}
