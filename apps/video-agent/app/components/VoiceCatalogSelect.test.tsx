import "@testing-library/jest-dom/vitest";
import { fireEvent, render, screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { VoiceCatalogEntry } from "../lib/api";
import { VoiceCatalogSelect } from "./VoiceCatalogSelect";

const voices: VoiceCatalogEntry[] = [
  voice({
    voice_id: "voice-zh-female",
    voice_type: "zh_female_cancan",
    name: "灿灿",
    gender: "female",
    age: "adult",
    languages: [{ Language: "zh-cn", Text: "试听" }],
    description: "温暖自然，适合知识旁白",
  }),
  voice({
    voice_id: "voice-en-male",
    voice_type: "en_male_alastor",
    name: "Alastor 2.0",
    gender: "male",
    age: "young",
    languages: [{ Language: "en", Text: "Preview" }],
    description: "声音尖锐，有侵略性",
  }),
  voice({
    voice_id: "voice-multi-female",
    voice_type: "multi_female_nova",
    name: "Nova",
    gender: "female",
    age: "young",
    languages: [
      { Language: "zh-cn", Text: "试听" },
      { Language: "en", Text: "Preview" },
    ],
    description: "自然的多语言女声",
  }),
];

describe("VoiceCatalogSelect", () => {
  it("支持数量、搜索、语言和声线筛选，并可选择结果", () => {
    const onChange = vi.fn();
    render(
      <VoiceCatalogSelect
        voices={voices}
        selectedVoice={voices[0]}
        selectedVoiceType={voices[0].voice_type}
        selectedVoiceLabel={voices[0].name}
        invalid={false}
        disabled={false}
        onChange={onChange}
        variant="compact"
      />,
    );

    fireEvent.click(screen.getByRole("combobox", { name: "音色" }));
    expect(screen.getByText("3 个可用")).toBeVisible();

    const languageFilters = screen.getByRole("group", { name: "按语言筛选音色" });
    const genderFilters = screen.getByRole("group", { name: "按声线筛选音色" });
    expect(within(languageFilters).getAllByRole("button")).toHaveLength(3);
    expect(within(genderFilters).getAllByRole("button")).toHaveLength(2);

    fireEvent.click(within(languageFilters).getByRole("button", { name: "英文" }));
    fireEvent.click(within(genderFilters).getByRole("button", { name: "男声" }));
    fireEvent.change(screen.getByRole("searchbox", { name: "搜索音色" }), {
      target: { value: "侵略性" },
    });

    const listbox = screen.getByRole("listbox", { name: "可用音色" });
    expect(within(listbox).getAllByRole("option")).toHaveLength(1);
    fireEvent.click(within(listbox).getByRole("option", { name: /Alastor 2\.0/ }));
    expect(onChange).toHaveBeenCalledWith("en_male_alastor");
  });

  it("支持方向键和 Enter 选择筛选后的音色", () => {
    const onChange = vi.fn();
    render(
      <VoiceCatalogSelect
        voices={voices}
        selectedVoice={voices[0]}
        selectedVoiceType={voices[0].voice_type}
        selectedVoiceLabel={voices[0].name}
        invalid={false}
        disabled={false}
        onChange={onChange}
        variant="compact"
      />,
    );

    fireEvent.click(screen.getByRole("combobox", { name: "音色" }));
    const search = screen.getByRole("searchbox", { name: "搜索音色" });
    fireEvent.keyDown(search, { key: "ArrowDown" });
    fireEvent.keyDown(search, { key: "Enter" });
    expect(onChange).toHaveBeenCalledWith("en_male_alastor");
  });

  it("保留并明确标记当前模型中失效的原音色", () => {
    render(
      <VoiceCatalogSelect
        voices={voices.slice(1)}
        selectedVoice={null}
        selectedVoiceType="zh_female_cancan"
        selectedVoiceLabel="灿灿"
        invalid
        disabled={false}
        onChange={() => undefined}
        variant="compact"
      />,
    );

    const trigger = screen.getByRole("combobox", { name: "音色" });
    expect(trigger).toHaveTextContent("灿灿（已失效）");
    fireEvent.click(trigger);
    expect(screen.getByRole("option", { name: /灿灿.*当前模型不可用.*已失效/ })).toHaveAttribute("aria-disabled", "true");
  });
});

function voice(overrides: Partial<VoiceCatalogEntry>): VoiceCatalogEntry {
  return {
    voice_id: "voice-id",
    voice_type: "voice-type",
    resource_id: "seed-tts-2.0",
    name: "音色",
    avatar_url: null,
    gender: null,
    age: null,
    categories: [],
    normal_labels: [],
    special_labels: [],
    trial_url: null,
    short_trial_url: null,
    languages: [],
    emotions: [],
    description: "",
    is_available: true,
    catalog_version: 1,
    created_at: "2026-07-21T00:00:00Z",
    updated_at: "2026-07-21T00:00:00Z",
    ...overrides,
  };
}
