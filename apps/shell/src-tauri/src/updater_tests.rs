use super::*;
use std::{
    collections::HashMap,
    io::{Read, Write},
    net::TcpListener,
    sync::atomic::AtomicUsize,
    task::{Context, Poll, Waker},
};
use tauri::test::{mock_builder, mock_context, noop_assets, MockRuntime};

#[derive(Default)]
struct FakeHost {
    paused: AtomicBool,
    pauses: AtomicUsize,
    recoveries: AtomicUsize,
    fail_pause: AtomicBool,
    fail_recovery: AtomicBool,
}
impl HostControl for FakeHost {
    fn pause(&self) -> Result<()> {
        self.pauses.fetch_add(1, Ordering::SeqCst);
        self.paused.store(true, Ordering::SeqCst);
        if self.fail_pause.load(Ordering::SeqCst) {
            Err(UpdateError::new(
                "host_failed",
                "test graceful shutdown failed",
            ))
        } else {
            Ok(())
        }
    }
    fn recover(&self) -> Result<()> {
        self.recoveries.fetch_add(1, Ordering::SeqCst);
        self.paused.store(false, Ordering::SeqCst);
        if self.fail_recovery.load(Ordering::SeqCst) {
            Err(UpdateError::new(
                "host_recovery_failed",
                "test recovery failed",
            ))
        } else {
            Ok(())
        }
    }
}

struct Response {
    status: u16,
    body: Vec<u8>,
    delay: Duration,
}
impl Response {
    fn ok(body: impl Into<Vec<u8>>) -> Self {
        Self {
            status: 200,
            body: body.into(),
            delay: Duration::ZERO,
        }
    }
}
struct Server {
    base: String,
    requests: Arc<AtomicUsize>,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Server {
    fn new(responses: impl FnOnce(&str) -> HashMap<String, Response>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let responses = responses(&base);
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = stop.clone();
        let requests = Arc::new(AtomicUsize::new(0));
        let count = requests.clone();
        let thread = std::thread::spawn(move || {
            while !stopped.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((mut socket, _)) => {
                        socket
                            .set_read_timeout(Some(Duration::from_secs(1)))
                            .unwrap();
                        let mut header = Vec::new();
                        let mut byte = [0];
                        while header.len() < 8192 && !header.ends_with(b"\r\n\r\n") {
                            if socket.read(&mut byte).ok() != Some(1) {
                                break;
                            }
                            header.push(byte[0]);
                        }
                        count.fetch_add(1, Ordering::SeqCst);
                        let header = String::from_utf8_lossy(&header);
                        let path = header.split_whitespace().nth(1).unwrap_or("");
                        let fallback = Response {
                            status: 404,
                            body: vec![],
                            delay: Duration::ZERO,
                        };
                        let response = responses.get(path).unwrap_or(&fallback);
                        std::thread::sleep(response.delay);
                        let _ = write!(socket, "HTTP/1.1 {} Test\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n", response.status, response.body.len());
                        let _ = socket.write_all(&response.body);
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(2))
                    }
                    Err(_) => break,
                }
            }
        });
        Self {
            base,
            requests,
            stop,
            thread: Some(thread),
        }
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        self.thread.take().unwrap().join().unwrap();
    }
}
fn sign(bytes: &[u8], comment: &str) -> (String, String) {
    // The secret exists only in this test's memory, never in a repository/file.
    let keypair = minisign::KeyPair::generate_unencrypted_keypair().unwrap();
    let key = STANDARD.encode(keypair.pk.to_box().unwrap().to_string());
    let signature =
        minisign::sign(Some(&keypair.pk), &keypair.sk, bytes, Some(comment), None).unwrap();
    (key, STANDARD.encode(signature.to_string()))
}
fn official(
    server: &Server,
    key: &str,
    timeout: Duration,
    secure_transport: bool,
) -> (tauri::App<MockRuntime>, Arc<dyn Backend>) {
    let mut context = mock_context(noop_assets());
    context.config_mut().plugins.0.insert(
        "updater".into(),
        serde_json::json!({
            "pubkey": key, "dangerousInsecureTransportProtocol": true, "requireSignedVersion": true,
        }),
    );
    let app = mock_builder()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .build(context)
        .unwrap();
    let updater = app
        .updater_builder()
        .endpoints(vec![format!("{}/latest", server.base).parse().unwrap()])
        .unwrap()
        .no_proxy()
        .timeout(timeout)
        .configure_client(move |client| client.https_only(secure_transport))
        .build()
        .unwrap();
    (
        app,
        Arc::new(OfficialBackend {
            updater,
            secure_transport,
        }),
    )
}
fn service(backend: Arc<dyn Backend>, host: Arc<FakeHost>) -> UpdateService {
    UpdateService::new(Some(backend), None, host, Arc::new(|_| {}))
}
fn manifest(base: &str, version: &str, signature: &str) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({
        "version": version, "notes": "# Changes\nSynthetic updater fixture", "pub_date": "2026-10-07T00:00:00Z",
        "platforms": { "windows-x86_64": { "url": format!("{base}/artifact"), "signature": signature } }
    })).unwrap()
}

