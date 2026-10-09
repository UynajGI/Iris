use super::*;

fn setup() -> (tempfile::TempDir, IrisMcp) {
    let temp = tempfile::tempdir().unwrap();
    let state = AppState::new(
        &temp.path().join("library.sqlite3"),
        temp.path().join("models"),
    )
    .unwrap();
    let server = IrisMcp::new(state, temp.path(), &[temp.path().to_owned()], false).unwrap();
    (temp, server)
}
async fn call(server: &IrisMcp, name: &str, args: Value) -> CallToolResult {
    server
        .invoke(name, args.as_object().unwrap().clone())
        .await
        .unwrap()
}
fn output(result: &CallToolResult) -> Value {
    serde_json::to_value(result).unwrap()["structuredContent"]["result"].clone()
}

#[tokio::test]
async fn retries_are_durable_and_agent_marks_are_not_human_decisions() {
    let (temp, server) = setup();
    let args = json!({"request_id":"create-1","data":{"root":temp.path()}});
    let first = call(&server, "iris_create_project", args.clone()).await;
    assert_eq!(
        serde_json::to_value(&first).unwrap(),
        serde_json::to_value(call(&server, "iris_create_project", args.clone()).await).unwrap()
    );
    let id = output(&first)["id"].as_i64().unwrap();
    let changed_args =
        json!({"request_id":"create-1","data":{"root":temp.path(),"auto_device":true}});
    assert!(server
        .invoke(
            "iris_create_project",
            changed_args.as_object().unwrap().clone()
        )
        .await
        .is_err());
    let photo_id = {
        let services = server.state.services.lock().unwrap();
        services.store.conn.execute("INSERT INTO photos(project_id,path,filename,format,mtime,size_bytes,width,height) VALUES(?1,'a.jpg','a.jpg','jpeg',0,1,1,1)",[id]).unwrap();
        services.store.conn.last_insert_rowid()
    };
    let marks = json!({"request_id":"marks-1","project_id":id,"data":{"photo_ids":[photo_id],"decision":"keep","rating":4,"color_label":"blue"}});
    let marked = call(&server, "iris_set_marks", marks.clone()).await;
    assert_ne!(marked.is_error, Some(true));
    let services = server.state.services.lock().unwrap();
    let source: String = services
        .store
        .conn
        .query_row(
            "SELECT source FROM decisions ORDER BY id DESC LIMIT 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(source, "agent:mcp");
    drop(services);
    let reopened = IrisMcp::new(
        server.state.clone(),
        temp.path(),
        &[temp.path().to_owned()],
        false,
    )
    .unwrap();
    assert_eq!(
        serde_json::to_value(&marked).unwrap(),
        serde_json::to_value(call(&reopened, "iris_set_marks", marks).await).unwrap()
    );
    call(
        &reopened,
        "iris_undo",
        json!({"project_id":id,"request_id":"undo-1"}),
    )
    .await;
    let photo = reopened
        .state
        .services
        .lock()
        .unwrap()
        .photo(photo_id)
        .unwrap();
    assert_eq!(photo.rating, 0);
    assert_eq!(photo.decision, iris_core::Action::Pending);
}

#[tokio::test]
async fn outside_paths_and_hidden_write_tools_are_rejected() {
    let (temp, server) = setup();
    let other = tempfile::tempdir().unwrap();
    assert!(server
        .invoke(
            "iris_create_project",
            json!({"request_id":"bad","data":{"root":other.path()}})
                .as_object()
                .unwrap()
                .clone()
        )
        .await
        .is_err());
    let read_only = IrisMcp::new(
        server.state.clone(),
        temp.path(),
        &[temp.path().to_owned()],
        true,
    )
    .unwrap();
    assert!(read_only.endpoints.iter().all(|e| e.method == "get"));
    assert!(read_only
        .invoke(
            "iris_create_project",
            json!({"request_id":"bad","data":{"root":temp.path()}})
                .as_object()
                .unwrap()
                .clone()
        )
        .await
        .is_err());
    assert!(server
        .invoke(
            "iris_quarantine_commit",
            json!({"project_id":1,"request_id":"bad","data":{"manifest_id":"x"}})
                .as_object()
                .unwrap()
                .clone()
        )
        .await
        .is_err());
}

#[test]
fn unknown_write_outcome_is_never_replayed() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("audit.jsonl");
    let args = json!({"data":1});
    let mut log = journal::Journal::open(&path).unwrap();
    log.append("op", "tool", &args, None).unwrap();
    drop(log);
    let log = journal::Journal::open(&path).unwrap();
    assert!(log
        .existing("op", "tool", &args)
        .unwrap_err()
        .to_string()
        .contains("unknown outcome"));
}

#[tokio::test]
async fn private_default_cache_does_not_require_photo_root_authorization() {
    let temp = tempfile::tempdir().unwrap();
    let photos = temp.path().join("photos");
    let data = temp.path().join("data");
    std::fs::create_dir(&photos).unwrap();
    let state = AppState::new(&data.join("library.sqlite3"), temp.path().join("models")).unwrap();
    let server = IrisMcp::new(state, &data, &[photos.clone()], false).unwrap();
    let project = output(
        &call(
            &server,
            "iris_create_project",
            json!({"request_id":"create","data":{"root":photos}}),
        )
        .await,
    );
    let result = call(
        &server,
        "iris_cache_status",
        json!({"project_id":project["id"]}),
    )
    .await;
    assert_ne!(result.is_error, Some(true));
}

#[test]
fn catalog_references_resolve_and_images_are_explicit() {
    let tools = catalog::catalog(false);
    assert!(tools.len() >= 40);
    let mut names = std::collections::HashSet::new();
    for e in tools {
        assert!(names.insert(e.name));
        let schema = serde_json::to_value(&e.tool).unwrap()["inputSchema"].clone();
        assert!(!schema.to_string().contains("#/components/schemas/"));
        if e.method != "get" {
            assert!(schema["required"]
                .as_array()
                .unwrap()
                .contains(&json!("request_id")));
        }
    }
}
