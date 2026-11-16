use std::{env, path::Path, process::Stdio, time::Duration};

use rmcp::{
    ClientLifecycleMode, ClientServiceExt, RoleClient,
    model::{CallToolRequestParams, ClientConfig, ProtocolVersion},
    service::RunningService,
    transport::{
        StreamableHttpClientTransport, streamable_http_client::StreamableHttpClientTransportConfig,
    },
};
use serde_json::{Value, json};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::{Child, Command},
    time::timeout,
};
use uuid::Uuid;

type Client = RunningService<RoleClient, ClientConfig>;

const PROJECT: &str = "synthetic-init";
const FIXTURE: &str = "[SYNTHETIC] MCP 왕복\r\nsecond line\n";

struct Credentials {
    collector: String,
    reader: String,
}

fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap()
}

async fn connect(endpoint: &str, token: &str, lifecycle: ClientLifecycleMode) -> Client {
    let transport = StreamableHttpClientTransport::with_client(
        http_client(),
        StreamableHttpClientTransportConfig::with_uri(endpoint).auth_header(token),
    );
    let config = ClientConfig::default().with_protocol_version(ProtocolVersion::V_2025_11_25);
    config
        .serve_with_lifecycle(transport, lifecycle)
        .await
        .expect("MCP connection")
}

async fn call(client: &Client, tool: &str, args: Value, expected_error: Option<&str>) -> Value {
    let mut request = CallToolRequestParams::new(tool.to_owned());
    if let Some(args) = args.as_object() {
        request = request.with_arguments(args.clone());
    }
    let response = client
        .call_tool(request)
        .await
        .unwrap_or_else(|err| panic!("{tool}: {err}"));
    assert_eq!(
        response.is_error,
        Some(expected_error.is_some()),
        "{tool}: {:?}",
        response.content
    );
    let value = response.structured_content.expect("structured response");
    if let Some(code) = expected_error {
        assert_eq!(value, json!({"code": code}), "{tool}");
    }
    value
}

async fn round_trip(
    endpoint: &str,
    project: &str,
    credentials: &Credentials,
    request_id: &str,
) -> Value {
    let collector = connect(
        endpoint,
        &credentials.collector,
        ClientLifecycleMode::Initialize,
    )
    .await;
    assert_eq!(
        collector.peer_info().expect("initialized").protocol_version,
        ProtocolVersion::V_2025_11_25
    );
    let reader = connect(
        endpoint,
        &credentials.reader,
        ClientLifecycleMode::Initialize,
    )
    .await;
    let mut names: Vec<_> = collector
        .list_tools(None)
        .await
        .unwrap()
        .tools
        .into_iter()
        .map(|tool| tool.name)
        .collect();
    names.sort_unstable();
    assert_eq!(names, ["get_result", "record_result", "server_status"]);
    let status = call(&collector, "server_status", Value::Null, None).await;
    assert_eq!(
        status,
        json!({"status": "ready", "project_id": project, "role": "collector"})
    );
    let status = call(&reader, "server_status", Value::Null, None).await;
    assert_eq!(
        status,
        json!({"status": "ready", "project_id": project, "role": "reader"})
    );
    println!("MCP 2025-11-25 initialize, discovery, status: PASS");

    let lookup = json!({"project_id": project, "request_id": request_id});
    let input = json!({"project_id": project, "request_id": request_id, "result": FIXTURE});
    let other_project = format!("other-{request_id}");

    call(&reader, "get_result", lookup.clone(), Some("NOT_FOUND")).await;
    call(&reader, "record_result", input.clone(), Some("FORBIDDEN")).await;
    call(
        &collector,
        "record_result",
        json!({"project_id": other_project, "request_id": request_id, "result": FIXTURE}),
        Some("FORBIDDEN"),
    )
    .await;
    call(
        &reader,
        "get_result",
        json!({"project_id": other_project, "request_id": request_id}),
        Some("FORBIDDEN"),
    )
    .await;
    call(
        &collector,
        "record_result",
        json!({"project_id": project, "request_id": request_id, "result": "ordinary text"}),
        Some("INVALID_INPUT"),
    )
    .await;
    println!("Missing result, invalid input, role and project isolation: PASS");

    let (a, b) = tokio::join!(
        call(&collector, "record_result", input.clone(), None),
        call(&collector, "record_result", input, None),
    );
    let (stored, replayed) = if a["status"] == "stored" {
        (a, b)
    } else {
        (b, a)
    };
    assert_eq!(stored["status"], "stored");
    assert_eq!(replayed["status"], "replayed");
    let receipt = &stored["receipt"];
    assert_eq!(receipt, &replayed["receipt"]);
    assert_eq!(receipt["result"], FIXTURE);
    assert_eq!(receipt["project_id"], project);
    assert_eq!(receipt["request_id"], request_id);
    Uuid::parse_str(receipt["receipt_id"].as_str().unwrap()).expect("receipt UUID");

    call(&collector, "record_result", json!({"project_id": project, "request_id": request_id, "result": "[SYNTHETIC] conflicting input"}), Some("CONFLICT")).await;
    let found = call(&reader, "get_result", lookup, None).await;
    assert_eq!(found, json!({"status": "found", "receipt": receipt}));
    println!(
        "Concurrent save/replay, conflict, exact readback: PASS project={project} request={request_id} receipt={}",
        receipt["receipt_id"]
    );
    collector.cancel().await.unwrap();
    reader.cancel().await.unwrap();
    receipt.clone()
}

