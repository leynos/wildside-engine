//! Solve command implementation for the Wildside CLI.

use camino::{Utf8Path, Utf8PathBuf};
use clap::Parser;
use ortho_config::{OrthoConfig, SubcmdConfigMerge};
use serde::{Deserialize, Serialize};
use std::io::{BufReader, Write};
#[cfg(feature = "store-sqlite")]
use wildside_core::SqlitePoiStore;
use wildside_core::{SolveRequest, SolveResponse, Solver};
#[cfg(feature = "store-sqlite")]
use wildside_data::routing::HttpTravelTimeProvider;
use wildside_data::routing::HttpTravelTimeProviderConfig;
use wildside_fs::open_utf8_file;
#[cfg(feature = "store-sqlite")]
use wildside_scorer::UserRelevanceScorer;
#[cfg(all(
    feature = "store-sqlite",
    feature = "solver-ortools",
    not(feature = "solver-vrp")
))]
use wildside_solver_ortools::OrtoolsSolver;
#[cfg(all(feature = "store-sqlite", feature = "solver-vrp"))]
use wildside_solver_vrp::VrpSolver;

use crate::{
    ARG_SOLVE_ARTEFACTS_DIR, ARG_SOLVE_OSRM_BASE_URL, ARG_SOLVE_POIS_DB, ARG_SOLVE_POPULARITY,
    ARG_SOLVE_REQUEST, ARG_SOLVE_SPATIAL_INDEX, CliError, ENV_SOLVE_REQUEST,
};

/// CLI arguments for the `solve` subcommand.
#[derive(Debug, Clone, Parser, Deserialize, Serialize, OrthoConfig, Default)]
#[command(
    long_about = "Solve a tour request by loading prepared artefacts \
                 (pois.db, pois.rstar, popularity.bin) and querying an OSRM \
                 instance for travel time matrices. The request itself is \
                 provided as a JSON-encoded SolveRequest.",
    about = "Solve an orienteering request"
)]
#[ortho_config(prefix = "WILDSIDE")]
pub(crate) struct SolveArgs {
    /// Path to a JSON file containing a SolveRequest.
    #[arg(value_name = "path")]
    #[serde(default)]
    pub(crate) request_path: Option<Utf8PathBuf>,
    /// Directory containing the default artefact filenames.
    #[arg(long = ARG_SOLVE_ARTEFACTS_DIR, value_name = "dir")]
    #[serde(default)]
    pub(crate) artefacts_dir: Option<Utf8PathBuf>,
    /// Override the path to the SQLite POI store (`pois.db`).
    #[arg(long = ARG_SOLVE_POIS_DB, value_name = "path")]
    #[serde(default)]
    pub(crate) pois_db: Option<Utf8PathBuf>,
    /// Override the path to the persisted spatial index (`pois.rstar`).
    #[arg(long = ARG_SOLVE_SPATIAL_INDEX, value_name = "path")]
    #[serde(default)]
    pub(crate) spatial_index: Option<Utf8PathBuf>,
    /// Override the path to pre-computed popularity scores (`popularity.bin`).
    #[arg(long = ARG_SOLVE_POPULARITY, value_name = "path")]
    #[serde(default)]
    pub(crate) popularity: Option<Utf8PathBuf>,
    /// Base URL for the OSRM server (e.g. "http://localhost:5000").
    #[arg(long = ARG_SOLVE_OSRM_BASE_URL, value_name = "url")]
    #[serde(default)]
    pub(crate) osrm_base_url: Option<String>,
}

impl SolveArgs {
    pub(crate) fn into_config(self) -> Result<SolveConfig, CliError> {
        let merged = self.load_and_merge().map_err(CliError::Configuration)?;
        SolveConfig::try_from(merged)
    }
}

/// Resolved `solve` command configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SolveConfig {
    /// Path to the JSON request file.
    pub(crate) request_path: Utf8PathBuf,
    /// Path to `pois.db` SQLite database.
    pub(crate) pois_db: Utf8PathBuf,
    /// Path to `pois.rstar` persisted spatial index.
    pub(crate) spatial_index: Utf8PathBuf,
    /// Path to `popularity.bin` popularity scores.
    pub(crate) popularity: Utf8PathBuf,
    /// Base URL for the OSRM table service.
    pub(crate) osrm_base_url: String,
}

