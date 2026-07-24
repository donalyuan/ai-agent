import {
  createModels,
  createProvider,
  type Api,
  type Model,
  type Models,
} from "@earendil-works/pi-ai";
import * as openAiCompletionsApi from "@earendil-works/pi-ai/api/openai-completions";
import * as openAiResponsesApi from "@earendil-works/pi-ai/api/openai-responses";
import type { Pool, QueryResultRow } from "pg";
import type { ThinkingLevel } from "@earendil-works/pi-agent-core";

import { RuntimeError } from "./errors.js";
import { redactUrl } from "./redaction.js";

export type SupportedTextProtocol = "openai_responses" | "openai_chat_completions";

interface AiModelRow extends QueryResultRow {
  id: string;
  provider_name: string;
  api_protocol: string;
  auth_scheme: string;
  request_base_url: string;
  upstream_model: string;
  api_key: string;
  timeout_seconds: number;
  reasoning_effort: string | null;
  max_output_tokens: number | null;
  settings: unknown;
}

export interface ResolvedTextModel {
  id: string;
  providerName: string;
  protocol: SupportedTextProtocol;
  requestBaseUrl: string;
  upstreamModel: string;
  apiKey: string;
  timeoutMs: number;
  reasoningEffort?: string;
  maxOutputTokens: number;
  contextWindow: number;
}

export interface ModelSnapshot {
  model_id: string;
  provider: string;
  protocol: SupportedTextProtocol;
  request_base_url: string;
  upstream_model: string;
  reasoning_effort: string | null;
  max_output_tokens: number;
  timeout_seconds: number;
}

export interface PiModelRuntime {
  models: Models;
  model: Model<Api>;
  streamOptions: { timeoutMs: number };
  thinkingLevel: ThinkingLevel;
  snapshot: ModelSnapshot;
  secrets: readonly string[];
}

function positiveSetting(settings: unknown, key: string, fallback: number): number {
  if (settings === null || typeof settings !== "object" || Array.isArray(settings)) return fallback;
  const value = (settings as Record<string, unknown>)[key];
  return typeof value === "number" && Number.isSafeInteger(value) && value > 0 ? value : fallback;
}

function mapRow(row: AiModelRow): ResolvedTextModel {
  if (row.auth_scheme !== "bearer") {
    throw new RuntimeError("model_incompatible", 422, "文本模型必须使用 bearer 认证");
  }
  if (row.api_protocol !== "openai_responses" && row.api_protocol !== "openai_chat_completions") {
    throw new RuntimeError("model_incompatible", 422, "模型协议不受 Agent Runtime 支持");
  }
  const apiKey = row.api_key.trim();
  const baseUrl = row.request_base_url.trim().replace(/\/+$/, "");
  const upstreamModel = row.upstream_model.trim();
  if (!apiKey || !baseUrl || !upstreamModel) {
    throw new RuntimeError("model_incompatible", 422, "模型运行配置不完整");
  }

  return {
    id: row.id,
    providerName: row.provider_name,
    protocol: row.api_protocol,
    requestBaseUrl: baseUrl,
    upstreamModel,
    apiKey,
    timeoutMs: row.timeout_seconds * 1_000,
    ...(row.reasoning_effort ? { reasoningEffort: row.reasoning_effort } : {}),
    maxOutputTokens: row.max_output_tokens ?? 4_096,
    contextWindow: positiveSetting(row.settings, "context_window", 128_000),
  };
}

export class ModelConfigRepository {
  constructor(private readonly pool: Pool) {}

  async resolveEnabledText(modelId: string): Promise<ResolvedTextModel> {
    const result = await this.pool.query<AiModelRow>(
      `SELECT id::text, provider_name, api_protocol, auth_scheme, request_base_url,
              upstream_model, api_key, timeout_seconds, reasoning_effort,
              max_output_tokens, settings
         FROM ai_models
        WHERE id = $1::uuid
          AND model_type = 'text'
          AND status = 'enabled'
          AND deleted_at IS NULL`,
      [modelId],
    );
    const row = result.rows[0];
    if (!row) {
      throw new RuntimeError("model_not_found", 404, "未找到可用的文本模型配置");
    }
    return mapRow(row);
  }

  async ping(): Promise<void> {
    await this.pool.query("SELECT 1");
  }
}

export function createPiModelRuntime(config: ResolvedTextModel): PiModelRuntime {
  const api = config.protocol === "openai_responses" ? "openai-responses" : "openai-completions";
  const providerId = `novex-model-${config.id}`;
  const model: Model<typeof api> = {
    id: config.upstreamModel,
    name: config.upstreamModel,
    api,
    provider: providerId,
    baseUrl: config.requestBaseUrl,
    reasoning: config.reasoningEffort !== undefined,
    input: ["text"],
    cost: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0 },
    contextWindow: config.contextWindow,
    maxTokens: config.maxOutputTokens,
  };
  const provider = createProvider({
    id: providerId,
    name: config.providerName,
    baseUrl: config.requestBaseUrl,
    auth: {
      apiKey: {
        name: "Novex model credential",
        resolve: async () => ({ auth: { apiKey: config.apiKey } }),
      },
    },
    models: [model],
    api: config.protocol === "openai_responses" ? openAiResponsesApi : openAiCompletionsApi,
  });
  const models = createModels();
  models.setProvider(provider);

  return {
    models,
    model,
    streamOptions: { timeoutMs: config.timeoutMs },
    thinkingLevel: parseThinkingLevel(config.reasoningEffort),
    snapshot: {
      model_id: config.id,
      provider: config.providerName,
      protocol: config.protocol,
      request_base_url: redactUrl(config.requestBaseUrl),
      upstream_model: config.upstreamModel,
      reasoning_effort: config.reasoningEffort ?? null,
      max_output_tokens: config.maxOutputTokens,
      timeout_seconds: config.timeoutMs / 1_000,
    },
    secrets: [config.apiKey],
  };
}

function parseThinkingLevel(value: string | undefined): ThinkingLevel {
  if (value === undefined) return "off";
  if (["minimal", "low", "medium", "high", "xhigh"].includes(value)) {
    return value as ThinkingLevel;
  }
  throw new RuntimeError("model_incompatible", 422, "模型 reasoning_effort 不受 Agent Runtime 支持");
}
