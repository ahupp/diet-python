use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use pyo3::prelude::*;
use pyo3::types::{PyBool, PyList, PyModule, PyTuple};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use soac_eval::jit;
use std::ffi::c_void;
use std::path::{Path, PathBuf};
use std::sync::Once;
use std::sync::atomic::{AtomicU64, Ordering};
use tower_http::services::ServeDir;

static NEXT_WEB_MODULE_ID: AtomicU64 = AtomicU64::new(1);
static PYTHON_INIT: Once = Once::new();

#[derive(Clone)]
pub struct AppState {
    repo_root: PathBuf,
    web_dir: PathBuf,
}

#[derive(Deserialize)]
struct InspectPipelineRequest {
    source: String,
}

#[derive(Deserialize)]
struct JitClifRequest {
    source: String,
    #[serde(rename = "functionId")]
    function_id: usize,
    qualname: Option<String>,
    #[serde(rename = "entryLabel")]
    entry_label: Option<String>,
}

#[derive(Serialize)]
pub struct JitClifResponse {
    pub clif: String,
    #[serde(rename = "cfgDot")]
    pub cfg_dot: String,
    #[serde(rename = "vcodeDisasm")]
    pub vcode_disasm: String,
    pub resolved_entry: String,
}

#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    error: String,
}

impl ApiError {
    fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            error: message.into(),
        }
    }

    fn internal(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            error: message.into(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, Json(json!({ "error": self.error }))).into_response()
    }
}

pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace crate should have a repo-root parent")
        .to_path_buf()
}

pub fn web_dir() -> PathBuf {
    repo_root().join("web")
}

pub fn app() -> Router {
    let state = AppState {
        repo_root: repo_root(),
        web_dir: web_dir(),
    };
    app_with_state(state)
}

pub fn app_with_state(state: AppState) -> Router {
    Router::new()
        .route("/api/inspect_pipeline", post(handle_inspect_pipeline))
        .route("/api/jit_clif", post(handle_jit_clif))
        .fallback_service(ServeDir::new(state.web_dir.clone()))
        .with_state(state)
}

pub fn prepare_python() {
    PYTHON_INIT.call_once(|| {
        configure_embedded_python_env();
        Python::initialize();
    });
}

fn configure_embedded_python_env() {
    let repo_root = repo_root();
    let python_home = repo_root.join("vendor/cpython");
    let mut python_path_entries = vec![python_home.join("Lib")];
    if let Some(build_lib_dir) = find_python_build_lib_dir(&python_home) {
        python_path_entries.push(build_lib_dir);
    }
    let python_path =
        std::env::join_paths(python_path_entries).expect("vendored CPython paths should be valid");
    // Configure the embedded interpreter to use the vendored CPython tree
    // before the first interpreter initialization.
    unsafe {
        std::env::set_var("PYTHONHOME", &python_home);
        std::env::set_var("PYTHONPATH", &python_path);
    }
}

fn find_python_build_lib_dir(python_home: &Path) -> Option<PathBuf> {
    let build_dir = python_home.join("build");
    let entries = std::fs::read_dir(build_dir).ok()?;
    for entry in entries {
        let path = entry.ok()?.path();
        if path.is_dir()
            && path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("lib."))
        {
            return Some(path);
        }
    }
    None
}

fn ensure_repo_root_on_sys_path(py: Python<'_>, repo_root: &Path) -> Result<(), ApiError> {
    let sys = PyModule::import(py, "sys").map_err(|err| ApiError::internal(err.to_string()))?;
    let path = sys
        .getattr("path")
        .map_err(|err| ApiError::internal(err.to_string()))?;
    let path = path
        .cast::<PyList>()
        .map_err(|err| ApiError::internal(err.to_string()))?;
    let repo_root = repo_root.to_string_lossy();
    let already_present = path.iter().any(|item| {
        item.extract::<String>()
            .map(|value| value == repo_root)
            .unwrap_or(false)
    });
    if !already_present {
        path.insert(0, repo_root.as_ref())
            .map_err(|err| ApiError::internal(err.to_string()))?;
    }
    Ok(())
}

fn lower_source_recorded(source: &str) -> Result<dp_transform::LoweringResult, ApiError> {
    dp_transform::lower_python_to_blockpy_recorded(source)
        .map_err(|err| ApiError::internal(err.to_string()))
}

pub fn register_named_plans_from_source(source: &str, module_name: &str) -> Result<(), String> {
    let output = lower_source_recorded(source).map_err(|err| err.error)?;
    jit::register_clif_module_plans(module_name, &output.codegen_module)?;
    Ok(())
}

fn register_plans_from_source(source: &str) -> Result<String, ApiError> {
    let module_name = format!(
        "_dp_web_{:016x}",
        NEXT_WEB_MODULE_ID.fetch_add(1, Ordering::Relaxed)
    );
    register_named_plans_from_source(source, module_name.as_str()).map_err(ApiError::internal)?;
    Ok(module_name)
}

fn inspect_pipeline_payload(source: &str) -> Result<Value, ApiError> {
    let output = lower_source_recorded(source)?;
    let payload = dp_transform::web_inspector::render_inspector_payload(source, &output);
    serde_json::from_str(&payload).map_err(|err| ApiError::internal(err.to_string()))
}

pub fn jit_debug_plan(module_name: &str, function_id: usize) -> Result<String, String> {
    let Some(function) = jit::lookup_blockpy_function(module_name, function_id) else {
        return Err(format!(
            "no specialized JIT plan for {module_name}.fn#{function_id}"
        ));
    };
    let block_info = function
        .blocks
        .iter()
        .map(|block| jit::jit_block_info(&function, block))
        .collect::<Vec<_>>();
    Ok(format!(
        "function:\n{function:#?}\n\njit_blocks:\n{block_info:#?}"
    ))
}