impl SolveConfig {
    pub(crate) fn validate_sources(&self) -> Result<(), CliError> {
        Self::require_existing(&self.request_path, ARG_SOLVE_REQUEST)?;
        Self::require_existing(&self.pois_db, ARG_SOLVE_POIS_DB)?;
        Self::require_existing(&self.spatial_index, ARG_SOLVE_SPATIAL_INDEX)?;
        Self::require_existing(&self.popularity, ARG_SOLVE_POPULARITY)?;
        Ok(())
    }

    fn require_existing(path: &Utf8Path, field: &'static str) -> Result<(), CliError> {
        match wildside_fs::file_is_file(path) {
            Ok(true) => Ok(()),
            Ok(false) => Err(CliError::SourcePathNotFile {
                field,
                path: path.to_path_buf(),
            }),
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
                Err(CliError::MissingSourceFile {
                    field,
                    path: path.to_path_buf(),
                })
            }
            Err(source) => Err(CliError::InspectSourcePath {
                field,
                path: path.to_path_buf(),
                source,
            }),
        }
    }
}

impl TryFrom<SolveArgs> for SolveConfig {
    type Error = CliError;

    fn try_from(args: SolveArgs) -> Result<Self, Self::Error> {
        let request_path = args.request_path.ok_or(CliError::MissingArgument {
            field: ARG_SOLVE_REQUEST,
            env: ENV_SOLVE_REQUEST,
        })?;

        let artefacts_dir = args.artefacts_dir.unwrap_or_else(|| Utf8PathBuf::from("."));
        let pois_db = args
            .pois_db
            .unwrap_or_else(|| artefacts_dir.join("pois.db"));
        let spatial_index = args
            .spatial_index
            .unwrap_or_else(|| artefacts_dir.join("pois.rstar"));
        let popularity = args
            .popularity
            .unwrap_or_else(|| artefacts_dir.join("popularity.bin"));

        let default_base_url = HttpTravelTimeProviderConfig::default().base_url;
        let osrm_base_url = args.osrm_base_url.unwrap_or(default_base_url);

        Ok(Self {
            request_path,
            pois_db,
            spatial_index,
            popularity,
            osrm_base_url,
        })
    }
}

/// Builds a solver instance for the current solve invocation.
pub(super) trait SolveSolverBuilder {
    fn build(&self, config: &SolveConfig) -> Result<Box<dyn Solver>, CliError>;
}

pub(super) struct DefaultSolveSolverBuilder;

#[cfg(feature = "store-sqlite")]
impl SolveSolverBuilder for DefaultSolveSolverBuilder {
    fn build(&self, config: &SolveConfig) -> Result<Box<dyn Solver>, CliError> {
        let store = SqlitePoiStore::open(
            config.pois_db.as_std_path(),
            config.spatial_index.as_std_path(),
        )?;
        let scorer = UserRelevanceScorer::with_defaults(&config.pois_db, &config.popularity)?;
        let provider =
            HttpTravelTimeProvider::new(config.osrm_base_url.clone()).map_err(|source| {
                CliError::BuildTravelTimeProvider {
                    base_url: config.osrm_base_url.clone(),
                    source,
                }
            })?;
        build_solver_with_features(store, provider, scorer)
    }
}

#[cfg(not(feature = "store-sqlite"))]
impl SolveSolverBuilder for DefaultSolveSolverBuilder {
    fn build(&self, _config: &SolveConfig) -> Result<Box<dyn Solver>, CliError> {
        Err(CliError::MissingFeature {
            feature: "store-sqlite",
            action: "solve",
        })
    }
}

#[cfg(all(feature = "store-sqlite", feature = "solver-vrp"))]
fn build_solver_with_features(
    store: SqlitePoiStore,
    provider: HttpTravelTimeProvider,
    scorer: UserRelevanceScorer,
) -> Result<Box<dyn Solver>, CliError> {
    Ok(Box::new(VrpSolver::new(store, provider, scorer)))
}

#[cfg(all(
    feature = "store-sqlite",
    not(feature = "solver-vrp"),
    feature = "solver-ortools"
))]
fn build_solver_with_features(
    store: SqlitePoiStore,
    provider: HttpTravelTimeProvider,
    scorer: UserRelevanceScorer,
) -> Result<Box<dyn Solver>, CliError> {
    Ok(Box::new(OrtoolsSolver::new(store, provider, scorer)))
}