#[test]
fn build_configuration_is_explicit_and_validates_trust() {
    assert!(matches!(
        Configuration::parse(None, None, None).unwrap(),
        Configuration::Disabled("installer_required")
    ));
    assert!(matches!(
        Configuration::parse(Some("nsis"), None, None).unwrap(),
        Configuration::Disabled("release_configuration_missing")
    ));
    let (key, _) = sign(b"fixture", "version:0.2.0");
    assert!(matches!(
        Configuration::parse(Some("nsis"), Some("https://example.com/latest"), Some(&key)).unwrap(),
        Configuration::Enabled(_)
    ));
    for (kind, endpoint, public_key) in [
        (Some("nsis"), Some("https://example.com"), None),
        (Some("nsis"), None, Some(key.as_str())),
        (None, Some("https://example.com"), Some(key.as_str())),
        (
            Some("portable"),
            Some("https://example.com"),
            Some(key.as_str()),
        ),
        (Some("unknown"), None, None),
        (Some("nsis"), Some("http://example.com"), Some(key.as_str())),
        (
            Some("nsis"),
            Some("https://user:secret@example.com"),
            Some(key.as_str()),
        ),
        (
            Some("nsis"),
            Some("https://example.com/#fragment"),
            Some(key.as_str()),
        ),
        (Some("nsis"), Some("garbage"), Some(key.as_str())),
        (
            Some("nsis"),
            Some("https://example.com"),
            Some("invalid-key"),
        ),
    ] {
        assert!(Configuration::parse(kind, endpoint, public_key).is_err());
    }
}

#[test]
fn offline_unconfigured_check_never_needs_a_backend_or_host() {
    let host = Arc::new(FakeHost::default());
    let service = UpdateService::new(
        None,
        Some("installer_required"),
        host.clone(),
        Arc::new(|_| {}),
    );
    assert_eq!(
        tauri::async_runtime::block_on(service.check())
            .unwrap()
            .state,
        UpdatePhase::NotConfigured
    );
    assert_eq!(
        tauri::async_runtime::block_on(service.download())
            .unwrap_err()
            .code,
        "not_configured"
    );
    assert_eq!(service.install().unwrap_err().code, "not_configured");
    assert_eq!(host.pauses.load(Ordering::SeqCst), 0);
}

#[test]
fn actual_plugin_checks_downloads_verifies_and_rejects_non_installer_without_leaving_host_paused() {
    tauri::async_runtime::block_on(async {
        // Signed inert text intentionally cannot be executed as a Windows installer.
        let bytes = b"synthetic data, not an executable";
        let (key, signature) = sign(bytes, "timestamp:1\tfile:test\tversion:0.2.0");
        let server = Server::new(|base| {
            HashMap::from([
                (
                    "/latest".into(),
                    Response::ok(manifest(base, "0.2.0", &signature)),
                ),
                ("/artifact".into(), Response::ok(bytes.to_vec())),
            ])
        });
        let (_app, backend) = official(&server, &key, Duration::from_secs(2), false);
        let host = Arc::new(FakeHost::default());
        let observations = Arc::new(Mutex::new(Vec::new()));
        let recorded = observations.clone();
        let service = UpdateService::new(
            Some(backend),
            None,
            host.clone(),
            Arc::new(move |status| recorded.lock().unwrap().push(status)),
        );
        assert_eq!(service.install().unwrap_err().code, "invalid_state");
        let status = service.check().await.unwrap();
        assert_eq!(status.state, UpdatePhase::Available);
        let metadata = status.update.unwrap();
        assert_eq!(metadata.version, "0.2.0");
        assert!(metadata.notes.unwrap().starts_with("# Changes"));
        assert!(metadata.published_at_unix.is_some());
        assert_eq!(service.install().unwrap_err().code, "invalid_state");
        assert_eq!(
            service.download().await.unwrap().state,
            UpdatePhase::Downloaded
        );
        let observations = observations.lock().unwrap();
        assert!(observations
            .iter()
            .any(|s| s.state == UpdatePhase::Downloading && s.downloaded_bytes > 0));
        assert_eq!(
            observations.last().unwrap().downloaded_bytes,
            bytes.len() as u64
        );
        drop(observations);
        assert_eq!(service.install().unwrap_err().code, "install_failed");
        assert!(!host.paused.load(Ordering::SeqCst));
        assert_eq!(host.recoveries.load(Ordering::SeqCst), 1);
        assert_eq!(service.install().unwrap_err().code, "invalid_state");
        assert_eq!(server.requests.load(Ordering::SeqCst), 2);
    });
}

