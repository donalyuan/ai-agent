import { readFileSync } from "node:fs";
import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { createApiClient } from "../../lib/api";
import { SoundSubtitlePage } from "./SoundSubtitlePage";

const soundPageStyles = readFileSync("app/styles.css", "utf8");

const projectId = "11111111-1111-4111-8111-111111111111";
const ttsModelId = "22222222-2222-4222-8222-222222222222";
const secondTtsModelId = "33333333-3333-4333-8333-333333333333";
const openAiTtsModelId = "aaaa3333-3333-4333-8333-333333333333";
const asrModelId = "44444444-4444-4444-8444-444444444444";
const textModelId = "55555555-5555-4555-8555-555555555555";
const sourceScriptId = "77777777-1111-4777-8777-777777777777";
const firstSceneId = "88888888-1111-4888-8888-888888888888";
const secondSceneId = "99999999-1111-4999-8999-999999999999";

const speechModels = [
  {
    model_id: ttsModelId,
    display_name: "豆包 TTS",
    model_type: "speech",
    provider_name: "火山引擎",
    api_protocol: "volcengine_tts_v3",
    upstream_model: "doubao-seed-tts-2.0",
    is_default: true,
  },
  {
    model_id: secondTtsModelId,
    display_name: "豆包 TTS 备用",
    model_type: "speech",
    provider_name: "火山引擎",
    api_protocol: "volcengine_tts_v3",
    upstream_model: "doubao-seed-tts-2.0",
    is_default: false,
  },
  {
    model_id: openAiTtsModelId,
    display_name: "ZeekAI Seed TTS",
    model_type: "speech",
    provider_name: "ZeekAI",
    api_protocol: "openai_audio_speech",
    upstream_model: "doubao-seed-tts-2.0",
    is_default: false,
  },
  {
    model_id: asrModelId,
    display_name: "豆包 ASR",
    model_type: "speech",
    provider_name: "火山引擎",
    api_protocol: "volcengine_asr_v3",
    upstream_model: "doubao-seed-asr-2.0",
    is_default: true,
  },
];

const textModels = [{
  model_id: textModelId,
  display_name: "GPT Text",
  model_type: "text",
  provider_name: "OpenAI",
  api_protocol: "openai_responses",
  upstream_model: "gpt-test",
  is_default: true,
}];

const firstVoice = {
  voice_id: "66666666-6666-4666-8666-666666666666",
  voice_type: "zh_female_fixture",
  resource_id: "seed-tts-2.0",
  name: "测试女声",
  avatar_url: null,
  gender: "female",
  age: "adult",
  categories: [],
  normal_labels: ["沉稳"],
  special_labels: [],
  trial_url: null,
  short_trial_url: null,
  languages: [
    { Language: "zh-cn", Text: "这是一段试听文案，不是语言名称。" },
    { Language: "zh", Text: "中文试听文案" },
    { Language: "en", Text: "English audition text" },
    { Language: "id", Text: "Teks audisi bahasa Indonesia" },
    { Language: "pt-br", Text: "Texto de audicao" },
    { Language: "ja", Text: "日本語の試聴文です" },
    { Language: "mx", Text: "Texto de prueba mexicano" },
    { Language: "vi", Text: "Van ban nghe thu" },
    { Language: "th", Text: "ข้อความทดลองฟัง" },
    { Language: "es-mx", Text: "Texto de prueba en espanol" },
    { Language: "fil", Text: "Teksto ng audition" },
    { Language: "fr", Text: "Texte d'audition" },
    { Language: "ru", Text: "Текст для прослушивания" },
    { Language: "de", Text: "Deutscher Probetext" },
    { Language: "ko", Text: "한국어 미리듣기 문장" },
    { Language: "ms", Text: "Teks pratonton bahasa Melayu" },
    { Language: "ar", Text: "نص المعاينة" },
    { Language: "it", Text: "Testo di anteprima" },
  ],
  emotions: [{ Label: "", Value: "", Icon: "" }],
  description: "适合知识旁白",
  is_available: true,
  catalog_version: 1,
  created_at: "2026-07-15T00:00:00Z",
  updated_at: "2026-07-15T00:00:00Z",
};