#[cfg(all(
    feature = "store-sqlite",
    not(feature = "solver-vrp"),
    not(feature = "solver-ortools")
))]
fn build_solver_with_features(
    _store: SqlitePoiStore,
    _provider: HttpTravelTimeProvider,
    _scorer: UserRelevanceScorer,
) -> Result<Box<dyn Solver>, CliError> {
    Err(CliError::MissingFeature {
        feature: "solver-vrp or solver-ortools",
        action: "solve",
    })
}

pub(super) fn run_solve(args: SolveArgs) -> Result<(), CliError> {
    let mut stdout = std::io::stdout().lock();
    let builder = DefaultSolveSolverBuilder;
    run_solve_with(args, &builder, &mut stdout)
}

pub(super) fn run_solve_with(
    args: SolveArgs,
    builder: &dyn SolveSolverBuilder,
    writer: &mut dyn Write,
) -> Result<(), CliError> {
    let response = execute_solve(args, builder)?;
    write_solve_response(writer, &response)
}

fn execute_solve(
    args: SolveArgs,
    builder: &dyn SolveSolverBuilder,
) -> Result<SolveResponse, CliError> {
    let config = resolve_solve_config(args)?;
    let request = load_solve_request(&config.request_path)?;
    request
        .validate_detailed()
        .map_err(|source| CliError::InvalidSolveRequest {
            path: config.request_path.clone(),
            source,
        })?;
    let solver = builder.build(&config)?;
    solver
        .solve(&request)
        .map_err(|source| CliError::Solve { source })
}

fn resolve_solve_config(args: SolveArgs) -> Result<SolveConfig, CliError> {
    let config = args.into_config()?;
    config.validate_sources()?;
    Ok(config)
}

/// Loads a JSON-encoded [`SolveRequest`] from disk.
pub(super) fn load_solve_request(path: &Utf8Path) -> Result<SolveRequest, CliError> {
    let file = open_utf8_file(path).map_err(|source| CliError::OpenSolveRequest {
        path: path.to_path_buf(),
        source,
    })?;
    let reader = BufReader::new(file);
    serde_json::from_reader(reader).map_err(|source| CliError::ParseSolveRequest {
        path: path.to_path_buf(),
        source,
    })
}

fn write_solve_response(writer: &mut dyn Write, response: &SolveResponse) -> Result<(), CliError> {
    let payload =
        serde_json::to_string_pretty(response).map_err(CliError::SerializeSolveResponse)?;
    writer
        .write_all(payload.as_bytes())
        .map_err(CliError::WriteSolveOutput)?;
    writer
        .write_all(b"\n")
        .map_err(CliError::WriteSolveOutput)?;
    Ok(())
}

#[cfg(test)]
pub(crate) fn config_from_layers_for_test(
    layers: Vec<ortho_config::MergeLayer<'static>>,
) -> Result<SolveConfig, CliError> {
    let merged = SolveArgs::merge_from_layers(layers).map_err(CliError::from)?;
    SolveConfig::try_from(merged)
}

#[cfg(test)]
mod feature_flag_tests {
    use rstest::rstest;

    #[cfg(feature = "solver-vrp")]
    const SOLVER_SELECTION: &str = "vrp";

    #[cfg(all(not(feature = "solver-vrp"), feature = "solver-ortools"))]
    const SOLVER_SELECTION: &str = "ortools";

    #[cfg(all(not(feature = "solver-vrp"), not(feature = "solver-ortools")))]
    const SOLVER_SELECTION: &str = "missing";

    #[cfg(feature = "solver-vrp")]
    #[rstest]
    fn solver_selection_prefers_vrp() {
        assert_eq!(SOLVER_SELECTION, "vrp");
    }

    #[cfg(all(not(feature = "solver-vrp"), feature = "solver-ortools"))]
    #[rstest]
    fn solver_selection_uses_ortools_when_vrp_disabled() {
        assert_eq!(SOLVER_SELECTION, "ortools");
    }

    #[cfg(all(not(feature = "solver-vrp"), not(feature = "solver-ortools")))]
    #[rstest]
    fn solver_selection_reports_missing_features() {
        assert_eq!(SOLVER_SELECTION, "missing");
    }
}
