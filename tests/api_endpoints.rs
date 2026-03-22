//! API endpoint integration tests for ORAC sidecar.
//!
//! Tests the HTTP GET endpoints: `/health`, `/field`, `/blackboard`,
//! `/metrics`, `/field/ghosts`, and `/consent/{sphere_id}`.
//!
//! Uses an in-process Axum server on an ephemeral port with `ureq` as client.
//! All ureq calls run via `spawn_blocking` to avoid blocking the tokio runtime.

#[cfg(feature = "api")]
mod api_tests {
    use std::sync::Arc;
    use std::time::Duration;

    use orac_sidecar::m1_core::m01_core_types::PaneId;
    use orac_sidecar::m1_core::m03_config::PvConfig;
    use orac_sidecar::m3_hooks::m10_hook_server::{build_router, OracState};

    /// Make a blocking GET request via `spawn_blocking` and return the response body as JSON.
    async fn get_json(url: &str) -> serde_json::Value {
        let url = url.to_owned();
        let body = tokio::task::spawn_blocking(move || {
            ureq::get(&url)
                .timeout(Duration::from_secs(5))
                .call()
                .expect("GET request failed")
                .into_string()
                .expect("read body")
        })
        .await
        .expect("spawn_blocking join");
        serde_json::from_str(&body).expect("parse json")
    }

    /// Make a blocking GET request via `spawn_blocking` and return the raw body string.
    async fn get_text(url: &str) -> (u16, String, String) {
        let url = url.to_owned();
        tokio::task::spawn_blocking(move || {
            let resp = ureq::get(&url)
                .timeout(Duration::from_secs(5))
                .call()
                .expect("GET request failed");
            let status = resp.status();
            let ct = resp.header("content-type").unwrap_or("").to_owned();
            let body = resp.into_string().expect("read body");
            (status, ct, body)
        })
        .await
        .expect("spawn_blocking join")
    }

    /// Make a blocking GET request and return the status code.
    async fn get_status(url: &str) -> u16 {
        let url = url.to_owned();
        tokio::task::spawn_blocking(move || {
            ureq::get(&url)
                .timeout(Duration::from_secs(5))
                .call()
                .expect("GET request failed")
                .status()
        })
        .await
        .expect("spawn_blocking join")
    }

    /// Make a blocking POST request and return the status code.
    async fn post_status(url: &str, body: &str) -> u16 {
        let url = url.to_owned();
        let body = body.to_owned();
        tokio::task::spawn_blocking(move || {
            ureq::post(&url)
                .timeout(Duration::from_secs(5))
                .set("Content-Type", "application/json")
                .send_string(&body)
                .expect("POST request failed")
                .status()
        })
        .await
        .expect("spawn_blocking join")
    }

    /// Spin up the ORAC router on an ephemeral port.
    ///
    /// Returns the base URL (e.g. `http://127.0.0.1:12345`) and a
    /// `JoinHandle` for the background server task.
    async fn start_test_server() -> (String, tokio::task::JoinHandle<()>) {
        let state = Arc::new(OracState::new(PvConfig::default()));

        // Seed some test data
        state.register_session("sess-001".into(), PaneId::new("alpha-left"));
        state.register_session("sess-002".into(), PaneId::new("beta-right"));

        let router = build_router(Arc::clone(&state));

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind ephemeral port");
        let addr = listener.local_addr().expect("local addr");
        let base_url = format!("http://{addr}");

        let handle = tokio::spawn(async move {
            axum::serve(listener, router)
                .await
                .expect("server failed");
        });

        // Brief yield to let the server start accepting
        tokio::time::sleep(Duration::from_millis(50)).await;

        (base_url, handle)
    }

