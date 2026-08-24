/* This file is generated from JSON Schema. Do not edit manually. */

/**
 * This interface was referenced by `PublishedWorkflowVersion`'s JSON-Schema
 * via the `definition` "uuid".
 */
export type Uuid = string;
/**
 * This interface was referenced by `PublishedWorkflowVersion`'s JSON-Schema
 * via the `definition` "hash".
 */
export type Hash = string;

export interface PublishedWorkflowVersion {
  id: Uuid;
  schema_version: "1.0.0";
  revision: number;
  status: "published";
  projectId: Uuid;
  templateKey: "drama-mvp-a-default";
  versionNumber: number;
  scopeType: "project";
  /**
   * @minItems 1
   * @maxItems 1
   */
  scopeIds: [Uuid];
  contentHash: Hash;
  definition: {
    /**
     * @minItems 8
     * @maxItems 8
     */
    nodes: never[];
    compatibilityLogicalOperations: {
      /**
       * @minItems 2
       * @maxItems 2
       */
      "media.generate": never[];
    };
    /**
     * @minItems 2
     * @maxItems 2
     */
    skills: never[];
    schemaVersion: "1.0.0";
  };
}
/**
 * This interface was referenced by `PublishedWorkflowVersion`'s JSON-Schema
 * via the `definition` "textGenerate".
 */
export interface Node {
  key: "text.generate";
  ports: TextGeneratePorts;
}
export interface TextGeneratePorts {
  input: "text.generate.input.v1";
  output: "text.generate.output.v1";
}
/**
 * This interface was referenced by `PublishedWorkflowVersion`'s JSON-Schema
 * via the `definition` "textReview".
 */
export interface Node1 {
  key: "text.review";
  ports: TextReviewPorts;
}
export interface TextReviewPorts {
  input: "text.review.input.v1";
  output: "text.review.output.v1";
}
/**
 * This interface was referenced by `PublishedWorkflowVersion`'s JSON-Schema
 * via the `definition` "imageGenerate".
 */
export interface Node2 {
  key: "media.generate.image";
  ports: ImageGeneratePorts;
}
export interface ImageGeneratePorts {
  input: "media.generate.image.input.v1";
  output: "media.generate.image.output.v1";
}
/**
 * This interface was referenced by `PublishedWorkflowVersion`'s JSON-Schema
 * via the `definition` "imageReview".
 */
export interface Node3 {
  key: "media.review.image";
  ports: ImageReviewPorts;
}
export interface ImageReviewPorts {
  input: "media.review.image.input.v1";
  output: "media.review.image.output.v1";
}
/**
 * This interface was referenced by `PublishedWorkflowVersion`'s JSON-Schema
 * via the `definition` "videoGenerate".
 */
export interface Node4 {
  key: "media.generate.video";
  ports: VideoGeneratePorts;
}
export interface VideoGeneratePorts {
  input: "media.generate.video.input.v1";
  output: "media.generate.video.output.v1";
}
/**
 * This interface was referenced by `PublishedWorkflowVersion`'s JSON-Schema
 * via the `definition` "videoReview".
 */
export interface Node5 {
  key: "media.review.video";
  ports: VideoReviewPorts;
}
export interface VideoReviewPorts {
  input: "media.review.video.input.v1";
  output: "media.review.video.output.v1";
}
/**
 * This interface was referenced by `PublishedWorkflowVersion`'s JSON-Schema
 * via the `definition` "mediaInspect".
 */
export interface Node6 {
  key: "media.inspect";
  ports: MediaInspectPorts;
}
export interface MediaInspectPorts {
  input: "media.inspect.input.v1";
  output: "media.inspect.output.v1";
}
/**
 * This interface was referenced by `PublishedWorkflowVersion`'s JSON-Schema
 * via the `definition` "timelineHandoff".
 */
export interface Node7 {
  key: "timeline.handoff";
  ports: TimelineHandoffPorts;
}
export interface TimelineHandoffPorts {
  input: "timeline.handoff.input.v1";
  output: "timeline.handoff.output.v1";
}
/**
 * This interface was referenced by `PublishedWorkflowVersion`'s JSON-Schema
 * via the `definition` "node".
 */
export interface Node8 {
  key: string;
  ports: {
    [k: string]: unknown;
  };
}
/**
 * This interface was referenced by `PublishedWorkflowVersion`'s JSON-Schema
 * via the `definition` "textGeneratePorts".
 */
export interface Ports {
  input: "text.generate.input.v1";
  output: "text.generate.output.v1";
}
/**
 * This interface was referenced by `PublishedWorkflowVersion`'s JSON-Schema
 * via the `definition` "textReviewPorts".
 */
export interface Ports1 {
  input: "text.review.input.v1";
  output: "text.review.output.v1";
}
/**
 * This interface was referenced by `PublishedWorkflowVersion`'s JSON-Schema
 * via the `definition` "imageGeneratePorts".
 */
export interface Ports2 {
  input: "media.generate.image.input.v1";
  output: "media.generate.image.output.v1";
}
/**
 * This interface was referenced by `PublishedWorkflowVersion`'s JSON-Schema
 * via the `definition` "imageReviewPorts".
 */
export interface Ports3 {
  input: "media.review.image.input.v1";
  output: "media.review.image.output.v1";
}
/**
 * This interface was referenced by `PublishedWorkflowVersion`'s JSON-Schema
 * via the `definition` "videoGeneratePorts".
 */
export interface Ports4 {
  input: "media.generate.video.input.v1";
  output: "media.generate.video.output.v1";
}
/**
 * This interface was referenced by `PublishedWorkflowVersion`'s JSON-Schema
 * via the `definition` "videoReviewPorts".
 */
export interface Ports5 {
  input: "media.review.video.input.v1";
  output: "media.review.video.output.v1";
}
/**
 * This interface was referenced by `PublishedWorkflowVersion`'s JSON-Schema
 * via the `definition` "mediaInspectPorts".
 */
export interface Ports6 {
  input: "media.inspect.input.v1";
  output: "media.inspect.output.v1";
}
/**
 * This interface was referenced by `PublishedWorkflowVersion`'s JSON-Schema
 * via the `definition` "timelineHandoffPorts".
 */
export interface Ports7 {
  input: "timeline.handoff.input.v1";
  output: "timeline.handoff.output.v1";
}
/**
 * This interface was referenced by `PublishedWorkflowVersion`'s JSON-Schema
 * via the `definition` "ports".
 */
export interface Ports8 {
  input: string;
  output: string;
}
