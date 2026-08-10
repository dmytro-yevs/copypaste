use copypaste_ui_lib::{UpdateProgress, UpdateStatus};

#[test]
fn updater_dtos_are_a_cross_platform_public_surface() {
    let statuses = [
        UpdateStatus::Unsupported,
        UpdateStatus::Unconfigured,
        UpdateStatus::Ready,
        UpdateStatus::UpToDate,
        UpdateStatus::Available {
            version: "2.0.1".into(),
        },
    ];
    let progress = [
        UpdateProgress::Downloading {
            downloaded: 7,
            total: Some(11),
        },
        UpdateProgress::Verifying,
        UpdateProgress::Installing,
    ];

    assert_eq!(
        serde_json::to_value(&statuses[0]).unwrap()["state"],
        "unsupported"
    );
    assert_eq!(
        serde_json::to_value(&statuses[4]).unwrap()["state"],
        "available"
    );
    assert_eq!(
        serde_json::to_value(&progress[0]).unwrap()["state"],
        "downloading"
    );
    assert_eq!(
        serde_json::to_value(&progress[2]).unwrap()["state"],
        "installing"
    );
}
