//! Integration tests for push protocol.

use bstr::BString;
use git_hash::ObjectId;
use git_protocol::push::{
    compute_push_objects, PushOptions, PushRefResult, PushResult, PushUpdate,
};
use git_protocol::remote::RefSpec;

#[test]
fn refspec_standard_fetch() {
    let spec = RefSpec::parse("+refs/heads/*:refs/remotes/origin/*").unwrap();
    assert!(spec.force);
    assert_eq!(
        spec.map_to_destination("refs/heads/main"),
        Some("refs/remotes/origin/main".into())
    );
    assert_eq!(
        spec.map_to_destination("refs/heads/feature/long-name"),
        Some("refs/remotes/origin/feature/long-name".into())
    );
    assert_eq!(spec.map_to_destination("refs/tags/v1.0"), None);
}

#[test]
fn refspec_tag_fetch() {
    let spec = RefSpec::parse("+refs/tags/*:refs/tags/*").unwrap();
    assert_eq!(
        spec.map_to_destination("refs/tags/v1.0"),
        Some("refs/tags/v1.0".into())
    );
    assert_eq!(spec.map_to_destination("refs/heads/main"), None);
}

#[test]
fn compute_push_objects_basic() {
    let local_a = ObjectId::from_hex("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").unwrap();
    let local_b = ObjectId::from_hex("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb").unwrap();
    let local_c = ObjectId::from_hex("cccccccccccccccccccccccccccccccccccccccc").unwrap();
    let remote_b = ObjectId::from_hex("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb").unwrap();

    let result = compute_push_objects(&[local_a, local_b, local_c], &[remote_b]);
    assert_eq!(result.len(), 2);
    assert!(result.contains(&local_a));
    assert!(result.contains(&local_c));
    assert!(!result.contains(&local_b));
}

#[test]
fn compute_push_objects_nothing_new() {
    let a = ObjectId::from_hex("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").unwrap();
    let result = compute_push_objects(&[a], &[a]);
    assert!(result.is_empty());
}

#[test]
fn push_update_create_ref() {
    let new_oid = ObjectId::from_hex("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").unwrap();
    let update = PushUpdate {
        local_oid: Some(new_oid),
        remote_ref: "refs/heads/new-branch".into(),
        force: false,
        expected_remote_oid: None,
    };
    assert!(update.local_oid.is_some());
    assert_eq!(update.remote_ref, "refs/heads/new-branch");
}

#[test]
fn push_update_delete_ref() {
    let update = PushUpdate {
        local_oid: None, // None means delete
        remote_ref: "refs/heads/old-branch".into(),
        force: false,
        expected_remote_oid: None,
    };
    assert!(update.local_oid.is_none());
}

#[test]
fn push_update_force_with_lease() {
    let expected_oid = ObjectId::from_hex("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").unwrap();
    let new_oid = ObjectId::from_hex("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb").unwrap();
    let actual_oid = ObjectId::from_hex("cccccccccccccccccccccccccccccccccccccccc").unwrap();

    let update = PushUpdate {
        local_oid: Some(new_oid),
        remote_ref: "refs/heads/main".into(),
        force: false,
        expected_remote_oid: Some(expected_oid),
    };

    // Simulate the check: advertised ref has `actual_oid`, but we expected `expected_oid`
    let _advertised = vec![(actual_oid, BString::from("refs/heads/main"))];

    // The push function would detect the mismatch and reject
    // Here we just verify the types work together
    assert_ne!(expected_oid, actual_oid);
    assert_eq!(update.expected_remote_oid, Some(expected_oid));
}

#[test]
fn push_result_all_ok() {
    let result = PushResult {
        ok: true,
        ref_results: vec![
            ("refs/heads/main".into(), PushRefResult::Ok),
            ("refs/heads/feature".into(), PushRefResult::Ok),
        ],
        server_message: None,
    };
    assert!(result.ok);
    assert_eq!(result.ref_results.len(), 2);
    assert!(result.ref_results.iter().all(|(_, r)| *r == PushRefResult::Ok));
}

#[test]
fn push_result_partial_failure() {
    let result = PushResult {
        ok: false,
        ref_results: vec![
            ("refs/heads/main".into(), PushRefResult::Ok),
            (
                "refs/heads/protected".into(),
                PushRefResult::Rejected("non-fast-forward".into()),
            ),
        ],
        server_message: None,
    };
    assert!(!result.ok);

    // Check specific ref results
    assert_eq!(result.ref_results[0].1, PushRefResult::Ok);
    match &result.ref_results[1].1 {
        PushRefResult::Rejected(reason) => {
            assert!(reason.contains("non-fast-forward"));
        }
        _ => panic!("expected rejection"),
    }
}

#[test]
fn push_options_default() {
    let opts = PushOptions::default();
    assert!(opts.progress);
    assert!(!opts.atomic);
    assert!(opts.push_options.is_empty());
    assert!(opts.thin);
}

#[test]
fn push_options_atomic() {
    let opts = PushOptions {
        atomic: true,
        ..PushOptions::default()
    };
    assert!(opts.atomic);
}

#[test]
fn push_options_with_push_option_strings() {
    let opts = PushOptions {
        push_options: vec!["ci.skip".into(), "merge_request.create".into()],
        ..PushOptions::default()
    };
    assert_eq!(opts.push_options.len(), 2);
    assert_eq!(opts.push_options[0], "ci.skip");
}