    // ──────────────────────────────────────────────────────────
    // /health
    // ──────────────────────────────────────────────────────────

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn health_returns_200() {
        let (base, handle) = start_test_server().await;
        let status = get_status(&format!("{base}/health")).await;
        assert_eq!(status, 200);
        handle.abort();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn health_has_required_fields() {
        let (base, handle) = start_test_server().await;
        let body = get_json(&format!("{base}/health")).await;

        assert_eq!(body["status"], "healthy");
        assert_eq!(body["service"], "orac-sidecar");
        assert!(body["port"].is_number());
        assert!(body["sessions"].is_number());
        assert!(body["uptime_ticks"].is_number());
        handle.abort();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn health_reports_correct_session_count() {
        let (base, handle) = start_test_server().await;
        let body = get_json(&format!("{base}/health")).await;
        assert_eq!(body["sessions"], 2);
        handle.abort();
    }

    // ──────────────────────────────────────────────────────────
    // Mock PV2 for /field tests
    // ──────────────────────────────────────────────────────────

    /// Start a mock PV2 server that returns canned health and spheres data.
    async fn start_mock_pv2() -> (String, tokio::task::JoinHandle<()>) {
        let health_handler = axum::routing::get(|| async {
            axum::Json(serde_json::json!({
                "r": 0.9276,
                "tick": 4567,
                "spheres": 12,
                "k": 1.5,
                "k_mod": 1.21
            }))
        });
        let spheres_handler = axum::routing::get(|| async {
            axum::Json(serde_json::json!([
                {"id": "sphere-1", "phase": 1.23},
                {"id": "sphere-2", "phase": 2.34}
            ]))
        });
        let router = axum::Router::new()
            .route("/health", health_handler)
            .route("/spheres", spheres_handler);

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind mock pv2");
        let addr = listener.local_addr().expect("mock pv2 addr");
        let base_url = format!("http://{addr}");

        let handle = tokio::spawn(async move {
            axum::serve(listener, router).await.expect("mock pv2 serve");
        });
        tokio::time::sleep(Duration::from_millis(50)).await;
        (base_url, handle)
    }

    /// Start ORAC server with a custom PV2 URL (pointing to mock).
    async fn start_test_server_with_pv2(pv2_url: &str) -> (String, tokio::task::JoinHandle<()>) {
        let state = Arc::new(OracState::with_urls(
            PvConfig::default(),
            pv2_url.to_owned(),
            "http://127.0.0.1:19999".into(),
            "http://127.0.0.1:19998".into(),
            "http://127.0.0.1:19997".into(),
        ));
        state.register_session("sess-001".into(), PaneId::new("alpha-left"));

        let router = build_router(Arc::clone(&state));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind ephemeral port");
        let addr = listener.local_addr().expect("local addr");
        let base_url = format!("http://{addr}");

        let handle = tokio::spawn(async move {
            axum::serve(listener, router).await.expect("server failed");
        });
        tokio::time::sleep(Duration::from_millis(50)).await;
        (base_url, handle)
    }

    // ──────────────────────────────────────────────────────────
    // /field
    // ──────────────────────────────────────────────────────────

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn field_returns_200() {
        let (base, handle) = start_test_server().await;
        let status = get_status(&format!("{base}/field")).await;
        assert_eq!(status, 200);
        handle.abort();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn field_has_source_field() {
        let (base, handle) = start_test_server().await;
        let body = get_json(&format!("{base}/field")).await;
        assert_eq!(body["source"], "pv2_proxy");
        assert!(body["tick"].is_number());
        handle.abort();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn field_returns_json_object() {
        let (base, handle) = start_test_server().await;
        let body = get_json(&format!("{base}/field")).await;
        assert!(body.is_object());
        handle.abort();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn field_returns_r_and_sphere_count_from_pv2() {
        let (pv2_url, pv2_handle) = start_mock_pv2().await;
        let (base, handle) = start_test_server_with_pv2(&pv2_url).await;

        let body = get_json(&format!("{base}/field")).await;

        assert_eq!(body["source"], "pv2_proxy");

        let r = body["r"].as_f64().expect("r should be a number");
        assert!(
            (r - 0.9276).abs() < 0.001,
            "r should be ~0.9276, got {r}"
        );

        let sphere_count = body["sphere_count"]
            .as_u64()
            .expect("sphere_count should be a number");
        assert_eq!(sphere_count, 12);

        handle.abort();
        pv2_handle.abort();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn field_returns_k_and_pv2_tick() {
        let (pv2_url, pv2_handle) = start_mock_pv2().await;
        let (base, handle) = start_test_server_with_pv2(&pv2_url).await;

        let body = get_json(&format!("{base}/field")).await;

        let k = body["k"].as_f64().expect("k should be a number");
        assert!((k - 1.5).abs() < 0.001, "k should be 1.5, got {k}");

        let pv2_tick = body["pv2_tick"].as_u64().expect("pv2_tick should be a number");
        assert_eq!(pv2_tick, 4567);

        handle.abort();
        pv2_handle.abort();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn field_returns_spheres_array_from_pv2() {
        let (pv2_url, pv2_handle) = start_mock_pv2().await;
        let (base, handle) = start_test_server_with_pv2(&pv2_url).await;

        let body = get_json(&format!("{base}/field")).await;

        let spheres = body["spheres"].as_array().expect("spheres should be an array");
        assert_eq!(spheres.len(), 2);
        assert_eq!(spheres[0]["id"], "sphere-1");

        handle.abort();
        pv2_handle.abort();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn field_graceful_when_pv2_unreachable() {
        // PV2 URL points to a port nobody is listening on
        let (base, handle) = start_test_server_with_pv2("http://127.0.0.1:19999").await;

        let body = get_json(&format!("{base}/field")).await;

        // Must still return a valid JSON with source and tick
        assert_eq!(body["source"], "pv2_proxy");
        assert!(body["tick"].is_number());
        // r and sphere_count should be absent (not proxied)
        assert!(body.get("r").is_none() || body["r"].is_null());

        handle.abort();
    }

    // ──────────────────────────────────────────────────────────
    // /blackboard
    // ──────────────────────────────────────────────────────────

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn blackboard_returns_200() {
        let (base, handle) = start_test_server().await;
        let status = get_status(&format!("{base}/blackboard")).await;
        assert_eq!(status, 200);
        handle.abort();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn blackboard_has_sessions_array() {
        let (base, handle) = start_test_server().await;
        let body = get_json(&format!("{base}/blackboard")).await;
        assert!(body["sessions"].is_array());
        assert!(body["fleet_size"].is_number());
        assert!(body["uptime_ticks"].is_number());
        handle.abort();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn blackboard_reports_seeded_sessions() {
        let (base, handle) = start_test_server().await;
        let body = get_json(&format!("{base}/blackboard")).await;

        let sessions = body["sessions"].as_array().expect("sessions is array");
        assert_eq!(sessions.len(), 2);
        assert_eq!(body["fleet_size"], 2);

        for sess in sessions {
            assert!(sess["session_id"].is_string());
            assert!(sess["pane_id"].is_string());
            assert!(sess["poll_counter"].is_number());
        }
        handle.abort();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn blackboard_contains_known_pane_ids() {
        let (base, handle) = start_test_server().await;
        let body = get_json(&format!("{base}/blackboard")).await;

        let sessions = body["sessions"].as_array().expect("sessions is array");
        let pane_ids: Vec<&str> = sessions
            .iter()
            .filter_map(|s| s["pane_id"].as_str())
            .collect();

        assert!(pane_ids.contains(&"alpha-left"));
        assert!(pane_ids.contains(&"beta-right"));
        handle.abort();
    }

    // ──────────────────────────────────────────────────────────
    // /metrics
    // ──────────────────────────────────────────────────────────

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn metrics_returns_200() {
        let (base, handle) = start_test_server().await;
        let status = get_status(&format!("{base}/metrics")).await;
        assert_eq!(status, 200);
        handle.abort();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn metrics_is_prometheus_text_format() {
        let (base, handle) = start_test_server().await;
        let (_, content_type, body) = get_text(&format!("{base}/metrics")).await;

        assert!(
            content_type.contains("text/plain"),
            "expected text/plain, got: {content_type}"
        );
        assert!(body.contains("# HELP orac_sessions_active"));
        assert!(body.contains("# TYPE orac_sessions_active gauge"));
        assert!(body.contains("orac_sessions_active"));
        assert!(body.contains("# HELP orac_uptime_ticks"));
        assert!(body.contains("# TYPE orac_uptime_ticks counter"));
        handle.abort();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn metrics_contains_session_count() {
        let (base, handle) = start_test_server().await;
        let (_, _, body) = get_text(&format!("{base}/metrics")).await;
        assert!(body.contains("orac_sessions_active 2"));
        handle.abort();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn metrics_all_data_lines_have_orac_prefix() {
        let (base, handle) = start_test_server().await;
        let (_, _, body) = get_text(&format!("{base}/metrics")).await;

        for line in body.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            // Data lines must start with orac_ prefix
            assert!(
                trimmed.starts_with("orac_"),
                "metric data line should start with orac_ prefix: {trimmed}"
            );
        }
        handle.abort();
    }

    // ──────────────────────────────────────────────────────────
    // /field/ghosts
    // ──────────────────────────────────────────────────────────

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn field_ghosts_returns_200() {
        let (base, handle) = start_test_server().await;
        let status = get_status(&format!("{base}/field/ghosts")).await;
        assert_eq!(status, 200);
        handle.abort();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn field_ghosts_empty_when_no_departures() {
        let (base, handle) = start_test_server().await;
        let body = get_json(&format!("{base}/field/ghosts")).await;

        let ghosts = body["ghosts"].as_array().expect("ghosts is array");
        assert!(ghosts.is_empty());
        handle.abort();
    }

    // ──────────────────────────────────────────────────────────
    // /consent/{sphere_id}
    // ──────────────────────────────────────────────────────────

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn consent_get_returns_200() {
        let (base, handle) = start_test_server().await;
        let status = get_status(&format!("{base}/consent/test-sphere")).await;
        assert_eq!(status, 200);
        handle.abort();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn consent_get_includes_sphere_id() {
        let (base, handle) = start_test_server().await;
        let body = get_json(&format!("{base}/consent/my-sphere")).await;
        assert_eq!(body["sphere_id"], "my-sphere");
        handle.abort();
    }

    // ──────────────────────────────────────────────────────────
    // Hook POST endpoints (smoke tests)
    // ──────────────────────────────────────────────────────────

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn hooks_session_start_returns_200() {
        let (base, handle) = start_test_server().await;
        let status = post_status(
            &format!("{base}/hooks/SessionStart"),
            r#"{"session_id": "test-sess"}"#,
        )
        .await;
        assert_eq!(status, 200);
        handle.abort();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn hooks_post_tool_use_returns_200() {
        let (base, handle) = start_test_server().await;
        let status = post_status(
            &format!("{base}/hooks/PostToolUse"),
            r#"{"tool_name": "Read"}"#,
        )
        .await;
        assert_eq!(status, 200);
        handle.abort();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn hooks_stop_returns_200() {
        let (base, handle) = start_test_server().await;
        let status = post_status(&format!("{base}/hooks/Stop"), r#"{}"#).await;
        assert_eq!(status, 200);
        handle.abort();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn hooks_permission_request_returns_200() {
        let (base, handle) = start_test_server().await;
        let status = post_status(
            &format!("{base}/hooks/PermissionRequest"),
            r#"{"tool_name": "Read"}"#,
        )
        .await;
        assert_eq!(status, 200);
        handle.abort();
    }

    // ──────────────────────────────────────────────────────────
    // 404 for unknown routes
    // ──────────────────────────────────────────────────────────

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn unknown_route_returns_404() {
        let (base, handle) = start_test_server().await;
        let url = format!("{base}/nonexistent");
        let status = tokio::task::spawn_blocking(move || {
            match ureq::get(&url)
                .timeout(Duration::from_secs(5))
                .call()
            {
                Ok(resp) => resp.status(),
                Err(ureq::Error::Status(code, _)) => code,
                Err(e) => panic!("unexpected error: {e}"),
            }
        })
        .await
        .expect("spawn_blocking join");

        assert_eq!(status, 404);
        handle.abort();
    }
}
