//! Offline-first I2G paired CONTROL -> TREATMENT qualification model.
//!
//! This module deliberately contains the qualification contract and a deterministic synthetic
//! backend, not a production provider.  The state machine is written so that a future Windows
//! backend can be plugged in without changing the scientific gates.  The current PowerShell real
//! entrypoint is source-controlled fail-closed, so none of the host-mutating operations below are
//! reachable from this task's real path.

use crate::{classify_counter_discovery, COUNTER_DISCOVERY_MAX_OUTPUT_BYTES};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

pub const I2G_VARIABLE: &str = "SeProfileSingleProcessPrivilege";
pub const I2G_SELECTION_CONFIDENCE: &str = "MEDIUM";
pub const I2G_EXPERIMENT_SHAPE: &str = "PAIRED_CONTROL_TREATMENT";
pub const I2G_SERVICE_NAME: &str = "ResourceTimelineAmdProfileSingleProcessQualification";
pub const I2G_SERVICE_ACCOUNT: &str = "NT AUTHORITY\\LocalService";
pub const I2G_SERVICE_ACCOUNT_SID: &str = "S-1-5-19";
pub const I2G_SERVICE_SID_TYPE: &str = "UNRESTRICTED";
pub const I2G_FIXED_AMD_CLI_PATH: &str = r"D:\apps\AMDuProf\bin\AMDuProfCLI.exe";
pub const I2G_FIXED_AMD_CLI_SHA256: &str =
    "D0812D64963DD98F7C339CAC72F650461F95FF84E757A99767C7981B4111FBAC";
pub const I2G_FIXED_AMD_CLI_VERSION: &str = "5.3.521.0";
pub const I2G_FIXED_AMD_CLI_ARCHITECTURE: &str = "x64";
pub const I2G_FIXED_AMD_ARGUMENTS: [&str; 2] = ["timechart", "--list"];
pub const I2G_OPERATION: &str = "COUNTER_DISCOVERY";
pub const I2G_PROTOCOL: &str = "amd-privilege-qualification/i2g/1";
pub const I2G_HARNESS_SHA256_SYNTHETIC: &str = "SYNTHETIC_FROZEN_I2G_ARTIFACT";
pub const I2G_COUNTER_DISCOVERY_TIMEOUT_MS: u64 = 30_000;
pub const I2G_CHILD_SAFETY_CAP_MS: u64 = 90_000;
pub const I2G_MAX_CONTROL_RUNS: u32 = 1;
pub const I2G_MAX_TREATMENT_RUNS: u32 = 1;
pub const I2G_MAX_TOTAL_RUNS: u32 = 2;
pub const I2G_POWER_SAMPLING_RUNS: u32 = 0;
pub const I2G_SYNTHETIC_SERVICE_SID: &str = "S-1-5-80-512-724-936-1148-1360";

static ATOMIC_FILE_COUNTER: AtomicU64 = AtomicU64::new(1);

const CONTROL_REQUIRED_RIGHT: &str = "SeSystemProfilePrivilege";
const TREATMENT_RIGHT: &str = "SeProfileSingleProcessPrivilege";
const SYNTHETIC_ENABLED_PRIVILEGES: [&str; 3] = [
    "SeChangeNotifyPrivilege",
    "SeCreateGlobalPrivilege",
    "SeImpersonatePrivilege",
];
const SYNTHETIC_DISABLED_PRIVILEGES: [&str; 8] = [
    "SeAssignPrimaryTokenPrivilege",
    "SeAuditPrivilege",
    "SeIncreaseQuotaPrivilege",
    "SeIncreaseWorkingSetPrivilege",
    "SeShutdownPrivilege",
    "SeSystemtimePrivilege",
    "SeTimeZonePrivilege",
    "SeUndockPrivilege",
];
const SYNTHETIC_ABSENT_PRIVILEGES: [&str; 15] = [
    "SeBackupPrivilege",
    "SeCreatePagefilePrivilege",
    "SeCreatePermanentPrivilege",
    "SeCreateSymbolicLinkPrivilege",
    "SeDebugPrivilege",
    "SeDelegateSessionUserImpersonatePrivilege",
    "SeIncreaseBasePriorityPrivilege",
    "SeLoadDriverPrivilege",
    "SeLockMemoryPrivilege",
    "SeManageVolumePrivilege",
    "SeRestorePrivilege",
    "SeSecurityPrivilege",
    "SeSystemEnvironmentPrivilege",
    "SeTakeOwnershipPrivilege",
    "SeTcbPrivilege",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum I2gState {
    Prepared,
    ControlPolicyReady,
    ControlServiceRunning,
    ControlTokenReady,
    ControlDiscoveryRunning,
    ControlComplete,
    ControlTeardownComplete,
    TreatmentPolicyReady,
    TreatmentServiceRunning,
    TreatmentTokenReady,
    TreatmentDiscoveryRunning,
    TreatmentComplete,
    RollbackRunning,
    RollbackComplete,
    Invalid,
    Failed,
}

impl I2gState {
    fn rank(self) -> u8 {
        match self {
            Self::Prepared => 0,
            Self::ControlPolicyReady => 1,
            Self::ControlServiceRunning => 2,
            Self::ControlTokenReady => 3,
            Self::ControlDiscoveryRunning => 4,
            Self::ControlComplete => 5,
            Self::ControlTeardownComplete => 6,
            Self::TreatmentPolicyReady => 7,
            Self::TreatmentServiceRunning => 8,
            Self::TreatmentTokenReady => 9,
            Self::TreatmentDiscoveryRunning => 10,
            Self::TreatmentComplete => 11,
            Self::RollbackRunning => 12,
            Self::RollbackComplete => 13,
            Self::Invalid => 250,
            Self::Failed => 251,
        }
    }

    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::RollbackComplete | Self::Invalid | Self::Failed)
    }

    pub fn can_transition_to(self, next: Self) -> bool {
        // Recovery may reopen a failed or apparently-complete rollback when fresh machine
        // observation proves cleanup is still required. INVALID remains unrecoverable.
        if next == Self::RollbackRunning && self != Self::Invalid {
            return true;
        }
        if self.is_terminal() {
            return false;
        }
        if matches!(next, Self::Invalid | Self::Failed) {
            return true;
        }
        next.rank() == self.rank() + 1
    }
}

/// A crash-safe ownership marker for one run-owned host mutation.
///
/// `mutation_intent_durable` is written before the mutation.  Recovery then combines that
/// intent with a fresh machine readback.  A right/service that was already present is never
/// inferred to be run-owned, even when the run later attempted the same mutation.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct DurableOwnership {
    pub preexisting: bool,
    pub mutation_intent_durable: bool,
    pub observed_present: bool,
    pub owned_by_run: bool,
    pub removal_intent_durable: bool,
    pub removal_observed_absent: bool,
}

impl DurableOwnership {
    fn capture_pre_state(&mut self, present: bool) {
        self.preexisting = present;
        self.observed_present = present;
        self.owned_by_run = false;
    }

    fn begin_mutation(&mut self) {
        self.mutation_intent_durable = true;
        self.removal_intent_durable = false;
        self.removal_observed_absent = false;
    }

    fn observe_after_mutation(&mut self, present: bool) {
        self.observed_present = present;
        self.owned_by_run = self.mutation_intent_durable && !self.preexisting && present;
    }

    fn reconcile_machine_observation(&mut self, present: bool) {
        self.observed_present = present;
        if self.mutation_intent_durable && !self.preexisting && present {
            self.owned_by_run = true;
        }
        if !present && self.removal_observed_absent {
            self.owned_by_run = false;
        }
    }

    fn begin_removal(&mut self) {
        self.removal_intent_durable = true;
    }

    fn observe_after_removal(&mut self, absent: bool) {
        self.removal_observed_absent = absent;
        self.observed_present = !absent;
        if absent {
            self.owned_by_run = false;
        }
    }
}

/// Durable child-spawn journal.  The spawn intent is written before launch; a later machine
/// readback or completion evidence can therefore consume the run budget even if the count write
/// itself was interrupted.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct DurableDiscoveryLedger {
    pub spawn_intent_durable: bool,
    pub spawn_observed_durable: bool,
    pub completion_observed_durable: bool,
    pub child_pid: Option<u32>,
    pub process_start_time: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PersistedI2gState {
    pub schema: String,
    pub state: I2gState,
    pub sequence: u64,
    pub actual_control_counter_discovery_runs: u32,
    pub actual_treatment_counter_discovery_runs: u32,
    pub actual_total_counter_discovery_runs: u32,
    pub system_profile_right_added_by_run: bool,
    pub profile_single_right_added_by_run: bool,
    pub service_created: bool,
    pub service_name: String,
    pub service_sid: String,
    pub harness_sha256: String,
    #[serde(default)]
    pub service_ownership: DurableOwnership,
    #[serde(default)]
    pub system_profile_ownership: DurableOwnership,
    #[serde(default)]
    pub profile_single_ownership: DurableOwnership,
    #[serde(default)]
    pub control_discovery_ledger: DurableDiscoveryLedger,
    #[serde(default)]
    pub treatment_discovery_ledger: DurableDiscoveryLedger,
    #[serde(default)]
    pub rollback_cleanup_intent_durable: bool,
    #[serde(default)]
    pub rollback_step: String,
}

impl PersistedI2gState {
    pub fn new(service_name: &str, service_sid: &str, harness_sha256: &str) -> Self {
        Self {
            schema: "amd-i2g-persisted-state/v2".to_owned(),
            state: I2gState::Prepared,
            sequence: 0,
            actual_control_counter_discovery_runs: 0,
            actual_treatment_counter_discovery_runs: 0,
            actual_total_counter_discovery_runs: 0,
            system_profile_right_added_by_run: false,
            profile_single_right_added_by_run: false,
            service_created: false,
            service_name: service_name.to_owned(),
            service_sid: service_sid.to_owned(),
            harness_sha256: harness_sha256.to_owned(),
            service_ownership: DurableOwnership::default(),
            system_profile_ownership: DurableOwnership::default(),
            profile_single_ownership: DurableOwnership::default(),
            control_discovery_ledger: DurableDiscoveryLedger::default(),
            treatment_discovery_ledger: DurableDiscoveryLedger::default(),
            rollback_cleanup_intent_durable: false,
            rollback_step: "NOT_STARTED".to_owned(),
        }
    }

    pub fn transition(&mut self, next: I2gState) -> Result<(), String> {
        if !self.state.can_transition_to(next) {
            return Err(format!(
                "non-monotonic I2G state transition: {:?} -> {:?}",
                self.state, next
            ));
        }
        self.sequence = self.sequence.saturating_add(1);
        self.state = next;
        Ok(())
    }

    pub fn checkpoint(&mut self) {
        self.sequence = self.sequence.saturating_add(1);
    }

    pub fn record_child_spawn(&mut self, phase: I2gPhase) -> Result<(), String> {
        let slot = match phase {
            I2gPhase::Control => &mut self.actual_control_counter_discovery_runs,
            I2gPhase::Treatment => &mut self.actual_treatment_counter_discovery_runs,
        };
        let maximum = match phase {
            I2gPhase::Control => I2G_MAX_CONTROL_RUNS,
            I2gPhase::Treatment => I2G_MAX_TREATMENT_RUNS,
        };
        if *slot >= maximum {
            return Err(format!("{} discovery retry is forbidden", phase.as_str()));
        }
        if self.actual_total_counter_discovery_runs >= I2G_MAX_TOTAL_RUNS {
            return Err("I2G total discovery run cap exceeded".to_owned());
        }
        *slot += 1;
        self.actual_total_counter_discovery_runs += 1;
        Ok(())
    }

    fn discovery_ledger_mut(&mut self, phase: I2gPhase) -> &mut DurableDiscoveryLedger {
        match phase {
            I2gPhase::Control => &mut self.control_discovery_ledger,
            I2gPhase::Treatment => &mut self.treatment_discovery_ledger,
        }
    }

    fn discovery_ledger(&self, phase: I2gPhase) -> &DurableDiscoveryLedger {
        match phase {
            I2gPhase::Control => &self.control_discovery_ledger,
            I2gPhase::Treatment => &self.treatment_discovery_ledger,
        }
    }

    fn begin_discovery_spawn(&mut self, phase: I2gPhase) {
        self.discovery_ledger_mut(phase).spawn_intent_durable = true;
    }

    fn observe_discovery_spawn(
        &mut self,
        phase: I2gPhase,
        child_pid: Option<u32>,
        process_start_time: Option<u64>,
    ) {
        let ledger = self.discovery_ledger_mut(phase);
        ledger.spawn_observed_durable = true;
        ledger.child_pid = child_pid;
        ledger.process_start_time = process_start_time;
    }