pub fn render_registered_jit_clif(
    repo_root: &Path,
    module_name: &str,
    function_id: usize,
) -> Result<JitClifResponse, String> {
    let function = jit::lookup_blockpy_function(module_name, function_id)
        .ok_or_else(|| format!("no specialized JIT plan for {module_name}.fn#{function_id}"))?;
    prepare_python();
    let rendered = Python::attach(|py| {
        ensure_repo_root_on_sys_path(py, repo_root).map_err(|err| err.error)?;
        PyModule::import(py, "__dp__").map_err(|err| err.to_string())?;
        let builtins = PyModule::import(py, "builtins").map_err(|err| err.to_string())?;
        let deleted_obj = builtins
            .getattr("__dp_DELETED")
            .map_err(|err| err.to_string())?;
        let empty_tuple = PyTuple::empty(py);
        let true_obj = PyBool::new(py, true).as_ptr() as *mut c_void;
        let false_obj = PyBool::new(py, false).as_ptr() as *mut c_void;
        unsafe {
            jit::render_cranelift_run_bb_specialized_with_cfg(
                &vec![std::ptr::null_mut::<c_void>(); function.blocks.len()],
                &function,
                true_obj,
                false_obj,
                deleted_obj.as_ptr() as *mut c_void,
                empty_tuple.as_ptr() as *mut c_void,
            )
        }
    })?;
    let entry_label = function
        .blocks
        .first()
        .map(|block| block.label.to_string())
        .unwrap_or_else(|| "<unknown>".to_string());
    Ok(JitClifResponse {
        clif: rendered.clif,
        cfg_dot: rendered.cfg_dot,
        vcode_disasm: rendered.vcode_disasm,
        resolved_entry: format!(
            "{}::__dp_fn_{}::{}",
            function.names.qualname, function_id, entry_label
        ),
    })
}

fn render_jit_clif(
    repo_root: &Path,
    source: &str,
    function_id: usize,
    qualname: Option<&str>,
    entry_label: &str,
) -> Result<JitClifResponse, ApiError> {
    let module_name = register_plans_from_source(source)?;
    let mut rendered = render_registered_jit_clif(repo_root, module_name.as_str(), function_id)
        .map_err(ApiError::internal)?;
    rendered.resolved_entry = format!(
        "{}::__dp_fn_{}::{}",
        qualname.unwrap_or("<unknown>"),
        function_id,
        entry_label
    );
    Ok(rendered)
}

async fn handle_inspect_pipeline(
    Json(request): Json<InspectPipelineRequest>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(inspect_pipeline_payload(request.source.as_str())?))
}

async fn handle_jit_clif(
    State(state): State<AppState>,
    Json(request): Json<JitClifRequest>,
) -> Result<Json<JitClifResponse>, ApiError> {
    let entry_label = request
        .entry_label
        .as_deref()
        .ok_or_else(|| ApiError::bad_request("entryLabel must be provided"))?;
    Ok(Json(render_jit_clif(
        &state.repo_root,
        request.source.as_str(),
        request.function_id,
        request.qualname.as_deref(),
        entry_label,
    )?))
}

#[cfg(test)]
mod test {
    use super::app;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use http_body_util::BodyExt;
    use serde_json::{Value, json};
    use tower::ServiceExt;

    async fn response_text(response: axum::response::Response) -> String {
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("body collection should succeed")
            .to_bytes();
        String::from_utf8(bytes.to_vec()).expect("response body should be utf-8")
    }

    #[tokio::test]
    async fn serves_index_and_inspect_pipeline() {
        let app = app();
        let response = app
            .clone()
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .expect("static request should succeed");
        assert_eq!(response.status(), StatusCode::OK);
        let html = response_text(response).await;
        assert!(html.contains("/api/inspect_pipeline"));
        assert!(html.contains("/api/jit_clif"));

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/inspect_pipeline")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({"source": "def classify(n):\n    return n\n"}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .expect("inspect request should succeed");
        assert_eq!(response.status(), StatusCode::OK);
        let payload: Value = serde_json::from_str(&response_text(response).await).unwrap();
        assert_eq!(payload["steps"][0]["key"], "input_source");
        assert_eq!(payload["functions"][0]["qualname"], "classify");
    }

    #[tokio::test]
    async fn renders_actual_clif() {
        let app = app();
        let source = "def classify(n):\n    return n\n";
        let inspect_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/inspect_pipeline")
                    .header("content-type", "application/json")
                    .body(Body::from(json!({ "source": source }).to_string()))
                    .unwrap(),
            )
            .await
            .expect("inspect request should succeed");
        let inspect_payload: Value =
            serde_json::from_str(&response_text(inspect_response).await).unwrap();
        let function = &inspect_payload["functions"][0];

        let clif_response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/jit_clif")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "source": source,
                            "functionId": function["functionId"],
                            "qualname": function["qualname"],
                            "entryLabel": function["entryLabel"],
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .expect("clif request should succeed");
        assert_eq!(clif_response.status(), StatusCode::OK);
        let payload: Value = serde_json::from_str(&response_text(clif_response).await).unwrap();
        assert!(payload["clif"].as_str().unwrap().contains("function"));
        assert!(payload["cfgDot"].as_str().unwrap().contains("digraph"));
        assert!(
            payload["resolved_entry"]
                .as_str()
                .unwrap()
                .starts_with("classify::__dp_fn_")
        );
    }
}
