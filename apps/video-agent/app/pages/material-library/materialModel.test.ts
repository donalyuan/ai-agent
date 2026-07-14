import { describe, expect, it } from "vitest";
import type { Material } from "../../lib/api";
import {
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
});
