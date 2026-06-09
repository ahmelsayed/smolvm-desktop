/** Host source kind for a secret ref (smolvm >= 1.0.0). */
export type SecretSource = "env" | "file";

/**
 * A guest-env-var ← host-source secret mapping authored at create/run time.
 * The value is resolved on the host at launch and never persisted to the
 * machine record. Maps onto smolvm's repeatable flags:
 * - `source === "env"`  → `--secret-env  GUEST=HOST_VAR`
 * - `source === "file"` → `--secret-file GUEST=/abs/path`
 */
export interface SecretRefMapping {
  /** Guest-side env var the secret is injected as. */
  env_name: string;
  source: SecretSource;
  /** Host env var name (env) or absolute host file path (file). */
  value: string;
}

/** A secret reference persisted on a machine record (`machine ls --json`
 *  `secret_refs`, smolvm >= 0.9.0). Read-only. */
export interface SecretRefView {
  env_name: string;
  /** Where the value comes from: "env" | "file". */
  source: string;
  /** Host env var name or file path. */
  target: string;
}
