use serde::{Deserialize, Serialize};

/// A guest-env-var ← host-source secret mapping authored at create/run time
/// (smolvm >= 1.0.0). The value is resolved on the host at launch and never
/// persisted to the machine record or a pack.
///
/// Maps directly onto smolvm's repeatable flags:
/// - `source == "env"`  → `--secret-env  GUEST=HOST_VAR`
/// - `source == "file"` → `--secret-file GUEST=/abs/path`
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SecretRefMapping {
    /// Guest-side env var the secret is injected as (the `KEY`).
    pub env_name: String,
    /// Host source kind: "env" | "file".
    pub source: String,
    /// Host env var name (source == "env") or absolute host file path
    /// (source == "file").
    pub value: String,
}

/// A secret reference persisted on a machine record, surfaced by
/// `machine ls --json` as `secret_refs` (smolvm >= 0.9.0). Read-only view.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SecretRefView {
    /// Guest-side env var name.
    pub env_name: String,
    /// Where the value comes from: "env" | "file".
    pub source: String,
    /// The ref target: host env var name or file path.
    pub target: String,
}
