//! Behaviour-driven step definitions driving the solve CLI scenarios.

use super::helpers::write_utf8;
use super::*;
use geo::Coord;
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use std::cell::RefCell;
use std::time::Duration;
use tempfile::TempDir;
use wildside_core::{
    Diagnostics, InterestProfile, Route, SolveError, SolveRequest, SolveRequestValidationError,
    SolveResponse, Solver, Theme,
};

#[derive(Debug)]
struct SolveWorld {
    _tmp: TempDir,
    artefacts_dir: Utf8PathBuf,
    request_path: Utf8PathBuf,
    include_request: RefCell<bool>,
    cli_args: RefCell<Vec<String>>,
    stdout: RefCell<Vec<u8>>,
    result: RefCell<Option<Result<(), CliError>>>,
}

impl SolveWorld {
    fn new() -> Self {
        let tmp = TempDir::new().expect("tempdir");
        let artefacts_dir =
            Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).expect("utf-8 workspace");
        let request_path = artefacts_dir.join("request.json");

        Self {
            _tmp: tmp,
            artefacts_dir,
            request_path,
            include_request: RefCell::new(true),
            cli_args: RefCell::new(Vec::new()),
            stdout: RefCell::new(Vec::new()),
            result: RefCell::new(None),
        }
    }

    fn ensure_default_artefacts_exist(&self) {
        write_utf8(&self.artefacts_dir.join("pois.db"), b"db");
        write_utf8(&self.artefacts_dir.join("pois.rstar"), b"index");
        write_utf8(&self.artefacts_dir.join("popularity.bin"), b"popularity");
    }

    fn build_command_line(&self) -> Vec<String> {
        let mut argv = vec!["wildside".to_string(), "solve".to_string()];
        if *self.include_request.borrow() {
            argv.push(self.request_path.as_str().to_string());
        }
        argv.extend([
            format!("--{ARG_SOLVE_ARTEFACTS_DIR}"),
            self.artefacts_dir.as_str().to_string(),
        ]);
        argv.extend(self.cli_args.borrow().iter().cloned());
        argv
    }
}

#[fixture]
fn world() -> SolveWorld {
    SolveWorld::new()
}

struct StubSolver {
    response: SolveResponse,
}

impl Solver for StubSolver {
    fn solve(&self, _request: &SolveRequest) -> Result<SolveResponse, SolveError> {
        Ok(self.response.clone())
    }
}

#[derive(Debug)]
struct StubSolveSolverBuilder {
    response: SolveResponse,
}

impl SolveSolverBuilder for StubSolveSolverBuilder {
    fn build(&self, _config: &SolveConfig) -> Result<Box<dyn Solver>, CliError> {
        Ok(Box::new(StubSolver {
            response: self.response.clone(),
        }))
    }
}

#[given("default solver artefacts exist on disk")]
fn default_artefacts_exist(#[from(world)] world: &SolveWorld) {
    world.ensure_default_artefacts_exist();
}

#[given("I omit the solve request path")]
fn omit_solve_request_path(#[from(world)] world: &SolveWorld) {
    *world.include_request.borrow_mut() = false;
}

#[given("a valid solve request exists on disk")]
fn valid_solve_request_exists(#[from(world)] world: &SolveWorld) {
    let interests = InterestProfile::new().with_weight(Theme::History, 0.8);
    let request = SolveRequest {
        start: Coord { x: -0.1, y: 51.5 },
        end: None,
        duration_minutes: 30,
        interests,
        seed: 1,
        max_nodes: Some(20),
    };
    let payload = serde_json::to_string_pretty(&request).expect("serialize request");
    write_utf8(&world.request_path, payload.as_bytes());
}

#[given("the solve request contains invalid JSON")]
fn solve_request_contains_invalid_json(#[from(world)] world: &SolveWorld) {
    write_utf8(&world.request_path, b"{ not valid json");
}

