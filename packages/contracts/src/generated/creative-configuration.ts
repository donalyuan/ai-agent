/* This file is generated from JSON Schema. Do not edit manually. */

export interface CreativeConfiguration {
  id: string;
  projectId: string;
  schema_version: string;
  projectRevision: number;
  creationMode: "original" | "adaptation";
  creativeBrief?: Brief;
  settings?: Settings;
  sourceBinding?: Binding;
  storySpecRef?: Ref;
}
/**
 * This interface was referenced by `CreativeConfiguration`'s JSON-Schema
 * via the `definition` "brief".
 */
export interface Brief {
  creative_brief_id: string;
  project_id: string;
  subject: string;
  genre: string;
  audience: string;
  character_premise: string;
  style: string;
  episode_duration_seconds: number;
  episode_count: number;
  scenes_per_episode: number;
  shots_per_scene: number;
  revision: number;
  schema_version: string;
  id: string;
  payload_hash: string;
}
/**
 * This interface was referenced by `CreativeConfiguration`'s JSON-Schema
 * via the `definition` "settings".
 */
export interface Settings {
  project_id: string;
  text_cost_confirmation_threshold: null | {
    amount: string;
    currency: string;
  };
  revision: number;
  schema_version: string;
  id: string;
  payload_hash: string;
}
/**
 * This interface was referenced by `CreativeConfiguration`'s JSON-Schema
 * via the `definition` "binding".
 */
export interface Binding {
  project_id: string;
  source_material_id: string;
  source_material_revision: number;
  source_content_hash: string;
  creative_brief_id: string;
  creative_brief_revision: number;
  creative_brief_payload_hash: string;
  parse_status: string;
  validation_status: string;
  binding_status: string;
  binding_version: string;
  schema_version: string;
  id: string;
}
/**
 * This interface was referenced by `CreativeConfiguration`'s JSON-Schema
 * via the `definition` "ref".
 */
export interface Ref {
  id: string;
  revision: number;
  hash: string;
  projectId: string;
}
