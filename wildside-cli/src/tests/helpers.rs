use super::*;
use std::{
    fs,
    path::{Path, PathBuf},
};
use tempfile::TempDir;

#[derive(Debug, Clone, Default)]
pub(super) struct LayerOverrides {
    pub(super) osm_pbf: Option<PathBuf>,
    pub(super) wikidata_dump: Option<PathBuf>,
}

pub(super) struct DatasetFiles {
    _dir: TempDir,
    cli_osm: PathBuf,
    cli_wikidata: PathBuf,
    config_osm: PathBuf,
    config_wikidata: PathBuf,
    env_wikidata: PathBuf,
}

impl DatasetFiles {
    pub(super) fn new() -> Self {
        let dir = TempDir::new().expect("tempdir");
        let cli_osm = dir.path().join("cli.osm.pbf");
        let cli_wikidata = dir.path().join("cli.wikidata.json.bz2");
        let config_osm = dir.path().join("config.osm.pbf");
        let config_wikidata = dir.path().join("config.wikidata.json.bz2");
        let env_wikidata = dir.path().join("env.wikidata.json.bz2");
        for path in [
            &cli_osm,
            &cli_wikidata,
            &config_osm,
            &config_wikidata,
            &env_wikidata,
        ] {
            fs::write(path, b"dataset contents").expect("write dataset file");
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

    pub(super) fn osm(&self) -> &Path {
        &self.cli_osm
    }

    pub(super) fn wikidata(&self) -> &Path {
        &self.cli_wikidata
    }

    pub(super) fn config_osm(&self) -> &Path {
        &self.config_osm
    }

    pub(super) fn config_wikidata(&self) -> &Path {
        &self.config_wikidata
    }

    pub(super) fn env_wikidata(&self) -> &Path {
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
        env_layer.as_ref().and_then(|layer| layer.osm_pbf.clone()),
        file_layer.as_ref().and_then(|layer| layer.osm_pbf.clone()),
    );
    merge_field(
        &mut cli_args.wikidata_dump,
        env_layer
            .as_ref()
            .and_then(|layer| layer.wikidata_dump.clone()),
        file_layer
            .as_ref()
            .and_then(|layer| layer.wikidata_dump.clone()),
    );
    run_ingest(cli_args)
}

fn merge_field<T: Clone>(target: &mut Option<T>, env_value: Option<T>, file_value: Option<T>) {
    if target.is_none()
        && let Some(value) = env_value.or(file_value)
    {
        *target = Some(value);
    }
}