const alastorVoice = {
  ...firstVoice,
  voice_id: "67676767-6767-4767-8767-676767676767",
  voice_type: "ICL_uranus_en_male_alastor_tob",
  name: "Alastor 2.0",
  gender: "男",
  age: "青年",
  normal_labels: [],
  languages: [{ Language: "en", Text: "Smile, smile darling, this is audition copy." }],
  description: "恐怖电影里的小丑，声音尖锐，有侵略性，擅长英语",
};

const chineseFemaleVoice = {
  ...firstVoice,
  voice_id: "68686868-6868-4868-8868-686868686868",
  voice_type: "zh_female_news_fixture",
  name: "新闻女声",
  normal_labels: ["清晰"],
  languages: [{ Language: "zh-cn", Text: "中文新闻试听文案" }],
  description: "适合新闻播报",
};

function catalog(modelId: string, maxInputCharacters = 3000) {
  const voices = modelId === ttsModelId
    ? [firstVoice, alastorVoice, chineseFemaleVoice]
    : [{ ...firstVoice, voice_id: "77777777-7777-4777-8777-777777777777", voice_type: "zh_male_fixture", name: "测试男声" }];
  return {
    model_id: modelId,
    source_model_id: modelId === openAiTtsModelId ? ttsModelId : modelId,
    model_settings: {
      supported_audio_formats: ["mp3"],
      default_audio_format: "mp3",
      supported_sample_rates: [24000],
      default_sample_rate: 24000,
      max_input_characters: maxInputCharacters,
      supports_word_timestamps: modelId !== openAiTtsModelId,
      word_timestamp_languages: modelId === openAiTtsModelId ? [] : ["zh-cn"],
      parameters: {
        speed_ratio: {
          type: "number",
          minimum: modelId === openAiTtsModelId ? 0.25 : 0.5,
          maximum: modelId === openAiTtsModelId ? 4 : 2,
        },
      },
    },
    last_sync: {
      sync_id: "88888888-8888-4888-8888-888888888888",
      model_id: modelId,
      trigger_source: "admin",
      status: "succeeded",
      page_limit: 100,
      page_count: 1,
      speaker_count: voices.length,
      error_summary: null,
      requested_at: "2026-07-15T00:00:00Z",
      started_at: "2026-07-15T00:00:00Z",
      completed_at: "2026-07-15T00:01:00Z",
      created_at: "2026-07-15T00:00:00Z",
      updated_at: "2026-07-15T00:01:00Z",
    },
    voices,
  };
}

function jsonResponse(body: unknown, status = 200) {
  return Promise.resolve(new Response(JSON.stringify(body), { status }));
}

const scriptSummaries = [
  {
    script_id: sourceScriptId,
    topic_id: null,
    source_topic_title: "停止内耗，从拆小目标开始",
    title: "别硬扛：稳定前进的方法",
    status: "approved",
    scene_count: 2,
    parent_id: null,
    created_at: "2026-07-15T00:00:00Z",
    updated_at: "2026-07-16T08:24:00Z",
  },
  {
    script_id: "77777777-2222-4777-8777-777777777777",
    topic_id: null,
    source_topic_title: "归档选题",
    title: "已经归档的脚本",
    status: "archived",
    scene_count: 1,
    parent_id: null,
    created_at: "2026-07-14T00:00:00Z",
    updated_at: "2026-07-14T08:24:00Z",
  },
];

const scriptDetail = {
  script_id: sourceScriptId,
  project_id: projectId,
  topic_id: null,
  topic_snapshot: { title: "停止内耗，从拆小目标开始" },
  title: "别硬扛：稳定前进的方法",
  hook: "停止内耗",
  status: "approved",
  parent_id: null,
  created_at: "2026-07-15T00:00:00Z",
  updated_at: "2026-07-16T08:24:00Z",
  scenes: [
    { scene_id: firstSceneId, sequence: 1, narration: "允许自己停一停。", visual_description: "停顿", emotion: "温暖", duration_sec: 5 },
    { scene_id: secondSceneId, sequence: 2, narration: "把目标拆小。", visual_description: "拆分", emotion: "平静", duration_sec: 5 },
  ],
};

