// This file is auto-generated from the Rust sources. Do not edit manually.
// Corresponding Rust file: src-tauri/src/state/process_state.rs

/**
 * Represents the state of a managed process.
 * Corresponds to the Rust enum `ProcessState`.
 * Note: The 'Crashed' state includes an error message.
 */
export type ProcessState =
  | 'Starting'
  | 'Running'
  | 'Stopping'
  | 'Stopped'
  | { Crashed: string };

/**
 * Metadata associated with a running or recently stopped process.
 * Corresponds to the Rust struct `ProcessMetadata`.
 */
export interface ProcessMetadata {
  id: string; // Uuid
  profile_id: string; // Uuid
  start_time: string; // DateTime<Utc> as ISO string
  state: ProcessState;
  pid: number; // u32
  account_uuid?: string | null;
  account_name?: string | null;
  minecraft_version?: string | null;
  modloader?: string | null;
  modloader_version?: string | null;
  norisk_pack?: string | null;
  profile_name?: string | null;
  profile_image_url?: string | null;
}

/**
 * DTO for submitting crash logs.
 * Corresponds to the Rust struct `CrashlogDto`.
 */
export interface CrashlogDto {
  /** Preferred: log body sent directly (no server-side URL fetch). */
  log_content?: string | null;
  /** Legacy: only mclo.gs URLs may be fetched when log_content is omitted. */
  mcLogsUrl?: string | null;
  metadata: ProcessMetadata | null;
}