#[test]
fn actual_plugin_rejects_tampered_wrong_key_and_unsigned_or_wrong_versions() {
    tauri::async_runtime::block_on(async {
        for mode in ["tampered", "wrong_key", "missing_version", "wrong_version"] {
            let original = b"synthetic fixture";
            let comment = match mode {
                "missing_version" => "timestamp:1",
                "wrong_version" => "version:0.1.0",
                _ => "version:0.2.0",
            };
            let (mut key, signature) = sign(original, comment);
            if mode == "wrong_key" {
                key = sign(b"other", "version:0.2.0").0;
            }
            let downloaded = if mode == "tampered" {
                b"modified fixture".to_vec()
            } else {
                original.to_vec()
            };
            let server = Server::new(|base| {
                HashMap::from([
                    (
                        "/latest".into(),
                        Response::ok(manifest(base, "0.2.0", &signature)),
                    ),
                    ("/artifact".into(), Response::ok(downloaded)),
                ])
            });
            let (_app, backend) = official(&server, &key, Duration::from_secs(2), false);
            let host = Arc::new(FakeHost::default());
            let service = service(backend, host.clone());
            service.check().await.unwrap();
            assert_eq!(
                service.download().await.unwrap_err().code,
                "download_failed",
                "{mode}"
            );
            assert_eq!(service.status().state, UpdatePhase::Failed);
            assert_eq!(service.install().unwrap_err().code, "invalid_state");
            assert_eq!(host.pauses.load(Ordering::SeqCst), 0);
        }
    });
}

#[test]
fn actual_plugin_handles_no_update_wrong_target_invalid_response_timeout_and_https_only() {
    tauri::async_runtime::block_on(async {
        let (key, signature) = sign(b"x", "version:0.2.0");
        for mode in [
            "old_version",
            "no_update",
            "wrong_target",
            "malformed",
            "timeout",
            "https_only",
        ] {
            let server = Server::new(|base| {
                let response = match mode {
                    "old_version" => Response::ok(manifest(base, "0.0.1", &signature)),
                    "no_update" => Response {
                        status: 204,
                        body: vec![],
                        delay: Duration::ZERO,
                    },
                    "wrong_target" => Response::ok(
                        serde_json::to_vec(
                            &serde_json::json!({"version":"0.2.0", "platforms": {}}),
                        )
                        .unwrap(),
                    ),
                    "timeout" => Response {
                        status: 200,
                        body: vec![],
                        delay: Duration::from_millis(200),
                    },
                    _ => Response::ok(b"malformed".to_vec()),
                };
                HashMap::from([("/latest".into(), response)])
            });
            let (_app, backend) = official(
                &server,
                &key,
                Duration::from_millis(75),
                mode == "https_only",
            );
            let service = service(backend, Arc::new(FakeHost::default()));
            let result = service.check().await;
            if matches!(mode, "old_version" | "no_update") {
                assert_eq!(result.unwrap().state, UpdatePhase::UpToDate);
            } else {
                assert_eq!(result.unwrap_err().code, "check_failed", "{mode}");
            }
            if mode == "https_only" {
                assert_eq!(server.requests.load(Ordering::SeqCst), 0);
            }
        }
    });
}

