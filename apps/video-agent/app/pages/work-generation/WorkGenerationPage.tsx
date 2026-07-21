"use client";

import { useEffect, useMemo, useState } from "react";
import type {
  AgentMessage,
  ApiClient,
  Material,
  ModelOption,
  Project,
  SceneVisualManifest,
  ScriptDetail,
  VoiceCatalogEntry,
  WorkPlanPayload,
  WorkPlanResponse,
} from "../../lib/api";
import {
  confirmWorkPlan,
  createAgentConversation,
  createWorkPlan,
  getVoiceCatalog,
  listMaterials,
  sendAgentMessage,
} from "../../lib/api";
import { VoiceCatalogSelect } from "../../components/VoiceCatalogSelect";
import { WorkspaceSelectField } from "../../components/WorkspaceSelectField";

type Props = {
  client: ApiClient;
  project: Project | undefined;
  script: ScriptDetail | null;
  manifest: SceneVisualManifest | null;
  textModels: ModelOption[];
  videoModels: ModelOption[];
  speechModels: ModelOption[];
  writesDisabled: boolean;
  onRunCreated?: (runId: string) => void;
};

const TTS_PROTOCOLS = new Set(["volcengine_tts_v3", "openai_audio_speech"]);

export function WorkGenerationPage({
  client,
  project,
  script,
  manifest,
  textModels,
  videoModels,
  speechModels,
  writesDisabled,
  onRunCreated,
}: Props) {
  const ttsModels = useMemo(
    () => speechModels.filter((model) => TTS_PROTOCOLS.has(model.api_protocol)),
    [speechModels],
  );
  const [llmModelId, setLlmModelId] = useState(defaultModelId(textModels));
  const [videoModelId, setVideoModelId] = useState(defaultModelId(videoModels));
  const [ttsModelId, setTtsModelId] = useState(defaultModelId(ttsModels));
  const [voices, setVoices] = useState<VoiceCatalogEntry[]>([]);
  const [ttsVoiceType, setTtsVoiceType] = useState("");
  const [ttsVoiceLabel, setTtsVoiceLabel] = useState("");
  const [voiceCatalogLoading, setVoiceCatalogLoading] = useState(false);
  const [durationStrategy, setDurationStrategy] =
    useState<WorkPlanPayload["duration_strategy"]>("preset30");
  const [customDuration, setCustomDuration] = useState("30");
  const [aspectRatio, setAspectRatio] = useState("16:9");
  const [resolution, setResolution] = useState("1080p");
  const [audioMode, setAudioMode] =
    useState<WorkPlanPayload["audio_mode"]>("independent_tts");
  const [burnSubtitles, setBurnSubtitles] = useState(true);
  const [audioMaterials, setAudioMaterials] = useState<Material[]>([]);
  const [selectedAudioMaterialIds, setSelectedAudioMaterialIds] = useState<string[]>([]);
  const [fullPrompt, setFullPrompt] = useState("");
  const [segmentPrompts, setSegmentPrompts] = useState<string[]>([]);
  const [plan, setPlan] = useState<WorkPlanResponse | null>(null);
  const [planDirty, setPlanDirty] = useState(false);
  const [messages, setMessages] = useState<AgentMessage[]>([]);
  const [draft, setDraft] = useState("");
  const [conversationId, setConversationId] = useState<string | null>(null);
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const [confirmedRunId, setConfirmedRunId] = useState<string | null>(null);

  const selectedVideo = useMemo(
    () => videoModels.find((model) => model.model_id === videoModelId),
    [videoModelId, videoModels],
  );
  const aspectRatios = capabilityStrings(selectedVideo, "aspect_ratios", ["16:9"]);
  const resolutions = capabilityStrings(selectedVideo, "resolutions", ["1080p"]);
  const audioSupported = selectedVideo?.capabilities?.generate_audio === true;
  const selectedVoice = useMemo(
    () => voices.find((voice) => voice.voice_type === ttsVoiceType) ?? null,
    [ttsVoiceType, voices],
  );
  const voiceInvalid = Boolean(ttsVoiceType) && (!selectedVoice || !selectedVoice.is_available);

  useEffect(() => {
    if (!llmModelId && textModels.length) setLlmModelId(defaultModelId(textModels));
  }, [llmModelId, textModels]);
  useEffect(() => {
    if (!videoModelId && videoModels.length) setVideoModelId(defaultModelId(videoModels));
  }, [videoModelId, videoModels]);
  useEffect(() => {
    if (!ttsModelId && ttsModels.length) setTtsModelId(defaultModelId(ttsModels));
  }, [ttsModelId, ttsModels]);
  useEffect(() => {
    let active = true;
    if (!ttsModelId) {
      setVoices([]);
      setVoiceCatalogLoading(false);
      return;
    }
    setVoiceCatalogLoading(true);
    setVoices([]);
    getVoiceCatalog(client, ttsModelId)
      .then((catalog) => {
        if (!active) return;
        const available = catalog.voices.filter((voice) => voice.is_available);
        setVoices(available);
        setTtsVoiceType((current) => {
          const currentVoice = available.find((voice) => voice.voice_type === current);
          if (currentVoice) {
            setTtsVoiceLabel(currentVoice.name);
            return current;
          }
          if (current) return current;
          const firstVoice = available[0];
          setTtsVoiceLabel(firstVoice?.name || "");
          return firstVoice?.voice_type || "";
        });
      })
      .catch((value) => {
        if (!active) return;
        setVoices([]);
        setError(value instanceof Error ? value.message : "音色目录读取失败");
      })
      .finally(() => {
        if (active) setVoiceCatalogLoading(false);
      });
    return () => {
      active = false;
    };
  }, [client, ttsModelId]);
  useEffect(() => {
    setAspectRatio((current) =>
      aspectRatios.includes(current) ? current : preferredCapability(aspectRatios, "16:9"),
    );
    setResolution((current) =>
      resolutions.includes(current) ? current : preferredCapability(resolutions, "1080p"),
    );
    if (!audioSupported) setAudioMode("independent_tts");
  }, [aspectRatios.join("\u0000"), audioSupported, resolutions.join("\u0000")]);
  useEffect(() => {
    let active = true;
    if (!project) {
      setAudioMaterials([]);
      setSelectedAudioMaterialIds([]);
      return;
    }
    listMaterials(client, project.project_id, { material_type: "audio", status: "active" })
      .then((response) => {
        if (active) setAudioMaterials(response.materials);
      })
      .catch((value) => {
        if (active) setError(value instanceof Error ? value.message : "已有音频读取失败");
      });
    return () => {
      active = false;
    };
  }, [client, project]);

  function invalidatePlan() {
    if (plan) setPlanDirty(true);
  }

  async function planWork() {
    if (!script || !manifest || !llmModelId || !videoModelId) return;
    const durationSeconds = durationStrategy === "custom" ? Number(customDuration) : undefined;
    if (durationStrategy === "custom" && (!Number.isInteger(durationSeconds) || durationSeconds! < 4 || durationSeconds! > 60)) {
      setError("自定义时长必须为 4~60 秒整数");
      return;
    }
    if (audioMode !== "seedance_original" && (!ttsModelId || !ttsVoiceType)) {
      setError("当前声音模式必须选择 TTS 模型和可用音色");
      return;
    }
    if (audioMode !== "seedance_original" && voiceInvalid) {
      setError("当前音色不适用于所选 TTS 模型，请重新选择");
      return;
    }
    setBusy(true);
    setError("");
    try {
      const response = await createWorkPlan(client, script.script_id, {
        llm_model_id: llmModelId,
        video_model_id: videoModelId,
        tts_model_id: audioMode === "seedance_original" ? null : ttsModelId,
        tts_voice_type: audioMode === "seedance_original" ? null : ttsVoiceType,
        duration_strategy: durationStrategy,
        duration_seconds: durationSeconds,
        narration_seconds: durationStrategy === "follow_narration"
          ? script.scenes.reduce((total, scene) => total + scene.duration_sec, 0)
          : undefined,
        aspect_ratio: aspectRatio,
        resolution,
        audio_mode: audioMode,
        full_prompt: fullPrompt,
        scene_prompts: manifest.scenes.map((scene) => scene.visual_description),
        segment_prompts: segmentPrompts.length ? segmentPrompts : undefined,
        audio_material_ids: selectedAudioMaterialIds,
        burn_subtitles: burnSubtitles,
      });
      setPlan(response);
      setSegmentPrompts(response.segments.map((segment) => String(segment.prompt ?? "")));
      setPlanDirty(false);
    } catch (value) {
      setError(value instanceof Error ? value.message : "作品计划生成失败");
    } finally {
      setBusy(false);
    }
  }

  async function confirm() {
    if (!plan || planDirty || plan.status !== "ready") return;
    setBusy(true);
    setError("");
    try {
      const response = await confirmWorkPlan(client, plan.plan_id, crypto.randomUUID());
      setConfirmedRunId(response.run_id);
      setPlan({ ...plan, status: "running" });
    } catch (value) {
      setError(value instanceof Error ? value.message : "作品确认失败");
    } finally {
      setBusy(false);
    }
  }

  async function sendMessage() {
    if (!project || !draft.trim() || !llmModelId) return;
    setBusy(true);
    setError("");
    try {
      const id = conversationId || (await createAgentConversation(client, {
        agent_type: "work",
        project_id: project.project_id,
        subject_type: plan ? "work" : undefined,
        subject_id: plan?.work_id,
        title: "作品生成 Agent",
        metadata: {},
      })).conversation_id;
      setConversationId(id);
      const response = await sendAgentMessage(client, id, {
        model_id: llmModelId,
        content: draft.trim(),
      });
      setMessages((current) => [...current, response.user_message, response.assistant_message]);
      setDraft("");
    } catch (value) {
      setError(value instanceof Error ? value.message : "Agent 对话失败");
    } finally {
      setBusy(false);
    }
  }

  if (!script || !manifest) {
    return <section className="workGenerationWorkspace"><h2>作品生成</h2><p>请先为脚本全部分镜确认主画面，再进入作品生成。</p></section>;
  }

  const canConfirm = Boolean(plan?.can_confirm && !planDirty && plan.status === "ready");
  const planState = plan?.status === "running"
    ? "生成中"
    : planDirty
      ? "计划已过期"
      : plan?.status === "ready"
        ? "校验通过"
        : "待规划";
  const doubleVoiceRisk = audioMode === "seedance_original_and_tts";
  const otherWarnings = Array.isArray(plan?.warnings)
    ? plan.warnings.filter((warning) => String(warning) !== "可能出现双重人声")
    : [];
  return (
    <section aria-label="作品生成工作区" className="workGenerationWorkspace">
      <header className="workGenerationHeader">
        <div>
          <p className="sectionKicker">作品生产 / 作品生成</p>
          <h2>{script.title}</h2>
          <p>{manifest.scenes.length} 个分镜 · 一次确认创建一部作品</p>
        </div>
          <div className="workGenerationHeaderStatus">
            <span>输入已就绪</span>
            <small>{manifest.scenes.length} 张主画面已锁定</small>
            {confirmedRunId ? <button className="secondaryButton" type="button" onClick={() => onRunCreated?.(confirmedRunId)}>查看生成任务</button> : null}
          </div>
      </header>
      <div className="workGenerationGrid">
        <aside aria-labelledby="work-generation-agent-title" className="workGenerationPanel workGenerationAgentPanel" role="region">
          <div className="workGenerationPanelHeader">
            <div>
              <p className="workGenerationEyebrow">WORK AGENT</p>
              <h3 id="work-generation-agent-title">作品 Agent</h3>
            </div>
            <span className="workGenerationRecommendBadge">只做推荐</span>
          </div>
          <dl className="workGenerationBinding">
            <div><dt>绑定脚本</dt><dd>{script.title}</dd></div>
            <div><dt>输入清单</dt><dd>{manifest.scenes.length} 个分镜 / {manifest.scenes.length} 张主画面</dd></div>
          </dl>
          <div className="workGenerationMessages" aria-live="polite">
            {messages.length ? messages.map((message) => (
              <article className={message.role === "user" ? "user" : "agent"} key={message.message_id}>
                <strong>{message.role === "user" ? "你" : "作品 Agent"}</strong>
                <p>{message.content}</p>
              </article>
            )) : (
              <article className="agent">
                <strong>作品 Agent</strong>
                <p>已读取完整主画面与脚本。你可以要求我调整节奏、角色连续性或提示词；确认前不会调用 Seedance。</p>
              </article>
            )}
          </div>
          <section className="workGenerationAudit" aria-label="工具步骤审计">
            <div className="workGenerationSectionTitle"><span>工具步骤</span><small>可审计</small></div>
            <ol>
              <li className="done"><span>1</span><div><strong>读取主画面清单</strong><small>read_scene_visual_manifest · {manifest.scenes.length}/{manifest.scenes.length}</small></div></li>
              <li className={plan ? "done" : "ready"}><span>2</span><div><strong>校验作品输入</strong><small>validate_work_inputs · {plan ? "已完成" : "已就绪"}</small></div></li>
              <li className={plan ? "done" : "pending"}><span>3</span><div><strong>生成作品计划</strong><small>build_work_plan · {plan ? `版本 ${plan.plan_version}` : "等待执行"}</small></div></li>
            </ol>
          </section>
          <div className="workGenerationComposer">
            <textarea aria-label="作品 Agent 消息" value={draft} onChange={(event) => setDraft(event.target.value)} placeholder="描述节奏、连续性或提示词修改" />
            <button className="secondaryButton" disabled={busy || !draft.trim()} onClick={sendMessage} type="button">发送</button>
          </div>
        </aside>

        <main aria-labelledby="work-generation-plan-title" className="workGenerationPanel workGenerationPlanPanel" role="region">
          <div className="workGenerationPanelHeader workGenerationPlanHeader">
            <div>
              <p className="workGenerationEyebrow">WORK PLAN</p>
              <h3 id="work-generation-plan-title">作品计划预览</h3>
            </div>
            <div className={`workGenerationPlanState ${plan?.status === "running" ? "running" : planDirty ? "stale" : plan ? "ready" : "pending"}`}>
              <span>{planState}</span>
              {plan ? <small>计划版本 {plan.plan_version}</small> : <small>尚未调用视频模型</small>}
            </div>
          </div>
          <div className="workGenerationPlanScroll">
            <section className="workGenerationPlanSection">
              <div className="workGenerationSectionTitle"><span>生成流程</span><small>一次确认，后台自动拆分</small></div>
              <ol className="workGenerationFlow">
                <li className="done"><span>1</span><strong>输入校验</strong><small>已完成</small></li>
                <li className={plan ? "done" : "active"}><span>2</span><strong>Agent 规划</strong><small>{plan ? "已完成" : "当前步骤"}</small></li>
                <li className={plan?.status === "running" ? "done" : plan ? "active" : "pending"}><span>3</span><strong>人工确认</strong><small>{plan?.status === "running" ? "已完成" : plan ? "待确认" : "等待计划"}</small></li>
                <li className={plan?.status === "running" ? "active" : "pending"}><span>4</span><strong>生成合成</strong><small>{plan?.status === "running" ? "执行中" : "尚未开始"}</small></li>
              </ol>
            </section>

            <section className="workGenerationPlanSection">
              <div className="workGenerationSectionTitle"><span>主画面</span><small>{manifest.scenes.length} 张已锁定</small></div>
              <ol className="workGenerationScenes">
                {manifest.scenes.map((scene) => (
                  <li key={scene.scene_id}>
                    <img alt={`第 ${scene.sequence} 镜主画面`} src={scene.file_url} />
                    <div><strong>第 {scene.sequence} 镜</strong><span>{scene.duration_sec} 秒</span></div>
                    <p>{scene.visual_description}</p>
                  </li>
                ))}
              </ol>
            </section>

            <section className="workGenerationPlanSection">
              <div className="workGenerationSectionTitle"><span>全片提示词</span><small>角色、空间与镜头连续性</small></div>
              <label className="workGenerationPromptField">
                <span className="srOnly">全片提示词</span>
                <textarea aria-label="全片提示词" value={fullPrompt} onChange={(event) => { invalidatePlan(); setFullPrompt(event.target.value); }} placeholder="由 Agent 汇总全片意图；也可在生成计划前直接补充约束。" />
              </label>
            </section>

            <section className="workGenerationPlanSection">
              <div className="workGenerationSectionTitle"><span>Seedance 分段计划</span><small>单段 4~15 秒 / 最多 9 张参考图</small></div>
              {plan ? (
                <div className="workGenerationSegments">
                  {plan.segments.map((segment, index) => (
                    <label key={String(segment.sequence ?? index)}>
                      <span>第 {index + 1} 段</span>
                      <small>{String(segment.duration_seconds ?? "-")} 秒</small>
                      <textarea aria-label={`第 ${index + 1} 段提示词`} value={segmentPrompts[index] ?? ""} onChange={(event) => { invalidatePlan(); setSegmentPrompts((current) => current.map((value, currentIndex) => currentIndex === index ? event.target.value : value)); }} />
                    </label>
                  ))}
                </div>
              ) : (
                <div className="workGenerationSegmentPlaceholder">
                  <div><span>01</span><p>等待 Agent 按镜头边界生成合法分段</p></div>
                  <div><span>02</span><p>分段数量与时长将随参数实时校验</p></div>
                </div>
              )}
              {planDirty ? <p className="workGenerationStale">计划已过期，请重新生成计划</p> : null}
              {otherWarnings.map((warning) => <p className="workGenerationWarning" key={String(warning)}>{String(warning)}</p>)}
            </section>

            <section className="workGenerationPlanSection">
              <div className="workGenerationSectionTitle"><span>资源用量</span><small>调用规模</small></div>
              <dl className="workGenerationUsage">
                <div><dt>视频任务数</dt><dd>{String(plan?.resource_usage.video_task_count ?? "-")}</dd></div>
                <div><dt>视频秒数</dt><dd>{plan ? `${String(plan.resource_usage.video_seconds ?? "-")} 秒` : "-"}</dd></div>
                <div><dt>TTS 字符</dt><dd>{String(plan?.resource_usage.tts_characters ?? "-")}</dd></div>
                <div><dt>ASR 时长</dt><dd>{plan ? `${String(plan.resource_usage.asr_seconds ?? "-")} 秒` : "-"}</dd></div>
              </dl>
            </section>
          </div>
        </main>

        <aside aria-labelledby="work-generation-params-title" className="workGenerationPanel workGenerationParamsPanel" role="region">
          <div className="workGenerationPanelHeader">
            <div>
              <p className="workGenerationEyebrow">CONFIRMATION</p>
              <h3 id="work-generation-params-title">参数确认</h3>
            </div>
            <span className="workGenerationParamsState">{plan ? "已规划" : "待规划"}</span>
          </div>
          <div className="workGenerationParamsScroll">
            <section className="workGenerationParamsSection">
              <h4>模型</h4>
              <WorkspaceSelectField label="方案 LLM" value={llmModelId} onChange={(event) => { invalidatePlan(); setLlmModelId(event.target.value); }}>{textModels.map((model) => <option key={model.model_id} value={model.model_id}>{model.display_name}</option>)}</WorkspaceSelectField>
              <WorkspaceSelectField label="视频模型" value={videoModelId} onChange={(event) => { invalidatePlan(); setVideoModelId(event.target.value); }}>{videoModels.map((model) => <option key={model.model_id} value={model.model_id}>{model.display_name}</option>)}</WorkspaceSelectField>
              <div className="workGenerationInlineFields workGenerationTtsFields">
                <WorkspaceSelectField label="TTS 模型" disabled={audioMode === "seedance_original"} value={ttsModelId} onChange={(event) => { invalidatePlan(); setTtsModelId(event.target.value); }}>{ttsModels.map((model) => <option key={model.model_id} value={model.model_id}>{model.display_name}</option>)}</WorkspaceSelectField>
                <div className="workGenerationVoiceField">
                  <span className="workspaceSelectLabel">音色</span>
                  <VoiceCatalogSelect
                    voices={voices}
                    selectedVoice={selectedVoice}
                    selectedVoiceType={ttsVoiceType}
                    selectedVoiceLabel={ttsVoiceLabel}
                    invalid={voiceInvalid}
                    disabled={audioMode === "seedance_original" || voiceCatalogLoading}
                    popoverWidth={650}
                    variant="compact"
                    onChange={(voiceType) => {
                      const voice = voices.find((item) => item.voice_type === voiceType);
                      invalidatePlan();
                      setTtsVoiceType(voiceType);
                      setTtsVoiceLabel(voice?.name || voiceType);
                    }}
                  />
                </div>
              </div>
            </section>
            <section className="workGenerationParamsSection">
              <h4>输出</h4>
              <WorkspaceSelectField label="成片时长" value={durationStrategy} onChange={(event) => { invalidatePlan(); setDurationStrategy(event.target.value as WorkPlanPayload["duration_strategy"]); }}><option value="preset15">15 秒</option><option value="preset30">30 秒</option><option value="preset45">45 秒</option><option value="preset60">60 秒</option><option value="custom">自定义</option><option value="follow_narration">跟随配音</option></WorkspaceSelectField>
              {durationStrategy === "custom" ? <label>自定义时长（秒）<input aria-label="自定义时长（秒）" min="4" max="60" step="1" type="number" value={customDuration} onChange={(event) => { invalidatePlan(); setCustomDuration(event.target.value); }} /></label> : null}
              <div className="workGenerationInlineFields">
                <WorkspaceSelectField label="画面比例" value={aspectRatio} onChange={(event) => { invalidatePlan(); setAspectRatio(event.target.value); }}>{aspectRatios.map((value) => <option key={value} value={value}>{value}</option>)}</WorkspaceSelectField>
                <WorkspaceSelectField label="分辨率" value={resolution} onChange={(event) => { invalidatePlan(); setResolution(event.target.value); }}>{resolutions.map((value) => <option key={value} value={value}>{value}</option>)}</WorkspaceSelectField>
              </div>
            </section>
            <section className="workGenerationParamsSection">
              <h4>声音与字幕</h4>
              <WorkspaceSelectField label="声音模式" value={audioMode} onChange={(event) => { invalidatePlan(); setAudioMode(event.target.value as WorkPlanPayload["audio_mode"]); }}><option value="independent_tts">独立 TTS</option>{audioSupported ? <><option value="seedance_original">Seedance 原声</option><option value="seedance_original_and_tts">Seedance 原声 + TTS</option></> : null}</WorkspaceSelectField>
              <fieldset aria-label="已有音频素材" className="workGenerationAudioMaterials">
                <legend>已有音频素材</legend>
                {audioMaterials.length ? audioMaterials.map((material) => <label key={material.material_id}><input type="checkbox" checked={selectedAudioMaterialIds.includes(material.material_id)} onChange={(event) => { invalidatePlan(); setSelectedAudioMaterialIds((current) => event.target.checked ? [...current, material.material_id] : current.filter((id) => id !== material.material_id)); }} />{material.file_name} · {material.audio_usage ?? "other"}</label>) : <span>暂无可用 BGM、环境音或动作音效</span>}
              </fieldset>
              <div className="workGenerationSubtitleOptions">
                <label><input aria-label="烧录字幕" type="checkbox" checked={burnSubtitles} onChange={(event) => { invalidatePlan(); setBurnSubtitles(event.target.checked); }} />烧录字幕</label>
                <span>独立 SRT 始终保存</span>
              </div>
              {doubleVoiceRisk ? <div className="workGenerationRisk"><strong>双重人声风险</strong><p>可能出现双重人声。Seedance 原声中的人声不可分离，混音时仅做整体 ducking。</p></div> : null}
            </section>
            {plan ? <section className="workGenerationParamsSection workGenerationSnapshotSection">
              <h4>确认快照</h4>
              <dl className="workGenerationSnapshot">
                <div><dt>方案 LLM</dt><dd>{modelName(textModels, llmModelId)}</dd></div>
                <div><dt>视频模型</dt><dd>{modelName(videoModels, videoModelId)}</dd></div>
                <div><dt>输出</dt><dd>{aspectRatio} · {resolution}</dd></div>
                <div><dt>声音</dt><dd>{audioModeLabel(audioMode)}</dd></div>
                <div><dt>音色</dt><dd>{selectedVoice?.name || ttsVoiceLabel || "不使用独立 TTS"}</dd></div>
                <div><dt>字幕</dt><dd>{burnSubtitles ? "烧录 + SRT" : "独立 SRT"}</dd></div>
              </dl>
            </section> : null}
          </div>
          <div className="workGenerationActions">
            <button className="secondaryButton" disabled={busy || !llmModelId || !videoModelId} onClick={planWork} type="button">{plan ? "重新生成计划" : "生成计划"}</button>
            <button className="primaryButton" disabled={writesDisabled || busy || !canConfirm} onClick={confirm} type="button">确认生成作品</button>
          </div>
        </aside>
      </div>
      {error ? <p role="alert" className="errorText">{error}</p> : null}
    </section>
  );
}

function defaultModelId(models: ModelOption[]) {
  return models.find((model) => model.is_default)?.model_id || models[0]?.model_id || "";
}

function capabilityStrings(model: ModelOption | undefined, key: string, fallback: string[]) {
  const value = model?.capabilities?.[key];
  return Array.isArray(value) && value.every((item) => typeof item === "string") && value.length
    ? value as string[]
    : fallback;
}

function preferredCapability(values: string[], preferred: string) {
  return values.includes(preferred) ? preferred : values[0] || preferred;
}

function modelName(models: ModelOption[], id: string) {
  return models.find((model) => model.model_id === id)?.display_name || "不可用模型";
}

function audioModeLabel(mode: WorkPlanPayload["audio_mode"]) {
  if (mode === "seedance_original") return "Seedance 原声";
  if (mode === "seedance_original_and_tts") return "Seedance 原声 + TTS";
  return "独立 TTS";
}
