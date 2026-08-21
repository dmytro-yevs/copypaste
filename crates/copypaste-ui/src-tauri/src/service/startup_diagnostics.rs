use std::path::Path;

use super::ServiceState;

const TARGET: &str = "copypaste_ui::service::startup";

#[derive(Clone, Copy)]
pub(crate) enum StartBranch {
    RejectMissing,
    Spawn,
    UseRunning,
    ReturnUnhealthy,
}

#[derive(Clone, Copy)]
#[cfg(any(windows, test))]
pub(super) enum JobStage {
    Create,
    Query,
    Configure,
    Assign,
    Unknown,
}

pub(crate) fn app_started() {
    emit(app_event("app-start", std::process::id()));
}

pub(crate) fn app_stopping() {
    emit(app_event("app-stop", std::process::id()));
}

pub(super) fn initial_state(state: &ServiceState) {
    emit(format!(
        "initial-service-state outcome={}",
        state_name(state)
    ));
}

pub(super) fn branch(branch: StartBranch) {
    emit(format!("service-start branch={}", branch_name(branch)));
}

pub(super) fn child_started(binary: &Path, pid: u32) {
    emit(child_started_message(binary, pid, std::process::id()));
}

pub(super) fn child_spawn_failed(error: &std::io::Error) {
    emit(format!(
        "daemon-spawn-failed os_code={}",
        os_code(error.raw_os_error())
    ));
}

pub(super) fn child_exited(code: Option<i32>) {
    emit(child_exit_message(code));
}

#[cfg(windows)]
pub(super) fn job_adopted() {
    emit(job_adoption_success().to_owned());
}

#[cfg(windows)]
pub(super) fn job_adoption_failed(stage: JobStage, code: Option<i32>) {
    emit(job_adoption_failure(stage, code));
}

fn emit(message: String) {
    tracing::info!(target: TARGET, "{message}");
}

fn app_event(kind: &str, pid: u32) -> String {
    format!("{kind} pid={pid}")
}

fn child_started_message(binary: &Path, pid: u32, parent_pid: u32) -> String {
    format!(
        "daemon-start pid={pid} parent_pid={parent_pid} executable={}",
        executable_identity(binary)
    )
}

fn child_exit_message(code: Option<i32>) -> String {
    format!("daemon-exit exit_code={}", exit_code(code))
}

fn branch_name(branch: StartBranch) -> &'static str {
    match branch {
        StartBranch::RejectMissing => "reject-missing",
        StartBranch::Spawn => "spawn",
        StartBranch::UseRunning => "use-running",
        StartBranch::ReturnUnhealthy => "return-unhealthy",
    }
}

fn state_name(state: &ServiceState) -> &'static str {
    match state {
        ServiceState::Running { .. } => "running",
        ServiceState::Stopped => "stopped",
        ServiceState::NotInstalled => "not-installed",
        ServiceState::Unhealthy => "unhealthy",
    }
}

#[cfg(any(windows, test))]
pub(super) fn job_stage_name(stage: JobStage) -> &'static str {
    match stage {
        JobStage::Create => "create",
        JobStage::Query => "query",
        JobStage::Configure => "configure",
        JobStage::Assign => "assign",
        JobStage::Unknown => "unknown",
    }
}

#[cfg(any(windows, test))]
fn job_adoption_success() -> &'static str {
    "daemon-job-adoption outcome=success"
}

#[cfg(any(windows, test))]
fn job_adoption_failure(stage: JobStage, code: Option<i32>) -> String {
    format!(
        "daemon-job-adoption outcome=failed stage={} os_code={}",
        job_stage_name(stage),
        os_code(code)
    )
}

fn executable_identity(binary: &Path) -> &'static str {
    let expected = format!("copypaste-daemon{}", std::env::consts::EXE_SUFFIX);
    if binary
        .file_name()
        .is_some_and(|name| name.eq_ignore_ascii_case(std::ffi::OsStr::new(&expected)))
    {
        "copypaste-daemon"
    } else {
        "override"
    }
}

fn exit_code(code: Option<i32>) -> String {
    code.map_or_else(|| "unavailable".to_owned(), |value| value.to_string())
}

fn os_code(code: Option<i32>) -> String {
    code.map_or_else(|| "unavailable".to_owned(), |value| value.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_initial_state_has_a_distinct_outcome() {
        let running = ServiceState::Running {
            version: "1".into(),
            matches_app: true,
            ours: false,
        };
        let outcomes = [
            state_name(&running),
            state_name(&ServiceState::Stopped),
            state_name(&ServiceState::NotInstalled),
            state_name(&ServiceState::Unhealthy),
        ];
        assert_eq!(
            outcomes,
            ["running", "stopped", "not-installed", "unhealthy"]
        );
    }

    #[test]
    fn every_start_branch_is_distinct() {
        assert_eq!(branch_name(StartBranch::RejectMissing), "reject-missing");
        assert_eq!(branch_name(StartBranch::Spawn), "spawn");
        assert_eq!(branch_name(StartBranch::UseRunning), "use-running");
        assert_eq!(
            branch_name(StartBranch::ReturnUnhealthy),
            "return-unhealthy"
        );
    }

    #[test]
    fn executable_identity_never_exposes_an_override_path() {
        assert_eq!(app_event("app-start", 40), "app-start pid=40");
        assert_eq!(app_event("app-stop", 40), "app-stop pid=40");
        assert_eq!(
            child_started_message(Path::new("/Users/private/secret-daemon"), 42, 40),
            "daemon-start pid=42 parent_pid=40 executable=override"
        );
        assert_eq!(
            executable_identity(Path::new("/Users/private/secret-daemon")),
            "override"
        );
        assert!(!executable_identity(Path::new("/Users/private/secret-daemon")).contains('/'));
    }

    #[test]
    fn job_failures_report_only_stage_and_numeric_code() {
        assert_eq!(
            job_adoption_success(),
            "daemon-job-adoption outcome=success"
        );
        assert_eq!(job_stage_name(JobStage::Create), "create");
        assert_eq!(job_stage_name(JobStage::Query), "query");
        assert_eq!(job_stage_name(JobStage::Configure), "configure");
        assert_eq!(job_stage_name(JobStage::Assign), "assign");
        assert_eq!(job_stage_name(JobStage::Unknown), "unknown");
        assert_eq!(os_code(Some(5)), "5");
        assert_eq!(os_code(None), "unavailable");
        assert_eq!(
            job_adoption_failure(JobStage::Assign, Some(5)),
            "daemon-job-adoption outcome=failed stage=assign os_code=5"
        );
    }

    #[test]
    fn child_exit_distinguishes_numeric_and_unavailable_codes() {
        assert_eq!(exit_code(Some(23)), "23");
        assert_eq!(exit_code(None), "unavailable");
        assert_eq!(child_exit_message(Some(23)), "daemon-exit exit_code=23");
        assert_eq!(
            child_exit_message(None),
            "daemon-exit exit_code=unavailable"
        );
    }
}