struct PendingBackend;
impl Backend for PendingBackend {
    fn check(&self) -> Task<'_, Option<Arc<dyn Candidate>>> {
        Box::pin(std::future::pending())
    }
}
#[test]
fn concurrent_calls_are_rejected_and_dropped_operations_release_the_reservation() {
    let service = service(Arc::new(PendingBackend), Arc::new(FakeHost::default()));
    let mut check = Box::pin(service.check());
    let mut cx = Context::from_waker(Waker::noop());
    assert!(matches!(check.as_mut().poll(&mut cx), Poll::Pending));
    assert_eq!(
        tauri::async_runtime::block_on(service.check())
            .unwrap_err()
            .code,
        "busy"
    );
    assert_eq!(
        tauri::async_runtime::block_on(service.download())
            .unwrap_err()
            .code,
        "busy"
    );
    assert_eq!(service.install().unwrap_err().code, "busy");
    drop(check);
    assert_eq!(service.status().error.unwrap().code, "operation_aborted");
    let mut retry = Box::pin(service.check());
    assert!(matches!(retry.as_mut().poll(&mut cx), Poll::Pending));
}

#[test]
fn host_shutdown_failure_prevents_installation_and_attempts_recovery() {
    struct NeverInstall;
    impl Candidate for NeverInstall {
        fn metadata(&self) -> UpdateMetadata {
            unreachable!()
        }
        fn download_verified(&self, _: Progress) -> Task<'_, Vec<u8>> {
            unreachable!()
        }
        fn install(&self, _: &[u8]) -> Result<()> {
            panic!("must not install if daemon did not stop");
        }
    }
    let host = Arc::new(FakeHost::default());
    host.fail_pause.store(true, Ordering::SeqCst);
    let service = service(Arc::new(PendingBackend), host.clone());
    service.change(|inner| {
        inner.candidate = Some(Arc::new(NeverInstall));
        inner.verified = Some(vec![1]);
        inner.status.state = UpdatePhase::Downloaded;
    });
    assert_eq!(service.install().unwrap_err().code, "host_failed");
    assert_eq!(host.recoveries.load(Ordering::SeqCst), 1);
    assert!(!host.paused.load(Ordering::SeqCst));
    assert_eq!(service.install().unwrap_err().code, "invalid_state");
}

#[test]
fn installer_launch_failure_after_pre_exit_hook_restores_host_and_retains_status_service() {
    struct LaunchFailure(Arc<FakeHost>);
    impl Candidate for LaunchFailure {
        fn metadata(&self) -> UpdateMetadata {
            unreachable!()
        }
        fn download_verified(&self, _: Progress) -> Task<'_, Vec<u8>> {
            unreachable!()
        }
        fn install(&self, _: &[u8]) -> Result<()> {
            assert!(self.0.paused.load(Ordering::SeqCst));
            // Model the official Windows ordering: pre-exit hook, then launch,
            // which can fail. There must be no destructive webview cleanup here.
            self.0.pause()?;
            Err(UpdateError::new(
                "install_failed",
                "simulated ShellExecuteW failure",
            ))
        }
    }
    for recovery_fails in [false, true] {
        let host = Arc::new(FakeHost::default());
        host.fail_recovery.store(recovery_fails, Ordering::SeqCst);
        let service = service(Arc::new(PendingBackend), host.clone());
        service.change(|inner| {
            inner.candidate = Some(Arc::new(LaunchFailure(host.clone())));
            inner.verified = Some(vec![1]);
            inner.status.state = UpdatePhase::Downloaded;
        });
        let code = if recovery_fails {
            "host_recovery_failed"
        } else {
            "install_failed"
        };
        assert_eq!(service.install().unwrap_err().code, code);
        assert_eq!(host.pauses.load(Ordering::SeqCst), 2);
        assert_eq!(host.recoveries.load(Ordering::SeqCst), 1);
        assert!(!host.paused.load(Ordering::SeqCst));
        assert_eq!(service.status().state, UpdatePhase::Failed);
        assert_eq!(service.status().error.unwrap().code, code);
        assert_eq!(service.install().unwrap_err().code, "invalid_state");
    }
}

#[test]
fn release_tool_verifies_signature_key_and_exact_signed_version() {
    let bytes = b"read-only release verifier fixture";
    let (key, signature) = sign(bytes, "timestamp:1\tfile:test\tversion:0.2.0");
    verify_release_artifact(bytes, &signature, &key, "0.2.0").unwrap();
    assert!(verify_release_artifact(b"tampered", &signature, &key, "0.2.0").is_err());
    assert!(
        verify_release_artifact(bytes, &signature, &sign(bytes, "version:0.2.0").0, "0.2.0")
            .is_err()
    );
    assert!(verify_release_artifact(bytes, &signature, &key, "0.3.0").is_err());
    for comment in ["timestamp:1", "version:0.2.0\tversion:0.3.0"] {
        let (key, signature) = sign(bytes, comment);
        assert!(verify_release_artifact(bytes, &signature, &key, "0.2.0").is_err());
    }
}