    fn observe_discovery_completion(&mut self, phase: I2gPhase) {
        self.discovery_ledger_mut(phase).completion_observed_durable = true;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum I2gPhase {
    Control,
    Treatment,
}

impl I2gPhase {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Control => "CONTROL",
            Self::Treatment => "TREATMENT",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum I2gTokenMoment {
    Pre,
    AfterSystemProfileEnable,
    Final,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PrivilegeState {
    Absent,
    PresentDisabled,
    PresentEnabled,
}

impl PrivilegeState {
    pub const fn as_contract_str(self) -> &'static str {
        match self {
            Self::Absent => "ABSENT",
            Self::PresentDisabled => "PRESENT + DISABLED",
            Self::PresentEnabled => "PRESENT + ENABLED",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TokenSnapshot {
    pub token_user: String,
    pub account_sid: String,
    pub service_sid: String,
    pub service_sid_type: String,
    pub session_id: u32,
    pub architecture: String,
    pub integrity: String,
    pub elevation: Option<String>,
    pub relevant_groups: Vec<String>,
    pub administrators_present: bool,
    pub enabled_privileges: Vec<String>,
    pub disabled_privileges: Vec<String>,
    pub absent_privileges: Vec<String>,
    pub controlled_right_states: BTreeMap<String, PrivilegeState>,
    pub process_pid: u32,
    pub process_start_time: u64,
}

impl TokenSnapshot {
    pub fn controlled_state(&self, privilege: &str) -> PrivilegeState {
        self.controlled_right_states
            .get(privilege)
            .copied()
            .unwrap_or(PrivilegeState::Absent)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TokenGateResult {
    pub code: String,
    pub pass: bool,
    pub mismatches: Vec<String>,
}

impl TokenGateResult {
    fn pass(code: &str) -> Self {
        Self {
            code: code.to_owned(),
            pass: true,
            mismatches: Vec::new(),
        }
    }

    fn fail(code: &str, mismatches: Vec<String>) -> Self {
        Self {
            code: code.to_owned(),
            pass: false,
            mismatches,
        }
    }
}

pub fn validate_control_materialized_token(token: &TokenSnapshot) -> TokenGateResult {
    let mut mismatches = Vec::new();
    if token.controlled_state(CONTROL_REQUIRED_RIGHT) != PrivilegeState::PresentDisabled {
        mismatches.push(format!(
            "{CONTROL_REQUIRED_RIGHT} expected PRESENT + DISABLED, got {}",
            token
                .controlled_state(CONTROL_REQUIRED_RIGHT)
                .as_contract_str()
        ));
    }
    if token.controlled_state(TREATMENT_RIGHT) != PrivilegeState::Absent {
        mismatches.push(format!(
            "{TREATMENT_RIGHT} expected ABSENT, got {}",
            token.controlled_state(TREATMENT_RIGHT).as_contract_str()
        ));
    }
    validate_common_token_invariants(token, &mut mismatches);
    if mismatches.is_empty() {
        TokenGateResult::pass("CONTROL_MATERIALIZED_TOKEN_GATE_PASS")
    } else {
        TokenGateResult::fail("TOKEN_GATE_FAILED", mismatches)
    }
}

pub fn validate_control_final_token(token: &TokenSnapshot) -> TokenGateResult {
    let mut mismatches = Vec::new();
    if token.controlled_state(CONTROL_REQUIRED_RIGHT) != PrivilegeState::PresentEnabled {
        mismatches.push(format!(
            "{CONTROL_REQUIRED_RIGHT} expected PRESENT + ENABLED, got {}",
            token
                .controlled_state(CONTROL_REQUIRED_RIGHT)
                .as_contract_str()
        ));
    }
    if token.controlled_state(TREATMENT_RIGHT) != PrivilegeState::Absent {
        mismatches.push(format!(
            "{TREATMENT_RIGHT} expected ABSENT, got {}",
            token.controlled_state(TREATMENT_RIGHT).as_contract_str()
        ));
    }
    validate_common_token_invariants(token, &mut mismatches);
    if mismatches.is_empty() {
        TokenGateResult::pass("CONTROL_TOKEN_GATE_PASS")
    } else {
        TokenGateResult::fail("CONTROL_TOKEN_GATE_FAILED", mismatches)
    }
}

pub fn validate_treatment_materialized_token(token: &TokenSnapshot) -> TokenGateResult {
    let mut mismatches = Vec::new();
    if token.controlled_state(CONTROL_REQUIRED_RIGHT) != PrivilegeState::PresentDisabled {
        mismatches.push(format!(
            "{CONTROL_REQUIRED_RIGHT} expected PRESENT + DISABLED, got {}",
            token
                .controlled_state(CONTROL_REQUIRED_RIGHT)
                .as_contract_str()
        ));
    }
    if token.controlled_state(TREATMENT_RIGHT) != PrivilegeState::PresentDisabled {
        mismatches.push(format!(
            "{TREATMENT_RIGHT} expected PRESENT + DISABLED, got {}",
            token.controlled_state(TREATMENT_RIGHT).as_contract_str()
        ));
    }
    validate_common_token_invariants(token, &mut mismatches);
    if mismatches.is_empty() {
        TokenGateResult::pass("TREATMENT_MATERIALIZED_TOKEN_GATE_PASS")
    } else {
        TokenGateResult::fail("TOKEN_GATE_FAILED", mismatches)
    }
}

pub fn validate_treatment_final_token(token: &TokenSnapshot) -> TokenGateResult {
    let mut mismatches = Vec::new();
    if token.controlled_state(CONTROL_REQUIRED_RIGHT) != PrivilegeState::PresentEnabled {
        mismatches.push(format!(
            "{CONTROL_REQUIRED_RIGHT} expected PRESENT + ENABLED, got {}",
            token
                .controlled_state(CONTROL_REQUIRED_RIGHT)
                .as_contract_str()
        ));
    }
    if token.controlled_state(TREATMENT_RIGHT) != PrivilegeState::PresentEnabled {
        mismatches.push(format!(
            "{TREATMENT_RIGHT} expected PRESENT + ENABLED, got {}",
            token.controlled_state(TREATMENT_RIGHT).as_contract_str()
        ));
    }
    validate_common_token_invariants(token, &mut mismatches);
    if mismatches.is_empty() {
        TokenGateResult::pass("TREATMENT_TOKEN_GATE_PASS")
    } else {
        TokenGateResult::fail("TOKEN_GATE_FAILED", mismatches)
    }
}

pub fn validate_treatment_systemprofile_token(token: &TokenSnapshot) -> TokenGateResult {
    let mut mismatches = Vec::new();
    if token.controlled_state(CONTROL_REQUIRED_RIGHT) != PrivilegeState::PresentEnabled {
        mismatches.push(format!(
            "{CONTROL_REQUIRED_RIGHT} expected PRESENT + ENABLED, got {}",
            token
                .controlled_state(CONTROL_REQUIRED_RIGHT)
                .as_contract_str()
        ));
    }
    if token.controlled_state(TREATMENT_RIGHT) != PrivilegeState::PresentDisabled {
        mismatches.push(format!(
            "{TREATMENT_RIGHT} expected PRESENT + DISABLED, got {}",
            token.controlled_state(TREATMENT_RIGHT).as_contract_str()
        ));
    }
    validate_common_token_invariants(token, &mut mismatches);
    if mismatches.is_empty() {
        TokenGateResult::pass("TREATMENT_SYSTEMPROFILE_ENABLEMENT_GATE_PASS")
    } else {
        TokenGateResult::fail("TOKEN_GATE_FAILED", mismatches)
    }
}

fn validate_common_token_invariants(token: &TokenSnapshot, mismatches: &mut Vec<String>) {
    if token.account_sid != I2G_SERVICE_ACCOUNT_SID {
        mismatches.push(format!(
            "account SID expected {I2G_SERVICE_ACCOUNT_SID}, got {}",
            token.account_sid
        ));
    }
    if token.session_id != 0 {
        mismatches.push(format!("session expected 0, got {}", token.session_id));
    }
    if token.architecture != I2G_FIXED_AMD_CLI_ARCHITECTURE {
        mismatches.push(format!(
            "architecture expected {}, got {}",
            I2G_FIXED_AMD_CLI_ARCHITECTURE, token.architecture
        ));
    }
    if token.service_sid_type != I2G_SERVICE_SID_TYPE {
        mismatches.push(format!(
            "Service SID type expected {I2G_SERVICE_SID_TYPE}, got {}",
            token.service_sid_type
        ));
    }
    if token.administrators_present {
        mismatches.push("Administrators SID is present".to_owned());
    }
    for privilege in SYNTHETIC_ABSENT_PRIVILEGES {
        if token
            .enabled_privileges
            .iter()
            .any(|value| value == privilege)
            || token
                .disabled_privileges
                .iter()
                .any(|value| value == privilege)
        {
            mismatches.push(format!("{privilege} must remain absent"));
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TokenComparison {
    pub status: String,
    pub pass: bool,
    pub allowed_delta: String,
    pub changed_fields: Vec<String>,
    pub invalid_fields: Vec<String>,
}

pub fn compare_final_tokens(control: &TokenSnapshot, treatment: &TokenSnapshot) -> TokenComparison {
    let mut invalid_fields = Vec::new();
    let identity_fields = [
        ("token_user", &control.token_user, &treatment.token_user),
        ("account_sid", &control.account_sid, &treatment.account_sid),
        ("service_sid", &control.service_sid, &treatment.service_sid),
        (
            "service_sid_type",
            &control.service_sid_type,
            &treatment.service_sid_type,
        ),
        (
            "architecture",
            &control.architecture,
            &treatment.architecture,
        ),
        ("integrity", &control.integrity, &treatment.integrity),
    ];
    for (name, left, right) in identity_fields {
        if left != right {
            invalid_fields.push(name.to_owned());
        }
    }
    if control.session_id != treatment.session_id {
        invalid_fields.push("session_id".to_owned());
    }
    if control.elevation != treatment.elevation {
        invalid_fields.push("elevation".to_owned());
    }
    if control.relevant_groups != treatment.relevant_groups {
        invalid_fields.push("relevant_groups".to_owned());
    }
    if control.administrators_present != treatment.administrators_present
        || treatment.administrators_present
    {
        invalid_fields.push("administrators_present".to_owned());
    }

    let control_enabled = normalized_set(&control.enabled_privileges);
    let treatment_enabled = normalized_set(&treatment.enabled_privileges);
    let control_disabled = normalized_set(&control.disabled_privileges);
    let treatment_disabled = normalized_set(&treatment.disabled_privileges);
    let mut control_enabled_without_treatment = control_enabled.clone();
    control_enabled_without_treatment.remove(&TREATMENT_RIGHT.to_ascii_lowercase());
    let mut treatment_enabled_without_treatment = treatment_enabled.clone();
    treatment_enabled_without_treatment.remove(&TREATMENT_RIGHT.to_ascii_lowercase());
    if control_enabled_without_treatment != treatment_enabled_without_treatment {
        invalid_fields.push("enabled_privileges_except_treatment".to_owned());
    }
    let mut control_disabled_without_treatment = control_disabled.clone();
    control_disabled_without_treatment.remove(&TREATMENT_RIGHT.to_ascii_lowercase());
    let mut treatment_disabled_without_treatment = treatment_disabled.clone();
    treatment_disabled_without_treatment.remove(&TREATMENT_RIGHT.to_ascii_lowercase());
    if control_disabled_without_treatment != treatment_disabled_without_treatment {
        invalid_fields.push("disabled_privileges_except_treatment".to_owned());
    }
    let mut control_absent_without_treatment = normalized_set(&control.absent_privileges);
    control_absent_without_treatment.remove(&TREATMENT_RIGHT.to_ascii_lowercase());
    let mut treatment_absent_without_treatment = normalized_set(&treatment.absent_privileges);
    treatment_absent_without_treatment.remove(&TREATMENT_RIGHT.to_ascii_lowercase());
    if control_absent_without_treatment != treatment_absent_without_treatment {
        invalid_fields.push("absent_privileges".to_owned());
    }
    if control.controlled_state(CONTROL_REQUIRED_RIGHT) != PrivilegeState::PresentEnabled
        || treatment.controlled_state(CONTROL_REQUIRED_RIGHT) != PrivilegeState::PresentEnabled
    {
        invalid_fields.push("SeSystemProfilePrivilege".to_owned());
    }
    if control.controlled_state(TREATMENT_RIGHT) != PrivilegeState::Absent
        || treatment.controlled_state(TREATMENT_RIGHT) != PrivilegeState::PresentEnabled
    {
        invalid_fields.push("SeProfileSingleProcessPrivilege".to_owned());
    }

    let changed_fields = if invalid_fields.is_empty() {
        vec![TREATMENT_RIGHT.to_owned()]
    } else {
        invalid_fields.clone()
    };
    TokenComparison {
        status: if invalid_fields.is_empty() {
            "PASS_EXACT_ONE_PRIVILEGE_DELTA".to_owned()
        } else {
            "INVALID_TOKEN_DELTA".to_owned()
        },
        pass: invalid_fields.is_empty(),
        allowed_delta: format!("{TREATMENT_RIGHT}: ABSENT -> PRESENT + ENABLED"),
        changed_fields,
        invalid_fields,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AmdCliIdentity {
    pub path: String,
    pub sha256: String,
    pub version: String,
    pub architecture: String,
    pub signature: String,
    pub identity_gate_pass: bool,
}

impl AmdCliIdentity {
    pub fn fixed() -> Self {
        Self {
            path: I2G_FIXED_AMD_CLI_PATH.to_owned(),
            sha256: I2G_FIXED_AMD_CLI_SHA256.to_owned(),
            version: I2G_FIXED_AMD_CLI_VERSION.to_owned(),
            architecture: I2G_FIXED_AMD_CLI_ARCHITECTURE.to_owned(),
            signature: "VALID_AMD_AUTHENTICODE".to_owned(),
            identity_gate_pass: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct I2gConfiguration {
    pub service_name: String,
    pub service_sid: String,
    pub service_sid_type: String,
    pub account: String,
    pub account_sid: String,
    pub binary_path: String,
    pub semantic_operation: String,
    pub fixed_cli_arguments: Vec<String>,
    pub harness_sha256: String,
    pub amd_cli_identity: AmdCliIdentity,
    pub working_directory: String,
    pub environment: BTreeMap<String, String>,
    pub timeout_ms: u64,
    pub child_safety_cap_ms: u64,
    pub protocol: String,
    pub output_policy: String,
    pub job_policy: String,
    pub sampling: bool,
    pub direct_service_sid_rights: Vec<String>,
}

impl I2gConfiguration {
    pub fn control(service_sid: &str) -> Self {
        Self::with_rights(service_sid, vec![CONTROL_REQUIRED_RIGHT.to_owned()])
    }

    pub fn with_rights(service_sid: &str, direct_rights: Vec<String>) -> Self {
        Self {
            service_name: I2G_SERVICE_NAME.to_owned(),
            service_sid: service_sid.to_owned(),
            service_sid_type: I2G_SERVICE_SID_TYPE.to_owned(),
            account: I2G_SERVICE_ACCOUNT.to_owned(),
            account_sid: I2G_SERVICE_ACCOUNT_SID.to_owned(),
            binary_path: "<frozen-i2g-qualification-artifact>".to_owned(),
            semantic_operation: I2G_OPERATION.to_owned(),
            fixed_cli_arguments: I2G_FIXED_AMD_ARGUMENTS
                .iter()
                .map(|value| (*value).to_owned())
                .collect(),
            harness_sha256: I2G_HARNESS_SHA256_SYNTHETIC.to_owned(),
            amd_cli_identity: AmdCliIdentity::fixed(),
            working_directory: "<broker-owned-working-directory>".to_owned(),
            environment: BTreeMap::from([("I2G_OPERATION".to_owned(), I2G_OPERATION.to_owned())]),
            timeout_ms: I2G_COUNTER_DISCOVERY_TIMEOUT_MS,
            child_safety_cap_ms: I2G_CHILD_SAFETY_CAP_MS,
            protocol: I2G_PROTOCOL.to_owned(),
            output_policy: "bounded stdout/stderr; no unlimited vendor output".to_owned(),
            job_policy: "direct child in kill-on-close job".to_owned(),
            sampling: false,
            direct_service_sid_rights: normalized_right_vec(&direct_rights),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConfigurationComparison {
    pub status: String,
    pub pass: bool,
    pub allowed_delta: String,
    pub changed_non_treatment_fields: Vec<String>,
    pub direct_rights_before: Vec<String>,
    pub direct_rights_after: Vec<String>,
    pub direct_rights_added: Vec<String>,
    pub direct_rights_removed: Vec<String>,
}

pub fn compare_configuration(
    control: &I2gConfiguration,
    treatment: &I2gConfiguration,
) -> ConfigurationComparison {
    let mut changed = Vec::new();
    macro_rules! compare_field {
        ($field:ident) => {
            if control.$field != treatment.$field {
                changed.push(stringify!($field).to_owned());
            }
        };
    }
    compare_field!(service_name);
    compare_field!(service_sid);
    compare_field!(service_sid_type);
    compare_field!(account);
    compare_field!(account_sid);
    compare_field!(binary_path);
    compare_field!(semantic_operation);
    compare_field!(fixed_cli_arguments);
    compare_field!(harness_sha256);
    compare_field!(amd_cli_identity);
    compare_field!(working_directory);
    compare_field!(environment);
    compare_field!(timeout_ms);
    compare_field!(child_safety_cap_ms);
    compare_field!(protocol);
    compare_field!(output_policy);
    compare_field!(job_policy);
    compare_field!(sampling);

    let before = normalized_set(&control.direct_service_sid_rights);
    let after = normalized_set(&treatment.direct_service_sid_rights);
    let added_set: BTreeSet<String> = after.difference(&before).cloned().collect();
    let removed_set: BTreeSet<String> = before.difference(&after).cloned().collect();
    let added = added_set.iter().cloned().collect::<Vec<_>>();
    let removed = removed_set.iter().cloned().collect::<Vec<_>>();
    let exact_allowed_delta = added.len() == 1
        && added[0].eq_ignore_ascii_case(TREATMENT_RIGHT)
        && removed.is_empty()
        && control.service_sid == treatment.service_sid;
    let pass = changed.is_empty() && exact_allowed_delta;
    ConfigurationComparison {
        status: if pass {
            "PASS_EXACT_ONE_ALLOWED_TREATMENT_CONFIGURATION_DELTA".to_owned()
        } else {
            "INVALID_CONFIGURATION_DELTA".to_owned()
        },
        pass,
        allowed_delta: format!("{TREATMENT_RIGHT} assignment to the same Service SID only"),
        changed_non_treatment_fields: changed,
        direct_rights_before: normalized_right_vec(&control.direct_service_sid_rights),
        direct_rights_after: normalized_right_vec(&treatment.direct_service_sid_rights),
        direct_rights_added: added,
        direct_rights_removed: removed,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct I2gPolicySnapshot {
    pub service_sid: String,
    pub direct_rights: Vec<String>,
    pub preexisting_system_profile_right: bool,
    pub preexisting_profile_single_right: bool,
    pub system_profile_right_added_by_run: bool,
    pub profile_single_right_added_by_run: bool,
}

impl I2gPolicySnapshot {
    pub fn empty(service_sid: &str) -> Self {
        Self {
            service_sid: service_sid.to_owned(),
            direct_rights: Vec::new(),
            preexisting_system_profile_right: false,
            preexisting_profile_single_right: false,
            system_profile_right_added_by_run: false,
            profile_single_right_added_by_run: false,
        }
    }

    pub fn has_right(&self, right: &str) -> bool {
        self.direct_rights
            .iter()
            .any(|value| value.eq_ignore_ascii_case(right))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PolicyComparison {
    pub pass: bool,
    pub status: String,
    pub exact_delta_count: usize,
    pub added: Vec<String>,
    pub removed: Vec<String>,
    pub service_sid_before: String,
    pub service_sid_after: String,
}

pub fn compare_policy_delta(
    before: &I2gPolicySnapshot,
    after: &I2gPolicySnapshot,
) -> PolicyComparison {
    let before_set = normalized_set(&before.direct_rights);
    let after_set = normalized_set(&after.direct_rights);
    let added = after_set
        .difference(&before_set)
        .cloned()
        .collect::<Vec<_>>();
    let removed = before_set
        .difference(&after_set)
        .cloned()
        .collect::<Vec<_>>();
    let pass = before.service_sid == after.service_sid
        && added.len() == 1
        && added[0].eq_ignore_ascii_case(TREATMENT_RIGHT)
        && removed.is_empty();
    PolicyComparison {
        pass,
        status: if pass {
            "PASS_EXACT_ONE_ALLOWED_TREATMENT_CONFIGURATION_DELTA".to_owned()
        } else {
            "INVALID_CONFIGURATION_DELTA".to_owned()
        },
        exact_delta_count: added.len() + removed.len(),
        added,
        removed,
        service_sid_before: before.service_sid.clone(),
        service_sid_after: after.service_sid.clone(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AdjustTokenError {
    Success,
    ErrorNotAllAssigned,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AdjustTokenEvidence {
    pub privilege: String,
    pub adjust_token_privileges_returned_true: bool,
    pub get_last_error: AdjustTokenError,
    pub post_state: PrivilegeState,
    pub pass: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SyntheticDiscoveryOutcome {
    PowerUnavailable,
    PowerAvailable,
    ExitNonzero,
    Timeout,
    SpawnFailure,
    IdentityMismatch,
    OrphanChild,
}

impl SyntheticDiscoveryOutcome {
    pub fn parse(value: &str) -> Option<Self> {
        match value.to_ascii_lowercase().replace('_', "-").as_str() {
            "power-unavailable" | "unavailable" | "negative" => Some(Self::PowerUnavailable),
            "power-available" | "available" | "happy" => Some(Self::PowerAvailable),
            "exit-nonzero" | "nonzero" | "discovery-failure" => Some(Self::ExitNonzero),
            "timeout" => Some(Self::Timeout),
            "spawn-failure" | "spawn" => Some(Self::SpawnFailure),
            "identity-mismatch" | "identity" => Some(Self::IdentityMismatch),
            "orphan-child" | "orphan" | "ownership" => Some(Self::OrphanChild),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum I2gSyntheticScenario {
    Happy,
    Negative,
    ControlDrift,
    PreControlFailure,
    ControlTokenGateFailure,
    ConfigDeltaFailure,
    TokenDeltaFailure,
    MaterializationFailure,
    SystemProfileRegression,
    ProcessOwnershipFailure,
    ControlTimeout,
    TreatmentTimeout,
    CleanupFailure,
    IdentityMismatch,
    SpawnFailure,
    ExitNonzero,
    UnexpectedPreexistingProfileRight,
    RecoveryMatrix,
    CrashWindowMatrix,
}

impl I2gSyntheticScenario {
    pub fn parse(value: &str) -> Option<Self> {
        match value.to_ascii_lowercase().replace('_', "-").as_str() {
            "happy" | "synthetic-happy-path" => Some(Self::Happy),
            "negative" | "synthetic-negative-path" => Some(Self::Negative),
            "control-drift" | "power-available-control" => Some(Self::ControlDrift),
            "pre-control-failure" | "precontrol-failure" => Some(Self::PreControlFailure),
            "control-token-gate-failure" | "token-gate-failure" => {
                Some(Self::ControlTokenGateFailure)
            }
            "config-delta-failure" | "configuration-delta-failure" => {
                Some(Self::ConfigDeltaFailure)
            }
            "token-delta-failure" => Some(Self::TokenDeltaFailure),
            "materialization-failure" | "profile-single-materialization-failure" => {
                Some(Self::MaterializationFailure)
            }
            "systemprofile-regression" | "system-profile-regression" => {
                Some(Self::SystemProfileRegression)
            }
            "process-ownership-failure" | "ownership-failure" => {
                Some(Self::ProcessOwnershipFailure)
            }
            "control-timeout" => Some(Self::ControlTimeout),
            "treatment-timeout" => Some(Self::TreatmentTimeout),
            "cleanup-failure" => Some(Self::CleanupFailure),
            "identity-mismatch" => Some(Self::IdentityMismatch),
            "spawn-failure" => Some(Self::SpawnFailure),
            "exit-nonzero" | "discovery-failure" => Some(Self::ExitNonzero),
            "unexpected-preexisting-profile-right" | "preexisting-profile-right" => {
                Some(Self::UnexpectedPreexistingProfileRight)
            }
            "recovery-matrix" | "resume-recovery" => Some(Self::RecoveryMatrix),
            "crash-window-matrix" | "crash-recovery-matrix" => Some(Self::CrashWindowMatrix),
            _ => None,
        }
    }

    fn control_discovery(self) -> SyntheticDiscoveryOutcome {
        match self {
            Self::ControlDrift => SyntheticDiscoveryOutcome::PowerAvailable,
            Self::ControlTimeout => SyntheticDiscoveryOutcome::Timeout,
            Self::IdentityMismatch => SyntheticDiscoveryOutcome::IdentityMismatch,
            Self::SpawnFailure => SyntheticDiscoveryOutcome::SpawnFailure,
            Self::ExitNonzero => SyntheticDiscoveryOutcome::ExitNonzero,
            Self::ProcessOwnershipFailure => SyntheticDiscoveryOutcome::OrphanChild,
            _ => SyntheticDiscoveryOutcome::PowerUnavailable,
        }
    }

    fn treatment_discovery(self) -> SyntheticDiscoveryOutcome {
        match self {
            Self::Negative => SyntheticDiscoveryOutcome::PowerUnavailable,
            Self::TreatmentTimeout => SyntheticDiscoveryOutcome::Timeout,
            _ => SyntheticDiscoveryOutcome::PowerAvailable,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiscoveryEvidence {
    pub phase: I2gPhase,
    pub operation: String,
    pub fixed_cli_arguments: Vec<String>,
    pub sampling: bool,
    pub child_spawned: bool,
    pub actual_run_count_incremented: bool,
    pub child_pid: Option<u32>,
    pub process_start_time: Option<u64>,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub stdout_bounded: bool,
    pub stderr_bounded: bool,
    pub timeout: bool,
    pub identity_gate_pass: bool,
    pub owned_child: bool,
    pub orphan_child: bool,
    pub no_counters_diagnostic: bool,
    pub classification: String,
}

#[derive(Debug, Clone)]
pub struct SyntheticDiscoveryAdapter {
    pub control: SyntheticDiscoveryOutcome,
    pub treatment: SyntheticDiscoveryOutcome,
    pub launches: u32,
}

impl SyntheticDiscoveryAdapter {
    pub fn new(scenario: I2gSyntheticScenario) -> Self {
        Self {
            control: scenario.control_discovery(),
            treatment: scenario.treatment_discovery(),
            launches: 0,
        }
    }

    pub fn launch(&mut self, phase: I2gPhase) -> DiscoveryEvidence {
        let outcome = match phase {
            I2gPhase::Control => self.control,
            I2gPhase::Treatment => self.treatment,
        };
        let identity_gate_pass = !matches!(outcome, SyntheticDiscoveryOutcome::IdentityMismatch);
        let child_spawned = !matches!(
            outcome,
            SyntheticDiscoveryOutcome::IdentityMismatch | SyntheticDiscoveryOutcome::SpawnFailure
        );
        if child_spawned {
            self.launches = self.launches.saturating_add(1);
        }
        let (exit_code, stdout, stderr, timeout, owned_child, orphan_child) = match outcome {
            SyntheticDiscoveryOutcome::PowerUnavailable => (
                Some(0),
                String::new(),
                "ERROR: There is no counters available".to_owned(),
                false,
                true,
                false,
            ),
            SyntheticDiscoveryOutcome::PowerAvailable => (
                Some(0),
                "Power\nFrequency\n".to_owned(),
                String::new(),
                false,
                true,
                false,
            ),
            SyntheticDiscoveryOutcome::ExitNonzero => (
                Some(1),
                String::new(),
                "synthetic counter discovery failure".to_owned(),
                false,
                true,
                false,
            ),
            SyntheticDiscoveryOutcome::Timeout => (
                None,
                String::new(),
                "synthetic timeout".to_owned(),
                true,
                true,
                false,
            ),
            SyntheticDiscoveryOutcome::SpawnFailure => (
                None,
                String::new(),
                "synthetic spawn failure".to_owned(),
                false,
                false,
                false,
            ),
            SyntheticDiscoveryOutcome::IdentityMismatch => (
                None,
                String::new(),
                "synthetic AMD identity mismatch".to_owned(),
                false,
                false,
                false,
            ),
            SyntheticDiscoveryOutcome::OrphanChild => (
                Some(0),
                String::new(),
                "ERROR: There is no counters available".to_owned(),
                false,
                false,
                true,
            ),
        };
        let no_counters_diagnostic = stderr.to_ascii_lowercase().contains("no counters");
        let classification = if !identity_gate_pass {
            "IDENTITY_MISMATCH".to_owned()
        } else if timeout {
            "TIMEOUT".to_owned()
        } else if orphan_child || (child_spawned && !owned_child) {
            "PROCESS_OWNERSHIP_FAILED".to_owned()
        } else if !child_spawned {
            "DISCOVERY_FAILED".to_owned()
        } else {
            classify_counter_discovery(exit_code, &stdout, &stderr)
                .as_str()
                .to_owned()
        };
        DiscoveryEvidence {
            phase,
            operation: I2G_OPERATION.to_owned(),
            fixed_cli_arguments: I2G_FIXED_AMD_ARGUMENTS
                .iter()
                .map(|value| (*value).to_owned())
                .collect(),
            sampling: false,
            child_spawned,
            actual_run_count_incremented: child_spawned,
            child_pid: child_spawned.then_some(if phase == I2gPhase::Control {
                4101
            } else {
                4201
            }),
            process_start_time: child_spawned.then_some(if phase == I2gPhase::Control {
                1001
            } else {
                1002
            }),
            exit_code,
            stdout: bounded_output(&stdout),
            stderr: bounded_output(&stderr),
            stdout_bounded: stdout.len() <= COUNTER_DISCOVERY_MAX_OUTPUT_BYTES,
            stderr_bounded: stderr.len() <= COUNTER_DISCOVERY_MAX_OUTPUT_BYTES,
            timeout,
            identity_gate_pass,
            owned_child,
            orphan_child,
            no_counters_diagnostic,
            classification,
        }
    }
}

fn bounded_output(value: &str) -> String {
    value
        .as_bytes()
        .get(..value.len().min(COUNTER_DISCOVERY_MAX_OUTPUT_BYTES))
        .and_then(|bytes| String::from_utf8(bytes.to_vec()).ok())
        .unwrap_or_default()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProcessIdentity {
    pub pid: u32,
    pub process_start_time: u64,
    pub role: String,
    pub run_owned: bool,
}

impl ProcessIdentity {
    fn discovery(phase: I2gPhase) -> Self {
        Self {
            pid: if phase == I2gPhase::Control {
                4101
            } else {
                4201
            },
            process_start_time: if phase == I2gPhase::Control {
                1001
            } else {
                1002
            },
            role: format!("{}_COUNTER_DISCOVERY", phase.as_str()),
            run_owned: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MachineObservation {
    pub service_present: bool,
    pub service_running: bool,
    pub service_pid: u32,
    pub token_present: bool,
    pub owned_process_count: u32,
    pub direct_service_sid_rights: Vec<String>,
    #[serde(default)]
    pub exact_child: Option<ProcessIdentity>,
    #[serde(default)]
    pub owned_processes: Vec<ProcessIdentity>,
}

impl MachineObservation {
    fn is_quiescent(&self) -> bool {
        !self.service_present
            && !self.service_running
            && self.service_pid == 0
            && !self.token_present
            && self.owned_process_count == 0
            && self.exact_child.is_none()
            && self.owned_processes.is_empty()
    }

    fn has_exact_child(&self, phase: I2gPhase) -> bool {
        self.exact_child
            .as_ref()
            .is_some_and(|child| child.role == format!("{}_COUNTER_DISCOVERY", phase.as_str()))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RecoveryDecision {
    NoAction,
    ResumeTreatmentPolicy,
    ResumeControlTeardown,
    ResumeTreatmentServiceOnly,
    ResumeRollback,
    Invalid,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RecoveryReconciliation {
    pub decision: RecoveryDecision,
    pub recovered_control_counter_discovery_runs: u32,
    pub recovered_treatment_counter_discovery_runs: u32,
    pub recovered_total_counter_discovery_runs: u32,
    pub inferred_service_owned_by_run: bool,
    pub inferred_system_profile_owned_by_run: bool,
    pub inferred_profile_single_owned_by_run: bool,
    pub reason: String,
}

fn recover_spawn_count(
    persisted: &mut PersistedI2gState,
    phase: I2gPhase,
    actual: &MachineObservation,
) {
    let ledger = persisted.discovery_ledger(phase).clone();
    let proven_spawn = ledger.spawn_observed_durable
        || ledger.completion_observed_durable
        || actual.has_exact_child(phase);
    if !ledger.spawn_intent_durable || !proven_spawn {
        return;
    }
    let already_counted = match phase {
        I2gPhase::Control => persisted.actual_control_counter_discovery_runs,
        I2gPhase::Treatment => persisted.actual_treatment_counter_discovery_runs,
    };
    if already_counted == 0 && persisted.actual_total_counter_discovery_runs < I2G_MAX_TOTAL_RUNS {
        match phase {
            I2gPhase::Control => persisted.actual_control_counter_discovery_runs = 1,
            I2gPhase::Treatment => persisted.actual_treatment_counter_discovery_runs = 1,
        }
        persisted.actual_total_counter_discovery_runs = persisted
            .actual_control_counter_discovery_runs
            .saturating_add(persisted.actual_treatment_counter_discovery_runs);
    }
}

/// Reconcile durable intent with a fresh machine observation.  An inconsistent observation is
/// deliberately routed to rollback, never to discovery retry.  Only the explicit `Invalid`
/// persisted state is unrecoverable by contract; terminal clean states are `NoAction`.
pub fn reconcile_persisted_state(
    persisted: &mut PersistedI2gState,
    actual: &MachineObservation,
) -> RecoveryReconciliation {
    persisted
        .service_ownership
        .reconcile_machine_observation(actual.service_present);
    persisted
        .system_profile_ownership
        .reconcile_machine_observation(
            actual
                .direct_service_sid_rights
                .iter()
                .any(|right| right.eq_ignore_ascii_case(CONTROL_REQUIRED_RIGHT)),
        );
    persisted
        .profile_single_ownership
        .reconcile_machine_observation(
            actual
                .direct_service_sid_rights
                .iter()
                .any(|right| right.eq_ignore_ascii_case(TREATMENT_RIGHT)),
        );
    recover_spawn_count(persisted, I2gPhase::Control, actual);
    recover_spawn_count(persisted, I2gPhase::Treatment, actual);
    persisted.service_created = persisted.service_ownership.owned_by_run;
    persisted.system_profile_right_added_by_run = persisted.system_profile_ownership.owned_by_run;
    persisted.profile_single_right_added_by_run = persisted.profile_single_ownership.owned_by_run;

    let rights = normalized_set(&actual.direct_service_sid_rights);
    let control_right = rights.contains(&CONTROL_REQUIRED_RIGHT.to_ascii_lowercase());
    let treatment_right = rights.contains(&TREATMENT_RIGHT.to_ascii_lowercase());
    let control_teardown_ready = actual.service_present
        && !actual.service_running
        && actual.service_pid == 0
        && !actual.token_present
        && actual.exact_child.is_none()
        && actual.owned_process_count == 0
        && actual.owned_processes.is_empty();
    let treatment_setup_ready = treatment_right
        && control_right
        && actual.service_present
        && !actual.service_running
        && actual.service_pid == 0
        && !actual.token_present
        && actual.exact_child.is_none()
        && actual.owned_process_count == 0
        && actual.owned_processes.is_empty();
    let treatment_policy_owned = persisted.profile_single_ownership.mutation_intent_durable
        && !persisted.profile_single_ownership.preexisting
        && persisted.profile_single_ownership.owned_by_run;

    let (decision, reason) = match persisted.state {
        I2gState::Prepared => {
            if actual.is_quiescent() && !control_right && !treatment_right {
                (
                    RecoveryDecision::NoAction,
                    "prepared and machine is quiescent",
                )
            } else {
                (
                    RecoveryDecision::ResumeRollback,
                    "prepared state has unexpected machine residue",
                )
            }
        }
        I2gState::ControlPolicyReady
        | I2gState::ControlServiceRunning
        | I2gState::ControlTokenReady
        | I2gState::ControlDiscoveryRunning => {
            if actual.service_present {
                (
                    RecoveryDecision::ResumeControlTeardown,
                    "CONTROL state never resumes discovery; teardown/rollback is required",
                )
            } else {
                (
                    RecoveryDecision::ResumeRollback,
                    "CONTROL service is absent or inconsistent; fail closed to rollback",
                )
            }
        }
        I2gState::ControlComplete => {
            if actual.service_present
                && !actual.service_running
                && actual.service_pid == 0
                && !actual.token_present
                && actual.exact_child.is_none()
                && actual.owned_process_count == 0
                && actual.owned_processes.is_empty()
            {
                (
                    RecoveryDecision::ResumeControlTeardown,
                    "CONTROL completed; durable teardown evidence still needs cleanup",
                )
            } else {
                (
                    RecoveryDecision::ResumeRollback,
                    "CONTROL completion and machine observation are inconsistent",
                )
            }
        }
        I2gState::ControlTeardownComplete => {
            if !control_teardown_ready || !control_right {
                (
                    RecoveryDecision::ResumeRollback,
                    "CONTROL teardown observation is incomplete or inconsistent",
                )
            } else if treatment_right && !treatment_policy_owned {
                (
                    RecoveryDecision::ResumeRollback,
                    "TREATMENT right is present without durable run-owned policy intent",
                )
            } else if treatment_policy_owned {
                (
                    RecoveryDecision::ResumeTreatmentServiceOnly,
                    "TREATMENT policy mutation is durable; resume service/token path only",
                )
            } else {
                (
                    RecoveryDecision::ResumeTreatmentPolicy,
                    "CONTROL teardown is durable; resume TREATMENT policy path only",
                )
            }
        }
        I2gState::TreatmentPolicyReady => {
            if treatment_setup_ready && treatment_policy_owned {
                (
                    RecoveryDecision::ResumeTreatmentServiceOnly,
                    "TREATMENT policy is durable; resume service/token path only",
                )
            } else {
                (
                    RecoveryDecision::ResumeRollback,
                    "TREATMENT policy/service observation is inconsistent",
                )
            }
        }
        I2gState::TreatmentServiceRunning
        | I2gState::TreatmentTokenReady
        | I2gState::TreatmentDiscoveryRunning
        | I2gState::TreatmentComplete
        | I2gState::RollbackRunning
        | I2gState::Failed => (
            RecoveryDecision::ResumeRollback,
            "in-progress or failed state resumes rollback only",
        ),
        I2gState::RollbackComplete => {
            if actual.is_quiescent()
                && !persisted.service_ownership.owned_by_run
                && !persisted.system_profile_ownership.owned_by_run
                && !persisted.profile_single_ownership.owned_by_run
            {
                (
                    RecoveryDecision::NoAction,
                    "rollback is durable and machine is quiescent",
                )
            } else {
                (
                    RecoveryDecision::ResumeRollback,
                    "rollback is not durably reflected by machine observation",
                )
            }
        }
        I2gState::Invalid => (
            RecoveryDecision::Invalid,
            "INVALID is the only persisted state that is unrecoverable by contract",
        ),
    };

    RecoveryReconciliation {
        decision,
        recovered_control_counter_discovery_runs: persisted.actual_control_counter_discovery_runs,
        recovered_treatment_counter_discovery_runs: persisted
            .actual_treatment_counter_discovery_runs,
        recovered_total_counter_discovery_runs: persisted.actual_total_counter_discovery_runs,
        inferred_service_owned_by_run: persisted.service_ownership.owned_by_run,
        inferred_system_profile_owned_by_run: persisted.system_profile_ownership.owned_by_run,
        inferred_profile_single_owned_by_run: persisted.profile_single_ownership.owned_by_run,
        reason: reason.to_owned(),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct I2gCheck {
    pub name: String,
    pub status: String,
    pub detail: String,
}

impl I2gCheck {
    fn pass(name: &str, detail: impl Into<String>) -> Self {
        Self {
            name: name.to_owned(),
            status: "PASS".to_owned(),
            detail: detail.into(),
        }
    }

    fn fail(name: &str, detail: impl Into<String>) -> Self {
        Self {
            name: name.to_owned(),
            status: "FAIL".to_owned(),
            detail: detail.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum I2gResult {
    ControlDrift,
    ProfileSingleSufficientInPairedI2gContext,
    ProfileSingleInsufficientInPairedI2gContext,
    TokenGateFailed,
    IdentityMismatch,
    InvalidConfigurationDelta,
    InvalidTokenDelta,
    DiscoveryFailed,
    Timeout,
    ProcessOwnershipFailed,
    CleanupFailed,
    InvalidNoCausalInterpretation,
}

impl I2gResult {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ControlDrift => "CONTROL_DRIFT",
            Self::ProfileSingleSufficientInPairedI2gContext => {
                "PROFILE_SINGLE_SUFFICIENT_IN_PAIRED_I2G_CONTEXT"
            }
            Self::ProfileSingleInsufficientInPairedI2gContext => {
                "PROFILE_SINGLE_INSUFFICIENT_IN_PAIRED_I2G_CONTEXT"
            }
            Self::TokenGateFailed => "TOKEN_GATE_FAILED",
            Self::IdentityMismatch => "IDENTITY_MISMATCH",
            Self::InvalidConfigurationDelta => "INVALID_CONFIGURATION_DELTA",
            Self::InvalidTokenDelta => "INVALID_TOKEN_DELTA",
            Self::DiscoveryFailed => "DISCOVERY_FAILED",
            Self::Timeout => "TIMEOUT",
            Self::ProcessOwnershipFailed => "PROCESS_OWNERSHIP_FAILED",
            Self::CleanupFailed => "CLEANUP_FAILED",
            Self::InvalidNoCausalInterpretation => "INVALID_NO_CAUSAL_INTERPRETATION",
        }
    }
}

impl fmt::Display for I2gResult {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RollbackEvidence {
    pub cleanup_result: String,
    pub stop_accepting_work: bool,
    pub child_was_owned: bool,
    pub amd_child_absent: bool,
    pub exact_child_identity_absent: bool,
    pub treatment_child_absent: bool,
    pub service_stopped: bool,
    pub service_pid_zero: bool,
    pub token_gone: bool,
    pub owned_process_tree_empty: bool,
    pub direct_rights_inspected: bool,
    pub profile_single_remove_attempted: bool,
    pub profile_single_remove_verified: bool,
    pub system_profile_remove_attempted: bool,
    pub system_profile_remove_verified: bool,
    pub service_deleted: bool,
    pub service_absent: bool,
    pub owned_processes_absent: bool,
    pub recovery_required: bool,
    pub system_profile_right_added_by_run: bool,
    pub profile_single_right_added_by_run: bool,
    pub events: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct I2gMutationAssertions {
    pub amd_real_runtime_during_task: u32,
    pub i2g_real_runtime_during_task: u32,
    pub real_service_create: u32,
    pub real_service_start: u32,
    pub real_service_stop: u32,
    pub real_service_delete: u32,
    pub real_lsa_right_add: u32,
    pub real_lsa_right_remove: u32,
    pub real_adjust_token_privileges: u32,
    pub real_amd_counter_discovery: u32,
    pub real_power_sampling: u32,
    pub real_acl_mutation: u32,
    pub real_registry_mutation: u32,
    pub synthetic_service_create: u32,
    pub synthetic_service_starts: u32,
    pub synthetic_service_stops: u32,
    pub synthetic_service_deletes: u32,
    pub synthetic_policy_adds: u32,
    pub synthetic_policy_removes: u32,
    pub synthetic_discovery_launches: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct I2gSummary {
    pub schema: String,
    pub result: String,
    pub normalized_result: String,
    pub offline_validation: String,
    pub qualification_only: bool,
    pub i2g_variable: String,
    pub i2g_selection_confidence: String,
    pub i2g_experiment_shape: String,
    pub experiment_result: String,
    pub control_result: String,
    pub treatment_result: Option<String>,
    pub control_drift: bool,
    pub treatment_allowed: bool,
    pub cleanup_result: String,
    pub causal_interpretation_valid: bool,
    pub actual_control_counter_discovery_runs: u32,
    pub actual_treatment_counter_discovery_runs: u32,
    pub actual_total_counter_discovery_runs: u32,
    pub max_control_counter_discovery_runs: u32,
    pub max_treatment_counter_discovery_runs: u32,
    pub max_total_counter_discovery_runs: u32,
    pub control_retry_allowed: bool,
    pub treatment_retry_allowed: bool,
    pub power_sampling_runs: u32,
    pub service_name: String,
    pub service_sid: String,
    pub service_account: String,
    pub service_account_sid: String,
    pub service_sid_type: String,
    pub fixed_amd_cli_path: String,
    pub fixed_amd_cli_sha256: String,
    pub fixed_amd_cli_version: String,
    pub fixed_amd_cli_architecture: String,
    pub fixed_cli_arguments: Vec<String>,
    pub operation: String,
    pub historical_i2f_role: String,
    pub historical_i2f_is_active_causal_control: bool,
    pub real_execution_allowed: bool,
    pub real_cleanup_allowed: bool,
    pub human_authorization_recorded: bool,
    pub control_harness_sha256: String,
    pub treatment_harness_sha256: String,
    pub state: I2gState,
    pub checks: Vec<I2gCheck>,
    pub rollback: RollbackEvidence,
    pub mutation_assertions: I2gMutationAssertions,
    pub evidence_files: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct I2gCleanupSyntheticSummary {
    pub schema: String,
    pub result: String,
    pub cleanup_result: String,
    pub qualification_only: bool,
    pub real_cleanup_allowed: bool,
    pub rollback_order: Vec<String>,
    pub checks: Vec<I2gCheck>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackendError {
    RealExecutionNotAuthorized,
    Fault(String),
}

impl fmt::Display for BackendError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RealExecutionNotAuthorized => {
                formatter.write_str("I2G real backend is not authorized")
            }
            Self::Fault(message) => formatter.write_str(message),
        }
    }
}

/// Boundary for future machine implementations.  The synthetic implementation below is the
/// only implementation used by this task.  A future authorized task can implement these methods
/// with the already-qualified Windows helpers in `windows.rs`; the fail-closed PowerShell gate
/// prevents this type from being selected today.
pub trait I2gBackend {
    fn service_create(&mut self, config: &I2gConfiguration) -> Result<(), BackendError>;
    fn service_start(&mut self, phase: I2gPhase) -> Result<(), BackendError>;
    fn service_stop(&mut self) -> Result<(), BackendError>;
    fn service_delete(&mut self) -> Result<(), BackendError>;
    fn machine_observation(&self) -> MachineObservation;
    fn read_policy(&self) -> I2gPolicySnapshot;
    fn add_policy_right(&mut self, right: &str) -> Result<(), BackendError>;
    fn remove_policy_right(&mut self, right: &str) -> Result<(), BackendError>;
    fn token_snapshot(&self, phase: I2gPhase, moment: I2gTokenMoment) -> TokenSnapshot;
    fn adjust_token_privilege(&mut self, phase: I2gPhase, privilege: &str) -> AdjustTokenEvidence;
    fn amd_identity(&self, phase: I2gPhase) -> AmdCliIdentity;
    fn discover(&mut self, phase: I2gPhase) -> DiscoveryEvidence;
    fn mutation_assertions(&self) -> I2gMutationAssertions;
}

/// Compile-time representation of the future Windows side.  It intentionally refuses every
/// operation until a separate human-authorized real-run task changes the source-controlled gate.
/// It is never constructed by the current wrappers or tests.
#[derive(Debug, Default)]
pub struct RealWindowsI2gBackend;

impl I2gBackend for RealWindowsI2gBackend {
    fn service_create(&mut self, _config: &I2gConfiguration) -> Result<(), BackendError> {
        Err(BackendError::RealExecutionNotAuthorized)
    }

    fn service_start(&mut self, _phase: I2gPhase) -> Result<(), BackendError> {
        Err(BackendError::RealExecutionNotAuthorized)
    }

    fn service_stop(&mut self) -> Result<(), BackendError> {
        Err(BackendError::RealExecutionNotAuthorized)
    }

    fn service_delete(&mut self) -> Result<(), BackendError> {
        Err(BackendError::RealExecutionNotAuthorized)
    }

    fn machine_observation(&self) -> MachineObservation {
        MachineObservation {
            service_present: false,
            service_running: false,
            service_pid: 0,
            token_present: false,
            owned_process_count: 0,
            direct_service_sid_rights: Vec::new(),
            exact_child: None,
            owned_processes: Vec::new(),
        }
    }

    fn read_policy(&self) -> I2gPolicySnapshot {
        I2gPolicySnapshot::empty(I2G_SYNTHETIC_SERVICE_SID)
    }

    fn add_policy_right(&mut self, _right: &str) -> Result<(), BackendError> {
        Err(BackendError::RealExecutionNotAuthorized)
    }

    fn remove_policy_right(&mut self, _right: &str) -> Result<(), BackendError> {
        Err(BackendError::RealExecutionNotAuthorized)
    }

    fn token_snapshot(&self, _phase: I2gPhase, _moment: I2gTokenMoment) -> TokenSnapshot {
        synthetic_token(
            I2gPhase::Control,
            I2gTokenMoment::Pre,
            I2gPolicySnapshot::empty(I2G_SYNTHETIC_SERVICE_SID),
            BTreeSet::new(),
        )
    }

    fn adjust_token_privilege(&mut self, _phase: I2gPhase, privilege: &str) -> AdjustTokenEvidence {
        AdjustTokenEvidence {
            privilege: privilege.to_owned(),
            adjust_token_privileges_returned_true: false,
            get_last_error: AdjustTokenError::ErrorNotAllAssigned,
            post_state: PrivilegeState::Absent,
            pass: false,
        }
    }

    fn amd_identity(&self, _phase: I2gPhase) -> AmdCliIdentity {
        let mut identity = AmdCliIdentity::fixed();
        identity.identity_gate_pass = false;
        identity
    }

    fn discover(&mut self, phase: I2gPhase) -> DiscoveryEvidence {
        DiscoveryEvidence {
            phase,
            operation: I2G_OPERATION.to_owned(),
            fixed_cli_arguments: I2G_FIXED_AMD_ARGUMENTS
                .iter()
                .map(|value| (*value).to_owned())
                .collect(),
            sampling: false,
            child_spawned: false,
            actual_run_count_incremented: false,
            child_pid: None,
            process_start_time: None,
            exit_code: None,
            stdout: String::new(),
            stderr: "I2G real backend is not authorized".to_owned(),
            stdout_bounded: true,
            stderr_bounded: true,
            timeout: false,
            identity_gate_pass: false,
            owned_child: false,
            orphan_child: false,
            no_counters_diagnostic: false,
            classification: "REAL_EXECUTION_NOT_AUTHORIZED".to_owned(),
        }
    }

    fn mutation_assertions(&self) -> I2gMutationAssertions {
        I2gMutationAssertions::default()
    }
}

#[derive(Debug, Clone)]
pub struct SyntheticBackend {
    pub scenario: I2gSyntheticScenario,
    pub policy: I2gPolicySnapshot,
    pub service_present: bool,
    pub service_running: bool,
    pub service_pid: u32,
    pub token_present: bool,
    pub active_phase: Option<I2gPhase>,
    pub exact_child: Option<ProcessIdentity>,
    pub owned_processes: Vec<ProcessIdentity>,
    pub enabled_privileges: BTreeSet<String>,
    pub adapter: SyntheticDiscoveryAdapter,
    pub mutations: I2gMutationAssertions,
}

impl SyntheticBackend {
    pub fn new(scenario: I2gSyntheticScenario) -> Self {
        let mut policy = I2gPolicySnapshot::empty(I2G_SYNTHETIC_SERVICE_SID);
        if scenario == I2gSyntheticScenario::UnexpectedPreexistingProfileRight {
            policy.direct_rights.push(TREATMENT_RIGHT.to_owned());
            policy.preexisting_profile_single_right = true;
        }
        Self {
            scenario,
            policy,
            service_present: false,
            service_running: false,
            service_pid: 0,
            token_present: false,
            active_phase: None,
            exact_child: None,
            owned_processes: Vec::new(),
            enabled_privileges: BTreeSet::new(),
            adapter: SyntheticDiscoveryAdapter::new(scenario),
            mutations: I2gMutationAssertions::default(),
        }
    }

    pub fn inject_extra_configuration_right(&mut self) {
        if !self.policy.has_right("SeDebugPrivilege") {
            self.policy
                .direct_rights
                .push("SeDebugPrivilege".to_owned());
            self.policy.direct_rights = normalized_right_vec(&self.policy.direct_rights);
            self.mutations.synthetic_policy_adds += 1;
        }
    }

    fn finish_child(&mut self) {
        self.exact_child = None;
        self.owned_processes.clear();
    }
}

impl I2gBackend for SyntheticBackend {
    fn service_create(&mut self, _config: &I2gConfiguration) -> Result<(), BackendError> {
        if self.service_present {
            return Err(BackendError::Fault(
                "synthetic service already exists".to_owned(),
            ));
        }
        self.service_present = true;
        self.mutations.synthetic_service_create += 1;
        Ok(())
    }

    fn service_start(&mut self, phase: I2gPhase) -> Result<(), BackendError> {
        if !self.service_present || self.service_running {
            return Err(BackendError::Fault(
                "synthetic service cannot start".to_owned(),
            ));
        }
        self.service_running = true;
        self.service_pid = if phase == I2gPhase::Control {
            3001
        } else {
            3002
        };
        self.token_present = true;
        self.active_phase = Some(phase);
        self.enabled_privileges.clear();
        self.mutations.synthetic_service_starts += 1;
        Ok(())
    }

    fn service_stop(&mut self) -> Result<(), BackendError> {
        if self.service_running {
            self.service_running = false;
            self.service_pid = 0;
            self.token_present = false;
            self.active_phase = None;
            self.exact_child = None;
            self.owned_processes.clear();
            self.enabled_privileges.clear();
            self.mutations.synthetic_service_stops += 1;
        }
        Ok(())
    }

    fn service_delete(&mut self) -> Result<(), BackendError> {
        if self.service_running {
            return Err(BackendError::Fault(
                "synthetic delete while service running".to_owned(),
            ));
        }
        if self.service_present {
            self.service_present = false;
            self.mutations.synthetic_service_deletes += 1;
        }
        Ok(())
    }

    fn machine_observation(&self) -> MachineObservation {
        MachineObservation {
            service_present: self.service_present,
            service_running: self.service_running,
            service_pid: self.service_pid,
            token_present: self.token_present,
            owned_process_count: self.owned_processes.len() as u32,
            direct_service_sid_rights: normalized_right_vec(&self.policy.direct_rights),
            exact_child: self.exact_child.clone(),
            owned_processes: self.owned_processes.clone(),
        }
    }

    fn read_policy(&self) -> I2gPolicySnapshot {
        self.policy.clone()
    }

    fn add_policy_right(&mut self, right: &str) -> Result<(), BackendError> {
        if !self.policy.has_right(right) {
            self.policy.direct_rights.push(right.to_owned());
            self.policy.direct_rights = normalized_right_vec(&self.policy.direct_rights);
            self.mutations.synthetic_policy_adds += 1;
            if right.eq_ignore_ascii_case(CONTROL_REQUIRED_RIGHT) {
                self.policy.system_profile_right_added_by_run = true;
            }
            if right.eq_ignore_ascii_case(TREATMENT_RIGHT) {
                self.policy.profile_single_right_added_by_run = true;
            }
        }
        Ok(())
    }

    fn remove_policy_right(&mut self, right: &str) -> Result<(), BackendError> {
        if self.scenario == I2gSyntheticScenario::CleanupFailure
            && right.eq_ignore_ascii_case(TREATMENT_RIGHT)
        {
            return Err(BackendError::Fault(
                "synthetic ProfileSingle removal/readback failure".to_owned(),
            ));
        }
        if self.policy.has_right(right) {
            self.policy
                .direct_rights
                .retain(|value| !value.eq_ignore_ascii_case(right));
            self.policy.direct_rights = normalized_right_vec(&self.policy.direct_rights);
            self.mutations.synthetic_policy_removes += 1;
        }
        Ok(())
    }

    fn token_snapshot(&self, phase: I2gPhase, moment: I2gTokenMoment) -> TokenSnapshot {
        let mut enabled = self.enabled_privileges.clone();
        if phase == I2gPhase::Treatment
            && self.scenario == I2gSyntheticScenario::TokenDeltaFailure
            && moment == I2gTokenMoment::Final
        {
            enabled.insert("seauditprivilege".to_owned());
        }
        let mut snapshot = synthetic_token(phase, moment, self.policy.clone(), enabled);
        if self.scenario == I2gSyntheticScenario::TokenDeltaFailure
            && phase == I2gPhase::Treatment
            && moment == I2gTokenMoment::Final
        {
            snapshot
                .enabled_privileges
                .push("SeAuditPrivilege".to_owned());
            snapshot
                .disabled_privileges
                .retain(|value| value != "SeAuditPrivilege");
            snapshot.enabled_privileges.sort_unstable();
            snapshot.enabled_privileges.dedup();
        }
        if self.scenario == I2gSyntheticScenario::PreControlFailure && phase == I2gPhase::Control {
            snapshot
                .controlled_right_states
                .remove(CONTROL_REQUIRED_RIGHT);
            snapshot
                .disabled_privileges
                .retain(|value| value != CONTROL_REQUIRED_RIGHT);
        }
        if self.scenario == I2gSyntheticScenario::MaterializationFailure
            && phase == I2gPhase::Treatment
        {
            snapshot
                .controlled_right_states
                .insert(TREATMENT_RIGHT.to_owned(), PrivilegeState::Absent);
            snapshot
                .disabled_privileges
                .retain(|value| value != TREATMENT_RIGHT);
        }
        if self.scenario == I2gSyntheticScenario::SystemProfileRegression
            && phase == I2gPhase::Treatment
            && moment != I2gTokenMoment::Pre
        {
            snapshot.controlled_right_states.insert(
                CONTROL_REQUIRED_RIGHT.to_owned(),
                PrivilegeState::PresentDisabled,
            );
            snapshot
                .enabled_privileges
                .retain(|value| value != CONTROL_REQUIRED_RIGHT);
            if !snapshot
                .disabled_privileges
                .iter()
                .any(|value| value == CONTROL_REQUIRED_RIGHT)
            {
                snapshot
                    .disabled_privileges
                    .push(CONTROL_REQUIRED_RIGHT.to_owned());
            }
        }
        snapshot
    }

    fn adjust_token_privilege(&mut self, phase: I2gPhase, privilege: &str) -> AdjustTokenEvidence {
        if self.scenario == I2gSyntheticScenario::ControlTokenGateFailure
            && phase == I2gPhase::Control
            && privilege.eq_ignore_ascii_case(CONTROL_REQUIRED_RIGHT)
        {
            return AdjustTokenEvidence {
                privilege: privilege.to_owned(),
                adjust_token_privileges_returned_true: false,
                get_last_error: AdjustTokenError::ErrorNotAllAssigned,
                post_state: PrivilegeState::PresentDisabled,
                pass: false,
            };
        }
        if self.scenario == I2gSyntheticScenario::SystemProfileRegression
            && phase == I2gPhase::Treatment
            && privilege.eq_ignore_ascii_case(CONTROL_REQUIRED_RIGHT)
        {
            return AdjustTokenEvidence {
                privilege: privilege.to_owned(),
                adjust_token_privileges_returned_true: true,
                get_last_error: AdjustTokenError::Success,
                post_state: PrivilegeState::PresentDisabled,
                pass: false,
            };
        }
        self.enabled_privileges
            .insert(privilege.to_ascii_lowercase());
        let post_state = if self.policy.has_right(privilege) {
            PrivilegeState::PresentEnabled
        } else {
            PrivilegeState::Absent
        };
        AdjustTokenEvidence {
            privilege: privilege.to_owned(),
            adjust_token_privileges_returned_true: true,
            get_last_error: AdjustTokenError::Success,
            post_state,
            pass: post_state == PrivilegeState::PresentEnabled,
        }
    }

    fn amd_identity(&self, _phase: I2gPhase) -> AmdCliIdentity {
        let mut identity = AmdCliIdentity::fixed();
        if self.scenario == I2gSyntheticScenario::IdentityMismatch {
            identity.sha256 = "SYNTHETIC_DRIFTED_SHA256".to_owned();
            identity.identity_gate_pass = false;
        }
        identity
    }

    fn discover(&mut self, phase: I2gPhase) -> DiscoveryEvidence {
        let evidence = self.adapter.launch(phase);
        if evidence.child_spawned {
            self.mutations.synthetic_discovery_launches += 1;
            let mut child = ProcessIdentity::discovery(phase);
            child.run_owned = evidence.owned_child;
            self.exact_child = Some(child.clone());
            self.owned_processes = if child.run_owned {
                vec![child]
            } else {
                Vec::new()
            };
        }
        evidence
    }

    fn mutation_assertions(&self) -> I2gMutationAssertions {
        self.mutations.clone()
    }
}

fn synthetic_token(
    _phase: I2gPhase,
    _moment: I2gTokenMoment,
    policy: I2gPolicySnapshot,
    enabled: BTreeSet<String>,
) -> TokenSnapshot {
    let mut enabled_privileges = SYNTHETIC_ENABLED_PRIVILEGES
        .iter()
        .map(|value| (*value).to_owned())
        .collect::<Vec<_>>();
    let mut disabled_privileges = SYNTHETIC_DISABLED_PRIVILEGES
        .iter()
        .map(|value| (*value).to_owned())
        .collect::<Vec<_>>();
    let mut absent_privileges = SYNTHETIC_ABSENT_PRIVILEGES
        .iter()
        .map(|value| (*value).to_owned())
        .collect::<Vec<_>>();
    let mut controlled_right_states = BTreeMap::new();
    for right in [CONTROL_REQUIRED_RIGHT, TREATMENT_RIGHT] {
        if !policy.has_right(right) {
            controlled_right_states.insert(right.to_owned(), PrivilegeState::Absent);
            absent_privileges.push(right.to_owned());
        } else if enabled.contains(&right.to_ascii_lowercase()) {
            controlled_right_states.insert(right.to_owned(), PrivilegeState::PresentEnabled);
            enabled_privileges.push(right.to_owned());
        } else {
            controlled_right_states.insert(right.to_owned(), PrivilegeState::PresentDisabled);
            disabled_privileges.push(right.to_owned());
        }
    }
    if enabled.contains("sedebugprivilege") {
        enabled_privileges.push("SeDebugPrivilege".to_owned());
        absent_privileges.retain(|value| value != "SeDebugPrivilege");
    }
    for right in [CONTROL_REQUIRED_RIGHT, TREATMENT_RIGHT] {
        if enabled.contains(&right.to_ascii_lowercase()) {
            disabled_privileges.retain(|value| value != right);
            absent_privileges.retain(|value| value != right);
        }
    }
    enabled_privileges.sort_unstable();
    enabled_privileges.dedup();
    disabled_privileges.sort_unstable();
    disabled_privileges.dedup();
    absent_privileges.sort_unstable();
    absent_privileges.dedup();
    TokenSnapshot {
        token_user: I2G_SERVICE_ACCOUNT_SID.to_owned(),
        account_sid: I2G_SERVICE_ACCOUNT_SID.to_owned(),
        service_sid: policy.service_sid,
        service_sid_type: I2G_SERVICE_SID_TYPE.to_owned(),
        session_id: 0,
        architecture: I2G_FIXED_AMD_CLI_ARCHITECTURE.to_owned(),
        integrity: "System".to_owned(),
        elevation: Some("FULLY_TRUSTED_SERVICE".to_owned()),
        relevant_groups: vec![
            "S-1-5-18:ENABLED".to_owned(),
            format!("{}:ENABLED", I2G_SYNTHETIC_SERVICE_SID),
        ],
        administrators_present: false,
        enabled_privileges,
        disabled_privileges,
        absent_privileges,
        controlled_right_states,
        process_pid: if _phase == I2gPhase::Control {
            3001
        } else {
            3002
        },
        process_start_time: if _moment == I2gTokenMoment::Pre {
            2001
        } else {
            2002
        },
    }
}

struct AtomicEvidenceWriter {
    root: Option<PathBuf>,
    written: BTreeSet<String>,
}

impl AtomicEvidenceWriter {
    fn new(root: Option<&Path>) -> Self {
        Self {
            root: root.map(Path::to_path_buf),
            written: BTreeSet::new(),
        }
    }

    fn write<T: Serialize>(&mut self, name: &str, value: &T) -> Result<(), String> {
        if !self.written.insert(name.to_owned()) {
            return Err(format!("evidence file would be overwritten: {name}"));
        }
        if let Some(root) = &self.root {
            let path = root.join(name);
            atomic_write_json(&path, value).map_err(|error| format!("{name}: {error}"))?;
        }
        Ok(())
    }

    fn write_state(&mut self, state: &PersistedI2gState) -> Result<(), String> {
        self.write(&format!("STATE-{:04}.json", state.sequence), state)
    }

    fn contains(&self, name: &str) -> bool {
        self.written.contains(name)
    }

    fn files(&self) -> Vec<String> {
        self.written.iter().cloned().collect()
    }
}

/// Write one immutable JSON evidence object using temp -> flush -> close -> rename semantics.
/// Existing evidence is never silently overwritten, which keeps historical I2E/I2F roots safe.
pub fn atomic_write_json<T: Serialize>(path: &Path, value: &T) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let counter = ATOMIC_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("evidence.json");
    let temp_path = path.with_file_name(format!(".{file_name}.{counter}.tmp"));
    let bytes = serde_json::to_vec_pretty(value)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.to_string()))?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp_path)?;
    file.write_all(&bytes)?;
    file.flush()?;
    file.sync_all()?;
    drop(file);
    if path.exists() {
        let _ = fs::remove_file(&temp_path);
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "evidence target already exists",
        ));
    }
    match fs::rename(&temp_path, path) {
        Ok(()) => Ok(()),
        Err(error) => {
            let _ = fs::remove_file(&temp_path);
            Err(error)
        }
    }
}

fn normalized_set(values: &[String]) -> BTreeSet<String> {
    values
        .iter()
        .map(|value| value.to_ascii_lowercase())
        .collect()
}

fn normalized_right_vec(values: &[String]) -> Vec<String> {
    let mut result = values.to_vec();
    result.sort_by_key(|value| value.to_ascii_lowercase());
    result.dedup_by(|left, right| left.eq_ignore_ascii_case(right));
    result
}

fn write_value(writer: &mut AtomicEvidenceWriter, name: &str, value: Value) -> Result<(), String> {
    writer.write(name, &value)
}

struct RunContext {
    backend: SyntheticBackend,
    persisted: PersistedI2gState,
    writer: AtomicEvidenceWriter,
    checks: Vec<I2gCheck>,
    control_configuration: I2gConfiguration,
    treatment_configuration: I2gConfiguration,
    control_final_token: Option<TokenSnapshot>,
    treatment_final_token: Option<TokenSnapshot>,
    control_result: I2gResult,
    treatment_result: Option<I2gResult>,
    control_drift: bool,
    treatment_allowed: bool,
    failure_detail: Option<String>,
}

impl RunContext {
    fn new(scenario: I2gSyntheticScenario, evidence_root: Option<&Path>) -> Self {
        let backend = SyntheticBackend::new(scenario);
        let control_configuration = I2gConfiguration::control(I2G_SYNTHETIC_SERVICE_SID);
        let treatment_configuration = I2gConfiguration::with_rights(
            I2G_SYNTHETIC_SERVICE_SID,
            vec![
                CONTROL_REQUIRED_RIGHT.to_owned(),
                TREATMENT_RIGHT.to_owned(),
            ],
        );
        Self {
            persisted: PersistedI2gState::new(
                I2G_SERVICE_NAME,
                I2G_SYNTHETIC_SERVICE_SID,
                I2G_HARNESS_SHA256_SYNTHETIC,
            ),
            backend,
            writer: AtomicEvidenceWriter::new(evidence_root),
            checks: Vec::new(),
            control_configuration,
            treatment_configuration,
            control_final_token: None,
            treatment_final_token: None,
            control_result: I2gResult::InvalidNoCausalInterpretation,
            treatment_result: None,
            control_drift: false,
            treatment_allowed: false,
            failure_detail: None,
        }
    }

    fn check(&mut self, name: &str, pass: bool, detail: impl Into<String>) {
        self.checks.push(if pass {
            I2gCheck::pass(name, detail)
        } else {
            I2gCheck::fail(name, detail)
        });
    }

    fn transition(&mut self, next: I2gState) -> Result<(), String> {
        self.persisted.transition(next)?;
        self.writer.write_state(&self.persisted)
    }

    fn checkpoint(&mut self) -> Result<(), String> {
        self.persisted.checkpoint();
        self.writer.write_state(&self.persisted)
    }

    fn begin_service_mutation(&mut self, preexisting: bool) -> Result<(), String> {
        self.persisted
            .service_ownership
            .capture_pre_state(preexisting);
        self.persisted.service_ownership.begin_mutation();
        self.checkpoint()
    }

    fn observe_service_mutation(&mut self, present: bool) -> Result<(), String> {
        self.persisted
            .service_ownership
            .observe_after_mutation(present);
        self.persisted.service_created = self.persisted.service_ownership.owned_by_run;
        self.checkpoint()
    }

    fn begin_policy_mutation(&mut self, right: &str, preexisting: bool) -> Result<(), String> {
        let ownership = if right.eq_ignore_ascii_case(CONTROL_REQUIRED_RIGHT) {
            &mut self.persisted.system_profile_ownership
        } else {
            &mut self.persisted.profile_single_ownership
        };
        ownership.capture_pre_state(preexisting);
        ownership.begin_mutation();
        self.checkpoint()
    }

    fn observe_policy_mutation(&mut self, right: &str, present: bool) -> Result<(), String> {
        let ownership = if right.eq_ignore_ascii_case(CONTROL_REQUIRED_RIGHT) {
            &mut self.persisted.system_profile_ownership
        } else {
            &mut self.persisted.profile_single_ownership
        };
        ownership.observe_after_mutation(present);
        if right.eq_ignore_ascii_case(CONTROL_REQUIRED_RIGHT) {
            self.persisted.system_profile_right_added_by_run = ownership.owned_by_run;
        } else {
            self.persisted.profile_single_right_added_by_run = ownership.owned_by_run;
        }
        self.checkpoint()
    }

    fn begin_discovery(&mut self, phase: I2gPhase) -> Result<(), String> {
        self.persisted.begin_discovery_spawn(phase);
        self.checkpoint()
    }

    fn observe_discovery_spawn(
        &mut self,
        phase: I2gPhase,
        discovery: &DiscoveryEvidence,
    ) -> Result<(), String> {
        if discovery.child_spawned {
            self.persisted.observe_discovery_spawn(
                phase,
                discovery.child_pid,
                discovery.process_start_time,
            );
            self.checkpoint()?;
        }
        Ok(())
    }

    fn observe_discovery_completion(&mut self, phase: I2gPhase) -> Result<(), String> {
        self.persisted.observe_discovery_completion(phase);
        self.checkpoint()
    }

    fn write<T: Serialize>(&mut self, name: &str, value: &T) -> Result<(), String> {
        self.writer.write(name, value)
    }

    fn fail(&mut self, result: I2gResult, detail: impl Into<String>) {
        self.failure_detail = Some(detail.into());
        self.control_result = result;
        self.control_drift = true;
        self.treatment_allowed = false;
    }
}

fn map_discovery_failure(classification: &str) -> I2gResult {
    match classification {
        "IDENTITY_MISMATCH" => I2gResult::IdentityMismatch,
        "TIMEOUT" => I2gResult::Timeout,
        "PROCESS_OWNERSHIP_FAILED" => I2gResult::ProcessOwnershipFailed,
        "POWER_AVAILABLE" => I2gResult::ControlDrift,
        "DISCOVERY_FAILED" => I2gResult::DiscoveryFailed,
        _ => I2gResult::DiscoveryFailed,
    }
}

fn execute_control(ctx: &mut RunContext) -> Result<(), String> {
    let initial_policy = ctx.backend.read_policy();
    ctx.write(
        "CONTROL-POLICY.json",
        &json!({
            "phase": "CONTROL",
            "service_sid": initial_policy.service_sid.clone(),
            "preexisting_system_profile_right": initial_policy.preexisting_system_profile_right,
            "preexisting_profile_single_right": initial_policy.preexisting_profile_single_right,
            "direct_rights_before": initial_policy.direct_rights.clone(),
            "intended_direct_rights": [CONTROL_REQUIRED_RIGHT],
            "mutation": "SeSystemProfilePrivilege only",
        }),
    )?;
    if initial_policy.preexisting_profile_single_right {
        ctx.check(
            "PREEXISTING_PROFILE_SINGLE_RIGHT_REJECTED",
            false,
            "unexpected pre-existing treatment right stops before policy mutation",
        );
        ctx.fail(
            I2gResult::InvalidNoCausalInterpretation,
            "UNEXPECTED_PREEXISTING_TREATMENT_RIGHT",
        );
        return Ok(());
    }
    if ctx.backend.scenario == I2gSyntheticScenario::PreControlFailure {
        ctx.check(
            "PRE_CONTROL_TOKEN_GATE",
            false,
            "synthetic token gate failed before service/policy mutation",
        );
        ctx.fail(
            I2gResult::TokenGateFailed,
            "synthetic pre-CONTROL token gate failure",
        );
        return Ok(());
    }
    let service_preexisting = ctx.backend.machine_observation().service_present;
    if service_preexisting {
        ctx.fail(
            I2gResult::InvalidNoCausalInterpretation,
            "UNEXPECTED_PREEXISTING_SERVICE",
        );
        return Ok(());
    }
    ctx.begin_service_mutation(service_preexisting)?;
    ctx.backend
        .service_create(&ctx.control_configuration)
        .map_err(|error| error.to_string())?;
    ctx.observe_service_mutation(ctx.backend.machine_observation().service_present)?;
    ctx.transition(I2gState::ControlPolicyReady)?;
    ctx.begin_policy_mutation(
        CONTROL_REQUIRED_RIGHT,
        initial_policy.has_right(CONTROL_REQUIRED_RIGHT),
    )?;
    ctx.backend
        .add_policy_right(CONTROL_REQUIRED_RIGHT)
        .map_err(|error| error.to_string())?;
    let policy_after = ctx.backend.read_policy();
    ctx.observe_policy_mutation(
        CONTROL_REQUIRED_RIGHT,
        policy_after.has_right(CONTROL_REQUIRED_RIGHT),
    )?;
    ctx.check(
        "CONTROL_POLICY_EXACT_RIGHT",
        policy_after.direct_rights.len() == 1 && policy_after.has_right(CONTROL_REQUIRED_RIGHT),
        format!(
            "direct Service SID rights = {:?}",
            policy_after.direct_rights
        ),
    );
    ctx.backend
        .service_start(I2gPhase::Control)
        .map_err(|error| error.to_string())?;
    ctx.transition(I2gState::ControlServiceRunning)?;

    let token_pre = ctx
        .backend
        .token_snapshot(I2gPhase::Control, I2gTokenMoment::Pre);
    ctx.write("CONTROL-TOKEN-PRE.json", &token_pre)?;
    let materialized = validate_control_materialized_token(&token_pre);
    ctx.check(
        "CONTROL_MATERIALIZED_TOKEN_GATE",
        materialized.pass,
        format!("{} {:?}", materialized.code, materialized.mismatches),
    );
    if !materialized.pass {
        ctx.fail(
            I2gResult::TokenGateFailed,
            "CONTROL_TOKEN_GATE_FAILED at materialization",
        );
        return Ok(());
    }
    let adjustment = ctx
        .backend
        .adjust_token_privilege(I2gPhase::Control, CONTROL_REQUIRED_RIGHT);
    ctx.check(
        "CONTROL_ADJUST_TOKEN_PRIVILEGES",
        adjustment.pass
            && adjustment.adjust_token_privileges_returned_true
            && adjustment.get_last_error == AdjustTokenError::Success,
        format!("{adjustment:?}"),
    );
    let token_post = ctx
        .backend
        .token_snapshot(I2gPhase::Control, I2gTokenMoment::Final);
    ctx.write("CONTROL-TOKEN-POST.json", &token_post)?;
    let final_gate = validate_control_final_token(&token_post);
    ctx.check(
        "CONTROL_FINAL_TOKEN_GATE",
        adjustment.pass && final_gate.pass,
        format!("{} {:?}", final_gate.code, final_gate.mismatches),
    );
    if !adjustment.pass || !final_gate.pass {
        ctx.fail(
            I2gResult::TokenGateFailed,
            "CONTROL_TOKEN_GATE_FAILED after AdjustTokenPrivileges",
        );
        return Ok(());
    }
    ctx.control_final_token = Some(token_post);
    ctx.transition(I2gState::ControlTokenReady)?;

    let identity = ctx.backend.amd_identity(I2gPhase::Control);
    ctx.write("CONTROL-AMD-IDENTITY.json", &identity)?;
    ctx.check(
        "CONTROL_AMD_IDENTITY_GATE",
        identity.identity_gate_pass,
        format!("path={} sha256={}", identity.path, identity.sha256),
    );
    if !identity.identity_gate_pass {
        ctx.fail(ctx_identity_result(), "AMD_CLI_IDENTITY_MISMATCH");
        return Ok(());
    }
    ctx.transition(I2gState::ControlDiscoveryRunning)?;
    ctx.begin_discovery(I2gPhase::Control)?;
    let discovery = ctx.backend.discover(I2gPhase::Control);
    ctx.observe_discovery_spawn(I2gPhase::Control, &discovery)?;
    if discovery.child_spawned {
        ctx.persisted.record_child_spawn(I2gPhase::Control)?;
        ctx.checkpoint()?;
    }
    ctx.backend.finish_child();
    ctx.observe_discovery_completion(I2gPhase::Control)?;
    ctx.write("CONTROL-DISCOVERY.json", &discovery)?;
    let valid_negative = discovery.classification == "POWER_UNAVAILABLE"
        && discovery.identity_gate_pass
        && !discovery.sampling
        && discovery.child_spawned
        && discovery.actual_run_count_incremented
        && discovery.exit_code == Some(0)
        && discovery.no_counters_diagnostic
        && discovery.owned_child
        && !discovery.orphan_child
        && !discovery.timeout;
    ctx.check(
        "CONTROL_VALID_NEGATIVE_POWER_UNAVAILABLE",
        valid_negative,
        format!("classification={}", discovery.classification),
    );
    if valid_negative {
        ctx.control_result = I2gResult::ProfileSingleInsufficientInPairedI2gContext;
        // CONTROL's scientific result is kept separately as the required negative category.
        ctx.control_drift = false;
    } else {
        ctx.control_result = map_discovery_failure(&discovery.classification);
        ctx.control_drift = true;
        ctx.treatment_allowed = false;
    }
    ctx.transition(I2gState::ControlComplete)?;
    let child_was_owned = discovery.child_spawned && discovery.owned_child;
    let _ = ctx.backend.service_stop();
    let observation = ctx.backend.machine_observation();
    let teardown_pass = !observation.service_running
        && observation.service_pid == 0
        && !observation.token_present
        && observation.owned_process_count == 0
        && observation.exact_child.is_none()
        && observation.owned_processes.is_empty();
    let amd_child_absent = observation.exact_child.is_none()
        && observation.owned_process_count == 0
        && observation.owned_processes.is_empty();
    ctx.write(
        "CONTROL-TEARDOWN.json",
        &json!({
            "control_teardown": if teardown_pass { "PASS" } else { "FAILED" },
            "child_was_owned": child_was_owned,
            "amd_child_absent": amd_child_absent,
            "service_stopped": !observation.service_running,
            "service_pid": observation.service_pid,
            "control_token_gone": !observation.token_present,
            "owned_process_tree_empty": observation.owned_process_count == 0,
            "exact_child_identity_absent": observation.exact_child.is_none(),
        }),
    )?;
    ctx.check(
        "CONTROL_TEARDOWN_BEFORE_TREATMENT_MUTATION",
        teardown_pass,
        format!("{observation:?}"),
    );
    if teardown_pass {
        ctx.transition(I2gState::ControlTeardownComplete)?;
    } else {
        ctx.control_drift = true;
        ctx.treatment_allowed = false;
    }
    Ok(())
}

fn ctx_identity_result() -> I2gResult {
    I2gResult::IdentityMismatch
}

fn execute_treatment(ctx: &mut RunContext) -> Result<(), String> {
    if ctx.control_drift
        || ctx.control_result != I2gResult::ProfileSingleInsufficientInPairedI2gContext
    {
        ctx.treatment_allowed = false;
        return Ok(());
    }
    let before = ctx.backend.read_policy();
    ctx.begin_policy_mutation(TREATMENT_RIGHT, before.has_right(TREATMENT_RIGHT))?;
    ctx.backend
        .add_policy_right(TREATMENT_RIGHT)
        .map_err(|error| error.to_string())?;
    let after_add = ctx.backend.read_policy();
    ctx.observe_policy_mutation(TREATMENT_RIGHT, after_add.has_right(TREATMENT_RIGHT))?;
    if ctx.backend.scenario == I2gSyntheticScenario::ConfigDeltaFailure {
        ctx.backend.inject_extra_configuration_right();
    }
    let after = ctx.backend.read_policy();
    let policy_comparison = compare_policy_delta(&before, &after);
    ctx.treatment_configuration.direct_service_sid_rights = after.direct_rights.clone();
    ctx.write("TREATMENT-POLICY.json", &json!({
        "before": before.clone(),
        "after": after.clone(),
        "comparison": policy_comparison.clone(),
        "rollback_ownership": {
            "system_profile_right_added_by_run": ctx.persisted.system_profile_right_added_by_run,
            "profile_single_right_added_by_run": ctx.persisted.profile_single_right_added_by_run,
        },
    }))?;
    ctx.check(
        "TREATMENT_POLICY_EXACT_ONE_DELTA",
        policy_comparison.pass,
        format!("{policy_comparison:?}"),
    );
    ctx.transition(I2gState::TreatmentPolicyReady)?;
    if !policy_comparison.pass {
        ctx.control_result = I2gResult::InvalidConfigurationDelta;
        ctx.control_drift = true;
        ctx.treatment_allowed = false;
        return Ok(());
    }
    ctx.backend
        .service_start(I2gPhase::Treatment)
        .map_err(|error| error.to_string())?;
    ctx.transition(I2gState::TreatmentServiceRunning)?;
    let treatment_pre = ctx
        .backend
        .token_snapshot(I2gPhase::Treatment, I2gTokenMoment::Pre);
    ctx.write("TREATMENT-TOKEN-PRE.json", &treatment_pre)?;
    let treatment_materialized = validate_treatment_materialized_token(&treatment_pre);
    ctx.check(
        "TREATMENT_MATERIALIZED_TOKEN_GATE",
        treatment_materialized.pass,
        format!(
            "{} {:?}",
            treatment_materialized.code, treatment_materialized.mismatches
        ),
    );
    if !treatment_materialized.pass {
        ctx.treatment_result = Some(I2gResult::TokenGateFailed);
        ctx.control_result = I2gResult::TokenGateFailed;
        ctx.control_drift = true;
        ctx.treatment_allowed = false;
        return Ok(());
    }
    let system_adjustment = ctx
        .backend
        .adjust_token_privilege(I2gPhase::Treatment, CONTROL_REQUIRED_RIGHT);
    let system_token = ctx.backend.token_snapshot(
        I2gPhase::Treatment,
        I2gTokenMoment::AfterSystemProfileEnable,
    );
    ctx.write(
        "TREATMENT-TOKEN-SYSTEMPROFILE.json",
        &json!({
            "enablement": system_adjustment,
            "token": system_token,
        }),
    )?;
    let system_gate = validate_treatment_systemprofile_token(&system_token);
    ctx.check(
        "TREATMENT_SYSTEMPROFILE_ENABLEMENT_GATE",
        system_adjustment.pass && system_gate.pass,
        format!("{} {:?}", system_gate.code, system_gate.mismatches),
    );
    if !system_adjustment.pass || !system_gate.pass {
        ctx.treatment_result = Some(I2gResult::TokenGateFailed);
        ctx.control_result = I2gResult::TokenGateFailed;
        ctx.control_drift = true;
        ctx.treatment_allowed = false;
        return Ok(());
    }
    let profile_adjustment = ctx
        .backend
        .adjust_token_privilege(I2gPhase::Treatment, TREATMENT_RIGHT);
    let treatment_final = ctx
        .backend
        .token_snapshot(I2gPhase::Treatment, I2gTokenMoment::Final);
    ctx.write(
        "TREATMENT-TOKEN-FINAL.json",
        &json!({
            "enablement": profile_adjustment,
            "token": treatment_final,
        }),
    )?;
    let treatment_gate = validate_treatment_final_token(&treatment_final);
    ctx.check(
        "TREATMENT_FINAL_TOKEN_GATE",
        profile_adjustment.pass && treatment_gate.pass,
        format!("{} {:?}", treatment_gate.code, treatment_gate.mismatches),
    );
    if !profile_adjustment.pass || !treatment_gate.pass {
        ctx.treatment_result = Some(I2gResult::TokenGateFailed);
        ctx.control_result = I2gResult::TokenGateFailed;
        ctx.control_drift = true;
        ctx.treatment_allowed = false;
        return Ok(());
    }
    ctx.treatment_final_token = Some(treatment_final.clone());
    ctx.transition(I2gState::TreatmentTokenReady)?;
    let control_token = ctx
        .control_final_token
        .as_ref()
        .ok_or_else(|| "CONTROL final token is missing".to_owned())?;
    let token_comparison = compare_final_tokens(control_token, &treatment_final);
    let configuration_comparison =
        compare_configuration(&ctx.control_configuration, &ctx.treatment_configuration);
    ctx.write("PAIRED-TOKEN-COMPARISON.json", &token_comparison)?;
    ctx.write("PAIRED-CONFIG-COMPARISON.json", &configuration_comparison)?;
    ctx.check(
        "PAIRED_TOKEN_EXACT_ONE_PRIVILEGE_DELTA",
        token_comparison.pass,
        format!("{token_comparison:?}"),
    );
    ctx.check(
        "PAIRED_CONFIGURATION_EXACT_ONE_DELTA",
        configuration_comparison.pass,
        format!("{configuration_comparison:?}"),
    );
    if !token_comparison.pass {
        ctx.treatment_result = Some(I2gResult::InvalidTokenDelta);
        ctx.control_result = I2gResult::InvalidTokenDelta;
        ctx.control_drift = true;
        ctx.treatment_allowed = false;
        return Ok(());
    }
    if !configuration_comparison.pass {
        ctx.treatment_result = Some(I2gResult::InvalidConfigurationDelta);
        ctx.control_result = I2gResult::InvalidConfigurationDelta;
        ctx.control_drift = true;
        ctx.treatment_allowed = false;
        return Ok(());
    }
    ctx.treatment_allowed = true;
    ctx.transition(I2gState::TreatmentDiscoveryRunning)?;
    ctx.begin_discovery(I2gPhase::Treatment)?;
    let discovery = ctx.backend.discover(I2gPhase::Treatment);
    ctx.observe_discovery_spawn(I2gPhase::Treatment, &discovery)?;
    if discovery.child_spawned {
        ctx.persisted.record_child_spawn(I2gPhase::Treatment)?;
        ctx.checkpoint()?;
    }
    ctx.backend.finish_child();
    ctx.observe_discovery_completion(I2gPhase::Treatment)?;
    ctx.write(
        "TREATMENT-AMD-IDENTITY.json",
        &ctx.backend.amd_identity(I2gPhase::Treatment),
    )?;
    ctx.write("TREATMENT-DISCOVERY.json", &discovery)?;
    let result = match discovery.classification.as_str() {
        "POWER_AVAILABLE" => I2gResult::ProfileSingleSufficientInPairedI2gContext,
        "POWER_UNAVAILABLE" => I2gResult::ProfileSingleInsufficientInPairedI2gContext,
        other => map_discovery_failure(other),
    };
    ctx.treatment_result = Some(result);
    ctx.check(
        "TREATMENT_DISCOVERY_SINGLE_NON_SAMPLING_RUN",
        !discovery.sampling && discovery.child_spawned,
        format!("classification={}", discovery.classification),
    );
    ctx.transition(I2gState::TreatmentComplete)?;
    Ok(())
}

fn execute_rollback(ctx: &mut RunContext) -> Result<RollbackEvidence, String> {
    if ctx.persisted.state != I2gState::RollbackRunning {
        ctx.transition(I2gState::RollbackRunning)?;
    }
    let mut events = vec!["STOP_ACCEPTING_WORK".to_owned()];
    let mut cleanup_ok = true;
    ctx.persisted.rollback_cleanup_intent_durable = true;
    ctx.persisted.rollback_step = "ROLLBACK_STARTED".to_owned();
    ctx.checkpoint()?;

    let before_child_cleanup = ctx.backend.machine_observation();
    let child_was_owned = before_child_cleanup
        .exact_child
        .as_ref()
        .is_some_and(|child| child.run_owned);
    events.push("TREATMENT_CHILD_FINISHED_OR_KILLED_EXACT_OWNER".to_owned());
    ctx.backend.finish_child();
    let after_child_cleanup = ctx.backend.machine_observation();
    let exact_child_identity_absent = after_child_cleanup.exact_child.is_none();
    let amd_child_absent = exact_child_identity_absent
        && after_child_cleanup.owned_process_count == 0
        && after_child_cleanup.owned_processes.is_empty();
    ctx.persisted.rollback_step = "CHILD_CLEANUP_OBSERVED".to_owned();
    ctx.checkpoint()?;
    events.push("AMD_CHILD_ABSENT".to_owned());
    let _ = ctx.backend.service_stop();
    let stopped = ctx.backend.machine_observation();
    let service_stopped = !stopped.service_running;
    let service_pid_zero = stopped.service_pid == 0;
    let token_gone = !stopped.token_present;
    let owned_process_tree_empty = stopped.owned_process_count == 0
        && stopped.owned_processes.is_empty()
        && stopped.exact_child.is_none();
    cleanup_ok &= service_stopped
        && service_pid_zero
        && token_gone
        && owned_process_tree_empty
        && amd_child_absent;
    ctx.persisted.rollback_step = "SERVICE_STOP_OBSERVED".to_owned();
    ctx.checkpoint()?;
    events.push("SERVICE_STOPPED".to_owned());
    events.push("SERVICE_PID_ZERO_AND_TOKEN_GONE".to_owned());
    events.push("OWNED_PROCESS_TREE_EMPTY".to_owned());
    events.push("INSPECT_DIRECT_SERVICE_SID_RIGHTS".to_owned());

    let policy = ctx.backend.read_policy();
    ctx.persisted
        .service_ownership
        .reconcile_machine_observation(stopped.service_present);
    ctx.persisted
        .system_profile_ownership
        .reconcile_machine_observation(policy.has_right(CONTROL_REQUIRED_RIGHT));
    ctx.persisted
        .profile_single_ownership
        .reconcile_machine_observation(policy.has_right(TREATMENT_RIGHT));
    ctx.persisted.service_created = ctx.persisted.service_ownership.owned_by_run;
    ctx.persisted.system_profile_right_added_by_run =
        ctx.persisted.system_profile_ownership.owned_by_run;
    ctx.persisted.profile_single_right_added_by_run =
        ctx.persisted.profile_single_ownership.owned_by_run;

    let profile_owned = ctx.persisted.profile_single_ownership.owned_by_run;
    let mut profile_attempted = false;
    let mut profile_verified = !profile_owned;
    if profile_owned {
        profile_attempted = true;
        ctx.persisted.profile_single_ownership.begin_removal();
        ctx.persisted.rollback_step = "PROFILE_SINGLE_REMOVE_INTENT".to_owned();
        ctx.checkpoint()?;
        match ctx.backend.remove_policy_right(TREATMENT_RIGHT) {
            Ok(()) => {
                let rights = ctx.backend.read_policy();
                profile_verified = !rights.has_right(TREATMENT_RIGHT);
                ctx.persisted
                    .profile_single_ownership
                    .observe_after_removal(profile_verified);
                ctx.persisted.rollback_step = "PROFILE_SINGLE_REMOVE_OBSERVED".to_owned();
                ctx.checkpoint()?;
                events.push("REMOVE_PROFILE_SINGLE_DUAL_READBACK".to_owned());
            }
            Err(error) => {
                profile_verified = false;
                cleanup_ok = false;
                events.push(format!("PROFILE_SINGLE_REMOVE_FAILED: {error}"));
            }
        }
    } else {
        events.push("PROFILE_SINGLE_NOT_OWNED_NO_REMOVE".to_owned());
    }
    cleanup_ok &= profile_verified;

    let system_owned = ctx.persisted.system_profile_ownership.owned_by_run;
    let mut system_attempted = false;
    let mut system_verified = !system_owned;
    if system_owned {
        system_attempted = true;
        ctx.persisted.system_profile_ownership.begin_removal();
        ctx.persisted.rollback_step = "SYSTEM_PROFILE_REMOVE_INTENT".to_owned();
        ctx.checkpoint()?;
        match ctx.backend.remove_policy_right(CONTROL_REQUIRED_RIGHT) {
            Ok(()) => {
                let rights = ctx.backend.read_policy();
                system_verified = !rights.has_right(CONTROL_REQUIRED_RIGHT);
                ctx.persisted
                    .system_profile_ownership
                    .observe_after_removal(system_verified);
                ctx.persisted.rollback_step = "SYSTEM_PROFILE_REMOVE_OBSERVED".to_owned();
                ctx.checkpoint()?;
                events.push("REMOVE_SYSTEM_PROFILE_DUAL_READBACK".to_owned());
            }
            Err(error) => {
                system_verified = false;
                cleanup_ok = false;
                events.push(format!("SYSTEM_PROFILE_REMOVE_FAILED: {error}"));
            }
        }
    } else {
        events.push("SYSTEM_PROFILE_NOT_OWNED_NO_REMOVE".to_owned());
    }
    cleanup_ok &= system_verified;

    let service_owned = ctx.persisted.service_ownership.owned_by_run;
    let service_deleted = if service_owned {
        ctx.persisted.rollback_step = "SERVICE_DELETE_INTENT".to_owned();
        ctx.checkpoint()?;
        let deleted = ctx.backend.service_delete().is_ok();
        let service_present = ctx.backend.machine_observation().service_present;
        ctx.persisted
            .service_ownership
            .observe_after_removal(!service_present);
        ctx.persisted.rollback_step = "SERVICE_DELETE_OBSERVED".to_owned();
        ctx.checkpoint()?;
        deleted
    } else {
        true
    };
    let service_absent = !ctx.backend.machine_observation().service_present;
    cleanup_ok &= service_deleted && service_absent;
    events.push("DELETE_SERVICE".to_owned());
    events.push("SERVICE_ABSENT".to_owned());
    events.push("NO_OWNED_AMD_OR_BROKER_PROCESS".to_owned());
    if cleanup_ok {
        ctx.transition(I2gState::RollbackComplete)?;
    } else {
        ctx.check(
            "ROLLBACK_COMPLETE",
            false,
            "cleanup failure is retained separately and invalidates causal admissibility",
        );
        ctx.persisted.transition(I2gState::Failed)?;
        ctx.writer.write_state(&ctx.persisted)?;
    }
    Ok(RollbackEvidence {
        cleanup_result: if cleanup_ok { "PASS" } else { "FAILED" }.to_owned(),
        stop_accepting_work: true,
        child_was_owned,
        amd_child_absent,
        exact_child_identity_absent,
        treatment_child_absent: amd_child_absent,
        service_stopped,
        service_pid_zero,
        token_gone,
        owned_process_tree_empty,
        direct_rights_inspected: true,
        profile_single_remove_attempted: profile_attempted,
        profile_single_remove_verified: profile_verified,
        system_profile_remove_attempted: system_attempted,
        system_profile_remove_verified: system_verified,
        service_deleted,
        service_absent,
        owned_processes_absent: owned_process_tree_empty,
        recovery_required: !cleanup_ok,
        system_profile_right_added_by_run: system_owned,
        profile_single_right_added_by_run: profile_owned,
        events,
    })
}

fn required_evidence_files() -> [&'static str; 19] {
    [
        "EXPERIMENT-MANIFEST.json",
        "PRE-MACHINE-STATE.json",
        "CONTROL-POLICY.json",
        "CONTROL-TOKEN-PRE.json",
        "CONTROL-TOKEN-POST.json",
        "CONTROL-AMD-IDENTITY.json",
        "CONTROL-DISCOVERY.json",
        "CONTROL-TEARDOWN.json",
        "TREATMENT-POLICY.json",
        "TREATMENT-TOKEN-PRE.json",
        "TREATMENT-TOKEN-SYSTEMPROFILE.json",
        "TREATMENT-TOKEN-FINAL.json",
        "TREATMENT-AMD-IDENTITY.json",
        "TREATMENT-DISCOVERY.json",
        "PAIRED-CONFIG-COMPARISON.json",
        "PAIRED-TOKEN-COMPARISON.json",
        "PAIRED-RESULT.json",
        "ROLLBACK.json",
        "FINAL-SUMMARY.json",
    ]
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RecoveryExecutionSummary {
    pub schema: String,
    pub persisted_state: I2gState,
    pub decision: RecoveryDecision,
    pub recovery_required: bool,
    pub discovery_relaunched: bool,
    pub control_counter_discovery_runs: u32,
    pub treatment_counter_discovery_runs: u32,
    pub total_counter_discovery_runs: u32,
    pub cleanup_result: String,
    pub causal_interpretation_valid: bool,
    pub reason: String,
    pub rollback: Option<RollbackEvidence>,
}

/// Load only the latest durable state record.  This function never treats an evidence result as
/// authority for host state; callers must immediately obtain a fresh `MachineObservation` from
/// the backend and reconcile both sources.
pub fn load_latest_persisted_state(root: &Path) -> Result<PersistedI2gState, String> {
    let mut states = Vec::new();
    let entries = fs::read_dir(root).map_err(|error| format!("read persisted state: {error}"))?;
    for entry in entries {
        let entry = entry.map_err(|error| format!("read persisted state entry: {error}"))?;
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.starts_with("STATE-") || !name.ends_with(".json") {
            continue;
        }
        let state: PersistedI2gState = serde_json::from_slice(
            &fs::read(entry.path()).map_err(|error| format!("read {name}: {error}"))?,
        )
        .map_err(|error| format!("parse {name}: {error}"))?;
        states.push(state);
    }
    states
        .into_iter()
        .max_by_key(|state| state.sequence)
        .ok_or_else(|| "no durable I2G state record found".to_owned())
}

fn context_from_recovery(
    persisted: PersistedI2gState,
    backend: SyntheticBackend,
    evidence_root: &Path,
) -> RunContext {
    let mut context = RunContext::new(I2gSyntheticScenario::Happy, Some(evidence_root));
    context.persisted = persisted;
    context.backend = backend;
    context
}

fn resume_treatment_without_discovery(ctx: &mut RunContext) -> Result<(), String> {
    if ctx.persisted.state == I2gState::ControlTeardownComplete {
        let current = ctx.backend.read_policy();
        if !current.has_right(TREATMENT_RIGHT) {
            ctx.begin_policy_mutation(TREATMENT_RIGHT, false)?;
            ctx.backend
                .add_policy_right(TREATMENT_RIGHT)
                .map_err(|error| error.to_string())?;
            let after = ctx.backend.read_policy();
            ctx.observe_policy_mutation(TREATMENT_RIGHT, after.has_right(TREATMENT_RIGHT))?;
        }
        ctx.transition(I2gState::TreatmentPolicyReady)?;
    }
    if ctx.persisted.state == I2gState::TreatmentPolicyReady {
        if !ctx.backend.service_running {
            ctx.backend
                .service_start(I2gPhase::Treatment)
                .map_err(|error| error.to_string())?;
        }
        ctx.transition(I2gState::TreatmentServiceRunning)?;
        // Recovery may materialize a new treatment token, but it must not launch discovery.
        if ctx.backend.token_present {
            ctx.transition(I2gState::TreatmentTokenReady)?;
        }
    }
    Ok(())
}

/// Execute the recovery seam against a synthetic backend after a simulated process restart.
/// The backend is the machine world; the state file is the durable run journal.  No discovery
/// method is called on this path.
pub fn execute_synthetic_recovery(
    evidence_root: &Path,
    backend: &mut SyntheticBackend,
) -> Result<RecoveryExecutionSummary, String> {
    let persisted = load_latest_persisted_state(evidence_root)?;
    let initial_state = persisted.state;
    let launches_before = backend.adapter.launches;
    let mut context = context_from_recovery(persisted, backend.clone(), evidence_root);
    let observation = context.backend.machine_observation();
    let reconciliation = reconcile_persisted_state(&mut context.persisted, &observation);
    context.checkpoint()?;

    let rollback = match reconciliation.decision {
        RecoveryDecision::NoAction => None,
        RecoveryDecision::Invalid => None,
        RecoveryDecision::ResumeTreatmentPolicy | RecoveryDecision::ResumeTreatmentServiceOnly => {
            resume_treatment_without_discovery(&mut context)?;
            Some(execute_rollback(&mut context)?)
        }
        RecoveryDecision::ResumeControlTeardown | RecoveryDecision::ResumeRollback => {
            Some(execute_rollback(&mut context)?)
        }
    };
    *backend = context.backend.clone();
    let discovery_relaunched = backend.adapter.launches != launches_before;
    let cleanup_result = rollback
        .as_ref()
        .map(|value| value.cleanup_result.clone())
        .unwrap_or_else(|| {
            if reconciliation.decision == RecoveryDecision::Invalid {
                "FAILED".to_owned()
            } else {
                "PASS".to_owned()
            }
        });
    // A recovered/crashed pair is never scientifically admissible merely because cleanup passed.
    // The recovery seam is teardown/resume infrastructure, not a discovery completion proof.
    let causal_interpretation_valid = false;
    let recovery_required = reconciliation.decision != RecoveryDecision::NoAction
        || rollback
            .as_ref()
            .is_some_and(|value| value.recovery_required);
    let summary = RecoveryExecutionSummary {
        schema: "amd-i2g-recovery-execution/v1".to_owned(),
        persisted_state: initial_state,
        decision: reconciliation.decision,
        recovery_required,
        discovery_relaunched,
        control_counter_discovery_runs: context.persisted.actual_control_counter_discovery_runs,
        treatment_counter_discovery_runs: context.persisted.actual_treatment_counter_discovery_runs,
        total_counter_discovery_runs: context.persisted.actual_total_counter_discovery_runs,
        cleanup_result,
        causal_interpretation_valid,
        reason: reconciliation.reason,
        rollback,
    };
    atomic_write_json(&evidence_root.join("RECOVERY-SUMMARY.json"), &summary)
        .map_err(|error| format!("RECOVERY-SUMMARY.json: {error}"))?;
    Ok(summary)
}

fn execute_recovery_matrix() -> Vec<I2gCheck> {
    let quiescent = MachineObservation {
        service_present: false,
        service_running: false,
        service_pid: 0,
        token_present: false,
        owned_process_count: 0,
        direct_service_sid_rights: Vec::new(),
        exact_child: None,
        owned_processes: Vec::new(),
    };
    let service_ready = MachineObservation {
        service_present: true,
        service_running: false,
        service_pid: 0,
        token_present: false,
        owned_process_count: 0,
        direct_service_sid_rights: vec![CONTROL_REQUIRED_RIGHT.to_owned()],
        exact_child: None,
        owned_processes: Vec::new(),
    };
    let running = MachineObservation {
        service_present: true,
        service_running: true,
        service_pid: 7001,
        token_present: true,
        owned_process_count: 1,
        direct_service_sid_rights: vec![CONTROL_REQUIRED_RIGHT.to_owned()],
        exact_child: Some(ProcessIdentity::discovery(I2gPhase::Control)),
        owned_processes: vec![ProcessIdentity::discovery(I2gPhase::Control)],
    };
    let treatment_rights = MachineObservation {
        direct_service_sid_rights: vec![
            CONTROL_REQUIRED_RIGHT.to_owned(),
            TREATMENT_RIGHT.to_owned(),
        ],
        ..service_ready.clone()
    };
    let cases = vec![
        (
            I2gState::Prepared,
            quiescent.clone(),
            RecoveryDecision::NoAction,
            "RECOVERY_PREPARED",
        ),
        (
            I2gState::Prepared,
            running.clone(),
            RecoveryDecision::ResumeRollback,
            "RECOVERY_PREPARED_RESIDUE",
        ),
        (
            I2gState::ControlPolicyReady,
            service_ready.clone(),
            RecoveryDecision::ResumeControlTeardown,
            "RECOVERY_CONTROL_POLICY_READY",
        ),
        (
            I2gState::ControlServiceRunning,
            running.clone(),
            RecoveryDecision::ResumeControlTeardown,
            "RECOVERY_CONTROL_SERVICE_RUNNING",
        ),
        (
            I2gState::ControlTokenReady,
            running.clone(),
            RecoveryDecision::ResumeControlTeardown,
            "RECOVERY_CONTROL_TOKEN_READY",
        ),
        (
            I2gState::ControlDiscoveryRunning,
            running.clone(),
            RecoveryDecision::ResumeControlTeardown,
            "RECOVERY_CONTROL_DISCOVERY_RUNNING",
        ),
        (
            I2gState::ControlComplete,
            service_ready.clone(),
            RecoveryDecision::ResumeControlTeardown,
            "RECOVERY_CONTROL_COMPLETE",
        ),
        (
            I2gState::ControlTeardownComplete,
            service_ready.clone(),
            RecoveryDecision::ResumeTreatmentPolicy,
            "RECOVERY_CONTROL_TEARDOWN_COMPLETE",
        ),
        (
            I2gState::TreatmentPolicyReady,
            treatment_rights.clone(),
            RecoveryDecision::ResumeTreatmentServiceOnly,
            "RECOVERY_TREATMENT_POLICY_READY",
        ),
        (
            I2gState::TreatmentServiceRunning,
            running.clone(),
            RecoveryDecision::ResumeRollback,
            "RECOVERY_TREATMENT_SERVICE_RUNNING",
        ),
        (
            I2gState::TreatmentTokenReady,
            running.clone(),
            RecoveryDecision::ResumeRollback,
            "RECOVERY_TREATMENT_TOKEN_READY",
        ),
        (
            I2gState::TreatmentDiscoveryRunning,
            running.clone(),
            RecoveryDecision::ResumeRollback,
            "RECOVERY_TREATMENT_DISCOVERY_RUNNING",
        ),
        (
            I2gState::TreatmentComplete,
            quiescent.clone(),
            RecoveryDecision::ResumeRollback,
            "RECOVERY_TREATMENT_COMPLETE",
        ),
        (
            I2gState::RollbackRunning,
            quiescent.clone(),
            RecoveryDecision::ResumeRollback,
            "RECOVERY_ROLLBACK_RUNNING",
        ),
        (
            I2gState::RollbackComplete,
            quiescent.clone(),
            RecoveryDecision::NoAction,
            "RECOVERY_ROLLBACK_COMPLETE",
        ),
        (
            I2gState::Failed,
            quiescent.clone(),
            RecoveryDecision::ResumeRollback,
            "RECOVERY_FAILED",
        ),
        (
            I2gState::Invalid,
            quiescent,
            RecoveryDecision::Invalid,
            "RECOVERY_INVALID",
        ),
    ];
    cases
        .into_iter()
        .map(|(state, observation, expected, name)| {
            let mut persisted = PersistedI2gState::new(
                I2G_SERVICE_NAME,
                I2G_SYNTHETIC_SERVICE_SID,
                I2G_HARNESS_SHA256_SYNTHETIC,
            );
            persisted.state = state;
            if state == I2gState::TreatmentPolicyReady {
                persisted.profile_single_ownership.mutation_intent_durable = true;
                persisted.profile_single_ownership.owned_by_run = true;
            }
            let reconciliation = reconcile_persisted_state(&mut persisted, &observation);
            if reconciliation.decision == expected {
                I2gCheck::pass(name, format!("decision={:?}", reconciliation.decision))
            } else {
                I2gCheck::fail(
                    name,
                    format!("expected={expected:?} actual={:?}", reconciliation.decision),
                )
            }
        })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum I2gCrashPoint {
    ControlAfterServiceCreateBeforeOwnershipPersistence,
    ControlAfterSystemProfileAddBeforeOwnershipPersistence,
    ControlAfterServiceStart,
    ControlAfterTokenMaterialization,
    ControlAfterSystemProfileEnable,
    ControlAfterDiscoveryChildSpawnBeforeRunCountPersistence,
    ControlAfterDiscoveryExitBeforeResultPersistence,
    ControlDuringTeardown,
    TreatmentAfterProfileSingleAddBeforeOwnershipPersistence,
    TreatmentAfterServiceStart,
    TreatmentAfterTokenMaterialization,
    TreatmentAfterSystemProfileEnable,
    TreatmentAfterProfileSingleEnable,
    TreatmentAfterDiscoveryChildSpawnBeforeRunCountPersistence,
    TreatmentAfterDiscoveryExitBeforeResultPersistence,
    RollbackAfterChildCleanupBeforeServiceStop,
    RollbackAfterServiceStopBeforeRightRemoval,
    RollbackAfterProfileSingleRemoveBeforeDurableCleanupState,
    RollbackAfterSystemProfileRemoveBeforeDurableCleanupState,
    RollbackAfterServiceDeleteBeforeFinalSummary,
}

impl I2gCrashPoint {
    pub const fn all() -> [Self; 20] {
        [
            Self::ControlAfterServiceCreateBeforeOwnershipPersistence,
            Self::ControlAfterSystemProfileAddBeforeOwnershipPersistence,
            Self::ControlAfterServiceStart,
            Self::ControlAfterTokenMaterialization,
            Self::ControlAfterSystemProfileEnable,
            Self::ControlAfterDiscoveryChildSpawnBeforeRunCountPersistence,
            Self::ControlAfterDiscoveryExitBeforeResultPersistence,
            Self::ControlDuringTeardown,
            Self::TreatmentAfterProfileSingleAddBeforeOwnershipPersistence,
            Self::TreatmentAfterServiceStart,
            Self::TreatmentAfterTokenMaterialization,
            Self::TreatmentAfterSystemProfileEnable,
            Self::TreatmentAfterProfileSingleEnable,
            Self::TreatmentAfterDiscoveryChildSpawnBeforeRunCountPersistence,
            Self::TreatmentAfterDiscoveryExitBeforeResultPersistence,
            Self::RollbackAfterChildCleanupBeforeServiceStop,
            Self::RollbackAfterServiceStopBeforeRightRemoval,
            Self::RollbackAfterProfileSingleRemoveBeforeDurableCleanupState,
            Self::RollbackAfterSystemProfileRemoveBeforeDurableCleanupState,
            Self::RollbackAfterServiceDeleteBeforeFinalSummary,
        ]
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ControlAfterServiceCreateBeforeOwnershipPersistence => {
                "CONTROL_AFTER_SERVICE_CREATE_BEFORE_OWNERSHIP_PERSISTENCE"
            }
            Self::ControlAfterSystemProfileAddBeforeOwnershipPersistence => {
                "CONTROL_AFTER_SYSTEM_PROFILE_ADD_BEFORE_OWNERSHIP_PERSISTENCE"
            }
            Self::ControlAfterServiceStart => "CONTROL_AFTER_SERVICE_START",
            Self::ControlAfterTokenMaterialization => "CONTROL_AFTER_TOKEN_MATERIALIZATION",
            Self::ControlAfterSystemProfileEnable => "CONTROL_AFTER_SYSTEM_PROFILE_ENABLE",
            Self::ControlAfterDiscoveryChildSpawnBeforeRunCountPersistence => {
                "CONTROL_AFTER_DISCOVERY_CHILD_SPAWN_BEFORE_RUN_COUNT_PERSISTENCE"
            }
            Self::ControlAfterDiscoveryExitBeforeResultPersistence => {
                "CONTROL_AFTER_DISCOVERY_EXIT_BEFORE_RESULT_PERSISTENCE"
            }
            Self::ControlDuringTeardown => "CONTROL_DURING_TEARDOWN",
            Self::TreatmentAfterProfileSingleAddBeforeOwnershipPersistence => {
                "TREATMENT_AFTER_PROFILE_SINGLE_ADD_BEFORE_OWNERSHIP_PERSISTENCE"
            }
            Self::TreatmentAfterServiceStart => "TREATMENT_AFTER_SERVICE_START",
            Self::TreatmentAfterTokenMaterialization => "TREATMENT_AFTER_TOKEN_MATERIALIZATION",
            Self::TreatmentAfterSystemProfileEnable => "TREATMENT_AFTER_SYSTEM_PROFILE_ENABLE",
            Self::TreatmentAfterProfileSingleEnable => "TREATMENT_AFTER_PROFILE_SINGLE_ENABLE",
            Self::TreatmentAfterDiscoveryChildSpawnBeforeRunCountPersistence => {
                "TREATMENT_AFTER_DISCOVERY_CHILD_SPAWN_BEFORE_RUN_COUNT_PERSISTENCE"
            }
            Self::TreatmentAfterDiscoveryExitBeforeResultPersistence => {
                "TREATMENT_AFTER_DISCOVERY_EXIT_BEFORE_RESULT_PERSISTENCE"
            }
            Self::RollbackAfterChildCleanupBeforeServiceStop => {
                "ROLLBACK_AFTER_CHILD_CLEANUP_BEFORE_SERVICE_STOP"
            }
            Self::RollbackAfterServiceStopBeforeRightRemoval => {
                "ROLLBACK_AFTER_SERVICE_STOP_BEFORE_RIGHT_REMOVAL"
            }
            Self::RollbackAfterProfileSingleRemoveBeforeDurableCleanupState => {
                "ROLLBACK_AFTER_PROFILE_SINGLE_REMOVE_BEFORE_DURABLE_CLEANUP_STATE"
            }
            Self::RollbackAfterSystemProfileRemoveBeforeDurableCleanupState => {
                "ROLLBACK_AFTER_SYSTEM_PROFILE_REMOVE_BEFORE_DURABLE_CLEANUP_STATE"
            }
            Self::RollbackAfterServiceDeleteBeforeFinalSummary => {
                "ROLLBACK_AFTER_SERVICE_DELETE_BEFORE_FINAL_SUMMARY"
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CrashWindowCase {
    pub crash_point: String,
    pub recovery_decision: RecoveryDecision,
    pub recovery_required: bool,
    pub discovery_relaunched: bool,
    pub control_counter_discovery_runs: u32,
    pub treatment_counter_discovery_runs: u32,
    pub total_counter_discovery_runs: u32,
    pub no_power_sampling: bool,
    pub preexisting_right_safety: bool,
    pub run_owned_rights_recovered: bool,
    pub service_ownership_recovered: bool,
    pub child_process_absent: bool,
    pub cleanup_result: String,
    pub causal_interpretation_valid: bool,
    pub pass: bool,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CrashWindowMatrixSummary {
    pub schema: String,
    pub offline_validation: String,
    pub cases: Vec<CrashWindowCase>,
}

struct CrashFixture {
    backend: SyntheticBackend,
    persisted: PersistedI2gState,
    root: PathBuf,
}

impl CrashFixture {
    fn new(root: &Path) -> Result<Self, String> {
        fs::create_dir_all(root).map_err(|error| format!("create crash fixture: {error}"))?;
        let persisted = PersistedI2gState::new(
            I2G_SERVICE_NAME,
            I2G_SYNTHETIC_SERVICE_SID,
            I2G_HARNESS_SHA256_SYNTHETIC,
        );
        atomic_write_json(&root.join("STATE-0000.json"), &persisted)
            .map_err(|error| format!("initial crash state: {error}"))?;
        Ok(Self {
            backend: SyntheticBackend::new(I2gSyntheticScenario::Happy),
            persisted,
            root: root.to_path_buf(),
        })
    }

    fn checkpoint(&mut self) -> Result<(), String> {
        self.persisted.checkpoint();
        atomic_write_json(
            &self
                .root
                .join(format!("STATE-{:04}.json", self.persisted.sequence)),
            &self.persisted,
        )
        .map_err(|error| format!("crash checkpoint: {error}"))
    }

    fn transition(&mut self, next: I2gState) -> Result<(), String> {
        self.persisted.transition(next)?;
        atomic_write_json(
            &self
                .root
                .join(format!("STATE-{:04}.json", self.persisted.sequence)),
            &self.persisted,
        )
        .map_err(|error| format!("crash transition: {error}"))
    }

    fn begin_service_mutation(&mut self) -> Result<(), String> {
        self.persisted.service_ownership.capture_pre_state(false);
        self.persisted.service_ownership.begin_mutation();
        self.checkpoint()
    }

    fn observe_service_mutation(&mut self) -> Result<(), String> {
        self.persisted
            .service_ownership
            .observe_after_mutation(self.backend.service_present);
        self.persisted.service_created = self.persisted.service_ownership.owned_by_run;
        self.checkpoint()
    }

    fn begin_right_mutation(&mut self, right: &str) -> Result<(), String> {
        let ownership = if right.eq_ignore_ascii_case(CONTROL_REQUIRED_RIGHT) {
            &mut self.persisted.system_profile_ownership
        } else {
            &mut self.persisted.profile_single_ownership
        };
        ownership.capture_pre_state(false);
        ownership.begin_mutation();
        self.checkpoint()
    }

    fn observe_right_mutation(&mut self, right: &str) -> Result<(), String> {
        let present = self.backend.policy.has_right(right);
        let owned = {
            let ownership = if right.eq_ignore_ascii_case(CONTROL_REQUIRED_RIGHT) {
                &mut self.persisted.system_profile_ownership
            } else {
                &mut self.persisted.profile_single_ownership
            };
            ownership.observe_after_mutation(present);
            ownership.owned_by_run
        };
        if right.eq_ignore_ascii_case(CONTROL_REQUIRED_RIGHT) {
            self.persisted.system_profile_right_added_by_run = owned;
        } else {
            self.persisted.profile_single_right_added_by_run = owned;
        }
        self.checkpoint()
    }

    fn prepare_control_durable(&mut self) -> Result<(), String> {
        self.begin_service_mutation()?;
        self.backend
            .service_create(&I2gConfiguration::control(I2G_SYNTHETIC_SERVICE_SID))
            .map_err(|error| error.to_string())?;
        self.observe_service_mutation()?;
        self.transition(I2gState::ControlPolicyReady)?;
        self.begin_right_mutation(CONTROL_REQUIRED_RIGHT)?;
        self.backend
            .add_policy_right(CONTROL_REQUIRED_RIGHT)
            .map_err(|error| error.to_string())?;
        self.observe_right_mutation(CONTROL_REQUIRED_RIGHT)
    }

    fn prepare_treatment_policy(&mut self) -> Result<(), String> {
        self.prepare_control_durable()?;
        self.backend
            .service_start(I2gPhase::Control)
            .map_err(|error| error.to_string())?;
        self.transition(I2gState::ControlServiceRunning)?;
        self.transition(I2gState::ControlTokenReady)?;
        self.transition(I2gState::ControlDiscoveryRunning)?;
        self.backend
            .service_stop()
            .map_err(|error| error.to_string())?;
        self.transition(I2gState::ControlComplete)?;
        self.transition(I2gState::ControlTeardownComplete)
    }

    fn prepare_treatment_running(&mut self) -> Result<(), String> {
        self.prepare_treatment_policy()?;
        self.begin_right_mutation(TREATMENT_RIGHT)?;
        self.backend
            .add_policy_right(TREATMENT_RIGHT)
            .map_err(|error| error.to_string())?;
        self.observe_right_mutation(TREATMENT_RIGHT)?;
        self.transition(I2gState::TreatmentPolicyReady)?;
        self.backend
            .service_start(I2gPhase::Treatment)
            .map_err(|error| error.to_string())?;
        self.transition(I2gState::TreatmentServiceRunning)
    }

    fn prepare_discovery_running(&mut self) -> Result<(), String> {
        self.prepare_treatment_running()?;
        self.transition(I2gState::TreatmentTokenReady)?;
        self.transition(I2gState::TreatmentDiscoveryRunning)?;
        self.persisted.begin_discovery_spawn(I2gPhase::Treatment);
        self.checkpoint()?;
        let discovery = self.backend.discover(I2gPhase::Treatment);
        if discovery.child_spawned {
            self.persisted.observe_discovery_spawn(
                I2gPhase::Treatment,
                discovery.child_pid,
                discovery.process_start_time,
            );
            self.checkpoint()?;
            self.persisted.record_child_spawn(I2gPhase::Treatment)?;
            self.checkpoint()?;
        }
        Ok(())
    }

    fn simulate(&mut self, point: I2gCrashPoint) -> Result<(), String> {
        use I2gCrashPoint::*;
        match point {
            ControlAfterServiceCreateBeforeOwnershipPersistence => {
                self.begin_service_mutation()?;
                self.backend
                    .service_create(&I2gConfiguration::control(I2G_SYNTHETIC_SERVICE_SID))
                    .map_err(|error| error.to_string())?;
            }
            ControlAfterSystemProfileAddBeforeOwnershipPersistence => {
                self.prepare_control_durable()?;
                self.begin_right_mutation(CONTROL_REQUIRED_RIGHT)?;
                self.backend
                    .add_policy_right(CONTROL_REQUIRED_RIGHT)
                    .map_err(|error| error.to_string())?;
            }
            ControlAfterServiceStart => {
                self.prepare_control_durable()?;
                self.backend
                    .service_start(I2gPhase::Control)
                    .map_err(|error| error.to_string())?;
            }
            ControlAfterTokenMaterialization => {
                self.prepare_control_durable()?;
                self.backend
                    .service_start(I2gPhase::Control)
                    .map_err(|error| error.to_string())?;
                self.transition(I2gState::ControlServiceRunning)?;
            }
            ControlAfterSystemProfileEnable => {
                self.prepare_control_durable()?;
                self.backend
                    .service_start(I2gPhase::Control)
                    .map_err(|error| error.to_string())?;
                self.transition(I2gState::ControlServiceRunning)?;
                self.backend
                    .adjust_token_privilege(I2gPhase::Control, CONTROL_REQUIRED_RIGHT);
            }
            ControlAfterDiscoveryChildSpawnBeforeRunCountPersistence => {
                self.prepare_control_durable()?;
                self.backend
                    .service_start(I2gPhase::Control)
                    .map_err(|error| error.to_string())?;
                self.transition(I2gState::ControlServiceRunning)?;
                self.transition(I2gState::ControlTokenReady)?;
                self.transition(I2gState::ControlDiscoveryRunning)?;
                self.persisted.begin_discovery_spawn(I2gPhase::Control);
                self.checkpoint()?;
                self.backend.discover(I2gPhase::Control);
            }
            ControlAfterDiscoveryExitBeforeResultPersistence => {
                self.prepare_control_durable()?;
                self.backend
                    .service_start(I2gPhase::Control)
                    .map_err(|error| error.to_string())?;
                self.transition(I2gState::ControlServiceRunning)?;
                self.transition(I2gState::ControlTokenReady)?;
                self.transition(I2gState::ControlDiscoveryRunning)?;
                self.persisted.begin_discovery_spawn(I2gPhase::Control);
                self.checkpoint()?;
                let discovery = self.backend.discover(I2gPhase::Control);
                self.persisted.observe_discovery_spawn(
                    I2gPhase::Control,
                    discovery.child_pid,
                    discovery.process_start_time,
                );
                self.checkpoint()?;
                self.persisted.record_child_spawn(I2gPhase::Control)?;
                self.checkpoint()?;
                self.backend.finish_child();
            }
            ControlDuringTeardown => {
                self.prepare_control_durable()?;
                self.backend
                    .service_start(I2gPhase::Control)
                    .map_err(|error| error.to_string())?;
                self.transition(I2gState::ControlServiceRunning)?;
                self.transition(I2gState::ControlTokenReady)?;
                self.transition(I2gState::ControlDiscoveryRunning)?;
                self.transition(I2gState::ControlComplete)?;
                self.backend
                    .service_stop()
                    .map_err(|error| error.to_string())?;
            }
            TreatmentAfterProfileSingleAddBeforeOwnershipPersistence => {
                self.prepare_treatment_policy()?;
                self.begin_right_mutation(TREATMENT_RIGHT)?;
                self.backend
                    .add_policy_right(TREATMENT_RIGHT)
                    .map_err(|error| error.to_string())?;
            }
            TreatmentAfterServiceStart => {
                self.prepare_treatment_policy()?;
                self.begin_right_mutation(TREATMENT_RIGHT)?;
                self.backend
                    .add_policy_right(TREATMENT_RIGHT)
                    .map_err(|error| error.to_string())?;
                self.observe_right_mutation(TREATMENT_RIGHT)?;
                self.transition(I2gState::TreatmentPolicyReady)?;
                self.backend
                    .service_start(I2gPhase::Treatment)
                    .map_err(|error| error.to_string())?;
            }
            TreatmentAfterTokenMaterialization => {
                self.simulate(TreatmentAfterServiceStart)?;
                self.transition(I2gState::TreatmentServiceRunning)?;
            }
            TreatmentAfterSystemProfileEnable => {
                self.simulate(TreatmentAfterServiceStart)?;
                self.transition(I2gState::TreatmentServiceRunning)?;
                self.backend
                    .adjust_token_privilege(I2gPhase::Treatment, CONTROL_REQUIRED_RIGHT);
            }
            TreatmentAfterProfileSingleEnable => {
                self.simulate(TreatmentAfterSystemProfileEnable)?;
                self.backend
                    .adjust_token_privilege(I2gPhase::Treatment, TREATMENT_RIGHT);
            }
            TreatmentAfterDiscoveryChildSpawnBeforeRunCountPersistence => {
                self.simulate(TreatmentAfterProfileSingleEnable)?;
                self.transition(I2gState::TreatmentTokenReady)?;
                self.transition(I2gState::TreatmentDiscoveryRunning)?;
                self.persisted.begin_discovery_spawn(I2gPhase::Treatment);
                self.checkpoint()?;
                self.backend.discover(I2gPhase::Treatment);
            }
            TreatmentAfterDiscoveryExitBeforeResultPersistence => {
                self.simulate(TreatmentAfterProfileSingleEnable)?;
                self.transition(I2gState::TreatmentTokenReady)?;
                self.transition(I2gState::TreatmentDiscoveryRunning)?;
                self.persisted.begin_discovery_spawn(I2gPhase::Treatment);
                self.checkpoint()?;
                let discovery = self.backend.discover(I2gPhase::Treatment);
                self.persisted.observe_discovery_spawn(
                    I2gPhase::Treatment,
                    discovery.child_pid,
                    discovery.process_start_time,
                );
                self.checkpoint()?;
                self.persisted.record_child_spawn(I2gPhase::Treatment)?;
                self.checkpoint()?;
                self.backend.finish_child();
            }
            RollbackAfterChildCleanupBeforeServiceStop => {
                self.prepare_discovery_running()?;
                self.transition(I2gState::RollbackRunning)?;
                self.backend.finish_child();
            }
            RollbackAfterServiceStopBeforeRightRemoval => {
                self.prepare_discovery_running()?;
                self.transition(I2gState::RollbackRunning)?;
                self.backend.finish_child();
                self.backend
                    .service_stop()
                    .map_err(|error| error.to_string())?;
            }
            RollbackAfterProfileSingleRemoveBeforeDurableCleanupState => {
                self.prepare_discovery_running()?;
                self.transition(I2gState::RollbackRunning)?;
                self.backend.finish_child();
                self.backend
                    .service_stop()
                    .map_err(|error| error.to_string())?;
                self.persisted.profile_single_ownership.begin_removal();
                self.checkpoint()?;
                self.backend
                    .remove_policy_right(TREATMENT_RIGHT)
                    .map_err(|error| error.to_string())?;
            }
            RollbackAfterSystemProfileRemoveBeforeDurableCleanupState => {
                self.simulate(RollbackAfterProfileSingleRemoveBeforeDurableCleanupState)?;
                self.persisted
                    .profile_single_ownership
                    .observe_after_removal(true);
                self.checkpoint()?;
                self.persisted.system_profile_ownership.begin_removal();
                self.checkpoint()?;
                self.backend
                    .remove_policy_right(CONTROL_REQUIRED_RIGHT)
                    .map_err(|error| error.to_string())?;
            }
            RollbackAfterServiceDeleteBeforeFinalSummary => {
                self.simulate(RollbackAfterSystemProfileRemoveBeforeDurableCleanupState)?;
                self.persisted
                    .system_profile_ownership
                    .observe_after_removal(true);
                self.checkpoint()?;
                self.persisted.service_ownership.begin_removal();
                self.checkpoint()?;
                self.backend
                    .service_delete()
                    .map_err(|error| error.to_string())?;
            }
        }
        Ok(())
    }
}

fn verify_preexisting_right_safety() -> Result<bool, String> {
    let root = std::env::temp_dir().join(format!(
        "amd-i2g-preexisting-right-{}",
        ATOMIC_FILE_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&root).map_err(|error| format!("pre-existing fixture: {error}"))?;
    let mut backend = SyntheticBackend::new(I2gSyntheticScenario::Happy);
    backend
        .policy
        .direct_rights
        .push(CONTROL_REQUIRED_RIGHT.to_owned());
    backend.policy.preexisting_system_profile_right = true;
    let mut persisted = PersistedI2gState::new(
        I2G_SERVICE_NAME,
        I2G_SYNTHETIC_SERVICE_SID,
        I2G_HARNESS_SHA256_SYNTHETIC,
    );
    persisted.state = I2gState::RollbackRunning;
    persisted.system_profile_ownership.capture_pre_state(true);
    atomic_write_json(&root.join("STATE-0000.json"), &persisted)
        .map_err(|error| format!("pre-existing state: {error}"))?;
    let result = execute_synthetic_recovery(&root, &mut backend)?;
    let preserved = backend.policy.has_right(CONTROL_REQUIRED_RIGHT)
        && result.cleanup_result == "PASS"
        && !result
            .rollback
            .as_ref()
            .is_some_and(|rollback| rollback.system_profile_remove_attempted);
    let _ = fs::remove_dir_all(&root);
    Ok(preserved)
}

pub fn run_crash_window_matrix(
    evidence_root: Option<&Path>,
) -> Result<CrashWindowMatrixSummary, String> {
    let root = evidence_root
        .map(Path::to_path_buf)
        .unwrap_or_else(|| std::env::temp_dir().join("amd-i2g-crash-window-matrix"));
    fs::create_dir_all(&root).map_err(|error| format!("create crash matrix root: {error}"))?;
    let mut cases = Vec::new();
    for point in I2gCrashPoint::all() {
        let case_root = root.join(point.as_str());
        let mut fixture = CrashFixture::new(&case_root)?;
        fixture.simulate(point)?;
        let mut backend = fixture.backend;
        let recovery = execute_synthetic_recovery(&case_root, &mut backend)?;
        let preexisting_right_safety = verify_preexisting_right_safety()?;
        let run_owned_rights_recovered = !backend.policy.has_right(CONTROL_REQUIRED_RIGHT)
            && !backend.policy.has_right(TREATMENT_RIGHT);
        let service_ownership_recovered =
            !backend.service_present && !backend.service_running && backend.service_pid == 0;
        let child_process_absent = backend.exact_child.is_none()
            && backend.owned_processes.is_empty()
            && backend.machine_observation().owned_process_count == 0;
        let no_power_sampling = backend.mutations.real_power_sampling == 0;
        let max_runs = recovery.control_counter_discovery_runs <= I2G_MAX_CONTROL_RUNS
            && recovery.treatment_counter_discovery_runs <= I2G_MAX_TREATMENT_RUNS
            && recovery.total_counter_discovery_runs <= I2G_MAX_TOTAL_RUNS;
        let pass = recovery.recovery_required
            && !recovery.discovery_relaunched
            && max_runs
            && no_power_sampling
            && preexisting_right_safety
            && run_owned_rights_recovered
            && service_ownership_recovered
            && child_process_absent
            && recovery.cleanup_result == "PASS"
            && !recovery.causal_interpretation_valid;
        cases.push(CrashWindowCase {
            crash_point: point.as_str().to_owned(),
            recovery_decision: recovery.decision,
            recovery_required: recovery.recovery_required,
            discovery_relaunched: recovery.discovery_relaunched,
            control_counter_discovery_runs: recovery.control_counter_discovery_runs,
            treatment_counter_discovery_runs: recovery.treatment_counter_discovery_runs,
            total_counter_discovery_runs: recovery.total_counter_discovery_runs,
            no_power_sampling,
            preexisting_right_safety,
            run_owned_rights_recovered,
            service_ownership_recovered,
            child_process_absent,
            cleanup_result: recovery.cleanup_result,
            causal_interpretation_valid: recovery.causal_interpretation_valid,
            pass,
            detail: recovery.reason,
        });
    }
    let summary = CrashWindowMatrixSummary {
        schema: "amd-i2g-crash-window-matrix/v1".to_owned(),
        offline_validation: if cases.iter().all(|case| case.pass) {
            "PASS".to_owned()
        } else {
            "FAIL".to_owned()
        },
        cases,
    };
    atomic_write_json(&root.join("CRASH-WINDOW-MATRIX.json"), &summary)
        .map_err(|error| format!("CRASH-WINDOW-MATRIX.json: {error}"))?;
    Ok(summary)
}

pub fn run_synthetic(
    scenario: I2gSyntheticScenario,
    evidence_root: Option<&Path>,
) -> Result<I2gSummary, String> {
    if scenario == I2gSyntheticScenario::RecoveryMatrix {
        return run_recovery_matrix_summary(evidence_root);
    }
    let mut ctx = RunContext::new(scenario, evidence_root);
    ctx.writer.write_state(&ctx.persisted)?;
    ctx.write(
        "EXPERIMENT-MANIFEST.json",
        &json!({
            "schema": "amd-i2g-experiment-manifest/v1",
            "qualification_only": true,
            "i2g_variable": I2G_VARIABLE,
            "i2g_selection_confidence": I2G_SELECTION_CONFIDENCE,
            "experiment_shape": I2G_EXPERIMENT_SHAPE,
            "service_name": I2G_SERVICE_NAME,
            "service_account": I2G_SERVICE_ACCOUNT,
            "service_account_sid": I2G_SERVICE_ACCOUNT_SID,
            "service_sid": I2G_SYNTHETIC_SERVICE_SID,
            "service_sid_type": I2G_SERVICE_SID_TYPE,
            "control_harness_sha256": I2G_HARNESS_SHA256_SYNTHETIC,
            "treatment_harness_sha256": I2G_HARNESS_SHA256_SYNTHETIC,
            "fixed_operation": I2G_OPERATION,
            "fixed_cli_arguments": I2G_FIXED_AMD_ARGUMENTS,
            "sampling": false,
            "timeout_ms": I2G_COUNTER_DISCOVERY_TIMEOUT_MS,
            "child_safety_cap_ms": I2G_CHILD_SAFETY_CAP_MS,
            "planned_control_counter_discovery_runs": 1,
            "planned_treatment_counter_discovery_runs": 1,
            "max_total_counter_discovery_runs": I2G_MAX_TOTAL_RUNS,
            "control_retry_allowed": false,
            "treatment_retry_allowed": false,
            "human_real_run_authorization": "NOT_GRANTED",
            "real_execution_allowed": false,
        }),
    )?;
    ctx.write(
        "PRE-MACHINE-STATE.json",
        &json!({
            "service_present": false,
            "service_pid": 0,
            "owned_process_count": 0,
            "direct_service_sid_rights": [],
            "source": "synthetic backend; no host query",
        }),
    )?;

    execute_control(&mut ctx)?;
    if !ctx.control_drift {
        execute_treatment(&mut ctx)?;
    }
    if ctx.control_drift && ctx.control_result == I2gResult::InvalidNoCausalInterpretation {
        ctx.check(
            "CONTROL_DRIFT_STOPS_BEFORE_TREATMENT",
            !ctx.treatment_allowed,
            "treatment policy mutation and service start are blocked",
        );
    } else if ctx.control_drift {
        ctx.check(
            "CONTROL_DRIFT_STOPS_BEFORE_TREATMENT",
            !ctx.treatment_allowed,
            "control drift prevents treatment launch",
        );
    }
    let rollback = execute_rollback(&mut ctx)?;
    ctx.write("ROLLBACK.json", &rollback)?;
    let cleanup_pass = rollback.cleanup_result == "PASS";

    let experiment_result = match ctx.treatment_result {
        Some(I2gResult::ProfileSingleSufficientInPairedI2gContext) => {
            I2gResult::ProfileSingleSufficientInPairedI2gContext
        }
        Some(I2gResult::ProfileSingleInsufficientInPairedI2gContext) => {
            I2gResult::ProfileSingleInsufficientInPairedI2gContext
        }
        Some(other) => other,
        None => ctx.control_result,
    };
    let normalized_result = if !cleanup_pass {
        I2gResult::CleanupFailed
    } else {
        experiment_result
    };
    let causal_valid = cleanup_pass
        && matches!(
            experiment_result,
            I2gResult::ProfileSingleSufficientInPairedI2gContext
                | I2gResult::ProfileSingleInsufficientInPairedI2gContext
        );
    let no_retry = ctx.persisted.actual_control_counter_discovery_runs <= I2G_MAX_CONTROL_RUNS
        && ctx.persisted.actual_treatment_counter_discovery_runs <= I2G_MAX_TREATMENT_RUNS
        && ctx.persisted.actual_total_counter_discovery_runs <= I2G_MAX_TOTAL_RUNS
        && ctx.backend.adapter.launches <= I2G_MAX_TOTAL_RUNS;
    ctx.check(
        "NO_RETRY_RUN_COUNT_INVARIANT",
        no_retry,
        format!(
            "control={} treatment={} total={} launches={}",
            ctx.persisted.actual_control_counter_discovery_runs,
            ctx.persisted.actual_treatment_counter_discovery_runs,
            ctx.persisted.actual_total_counter_discovery_runs,
            ctx.backend.adapter.launches
        ),
    );
    ctx.check(
        "REAL_RUNTIME_ZERO",
        ctx.backend.mutations.amd_real_runtime_during_task == 0
            && ctx.backend.mutations.i2g_real_runtime_during_task == 0
            && ctx.backend.mutations.real_service_create == 0
            && ctx.backend.mutations.real_service_start == 0
            && ctx.backend.mutations.real_service_stop == 0
            && ctx.backend.mutations.real_service_delete == 0
            && ctx.backend.mutations.real_lsa_right_add == 0
            && ctx.backend.mutations.real_lsa_right_remove == 0
            && ctx.backend.mutations.real_adjust_token_privileges == 0
            && ctx.backend.mutations.real_amd_counter_discovery == 0
            && ctx.backend.mutations.real_power_sampling == 0
            && ctx.backend.mutations.real_acl_mutation == 0
            && ctx.backend.mutations.real_registry_mutation == 0,
        "all real host mutation/runtime counters remain zero",
    );
    let expected_result_pass = scenario_expected_result(scenario, normalized_result, &rollback);
    ctx.check(
        "SCENARIO_EXPECTED_RESULT",
        expected_result_pass,
        format!("scenario={scenario:?} result={normalized_result}"),
    );
    for name in required_evidence_files() {
        if !ctx.writer.contains(name)
            && name != "PAIRED-RESULT.json"
            && name != "FINAL-SUMMARY.json"
        {
            write_value(
                &mut ctx.writer,
                name,
                json!({
                    "schema": "amd-i2g-evidence/v1",
                    "status": "NOT_RUN",
                    "reason": "phase gate stopped before this evidence",
                }),
            )?;
        }
    }
    let mut evidence_files = ctx.writer.files();
    for required in ["PAIRED-RESULT.json", "FINAL-SUMMARY.json"] {
        if !evidence_files.iter().any(|name| name == required) {
            evidence_files.push(required.to_owned());
        }
    }
    evidence_files.sort();
    let mutation_assertions = ctx.backend.mutation_assertions();
    let summary = I2gSummary {
        schema: "amd-i2g-offline-summary/v1".to_owned(),
        result: normalized_result.as_str().to_owned(),
        normalized_result: normalized_result.as_str().to_owned(),
        offline_validation: if expected_result_pass && no_retry {
            "PASS".to_owned()
        } else {
            "FAIL".to_owned()
        },
        qualification_only: true,
        i2g_variable: I2G_VARIABLE.to_owned(),
        i2g_selection_confidence: I2G_SELECTION_CONFIDENCE.to_owned(),
        i2g_experiment_shape: I2G_EXPERIMENT_SHAPE.to_owned(),
        experiment_result: experiment_result.as_str().to_owned(),
        control_result: if ctx.control_result
            == I2gResult::ProfileSingleInsufficientInPairedI2gContext
        {
            "POWER_UNAVAILABLE".to_owned()
        } else {
            ctx.control_result.as_str().to_owned()
        },
        treatment_result: ctx.treatment_result.map(|value| match value {
            I2gResult::ProfileSingleSufficientInPairedI2gContext => "POWER_AVAILABLE".to_owned(),
            I2gResult::ProfileSingleInsufficientInPairedI2gContext => {
                "POWER_UNAVAILABLE".to_owned()
            }
            other => other.as_str().to_owned(),
        }),
        control_drift: ctx.control_drift,
        treatment_allowed: ctx.treatment_allowed,
        cleanup_result: rollback.cleanup_result.clone(),
        causal_interpretation_valid: causal_valid,
        actual_control_counter_discovery_runs: ctx.persisted.actual_control_counter_discovery_runs,
        actual_treatment_counter_discovery_runs: ctx
            .persisted
            .actual_treatment_counter_discovery_runs,
        actual_total_counter_discovery_runs: ctx.persisted.actual_total_counter_discovery_runs,
        max_control_counter_discovery_runs: I2G_MAX_CONTROL_RUNS,
        max_treatment_counter_discovery_runs: I2G_MAX_TREATMENT_RUNS,
        max_total_counter_discovery_runs: I2G_MAX_TOTAL_RUNS,
        control_retry_allowed: false,
        treatment_retry_allowed: false,
        power_sampling_runs: I2G_POWER_SAMPLING_RUNS,
        service_name: I2G_SERVICE_NAME.to_owned(),
        service_sid: I2G_SYNTHETIC_SERVICE_SID.to_owned(),
        service_account: I2G_SERVICE_ACCOUNT.to_owned(),
        service_account_sid: I2G_SERVICE_ACCOUNT_SID.to_owned(),
        service_sid_type: I2G_SERVICE_SID_TYPE.to_owned(),
        fixed_amd_cli_path: I2G_FIXED_AMD_CLI_PATH.to_owned(),
        fixed_amd_cli_sha256: I2G_FIXED_AMD_CLI_SHA256.to_owned(),
        fixed_amd_cli_version: I2G_FIXED_AMD_CLI_VERSION.to_owned(),
        fixed_amd_cli_architecture: I2G_FIXED_AMD_CLI_ARCHITECTURE.to_owned(),
        fixed_cli_arguments: I2G_FIXED_AMD_ARGUMENTS
            .iter()
            .map(|value| (*value).to_owned())
            .collect(),
        operation: I2G_OPERATION.to_owned(),
        historical_i2f_role: "PREDECESSOR_EVIDENCE_ONLY".to_owned(),
        historical_i2f_is_active_causal_control: false,
        real_execution_allowed: false,
        real_cleanup_allowed: false,
        human_authorization_recorded: false,
        control_harness_sha256: I2G_HARNESS_SHA256_SYNTHETIC.to_owned(),
        treatment_harness_sha256: I2G_HARNESS_SHA256_SYNTHETIC.to_owned(),
        state: ctx.persisted.state,
        checks: ctx.checks.clone(),
        rollback,
        mutation_assertions,
        evidence_files,
    };
    ctx.write(
        "PAIRED-RESULT.json",
        &json!({
            "experiment_result": summary.experiment_result.clone(),
            "normalized_result": summary.normalized_result.clone(),
            "cleanup_result": summary.cleanup_result.clone(),
            "causal_interpretation_valid": summary.causal_interpretation_valid,
            "control_result": summary.control_result.clone(),
            "treatment_result": summary.treatment_result.clone(),
            "actual_run_counts": {
                "control": summary.actual_control_counter_discovery_runs,
                "treatment": summary.actual_treatment_counter_discovery_runs,
                "total": summary.actual_total_counter_discovery_runs,
            },
        }),
    )?;
    ctx.write("FINAL-SUMMARY.json", &summary)?;
    Ok(summary)
}

fn scenario_expected_result(
    scenario: I2gSyntheticScenario,
    result: I2gResult,
    rollback: &RollbackEvidence,
) -> bool {
    match scenario {
        I2gSyntheticScenario::Happy => {
            result == I2gResult::ProfileSingleSufficientInPairedI2gContext
                && rollback.cleanup_result == "PASS"
        }
        I2gSyntheticScenario::Negative => {
            result == I2gResult::ProfileSingleInsufficientInPairedI2gContext
                && rollback.cleanup_result == "PASS"
        }
        I2gSyntheticScenario::ControlDrift => result == I2gResult::ControlDrift,
        I2gSyntheticScenario::PreControlFailure
        | I2gSyntheticScenario::ControlTokenGateFailure
        | I2gSyntheticScenario::MaterializationFailure
        | I2gSyntheticScenario::SystemProfileRegression => result == I2gResult::TokenGateFailed,
        I2gSyntheticScenario::ConfigDeltaFailure => result == I2gResult::InvalidConfigurationDelta,
        I2gSyntheticScenario::TokenDeltaFailure => result == I2gResult::InvalidTokenDelta,
        I2gSyntheticScenario::ProcessOwnershipFailure => {
            result == I2gResult::ProcessOwnershipFailed
        }
        I2gSyntheticScenario::ControlTimeout | I2gSyntheticScenario::TreatmentTimeout => {
            result == I2gResult::Timeout
        }
        I2gSyntheticScenario::CleanupFailure => {
            result == I2gResult::CleanupFailed && rollback.cleanup_result == "FAILED"
        }
        I2gSyntheticScenario::IdentityMismatch => result == I2gResult::IdentityMismatch,
        I2gSyntheticScenario::SpawnFailure | I2gSyntheticScenario::ExitNonzero => {
            result == I2gResult::DiscoveryFailed
        }
        I2gSyntheticScenario::UnexpectedPreexistingProfileRight => {
            result == I2gResult::InvalidNoCausalInterpretation
        }
        I2gSyntheticScenario::RecoveryMatrix | I2gSyntheticScenario::CrashWindowMatrix => false,
    }
}

fn run_recovery_matrix_summary(evidence_root: Option<&Path>) -> Result<I2gSummary, String> {
    let checks = execute_recovery_matrix();
    let pass = checks.iter().all(|check| check.status == "PASS");
    let rollback = RollbackEvidence {
        cleanup_result: "PASS".to_owned(),
        stop_accepting_work: true,
        child_was_owned: false,
        amd_child_absent: true,
        exact_child_identity_absent: true,
        treatment_child_absent: true,
        service_stopped: true,
        service_pid_zero: true,
        token_gone: true,
        owned_process_tree_empty: true,
        direct_rights_inspected: true,
        profile_single_remove_attempted: false,
        profile_single_remove_verified: true,
        system_profile_remove_attempted: false,
        system_profile_remove_verified: true,
        service_deleted: false,
        service_absent: true,
        owned_processes_absent: true,
        recovery_required: false,
        system_profile_right_added_by_run: false,
        profile_single_right_added_by_run: false,
        events: vec!["SYNTHETIC_RECOVERY_ONLY_NO_DISCOVERY_RUN".to_owned()],
    };
    let summary = I2gSummary {
        schema: "amd-i2g-offline-summary/v1".to_owned(),
        result: I2gResult::InvalidNoCausalInterpretation.as_str().to_owned(),
        normalized_result: I2gResult::InvalidNoCausalInterpretation.as_str().to_owned(),
        offline_validation: if pass { "PASS" } else { "FAIL" }.to_owned(),
        qualification_only: true,
        i2g_variable: I2G_VARIABLE.to_owned(),
        i2g_selection_confidence: I2G_SELECTION_CONFIDENCE.to_owned(),
        i2g_experiment_shape: I2G_EXPERIMENT_SHAPE.to_owned(),
        experiment_result: I2gResult::InvalidNoCausalInterpretation.as_str().to_owned(),
        control_result: "NOT_RUN".to_owned(),
        treatment_result: None,
        control_drift: false,
        treatment_allowed: false,
        cleanup_result: "PASS".to_owned(),
        causal_interpretation_valid: false,
        actual_control_counter_discovery_runs: 0,
        actual_treatment_counter_discovery_runs: 0,
        actual_total_counter_discovery_runs: 0,
        max_control_counter_discovery_runs: I2G_MAX_CONTROL_RUNS,
        max_treatment_counter_discovery_runs: I2G_MAX_TREATMENT_RUNS,
        max_total_counter_discovery_runs: I2G_MAX_TOTAL_RUNS,
        control_retry_allowed: false,
        treatment_retry_allowed: false,
        power_sampling_runs: 0,
        service_name: I2G_SERVICE_NAME.to_owned(),
        service_sid: I2G_SYNTHETIC_SERVICE_SID.to_owned(),
        service_account: I2G_SERVICE_ACCOUNT.to_owned(),
        service_account_sid: I2G_SERVICE_ACCOUNT_SID.to_owned(),
        service_sid_type: I2G_SERVICE_SID_TYPE.to_owned(),
        fixed_amd_cli_path: I2G_FIXED_AMD_CLI_PATH.to_owned(),
        fixed_amd_cli_sha256: I2G_FIXED_AMD_CLI_SHA256.to_owned(),
        fixed_amd_cli_version: I2G_FIXED_AMD_CLI_VERSION.to_owned(),
        fixed_amd_cli_architecture: I2G_FIXED_AMD_CLI_ARCHITECTURE.to_owned(),
        fixed_cli_arguments: I2G_FIXED_AMD_ARGUMENTS
            .iter()
            .map(|value| (*value).to_owned())
            .collect(),
        operation: I2G_OPERATION.to_owned(),
        historical_i2f_role: "PREDECESSOR_EVIDENCE_ONLY".to_owned(),
        historical_i2f_is_active_causal_control: false,
        real_execution_allowed: false,
        real_cleanup_allowed: false,
        human_authorization_recorded: false,
        control_harness_sha256: I2G_HARNESS_SHA256_SYNTHETIC.to_owned(),
        treatment_harness_sha256: I2G_HARNESS_SHA256_SYNTHETIC.to_owned(),
        state: I2gState::RollbackComplete,
        checks,
        rollback,
        mutation_assertions: I2gMutationAssertions::default(),
        evidence_files: Vec::new(),
    };
    if let Some(root) = evidence_root {
        let mut writer = AtomicEvidenceWriter::new(Some(root));
        writer.write("RECOVERY-MATRIX.json", &summary)?;
        writer.write("FINAL-SUMMARY.json", &summary)?;
    }
    Ok(summary)
}

pub fn run_synthetic_cleanup() -> I2gCleanupSyntheticSummary {
    let rollback_order = vec![
        "stop accepting work",
        "finish/kill exact treatment AMD child",
        "prove AMD child absent",
        "stop service",
        "prove PID=0 / token gone",
        "prove owned process tree empty",
        "inspect direct Service SID rights",
        "remove ProfileSingle if added by run",
        "verify ProfileSingle dual readback",
        "remove SystemProfile if added by run",
        "verify SystemProfile dual readback",
        "delete service",
        "verify service absent",
        "verify no owned AMD/broker process",
        "persist final rollback evidence",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect::<Vec<_>>();
    let checks = vec![
        I2gCheck::pass(
            "OWNERSHIP_SCOPED_RIGHT_REMOVAL",
            "only independently run-owned rights are removable",
        ),
        I2gCheck::pass(
            "NO_REMOVE_ALL_RIGHTS",
            "the synthetic cleanup plan never uses all=true",
        ),
        I2gCheck::pass(
            "ROLLBACK_ORDER",
            "process/service teardown precedes either policy removal",
        ),
    ];
    I2gCleanupSyntheticSummary {
        schema: "amd-i2g-synthetic-cleanup/v1".to_owned(),
        result: "PASS".to_owned(),
        cleanup_result: "PASS".to_owned(),
        qualification_only: true,
        real_cleanup_allowed: false,
        rollback_order,
        checks,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn run(scenario: I2gSyntheticScenario) -> I2gSummary {
        run_synthetic(scenario, None).expect("synthetic I2G run")
    }

    #[test]
    fn state_machine_is_monotonic_and_terminal() {
        let mut state = PersistedI2gState::new(
            I2G_SERVICE_NAME,
            I2G_SYNTHETIC_SERVICE_SID,
            I2G_HARNESS_SHA256_SYNTHETIC,
        );
        assert!(state.transition(I2gState::ControlPolicyReady).is_ok());
        assert!(state.transition(I2gState::ControlServiceRunning).is_ok());
        assert!(state.transition(I2gState::Prepared).is_err());
        assert!(state.transition(I2gState::RollbackRunning).is_ok());
        assert!(state.transition(I2gState::RollbackComplete).is_ok());
        assert!(state.transition(I2gState::Prepared).is_err());

        let mut failed = PersistedI2gState::new(
            I2G_SERVICE_NAME,
            I2G_SYNTHETIC_SERVICE_SID,
            I2G_HARNESS_SHA256_SYNTHETIC,
        );
        failed.state = I2gState::Failed;
        assert!(failed.transition(I2gState::RollbackRunning).is_ok());

        let mut completed = PersistedI2gState::new(
            I2G_SERVICE_NAME,
            I2G_SYNTHETIC_SERVICE_SID,
            I2G_HARNESS_SHA256_SYNTHETIC,
        );
        completed.state = I2gState::RollbackComplete;
        assert!(completed.transition(I2gState::RollbackRunning).is_ok());
    }

    #[test]
    fn happy_path_has_exact_pair_and_cleanup() {
        let summary = run(I2gSyntheticScenario::Happy);
        assert_eq!(
            summary.normalized_result,
            "PROFILE_SINGLE_SUFFICIENT_IN_PAIRED_I2G_CONTEXT"
        );
        assert_eq!(summary.cleanup_result, "PASS");
        assert_eq!(summary.actual_control_counter_discovery_runs, 1);
        assert_eq!(summary.actual_treatment_counter_discovery_runs, 1);
        assert_eq!(summary.actual_total_counter_discovery_runs, 2);
        assert!(summary.causal_interpretation_valid);
    }

    #[test]
    fn negative_path_preserves_insufficient_result() {
        let summary = run(I2gSyntheticScenario::Negative);
        assert_eq!(
            summary.normalized_result,
            "PROFILE_SINGLE_INSUFFICIENT_IN_PAIRED_I2G_CONTEXT"
        );
        assert_eq!(summary.actual_total_counter_discovery_runs, 2);
        assert_eq!(summary.cleanup_result, "PASS");
    }

    #[test]
    fn control_drift_blocks_treatment_and_removes_only_system_profile() {
        let summary = run(I2gSyntheticScenario::ControlDrift);
        assert_eq!(summary.normalized_result, "CONTROL_DRIFT");
        assert!(summary.control_drift);
        assert_eq!(summary.actual_control_counter_discovery_runs, 1);
        assert_eq!(summary.actual_treatment_counter_discovery_runs, 0);
        assert_eq!(summary.actual_total_counter_discovery_runs, 1);
        assert!(!summary.treatment_allowed);
        assert!(!summary.rollback.profile_single_remove_attempted);
        assert!(summary.rollback.system_profile_remove_attempted);
    }

    #[test]
    fn pre_control_failure_has_zero_runs_and_no_policy_rollback() {
        let summary = run(I2gSyntheticScenario::PreControlFailure);
        assert_eq!(summary.normalized_result, "TOKEN_GATE_FAILED");
        assert_eq!(summary.actual_total_counter_discovery_runs, 0);
        assert!(!summary.rollback.profile_single_remove_attempted);
        assert!(!summary.rollback.system_profile_remove_attempted);
    }

    #[test]
    fn configuration_delta_failure_never_starts_treatment() {
        let summary = run(I2gSyntheticScenario::ConfigDeltaFailure);
        assert_eq!(summary.normalized_result, "INVALID_CONFIGURATION_DELTA");
        assert_eq!(summary.actual_treatment_counter_discovery_runs, 0);
        assert_eq!(summary.mutation_assertions.synthetic_service_starts, 1);
    }

    #[test]
    fn token_delta_failure_blocks_treatment_discovery() {
        let summary = run(I2gSyntheticScenario::TokenDeltaFailure);
        assert_eq!(summary.normalized_result, "INVALID_TOKEN_DELTA");
        assert_eq!(summary.actual_treatment_counter_discovery_runs, 0);
    }

    #[test]
    fn materialization_and_system_profile_regressions_are_token_failures() {
        assert_eq!(
            run(I2gSyntheticScenario::MaterializationFailure).normalized_result,
            "TOKEN_GATE_FAILED"
        );
        assert_eq!(
            run(I2gSyntheticScenario::SystemProfileRegression).normalized_result,
            "TOKEN_GATE_FAILED"
        );
    }

    #[test]
    fn ownership_and_timeout_faults_count_only_spawned_children() {
        let ownership = run(I2gSyntheticScenario::ProcessOwnershipFailure);
        assert_eq!(ownership.normalized_result, "PROCESS_OWNERSHIP_FAILED");
        assert_eq!(ownership.actual_total_counter_discovery_runs, 1);
        let identity = run(I2gSyntheticScenario::IdentityMismatch);
        assert_eq!(identity.normalized_result, "IDENTITY_MISMATCH");
        assert_eq!(identity.actual_total_counter_discovery_runs, 0);
        let timeout = run(I2gSyntheticScenario::ControlTimeout);
        assert_eq!(timeout.normalized_result, "TIMEOUT");
        assert_eq!(timeout.actual_total_counter_discovery_runs, 1);
    }

    #[test]
    fn treatment_timeout_has_no_retry() {
        let summary = run(I2gSyntheticScenario::TreatmentTimeout);
        assert_eq!(summary.normalized_result, "TIMEOUT");
        assert_eq!(summary.actual_control_counter_discovery_runs, 1);
        assert_eq!(summary.actual_treatment_counter_discovery_runs, 1);
        assert_eq!(summary.actual_total_counter_discovery_runs, 2);
    }

    #[test]
    fn cleanup_failure_is_separate_from_scientific_result() {
        let summary = run(I2gSyntheticScenario::CleanupFailure);
        assert_eq!(summary.normalized_result, "CLEANUP_FAILED");
        assert_eq!(summary.cleanup_result, "FAILED");
        assert!(!summary.causal_interpretation_valid);
        assert_eq!(
            summary.experiment_result,
            "PROFILE_SINGLE_SUFFICIENT_IN_PAIRED_I2G_CONTEXT"
        );
    }

    #[test]
    fn final_token_comparator_allows_only_profile_single_delta() {
        let summary = run(I2gSyntheticScenario::Happy);
        assert!(summary.checks.iter().any(|check| check.name
            == "PAIRED_TOKEN_EXACT_ONE_PRIVILEGE_DELTA"
            && check.status == "PASS"));
        let mut control = synthetic_token(
            I2gPhase::Control,
            I2gTokenMoment::Final,
            I2gPolicySnapshot {
                service_sid: I2G_SYNTHETIC_SERVICE_SID.to_owned(),
                direct_rights: vec![CONTROL_REQUIRED_RIGHT.to_owned()],
                preexisting_system_profile_right: false,
                preexisting_profile_single_right: false,
                system_profile_right_added_by_run: true,
                profile_single_right_added_by_run: false,
            },
            BTreeSet::from([CONTROL_REQUIRED_RIGHT.to_ascii_lowercase()]),
        );
        let mut treatment = control.clone();
        treatment
            .controlled_right_states
            .insert(TREATMENT_RIGHT.to_owned(), PrivilegeState::PresentEnabled);
        treatment
            .enabled_privileges
            .push(TREATMENT_RIGHT.to_owned());
        treatment.enabled_privileges.sort_unstable();
        assert!(compare_final_tokens(&control, &treatment).pass);
        control.integrity = "High".to_owned();
        assert!(!compare_final_tokens(&control, &treatment).pass);
    }

    #[test]
    fn configuration_comparator_rejects_extra_change() {
        let control = I2gConfiguration::control(I2G_SYNTHETIC_SERVICE_SID);
        let mut treatment = I2gConfiguration::with_rights(
            I2G_SYNTHETIC_SERVICE_SID,
            vec![
                CONTROL_REQUIRED_RIGHT.to_owned(),
                TREATMENT_RIGHT.to_owned(),
            ],
        );
        assert!(compare_configuration(&control, &treatment).pass);
        treatment.timeout_ms += 1;
        assert!(!compare_configuration(&control, &treatment).pass);
    }

    #[test]
    fn recovery_uses_machine_observation_not_file_existence() {
        let actual = MachineObservation {
            service_present: true,
            service_running: false,
            service_pid: 0,
            token_present: false,
            owned_process_count: 0,
            direct_service_sid_rights: vec![CONTROL_REQUIRED_RIGHT.to_owned()],
            exact_child: None,
            owned_processes: Vec::new(),
        };
        let mut persisted = PersistedI2gState::new(
            I2G_SERVICE_NAME,
            I2G_SYNTHETIC_SERVICE_SID,
            I2G_HARNESS_SHA256_SYNTHETIC,
        );
        persisted.state = I2gState::ControlComplete;
        assert_eq!(
            reconcile_persisted_state(&mut persisted, &actual).decision,
            RecoveryDecision::ResumeControlTeardown
        );
        let inconsistent = MachineObservation {
            service_running: true,
            service_pid: 99,
            token_present: true,
            owned_process_count: 1,
            ..actual
        };
        assert_eq!(
            reconcile_persisted_state(&mut persisted, &inconsistent).decision,
            RecoveryDecision::ResumeRollback
        );
    }

    #[test]
    fn recovery_resumes_durable_treatment_policy_without_discovery() {
        let actual = MachineObservation {
            service_present: true,
            service_running: false,
            service_pid: 0,
            token_present: false,
            owned_process_count: 0,
            direct_service_sid_rights: vec![
                CONTROL_REQUIRED_RIGHT.to_owned(),
                TREATMENT_RIGHT.to_owned(),
            ],
            exact_child: None,
            owned_processes: Vec::new(),
        };
        let mut persisted = PersistedI2gState::new(
            I2G_SERVICE_NAME,
            I2G_SYNTHETIC_SERVICE_SID,
            I2G_HARNESS_SHA256_SYNTHETIC,
        );
        persisted.state = I2gState::TreatmentPolicyReady;
        persisted.profile_single_ownership.mutation_intent_durable = true;
        persisted.profile_single_ownership.owned_by_run = true;
        assert_eq!(
            reconcile_persisted_state(&mut persisted, &actual).decision,
            RecoveryDecision::ResumeTreatmentServiceOnly
        );

        let mut preexisting = persisted.clone();
        preexisting.profile_single_ownership.preexisting = true;
        preexisting.profile_single_ownership.owned_by_run = false;
        assert_eq!(
            reconcile_persisted_state(&mut preexisting, &actual).decision,
            RecoveryDecision::ResumeRollback
        );
    }

    #[test]
    fn rollback_complete_with_run_owned_rights_requires_recovery() {
        let actual = MachineObservation {
            service_present: false,
            service_running: false,
            service_pid: 0,
            token_present: false,
            owned_process_count: 0,
            direct_service_sid_rights: vec![CONTROL_REQUIRED_RIGHT.to_owned()],
            exact_child: None,
            owned_processes: Vec::new(),
        };
        let mut persisted = PersistedI2gState::new(
            I2G_SERVICE_NAME,
            I2G_SYNTHETIC_SERVICE_SID,
            I2G_HARNESS_SHA256_SYNTHETIC,
        );
        persisted.state = I2gState::RollbackComplete;
        persisted.system_profile_ownership.mutation_intent_durable = true;
        assert_eq!(
            reconcile_persisted_state(&mut persisted, &actual).decision,
            RecoveryDecision::ResumeRollback
        );
    }

    #[test]
    fn recovery_consumes_spawn_budget_from_durable_journal() {
        let running = MachineObservation {
            service_present: true,
            service_running: true,
            service_pid: 3001,
            token_present: true,
            owned_process_count: 1,
            direct_service_sid_rights: vec![CONTROL_REQUIRED_RIGHT.to_owned()],
            exact_child: Some(ProcessIdentity::discovery(I2gPhase::Control)),
            owned_processes: vec![ProcessIdentity::discovery(I2gPhase::Control)],
        };
        let mut persisted = PersistedI2gState::new(
            I2G_SERVICE_NAME,
            I2G_SYNTHETIC_SERVICE_SID,
            I2G_HARNESS_SHA256_SYNTHETIC,
        );
        persisted.state = I2gState::ControlDiscoveryRunning;
        persisted.control_discovery_ledger.spawn_intent_durable = true;
        persisted.control_discovery_ledger.spawn_observed_durable = true;
        let reconciliation = reconcile_persisted_state(&mut persisted, &running);
        assert_eq!(
            reconciliation.recovered_control_counter_discovery_runs,
            I2G_MAX_CONTROL_RUNS
        );
        assert_eq!(reconciliation.recovered_total_counter_discovery_runs, 1);
        assert_eq!(
            reconciliation.decision,
            RecoveryDecision::ResumeControlTeardown
        );

        let completed = MachineObservation {
            exact_child: None,
            owned_processes: Vec::new(),
            service_running: false,
            service_pid: 0,
            token_present: false,
            owned_process_count: 0,
            ..running.clone()
        };
        let mut completed_persisted = persisted.clone();
        completed_persisted.actual_control_counter_discovery_runs = 0;
        completed_persisted.actual_total_counter_discovery_runs = 0;
        completed_persisted
            .control_discovery_ledger
            .completion_observed_durable = true;
        let completed_reconciliation =
            reconcile_persisted_state(&mut completed_persisted, &completed);
        assert_eq!(
            completed_reconciliation.recovered_control_counter_discovery_runs,
            I2G_MAX_CONTROL_RUNS
        );
    }

    #[test]
    fn executable_recovery_reopens_failed_cleanup_without_discovery() {
        let root = std::env::temp_dir().join(format!(
            "i2g-recovery-failed-{}-{}",
            std::process::id(),
            ATOMIC_FILE_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).expect("create recovery fixture");
        let mut persisted = PersistedI2gState::new(
            I2G_SERVICE_NAME,
            I2G_SYNTHETIC_SERVICE_SID,
            I2G_HARNESS_SHA256_SYNTHETIC,
        );
        persisted.state = I2gState::Failed;
        persisted.service_ownership.mutation_intent_durable = true;
        persisted.system_profile_ownership.mutation_intent_durable = true;
        persisted.profile_single_ownership.mutation_intent_durable = true;
        atomic_write_json(&root.join("STATE-0000.json"), &persisted)
            .expect("write failed recovery state");

        let mut backend = SyntheticBackend::new(I2gSyntheticScenario::Happy);
        backend.service_present = true;
        backend.service_running = true;
        backend.service_pid = 3002;
        backend.token_present = true;
        backend.policy.direct_rights.extend([
            CONTROL_REQUIRED_RIGHT.to_owned(),
            TREATMENT_RIGHT.to_owned(),
        ]);
        let summary = execute_synthetic_recovery(&root, &mut backend)
            .expect("failed recovery should execute rollback");
        assert_eq!(summary.decision, RecoveryDecision::ResumeRollback);
        assert!(summary.recovery_required);
        assert!(!summary.discovery_relaunched);
        assert_eq!(summary.cleanup_result, "PASS");
        assert!(!backend.service_present);
        assert!(!backend.policy.has_right(CONTROL_REQUIRED_RIGHT));
        assert!(!backend.policy.has_right(TREATMENT_RIGHT));
        assert!(!summary.causal_interpretation_valid);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn real_backend_discovery_is_fail_closed_without_spawn() {
        let mut backend = RealWindowsI2gBackend;
        let evidence = backend.discover(I2gPhase::Control);
        assert!(!evidence.child_spawned);
        assert!(!evidence.actual_run_count_incremented);
        assert!(!evidence.identity_gate_pass);
        assert_eq!(evidence.classification, "REAL_EXECUTION_NOT_AUTHORIZED");
    }

    #[test]
    fn atomic_writer_does_not_overwrite_existing_evidence() {
        let root = std::env::temp_dir().join(format!(
            "i2g-atomic-test-{}-{}",
            std::process::id(),
            ATOMIC_FILE_COUNTER.load(Ordering::Relaxed)
        ));
        let path = root.join("evidence.json");
        atomic_write_json(&path, &json!({"value": 1})).expect("first write");
        assert!(atomic_write_json(&path, &json!({"value": 2})).is_err());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn cleanup_plan_is_offline_and_fail_closed() {
        let summary = run_synthetic_cleanup();
        assert_eq!(summary.result, "PASS");
        assert!(!summary.real_cleanup_allowed);
        assert_eq!(summary.rollback_order.len(), 15);
    }
}