function setupFetcher(options: {
  audioMaterials?: Array<Record<string, unknown>>;
  maxInputCharacters?: number;
  scripts?: Array<Record<string, unknown>>;
  scriptDetails?: Record<string, Record<string, unknown>>;
} = {}) {
  return vi.fn((input: RequestInfo | URL, init?: RequestInit) => {
    const url = String(input);
    if (url.endsWith("/api/model-options?type=speech")) return jsonResponse({ models: speechModels });
    if (url.endsWith("/api/model-options?type=text")) return jsonResponse({ models: textModels });
    if (url.includes("/voice-catalog")) {
      const modelId = url.includes(secondTtsModelId)
        ? secondTtsModelId
        : url.includes(openAiTtsModelId) ? openAiTtsModelId : ttsModelId;
      if (url.endsWith("/check") && init?.method === "POST") {
        return jsonResponse({ ...catalog(modelId).last_sync, status: "queued" }, 201);
      }
      return jsonResponse(catalog(modelId, options.maxInputCharacters));
    }
    if (url.includes(`/api/projects/${projectId}/scripts`)) {
      const scripts = options.scripts ?? scriptSummaries;
      return jsonResponse({ scripts, total: scripts.length, limit: 100, offset: 0 });
    }
    if (url.includes("/api/scripts/")) {
      const scriptId = url.split("/api/scripts/")[1]?.split("?")[0] ?? "";
      return jsonResponse(options.scriptDetails?.[scriptId] ?? scriptDetail);
    }
    if (url.includes("/materials?type=audio")) return jsonResponse({ materials: options.audioMaterials ?? [] });
    if (url.includes("/audio-materials/") && url.endsWith("/inspection")) {
      return jsonResponse({
        inspection_id: "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb",
        project_id: projectId,
        material_id: "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
        status: "succeeded",
        source_sha256: "a".repeat(64),
        file_size_bytes: 1024,
        duration_ms: 12500,
        container_format: "mp3",
        audio_codec: "mp3",
        sample_rate_hz: 24000,
        channel_count: 1,
        error_code: null,
        error_summary: null,
        started_at: "2026-07-15T00:00:00Z",
        completed_at: "2026-07-15T00:00:01Z",
        created_at: "2026-07-15T00:00:00Z",
        updated_at: "2026-07-15T00:00:01Z",
      });
    }
    if (url.endsWith("/sound-subtitle/tasks") && !init?.method) return jsonResponse({ tasks: [] });
    if (url.endsWith("/sound-subtitle/tasks/preflight")) {
      const body = JSON.parse(String(init?.body ?? "{}"));
      if (body.task_type === "asr") {
        return jsonResponse({
          task_type: "asr",
          model_id: asrModelId,
          model_display_name: "豆包 ASR",
          voice_snapshot: null,
          resource_usage: { audio_duration_ms: 12500, source_file_size_bytes: 1024, task_count: 1, output_count: 1 },
          normalized_parameters: { audio_format: "mp3", sample_rate: 24000 },
          confirmation_token: "asr-confirmation-token",
        });
      }
      return jsonResponse({
        task_type: "tts",
        model_id: ttsModelId,
        model_display_name: "豆包 TTS",
        voice_snapshot: { name: "测试女声" },
        resource_usage: { character_count: 4, task_count: 1, output_count: 1 },
        normalized_parameters: { audio_format: "mp3", sample_rate: 24000, speed_ratio: 1 },
        confirmation_token: "confirmation-token",
      });
    }
    if (url.endsWith("/sound-subtitle/tasks") && init?.method === "POST") {
      return jsonResponse({ task_id: "99999999-9999-4999-8999-999999999999", status: "queued", result: null }, 201);
    }
    if (url.endsWith("/api/agent/conversations") && init?.method === "POST") {
      return jsonResponse({ conversation_id: "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa" }, 201);
    }
    if (url.includes("/api/agent/conversations/") && init?.method === "POST") {
      return jsonResponse({
        user_message: { message_id: "user", role: "user", content: "推荐", metadata: {} },
        assistant_message: {
          message_id: "assistant",
          role: "assistant",
          content: "建议使用测试女声。",
          metadata: {
            recommended_voice_type: firstVoice.voice_type,
            language: "zh-cn",
            tts_text: "Agent 推荐文本",
            subtitle_segments: ["Agent 推荐文本"],
            parameters: { speed_ratio: 1.1 },
            requires_confirmation: true,
            tool_execution: false,
          },
        },
        run: { run_id: "run", status: "succeeded" },
      });
    }
    return jsonResponse({});
  });
}