#[given("the solve request contains invalid parameters")]
fn solve_request_contains_invalid_parameters(#[from(world)] world: &SolveWorld) {
    let request = SolveRequest {
        start: Coord { x: -0.1, y: 51.5 },
        end: None,
        duration_minutes: 0,
        interests: InterestProfile::new(),
        seed: 1,
        max_nodes: None,
    };
    let payload = serde_json::to_string_pretty(&request).expect("serialize request");
    write_utf8(&world.request_path, payload.as_bytes());
}

#[when("I run the solve command")]
fn run_solve_command(#[from(world)] world: &SolveWorld) {
    let invocation = world.build_command_line();
    let parsed = Cli::try_parse_from(invocation).map_err(CliError::from);
    let outcome = parsed.and_then(|cli| match cli.command {
        Command::Solve(args) => {
            let response = SolveResponse {
                route: Route::empty(),
                score: 1.0,
                diagnostics: Diagnostics {
                    solve_time: Duration::from_secs(0),
                    candidates_evaluated: 0,
                },
            };
            let builder = StubSolveSolverBuilder { response };
            let mut buffer = world.stdout.borrow_mut();
            run_solve_with(args, &builder, &mut *buffer)
        }
        Command::Ingest(_) => panic!("expected solve command"),
    });

    world.result.replace(Some(outcome));
}

#[then("the command succeeds and prints JSON output")]
fn command_succeeds_and_prints_json(#[from(world)] world: &SolveWorld) {
    let borrowed = world.result.borrow();
    let result = borrowed.as_ref().expect("result recorded");
    result.as_ref().expect("expected success");

    let stdout = String::from_utf8(world.stdout.borrow().clone()).expect("stdout utf-8");
    let response: SolveResponse =
        serde_json::from_str(&stdout).expect("output should be JSON solve response");
    assert_eq!(response.score, 1.0);
}

#[then("the command fails because the request JSON is invalid")]
fn command_fails_invalid_json(#[from(world)] world: &SolveWorld) {
    let borrowed = world.result.borrow();
    let error = borrowed
        .as_ref()
        .expect("result recorded")
        .as_ref()
        .expect_err("expected error");
    match error {
        CliError::ParseSolveRequest { .. } => {}
        other => panic!("expected ParseSolveRequest, found {other:?}"),
    }
}

#[then("the command fails because the request is invalid")]
fn command_fails_invalid_request(#[from(world)] world: &SolveWorld) {
    let borrowed = world.result.borrow();
    let error = borrowed
        .as_ref()
        .expect("result recorded")
        .as_ref()
        .expect_err("expected error");
    match error {
        CliError::InvalidSolveRequest { source, .. } => {
            assert_eq!(*source, SolveRequestValidationError::ZeroDuration);
        }
        other => panic!("expected InvalidSolveRequest, found {other:?}"),
    }
}

#[then("the command fails because the request path is missing")]
fn command_fails_missing_request_path(#[from(world)] world: &SolveWorld) {
    let borrowed = world.result.borrow();
    let error = borrowed
        .as_ref()
        .expect("result recorded")
        .as_ref()
        .expect_err("expected error");
    match error {
        CliError::MissingArgument { field, .. } => assert_eq!(*field, ARG_SOLVE_REQUEST),
        other => panic!("expected MissingArgument, found {other:?}"),
    }
}

macro_rules! register_solve_scenario {
    ($fn_name:ident, $scenario_title:literal) => {
        #[scenario(path = "tests/features/solve_command.feature", name = $scenario_title)]
        fn $fn_name(#[from(world)] world: SolveWorld) {
            let _ = world;
        }
    };
}

register_solve_scenario!(solve_happy_path, "solving a request from JSON");
register_solve_scenario!(solve_invalid_json, "rejecting invalid JSON input");
register_solve_scenario!(solve_invalid_request, "rejecting invalid solve requests");
register_solve_scenario!(solve_missing_request, "rejecting missing request paths");
