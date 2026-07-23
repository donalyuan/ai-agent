use novex_api::domain::publication::{
    PublicationPackage, PublicationPlanStatus, PublicationTarget, PublicationTargetStatus,
};
use uuid::Uuid;

fn target(status: PublicationTargetStatus) -> PublicationTarget {
    PublicationTarget::new(Uuid::new_v4(), "douyin", status).unwrap()
}

#[test]
fn target_requires_a_new_package_after_attention_before_rehandoff() {
    let mut target = target(PublicationTargetStatus::Ready);
    target.handoff().unwrap();
    target.needs_attention().unwrap();
    assert_eq!(target.status, PublicationTargetStatus::NeedsAttention);
    target.revise().unwrap();
    assert_eq!(target.draft_revision, 2);
    assert_eq!(target.status, PublicationTargetStatus::Draft);
    assert!(target.handoff().is_err());
    target.mark_ready().unwrap();
    target.handoff().unwrap();
}

#[test]
fn publication_plan_status_is_derived_from_independent_target_states() {
    assert_eq!(
        PublicationPlanStatus::derive(&[
            PublicationTargetStatus::Published,
            PublicationTargetStatus::HandedOff,
        ]),
        PublicationPlanStatus::PartiallyPublished
    );
    assert_eq!(
        PublicationPlanStatus::derive(&[PublicationTargetStatus::Cancelled]),
        PublicationPlanStatus::Cancelled
    );
}

#[test]
fn published_target_can_only_correct_its_result_not_transition_again() {
    let mut target = target(PublicationTargetStatus::HandedOff);
    target.publish().unwrap();
    assert!(target.can_correct_result());
    assert!(target.cancel().is_err());
    assert!(target.needs_attention().is_err());
}

#[test]
fn revision_change_invalidates_only_the_changed_targets_package() {
    let mut douyin = target(PublicationTargetStatus::Draft);
    let xiaohongshu = PublicationTarget::new(
        Uuid::new_v4(),
        "xiaohongshu",
        PublicationTargetStatus::Draft,
    )
    .unwrap();
    let douyin_package = PublicationPackage {
        id: Uuid::new_v4(),
        publication_target_id: douyin.id,
        draft_revision: 1,
        manifest_sha256: "a".repeat(64),
    };
    let xiaohongshu_package = PublicationPackage {
        id: Uuid::new_v4(),
        publication_target_id: xiaohongshu.id,
        draft_revision: 1,
        manifest_sha256: "b".repeat(64),
    };
    douyin.revise().unwrap();
    assert!(!douyin.package_is_current(&douyin_package));
    assert!(xiaohongshu.package_is_current(&xiaohongshu_package));
}

#[test]
fn only_supported_platforms_and_legal_transitions_are_accepted() {
    assert!(
        PublicationTarget::new(Uuid::new_v4(), "tiktok", PublicationTargetStatus::Draft).is_err()
    );

    let mut draft = target(PublicationTargetStatus::Draft);
    assert!(draft.handoff().is_err());
    assert!(draft.publish().is_err());
    draft.mark_ready().unwrap();
    assert!(draft.publish().is_err());
    draft.handoff().unwrap();
    draft.publish().unwrap();

    for status in [
        PublicationTargetStatus::Draft,
        PublicationTargetStatus::Ready,
        PublicationTargetStatus::HandedOff,
        PublicationTargetStatus::NeedsAttention,
    ] {
        let mut cancellable = target(status);
        cancellable.cancel().unwrap();
        assert_eq!(cancellable.status, PublicationTargetStatus::Cancelled);
    }
}
