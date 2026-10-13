// SPDX-License-Identifier: GPL-3.0-or-later
use iris_render::{
    color::ReferenceKind,
    document::{EditDocument, SourceBinding},
    engine::{RenderEngine, RenderQuality, RenderRequest},
    Frame,
};
use std::{
    collections::BTreeMap,
    sync::atomic::{AtomicUsize, Ordering},
};

fn document() -> EditDocument {
    EditDocument::new(SourceBinding {
        photo_id: "1".into(),
        fingerprint: "b".repeat(64),
        width: 32,
        height: 16,
        reference_kind: ReferenceKind::Display,
        raw_white_balance: None,
    })
}
fn source_document() -> EditDocument {
    let mut doc = document();
    doc.output_node = "source".into();
    doc
}
fn request(sequence: u64) -> RenderRequest {
    RenderRequest {
        revision: 3,
        session: "view".into(),
        sequence,
        quality: RenderQuality::Interactive,
        max_edge: 16,
    }
}

#[test]
fn linear_source_cache_reused_and_preview_precision_preserved() {
    let engine = RenderEngine::new(64 * 1024 * 1024, 1024 * 1024).unwrap();
    let calls = AtomicUsize::new(0);
    for sequence in 0..2 {
        let result = engine
            .render(
                &document(),
                &request(sequence),
                |_| {
                    calls.fetch_add(1, Ordering::SeqCst);
                    Frame::new(32, 16, vec![[0.00123, 0.00123, 0.00123, 1.0]; 512])
                },
                &BTreeMap::new(),
            )
            .unwrap();
        assert!(result.approximate);
        assert_eq!((result.frame.width, result.frame.height), (16, 8));
        assert_eq!(result.frame.pixels[0][0], 0.00123);
        engine.check_request("view", sequence).unwrap();
    }
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[test]
fn stale_requests_and_old_disconnect_cannot_cancel_new_request() {
    let engine = RenderEngine::new(64 * 1024 * 1024, 0).unwrap();
    let load = |_: &iris_render::RenderContext| Frame::new(32, 16, vec![[0.0, 0.0, 0.0, 1.0]; 512]);
    engine
        .render(&document(), &request(2), load, &BTreeMap::new())
        .unwrap();
    engine.cancel_request("view", 1).unwrap();
    engine.check_request("view", 2).unwrap();
    assert!(engine
        .render(&document(), &request(1), load, &BTreeMap::new())
        .is_err());
    engine.cancel_request("view", 2).unwrap();
    assert!(engine.check_request("view", 2).is_err());
}

#[test]
fn parent_cancellation_is_checked_before_decoding() {
    let engine = RenderEngine::new(64 * 1024 * 1024, 0).unwrap();
    let context = engine.request_context();
    context.cancel();
    let result = engine.render_with_context(
        &document(),
        &request(1),
        |_| panic!("cancelled request decoded source"),
        &BTreeMap::new(),
        &context,
    );
    assert!(result.is_err());
}

#[test]
fn proxy_cache_hit_reuses_float_pixels_without_reloading_uncached_source() {
    // Original8192 bytes cannot fit; its2048-byte float proxy can.
    let engine = RenderEngine::new(64 * 1024 * 1024, 2048).unwrap();
    let first = engine
        .render(
            &source_document(),
            &request(1),
            |_| Frame::new(32, 16, vec![[-0.1, 0.00123, 1.7, 0.6]; 512]),
            &BTreeMap::new(),
        )
        .unwrap();
    let second = engine
        .render(
            &source_document(),
            &request(2),
            |_| panic!("proxy hit must not decode or average original"),
            &BTreeMap::new(),
        )
        .unwrap();
    assert!(std::sync::Arc::ptr_eq(&first.frame, &second.frame));
    assert!(second.frame.pixels[0][0] < 0.0);
    assert!(second.frame.pixels[0][2] > 1.0);
    let retained = second.frame.clone();
    drop(first);
    drop(second);
    engine.clear_cache().unwrap();
    assert_eq!(engine.memory_used(), retained.byte_len());
    drop(retained);
    assert_eq!(engine.memory_used(), 0);
}

#[test]
fn shared_frame_admission_is_idempotent_and_follows_surviving_arcs() {
    let context = iris_render::RenderContext::new(16);
    let frame = std::sync::Arc::new(Frame::new(1, 1, vec![[0.2, 0.3, 0.4, 1.0]]).unwrap());
    std::thread::scope(|scope| {
        for _ in 0..8 {
            let context = &context;
            let frame = &frame;
            scope.spawn(move || context.track_frame(frame).unwrap());
        }
    });
    assert_eq!(context.memory_used(), 16);
    let clone = frame.clone();
    drop(frame);
    assert_eq!(context.memory_used(), 16);
    drop(clone);
    assert_eq!(context.memory_used(), 0);
    assert!(context.reserve(16).is_ok());
}

#[test]
fn cache_and_graph_source_alias_do_not_double_charge_shared_pixels() {
    let engine = RenderEngine::new(64 * 1024 * 1024, 1024 * 1024).unwrap();
    let result = engine
        .render(
            &source_document(),
            &request(1),
            |_| Frame::new(32, 16, vec![[0.2; 4]; 512]),
            &BTreeMap::new(),
        )
        .unwrap();
    assert_eq!(engine.memory_used(), (32 * 16 + 16 * 8) * 16);
    let clone = result.frame.clone();
    drop(result);
    engine.clear_cache().unwrap();
    assert_eq!(engine.memory_used(), clone.byte_len());
    drop(clone);
    assert_eq!(engine.memory_used(), 0);
}

#[test]
fn proxy_key_contains_binding_and_requested_edge() {
    let engine = RenderEngine::new(64 * 1024 * 1024, 1024 * 1024).unwrap();
    let calls = AtomicUsize::new(0);
    let mut doc = document();
    doc.source.width = 64;
    doc.source.height = 32;
    let mut req = request(1);
    let a = engine
        .render(
            &doc,
            &req,
            |_| {
                calls.fetch_add(1, Ordering::SeqCst);
                Frame::new(64, 32, vec![[0.2; 4]; 2048])
            },
            &BTreeMap::new(),
        )
        .unwrap();
    req.sequence = 2;
    req.max_edge = 32;
    let b = engine
        .render(
            &doc,
            &req,
            |_| panic!("second proxy should share decoded source"),
            &BTreeMap::new(),
        )
        .unwrap();
    assert_eq!((a.frame.width, b.frame.width), (16, 32));
    assert!(!std::sync::Arc::ptr_eq(&a.frame, &b.frame));
    doc.source.fingerprint = "c".repeat(64);
    req.sequence = 3;
    engine
        .render(
            &doc,
            &req,
            |_| {
                calls.fetch_add(1, Ordering::SeqCst);
                Frame::new(64, 32, vec![[0.3; 4]; 2048])
            },
            &BTreeMap::new(),
        )
        .unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

#[test]
fn completed_sessions_are_retired_but_old_finish_does_not_end_new_work() {
    let engine = RenderEngine::new(64 * 1024 * 1024, 1024 * 1024).unwrap();
    for i in 0..70 {
        let mut req = request(1);
        req.session = format!("session-{i}");
        engine
            .render(
                &document(),
                &req,
                |_| Frame::new(32, 16, vec![[0.2; 4]; 512]),
                &BTreeMap::new(),
            )
            .unwrap();
    }
    assert!(engine.check_request("session-0", 1).is_err());
    engine.check_request("session-69", 1).unwrap();
    let mut old = request(1);
    old.session = "session-0".into();
    assert!(engine
        .register_request(&old, &engine.request_context())
        .is_err());
    old.sequence = 2;
    let newer = engine
        .register_request(&old, &engine.request_context())
        .unwrap();
    engine.finish_request("session-0", 1).unwrap();
    engine.cancel_request("session-0", 1).unwrap();
    newer.check_cancelled().unwrap();
    engine.check_request("session-0", 2).unwrap();
}

#[test]
fn active_session_admission_requires_a_real_completion_or_cancellation() {
    let engine = RenderEngine::new(64 * 1024 * 1024, 0).unwrap();
    for i in 0..64 {
        let mut req = request(2);
        req.session = format!("queued-{i}");
        engine
            .register_request(&req, &engine.request_context())
            .unwrap();
    }
    let mut extra = request(2);
    extra.session = "extra".into();
    engine.finish_request("queued-0", 1).unwrap();
    assert!(engine
        .register_request(&extra, &engine.request_context())
        .is_err());
    engine.finish_request("queued-0", 2).unwrap();
    engine
        .register_request(&extra, &engine.request_context())
        .unwrap();
    engine.check_request("queued-1", 2).unwrap();
    engine.check_request("extra", 2).unwrap();
}
#[test]
fn identity_frame_copies_still_require_output_budget() {
    let frame = Frame::new(1, 1, vec![[0.2, 0.3, 0.4, 1.0]]).unwrap();
    let too_small = iris_render::RenderContext::new(15);
    assert!(frame.proxy(16, &too_small).is_err());
    assert!(frame.oriented(1, &too_small).is_err());
    let exact = iris_render::RenderContext::new(16);
    assert_eq!(frame.proxy(16, &exact).unwrap(), frame);
    assert_eq!(frame.oriented(1, &exact).unwrap(), frame);
    assert_eq!(exact.memory_used(), 0);
}