async fn launch(db: &Path, project: &str, credentials: &Credentials) -> (Child, String) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_synthetic-mcp-server"))
        .env("MCP_BIND", "127.0.0.1:0")
        .env("MCP_DB_PATH", db)
        .env("MCP_PROJECT", project)
        .env("MCP_COLLECTOR_TOKEN", &credentials.collector)
        .env("MCP_READER_TOKEN", &credentials.reader)
        .env_remove("MCP_ALLOWED_HOST")
        .stdout(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .expect("start server");

    let mut line = String::new();
    let count = timeout(
        Duration::from_secs(10),
        BufReader::new(child.stdout.take().unwrap()).read_line(&mut line),
    )
    .await
    .expect("server startup timed out")
    .expect("read server address");
    assert!(count > 0, "server stopped before reporting its listener");
    let address: std::net::SocketAddr = line.trim().parse().expect("listener address");
    (child, format!("http://{address}/mcp"))
}

#[tokio::test]
async fn local_mcp_round_trip_and_restart() {
    let temporary = tempfile::tempdir().unwrap();
    let db = temporary.path().join("results.sqlite3");
    let credentials = Credentials {
        collector: Uuid::new_v4().simple().to_string() + &Uuid::new_v4().simple().to_string(),
        reader: Uuid::new_v4().simple().to_string() + &Uuid::new_v4().simple().to_string(),
    };
    let request_id = Uuid::new_v4().to_string();
    let (mut server, endpoint) = launch(&db, PROJECT, &credentials).await;

    let http = http_client();
    let anonymous = http.post(&endpoint).send().await.unwrap();
    assert_eq!(anonymous.status(), 401);
    let invalid = http
        .post(&endpoint)
        .bearer_auth("invalid")
        .send()
        .await
        .unwrap();
    assert_eq!(invalid.status(), 401);
    let origin = http
        .post(&endpoint)
        .bearer_auth(&credentials.collector)
        .header("Origin", "https://untrusted.example")
        .send()
        .await
        .unwrap();
    assert_eq!(origin.status(), 403);
    println!("Missing/invalid credentials and untrusted Origin: PASS");

    let receipt = round_trip(&endpoint, PROJECT, &credentials, &request_id).await;
    server.kill().await.unwrap();

    let (mut restarted, endpoint) = launch(&db, PROJECT, &credentials).await;
    let reader = connect(
        &endpoint,
        &credentials.reader,
        ClientLifecycleMode::Discover {
            preferred_versions: vec![ProtocolVersion::V_2026_07_28],
        },
    )
    .await;
    let tools = reader.list_tools(None).await.unwrap();
    assert_eq!(tools.tools.len(), 3);
    let args = json!({"project_id": PROJECT, "request_id": request_id});
    let found = call(&reader, "get_result", args, None).await;
    assert_eq!(found, json!({"status": "found", "receipt": receipt}));
    println!("Server process restart and MCP 2026-07-28 discovery/readback: PASS");
    reader.cancel().await.unwrap();
    restarted.kill().await.unwrap();

    let (mut other, endpoint) = launch(&db, "other-project", &credentials).await;
    let reader = connect(
        &endpoint,
        &credentials.reader,
        ClientLifecycleMode::Initialize,
    )
    .await;
    call(
        &reader,
        "get_result",
        json!({"project_id": "other-project", "request_id": request_id}),
        Some("NOT_FOUND"),
    )
    .await;
    call(
        &reader,
        "get_result",
        json!({"project_id": PROJECT, "request_id": request_id}),
        Some("FORBIDDEN"),
    )
    .await;
    println!("Project isolation against the same database file: PASS");
    reader.cancel().await.unwrap();
    other.kill().await.unwrap();
}

#[tokio::test]
#[ignore = "requires an authorized external test endpoint"]
async fn remote_mcp_round_trip() {
    let endpoint = env::var("MCP_TEST_URL").expect("set MCP_TEST_URL");
    let url = reqwest::Url::parse(&endpoint).expect("valid MCP_TEST_URL");
    assert_eq!(
        url.scheme(),
        "https",
        "use HTTPS for remote bearer credentials"
    );
    assert!(
        url.username().is_empty()
            && url.password().is_none()
            && url.query().is_none()
            && url.fragment().is_none()
    );
    let credentials = Credentials {
        collector: env::var("MCP_COLLECTOR_TOKEN").expect("set MCP_COLLECTOR_TOKEN"),
        reader: env::var("MCP_READER_TOKEN").expect("set MCP_READER_TOKEN"),
    };
    let project = env::var("MCP_PROJECT").unwrap_or_else(|_| PROJECT.into());
    let request_id = Uuid::new_v4().to_string();
    round_trip(&endpoint, &project, &credentials, &request_id).await;
}
