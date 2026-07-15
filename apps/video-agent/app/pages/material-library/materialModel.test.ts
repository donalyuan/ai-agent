import { describe, expect, it } from "vitest";
import type { Material } from "../../lib/api";
import {
  audioUsageLabels,
  materialGenerationRows,
  materialSourceLabels,
  formatMaterialFileSummary,
  materialEditPayload,
  materialToForm,
} from "./materialModel";

const material: Material = {
  material_id: "abababab-abab-4aba-8aba-abababababab",
  project_id: "11111111-1111-4111-8111-111111111111",
  material_type: "image",
  file_url: "http://api.test/assets/uploads/project/cover.png",
  thumbnail_url: null,
  file_name: "封面素材",
  tags: ["封面"],
  metadata: {
    source: "user_upload",
    storage_provider: "local",
    mime_type: "image/png",
    format: "png",
    file_size_bytes: 2_515_456,
    width: 1920,
    height: 1080,
  },
  source: "user_upload",
  audio_usage: null,
  work_id: null,
  work_version_id: null,
  generation: null,
  usage_count: 0,
  status: "active",
  created_at: "2026-07-14T00:00:00Z",
  updated_at: "2026-07-14T00:00:00Z",
};

describe("素材编辑模型", () => {
  it("编辑名称和标签时保留所有系统字段", () => {
    const payload = materialEditPayload(material, {
      ...materialToForm(material),
      file_name: "新封面",
      tags_text: "封面，办公，封面",
    });

    expect(payload).toEqual({
      material_type: "image",
      file_url: material.file_url,
      thumbnail_url: null,
      file_name: "新封面",
      tags: ["封面", "办公"],
      metadata: material.metadata,
    });
  });

  it("将系统 metadata 格式化为只读文件摘要", () => {
    expect(formatMaterialFileSummary(material)).toBe("图片 · PNG · 1920 × 1080 · 2.4 MB");
  });

  it("将标准声音用途和素材来源映射为中文", () => {
    expect(audioUsageLabels.tts).toBe("TTS 配音");
    expect(audioUsageLabels.action_sfx).toBe("动作音效");
    expect(materialSourceLabels.work_generation).toBe("作品生成");
  });

  it("只从 TTS 生成快照提取允许展示的审计字段", () => {
    const generatedMaterial: Material = {
      ...material,
      material_type: "audio",
      source: "work_generation",
      audio_usage: "tts",
      work_id: "31313131-3131-4131-8131-313131313131",
      work_version_id: "32323232-3232-4232-8232-323232323232",
      generation: {
        work_id: "31313131-3131-4131-8131-313131313131",
        work_version_id: "32323232-3232-4232-8232-323232323232",
        generation_run_id: "33333333-3333-4333-8333-333333333333",
        generation_step_id: "34343434-3434-4434-8434-343434343434",
        artifact_role: "tts_audio",
        model_snapshot: { display_name: "豆包语音 2.0", api_key: "禁止展示" },
        voice_snapshot: {
          speaker_name: "灿灿",
          language: "zh-CN",
          emotion: "温暖",
          speed: 1.05,
          token: "禁止展示",
        },
        prompt_snapshot: { text_summary: "三段旁白，共 128 字", password: "禁止展示" },
        resource_usage: { characters: 128 },
        request_trace_id: "req_7P2K8",
        duration_sec: 31.4,
      },
    };

    const rows = materialGenerationRows(generatedMaterial);
    const rendered = JSON.stringify(rows);

    expect(rows).toEqual(expect.arrayContaining([
      expect.objectContaining({ label: "模型", value: "豆包语音 2.0" }),
      expect.objectContaining({ label: "音色", value: "灿灿" }),
      expect.objectContaining({ label: "声音参数", value: "情绪 温暖 · 语速 1.05" }),
      expect.objectContaining({ label: "文本摘要", value: "三段旁白，共 128 字" }),
      expect.objectContaining({ label: "语言 / 时长", value: "zh-CN · 31.4 秒" }),
      expect.objectContaining({ label: "request trace", value: "req_7P2K8" }),
    ]));
    expect(rendered).not.toContain("禁止展示");
    expect(rendered).not.toContain("api_key");
    expect(rendered).not.toContain("token");
  });

  it("字幕快照展示对齐来源和来源音频", () => {
    const rows = materialGenerationRows({
      ...material,
      material_type: "subtitle",
      source: "work_generation",
      generation: {
        alignment_source: "tts_timestamp",
        source_audio_material_id: "35353535-3535-4535-8535-353535353535",
        subtitle_format: "srt",
        timeline_snapshot: { version: "timeline-v3", language: "zh-CN" },
      },
    });

    expect(rows).toEqual(expect.arrayContaining([
      expect.objectContaining({ label: "对齐来源", value: "TTS 时间戳" }),
      expect.objectContaining({ label: "来源音频", value: "35353535-3535-4535-8535-353535353535" }),
      expect.objectContaining({ label: "字幕 / 时间轴", value: "SRT · timeline-v3" }),
    ]));
  });
});