describe("声音与字幕生成页面", () => {
  it("主表单、旁白和 Agent 使用已确认的可读字号层级", async () => {
    const { container } = render(
      <SoundSubtitlePage
        client={createApiClient({ baseUrl: "http://api.test", fetcher: setupFetcher() })}
        projectId={projectId}
        projectName="科技账号"
      />,
    );

    await waitFor(() => expect(screen.getByRole("combobox", { name: "音色" })).toHaveTextContent("测试女声"));
    const fontSize = (selector: string) => {
      const element = container.querySelector(selector);
      expect(element).not.toBeNull();
      const escapedSelector = selector.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
      const matches = Array.from(soundPageStyles.matchAll(new RegExp(`${escapedSelector}\\s*\\{[^}]*font-size:\\s*([^;]+);`, "g")));
      expect(matches.length).toBeGreaterThan(0);
      return matches.at(-1)?.[1].trim();
    };

    expect(fontSize(".soundModelTriggerCopy span")).toBe("12px");
    expect(fontSize(".soundModelTriggerCopy strong")).toBe("13px");
    expect(fontSize(".soundNarrationField textarea")).toBe("14px");
    expect(fontSize(".soundPrimaryActions .primaryAction")).toBe("13px");
    expect(fontSize(".soundAgentSessionRow > span")).toBe("11px");
    expect(fontSize(".soundAgentComposer textarea")).toBe("12px");
  });

  it("按确认原型展示页头与任务、配置、Agent 三栏且不再渲染底部任务表", async () => {
    const { container } = render(
      <SoundSubtitlePage
        client={createApiClient({ baseUrl: "http://api.test", fetcher: setupFetcher() })}
        projectId={projectId}
        projectName="科技账号"
      />,
    );

    await waitFor(() => expect(screen.getByRole("combobox", { name: "音色" })).toHaveTextContent("测试女声"));
    expect(screen.getByText("素材管理 / 声音与字幕生成")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "新建 TTS 任务" })).toBeInTheDocument();
    expect(screen.getByRole("complementary", { name: "配音任务列表" })).toBeInTheDocument();
    expect(screen.getByRole("region", { name: "TTS 配音配置" })).toBeInTheDocument();
    expect(screen.getByRole("complementary", { name: "声音 Agent" })).toBeInTheDocument();
    expect(screen.getByText("试听音频")).toBeInTheDocument();
    expect(screen.getByText("当前任务")).toBeInTheDocument();
    expect(screen.getByRole("slider", { name: "语速" })).toHaveValue("1");
    expect(screen.getByRole("slider", { name: "语速" }).parentElement).toHaveTextContent("1.0");
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
    expect(container.querySelector(".soundTaskSection")).not.toBeInTheDocument();
  });

  it("只展示双标签并按语言代码显示全部真实中文语言名称", async () => {
    render(
      <SoundSubtitlePage
        client={createApiClient({ baseUrl: "http://api.test", fetcher: setupFetcher() })}
        projectId={projectId}
        projectName="科技账号"
      />,
    );

    const voiceSelector = screen.getByRole("combobox", { name: "音色" });
    await waitFor(() => expect(voiceSelector).toHaveTextContent("测试女声"));
    expect(screen.getByRole("tab", { name: "TTS配音" })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: "字幕" })).toBeInTheDocument();
    expect(screen.queryByText("AI音乐")).not.toBeInTheDocument();
    expect(screen.queryByText("环境与音效")).not.toBeInTheDocument();
    expect(screen.queryByText("情绪风格")).not.toBeInTheDocument();
    const languageSelect = screen.getByRole("combobox", { name: "语言 / 口音" });
    fireEvent.click(languageSelect);
    const languageListbox = screen.getByRole("listbox", { name: "语言 / 口音选项" });
    const expectedLanguages = new Map([
      ["zh-cn", "简体中文"], ["zh", "中文"], ["en", "英语"], ["id", "印尼语"],
      ["pt-br", "巴西葡萄牙语"], ["ja", "日语"], ["mx", "墨西哥西语"], ["vi", "越南语"],
      ["th", "泰语"], ["es-mx", "西班牙语"], ["fil", "菲律宾语"], ["fr", "法语"],
      ["ru", "俄语"], ["de", "德语"], ["ko", "韩语"], ["ms", "马来语"],
      ["ar", "阿拉伯语"], ["it", "意大利语"],
    ]);
    for (const [value, label] of expectedLanguages) {
      expect(within(languageListbox).getByRole("option", { name: label })).toHaveAttribute("data-value", value);
    }
    fireEvent.click(within(languageListbox).getByRole("option", { name: "简体中文" }));
    expect(screen.queryByText("这是一段试听文案，不是语言名称。")).not.toBeInTheDocument();
    expect(screen.queryByText("English audition text")).not.toBeInTheDocument();
  });

  it("音色下拉展示真实原名、中文描述和标签并支持搜索与关闭", async () => {
    render(
      <SoundSubtitlePage
        client={createApiClient({ baseUrl: "http://api.test", fetcher: setupFetcher() })}
        projectId={projectId}
        projectName="科技账号"
      />,
    );

    const selector = screen.getByRole("combobox", { name: "音色" });
    await waitFor(() => expect(selector).toHaveTextContent("测试女声"));
    expect(selector).toHaveTextContent("适合知识旁白");
    expect(screen.getAllByText("适合知识旁白")).toHaveLength(1);
    fireEvent.click(selector);

    let listbox = screen.getByRole("listbox", { name: "可用音色" });
    expect(within(listbox).getByRole("option", { name: /测试女声.*适合知识旁白.*女.*成年.*简体中文/ })).toBeInTheDocument();
    expect(within(listbox).getByRole("option", { name: /Alastor 2\.0.*恐怖电影里的小丑.*男.*青年.*英语/ })).toBeInTheDocument();

    fireEvent.change(screen.getByRole("searchbox", { name: "搜索音色" }), { target: { value: "侵略性" } });
    listbox = screen.getByRole("listbox", { name: "可用音色" });
    const alastorOption = within(listbox).getByRole("option", { name: /Alastor 2\.0/ });
    expect(alastorOption).toBeInTheDocument();
    expect(within(listbox).queryByRole("option", { name: /测试女声/ })).not.toBeInTheDocument();

    fireEvent.click(alastorOption);
    expect(selector).toHaveTextContent("Alastor 2.0");
    expect(selector).toHaveTextContent("恐怖电影里的小丑");
    expect(screen.getByRole("combobox", { name: "语言 / 口音" })).toHaveTextContent("英语");

    fireEvent.click(selector);
    fireEvent.keyDown(document, { key: "Escape" });
    expect(screen.queryByRole("listbox", { name: "可用音色" })).not.toBeInTheDocument();
    fireEvent.click(selector);
    fireEvent.mouseDown(document.body);
    expect(screen.queryByRole("listbox", { name: "可用音色" })).not.toBeInTheDocument();
  });

  it("音色弹层使用语言与声线 Tag 交集筛选并保持扁平列表", async () => {
    render(
      <SoundSubtitlePage
        client={createApiClient({ baseUrl: "http://api.test", fetcher: setupFetcher() })}
        projectId={projectId}
        projectName="科技账号"
      />,
    );

    const selector = screen.getByRole("combobox", { name: "音色" });
    await waitFor(() => expect(selector).toHaveTextContent("测试女声"));
    fireEvent.click(selector);

    const languageFilters = screen.getByRole("group", { name: "按语言筛选音色" });
    const genderFilters = screen.getByRole("group", { name: "按声线筛选音色" });
    expect(within(languageFilters).getAllByRole("button").map((button) => button.textContent)).toEqual(["中文", "英文", "多语言"]);
    expect(within(genderFilters).getAllByRole("button").map((button) => button.textContent)).toEqual(["男声", "女声"]);

    const chineseFilter = within(languageFilters).getByRole("button", { name: "中文" });
    const femaleFilter = within(genderFilters).getByRole("button", { name: "女声" });
    fireEvent.click(chineseFilter);
    fireEvent.click(femaleFilter);

    let listbox = screen.getByRole("listbox", { name: "可用音色" });
    expect(within(listbox).getByRole("option", { name: /新闻女声/ })).toBeInTheDocument();
    expect(within(listbox).queryByRole("option", { name: /测试女声/ })).not.toBeInTheDocument();
    expect(within(listbox).queryByRole("option", { name: /Alastor 2\.0/ })).not.toBeInTheDocument();
    expect(listbox.querySelectorAll(":scope > [role='option']")).toHaveLength(1);

    fireEvent.click(chineseFilter);
    expect(chineseFilter).toHaveAttribute("aria-pressed", "false");
    fireEvent.change(screen.getByRole("searchbox", { name: "搜索音色" }), { target: { value: "知识" } });
    listbox = screen.getByRole("listbox", { name: "可用音色" });
    expect(within(listbox).getByRole("option", { name: /测试女声/ })).toBeInTheDocument();
    expect(within(listbox).queryByRole("option", { name: /新闻女声/ })).not.toBeInTheDocument();

    fireEvent.change(screen.getByRole("searchbox", { name: "搜索音色" }), { target: { value: "" } });
    fireEvent.click(within(languageFilters).getByRole("button", { name: "多语言" }));
    expect(within(listbox).getByRole("option", { name: /测试女声/ })).toBeInTheDocument();
    expect(within(listbox).queryByRole("option", { name: /新闻女声/ })).not.toBeInTheDocument();
  });

  it("TTS 模型使用与触发框对齐的自定义单选弹层", async () => {
    render(
      <SoundSubtitlePage
        client={createApiClient({ baseUrl: "http://api.test", fetcher: setupFetcher() })}
        projectId={projectId}
        projectName="科技账号"
      />,
    );
    const modelSelector = screen.getByRole("combobox", { name: "TTS 模型" });
    await waitFor(() => expect(modelSelector).toHaveTextContent("豆包 TTS"));
    expect(modelSelector.tagName).toBe("BUTTON");

    fireEvent.click(modelSelector);
    const listbox = screen.getByRole("listbox", { name: "TTS 模型选项" });
    expect(within(listbox).getByRole("option", { name: /豆包 TTS 备用/ })).toBeInTheDocument();
    fireEvent.click(within(listbox).getByRole("option", { name: /豆包 TTS 备用/ }));
    expect(modelSelector).toHaveTextContent("豆包 TTS 备用");
    await waitFor(() => expect(screen.getByRole("combobox", { name: "音色" })).toHaveTextContent("已失效"));

    fireEvent.click(modelSelector);
    fireEvent.keyDown(document, { key: "Escape" });
    expect(screen.queryByRole("listbox", { name: "TTS 模型选项" })).not.toBeInTheDocument();
  });

  it("OpenAI Audio Speech 中转可生成配音但阻止 TTS 时间戳字幕", async () => {
    render(
      <SoundSubtitlePage
        client={createApiClient({ baseUrl: "http://api.test", fetcher: setupFetcher() })}
        projectId={projectId}
        projectName="科技账号"
      />,
    );
    const modelSelector = screen.getByRole("combobox", { name: "TTS 模型" });
    await waitFor(() => expect(modelSelector).toHaveTextContent("豆包 TTS"));
    fireEvent.click(modelSelector);
    fireEvent.click(screen.getByRole("option", { name: /ZeekAI Seed TTS/ }));
    await waitFor(() => expect(modelSelector).toHaveTextContent("ZeekAI Seed TTS"));
    fireEvent.click(screen.getByRole("combobox", { name: "音色" }));
    fireEvent.click(screen.getByRole("option", { name: /测试男声/ }));
    fireEvent.change(screen.getByLabelText("配音文本"), { target: { value: "你好世界" } });
    await waitFor(() => {
      expect(screen.getByRole("button", { name: "生成配音" })).toBeEnabled();
    });

    fireEvent.click(screen.getByRole("tab", { name: "字幕" }));
    expect(screen.getByRole("button", { name: "TTS 字词时间戳" })).toBeDisabled();
    expect(screen.getByText("当前 TTS 中转模型不返回可信字词时间戳，请使用已有音频 ASR。")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "生成配音与字幕" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "已有音频 ASR" })).toBeEnabled();
  });

  it("切换模型后保留并标记失效音色，不静默替换", async () => {
    render(
      <SoundSubtitlePage
        client={createApiClient({ baseUrl: "http://api.test", fetcher: setupFetcher() })}
        projectId={projectId}
        projectName="科技账号"
      />,
    );
    const selector = screen.getByRole("combobox", { name: "音色" });
    await waitFor(() => expect(selector).toHaveTextContent("测试女声"));

    fireEvent.click(screen.getByRole("combobox", { name: "TTS 模型" }));
    fireEvent.click(screen.getByRole("option", { name: /豆包 TTS 备用/ }));

    await waitFor(() => expect(selector).toHaveTextContent("已失效"));
    fireEvent.click(selector);
    expect(screen.getByRole("option", { name: /测试女声.*已失效/ })).toHaveAttribute("aria-disabled", "true");
    expect(screen.getByRole("alert")).toHaveTextContent("原音色在当前模型中不可用");
    expect(screen.getByRole("button", { name: "生成配音" })).toBeDisabled();
  });

  it("声音 Agent 建议需手动应用且不会直接创建声音任务", async () => {
    const fetcher = setupFetcher();
    render(
      <SoundSubtitlePage
        client={createApiClient({ baseUrl: "http://api.test", fetcher })}
        projectId={projectId}
        projectName="科技账号"
      />,
    );
    await waitFor(() => expect(screen.getByRole("combobox", { name: "音色" })).toHaveTextContent("测试女声"));

    fireEvent.change(screen.getByLabelText("声音 Agent 输入"), { target: { value: "推荐沉稳声音" } });
    fireEvent.click(screen.getByRole("button", { name: "发送建议" }));

    expect(await screen.findByText("建议使用测试女声。")).toBeInTheDocument();
    expect(screen.getByLabelText("配音文本")).toHaveValue("");
    expect(fetcher.mock.calls.some(([url, init]) => String(url).endsWith("/sound-subtitle/tasks") && init?.method === "POST")).toBe(false);

    fireEvent.click(screen.getByRole("button", { name: "应用建议" }));
    expect(screen.getByLabelText("配音文本")).toHaveValue("Agent 推荐文本");
  });

  it("生成前展示字符数和任务数，确认后才创建任务", async () => {
    const fetcher = setupFetcher();
    render(
      <SoundSubtitlePage
        client={createApiClient({ baseUrl: "http://api.test", fetcher })}
        projectId={projectId}
        projectName="科技账号"
      />,
    );
    await waitFor(() => expect(screen.getByRole("combobox", { name: "音色" })).toHaveTextContent("测试女声"));
    fireEvent.change(screen.getByLabelText("配音文本"), { target: { value: "你好世界" } });

    fireEvent.click(screen.getByRole("button", { name: "生成配音" }));

    const dialog = await screen.findByRole("dialog", { name: "确认声音任务" });
    expect(within(dialog).getByText("4 字符")).toBeInTheDocument();
    expect(within(dialog).getByText("1 个任务")).toBeInTheDocument();
    expect(fetcher.mock.calls.filter(([url, init]) => String(url).endsWith("/sound-subtitle/tasks") && init?.method === "POST")).toHaveLength(0);
    const preflightCall = fetcher.mock.calls.find(([url]) => String(url).endsWith("/sound-subtitle/tasks/preflight"));
    const preflightPayload = JSON.parse(String(preflightCall?.[1]?.body));
    expect(preflightPayload).toMatchObject({ language: "zh-cn" });
    expect(preflightPayload).not.toHaveProperty("emotion");

    fireEvent.click(within(dialog).getByRole("button", { name: "确认生成" }));
    await waitFor(() => {
      expect(fetcher.mock.calls.filter(([url, init]) => String(url).endsWith("/sound-subtitle/tasks") && init?.method === "POST")).toHaveLength(1);
    });
  });

  it("从当前账号已有脚本选择分镜并明确替换旁白，同时提交来源引用", async () => {
    const fetcher = setupFetcher();
    render(
      <SoundSubtitlePage
        client={createApiClient({ baseUrl: "http://api.test", fetcher })}
        projectId={projectId}
        projectName="科技账号"
      />,
    );
    await waitFor(() => expect(screen.getByRole("combobox", { name: "音色" })).toHaveTextContent("测试女声"));
    fireEvent.change(screen.getByLabelText("配音文本"), { target: { value: "旧旁白" } });
    fireEvent.click(screen.getByRole("button", { name: "导入脚本" }));

    const importDialog = await screen.findByRole("dialog", { name: "从脚本创作导入旁白" });
    expect(within(importDialog).getByText("别硬扛：稳定前进的方法")).toBeInTheDocument();
    expect(within(importDialog).queryByText("已经归档的脚本")).not.toBeInTheDocument();
    expect(within(importDialog).getByText(/当前旁白已有 3 字/)).toBeInTheDocument();
    await waitFor(() => expect(within(importDialog).getByRole("checkbox", { name: "镜头 01" })).toBeChecked());
    expect(within(importDialog).getByRole("checkbox", { name: "镜头 02" })).toBeChecked();
    fireEvent.click(within(importDialog).getByRole("checkbox", { name: "镜头 02" }));
    fireEvent.click(within(importDialog).getByRole("button", { name: "替换并导入" }));
    expect(screen.getByLabelText("配音文本")).toHaveValue("允许自己停一停。");

    fireEvent.click(screen.getByRole("button", { name: "生成配音" }));
    await screen.findByRole("dialog", { name: "确认声音任务" });
    const preflightCall = fetcher.mock.calls.find(([url]) => String(url).endsWith("/sound-subtitle/tasks/preflight"));
    expect(JSON.parse(String(preflightCall?.[1]?.body))).toMatchObject({
      source_script_id: sourceScriptId,
      source_script_updated_at: scriptDetail.updated_at,
      source_script_scene_ids: [firstSceneId],
      text_content: "允许自己停一停。",
    });
  });

  it("脚本旁白超过当前模型字符上限时阻止导入且不截断", async () => {
    const fetcher = setupFetcher({ maxInputCharacters: 10 });
    render(
      <SoundSubtitlePage
        client={createApiClient({ baseUrl: "http://api.test", fetcher })}
        projectId={projectId}
        projectName="科技账号"
      />,
    );
    await waitFor(() => expect(screen.getByRole("combobox", { name: "音色" })).toHaveTextContent("测试女声"));
    fireEvent.click(screen.getByRole("button", { name: "导入脚本" }));
    const importDialog = await screen.findByRole("dialog", { name: "从脚本创作导入旁白" });
    await waitFor(() => expect(within(importDialog).getByRole("checkbox", { name: "镜头 01" })).toBeChecked());
    expect(within(importDialog).getByRole("alert")).toHaveTextContent("超过当前模型 10 字上限");
    expect(within(importDialog).getByRole("button", { name: "导入旁白" })).toBeDisabled();
    expect(screen.getByLabelText("配音文本")).toHaveValue("");
  });

  it("已有音频 ASR 必须先完成检查并使用服务端真实时长确认", async () => {
    const materialId = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
    const fetcher = setupFetcher({
      audioMaterials: [{
        material_id: materialId,
        project_id: projectId,
        material_type: "audio",
        file_url: "/assets/uploads/voice.mp3",
        thumbnail_url: null,
        file_name: "voice.mp3",
        tags: [],
        metadata: {},
        source: "user_upload",
        audio_usage: "other",
        work_id: null,
        work_version_id: null,
        generation: null,
        usage_count: 0,
        status: "active",
        created_at: "2026-07-15T00:00:00Z",
        updated_at: "2026-07-15T00:00:00Z",
      }],
    });
    render(
      <SoundSubtitlePage
        client={createApiClient({ baseUrl: "http://api.test", fetcher })}
        projectId={projectId}
        projectName="科技账号"
      />,
    );
    await waitFor(() => expect(screen.getByRole("combobox", { name: "音色" })).toHaveTextContent("测试女声"));
    fireEvent.click(screen.getByRole("tab", { name: "字幕" }));
    fireEvent.click(screen.getByRole("button", { name: "已有音频 ASR" }));
    fireEvent.change(screen.getByLabelText("已有音频素材"), { target: { value: materialId } });
    expect(screen.getByRole("button", { name: "生成字幕" })).toBeDisabled();

    fireEvent.click(screen.getByRole("button", { name: "检查音频" }));
    expect(await screen.findByText("12.5 秒")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "生成字幕" })).toBeEnabled();
    fireEvent.click(screen.getByRole("button", { name: "生成字幕" }));

    const dialog = await screen.findByRole("dialog", { name: "确认声音任务" });
    expect(within(dialog).getByText("12.5 秒")).toBeInTheDocument();
    const preflightCall = fetcher.mock.calls.find(([url]) => String(url).endsWith("/sound-subtitle/tasks/preflight"));
    expect(JSON.parse(String(preflightCall?.[1]?.body))).toMatchObject({
      task_type: "asr",
      model_id: asrModelId,
      source_audio_material_id: materialId,
      audio_inspection_id: "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb",
    });
  });
});
