"use client";

import {
  type CSSProperties,
  type KeyboardEvent as ReactKeyboardEvent,
  useCallback,
  useEffect,
  useId,
  useMemo,
  useRef,
  useState,
} from "react";
import { createPortal } from "react-dom";
import type { VoiceCatalogEntry } from "../lib/api";

type VoiceLanguageFilter = "chinese" | "english" | "multilingual";
type VoiceGenderFilter = "male" | "female";
type VoiceCatalogSelectVariant = "compact" | "detailed";

type VoiceSelectEntry = {
  key: string;
  voiceType: string;
  name: string;
  description: string;
  tags: string[];
  searchText: string;
  languageGroup: VoiceLanguageFilter;
  genderGroup: VoiceGenderFilter | null;
  disabled: boolean;
};

type Props = {
  voices: VoiceCatalogEntry[];
  selectedVoice: VoiceCatalogEntry | null;
  selectedVoiceType: string;
  selectedVoiceLabel: string;
  invalid: boolean;
  disabled: boolean;
  onChange: (voiceType: string) => void;
  variant?: VoiceCatalogSelectVariant;
  popoverWidth?: number;
};

const voiceLanguageFilters: Array<{ value: VoiceLanguageFilter; label: string; title?: string }> = [
  { value: "chinese", label: "中文" },
  { value: "english", label: "英文" },
  { value: "multilingual", label: "多语言", title: "包含其他语种、语言未知及支持多个语种的音色" },
];

const voiceGenderFilters: Array<{ value: VoiceGenderFilter; label: string }> = [
  { value: "male", label: "男声" },
  { value: "female", label: "女声" },
];

export function VoiceCatalogSelect({
  voices,
  selectedVoice,
  selectedVoiceType,
  selectedVoiceLabel,
  invalid,
  disabled,
  onChange,
  variant = "detailed",
  popoverWidth,
}: Props) {
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [languageFilter, setLanguageFilter] = useState<VoiceLanguageFilter | null>(null);
  const [genderFilter, setGenderFilter] = useState<VoiceGenderFilter | null>(null);
  const [activeIndex, setActiveIndex] = useState(0);
  const [popoverStyle, setPopoverStyle] = useState<CSSProperties | null>(null);
  const rootRef = useRef<HTMLDivElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const popoverRef = useRef<HTMLDivElement>(null);
  const searchRef = useRef<HTMLInputElement>(null);
  const listboxId = useId();

  const entries = useMemo(() => {
    const availableEntries = voices.map((voice) => voiceSelectEntry(voice));
    if (!invalid || !selectedVoiceType) return availableEntries;
    const invalidEntry = selectedVoice
      ? voiceSelectEntry(selectedVoice, true)
      : missingVoiceSelectEntry(selectedVoiceType, selectedVoiceLabel);
    return [invalidEntry, ...availableEntries];
  }, [invalid, selectedVoice, selectedVoiceLabel, selectedVoiceType, voices]);

  const filteredEntries = useMemo(() => {
    const normalizedQuery = query.trim().toLocaleLowerCase();
    return entries.filter((entry) => (
      (!normalizedQuery || entry.searchText.includes(normalizedQuery))
      && (!languageFilter || entry.languageGroup === languageFilter)
      && (!genderFilter || entry.genderGroup === genderFilter)
    ));
  }, [entries, genderFilter, languageFilter, query]);

  const triggerName = selectedVoice?.name || selectedVoiceLabel || "请选择音色";
  const triggerDescription = invalid
    ? `${selectedVoice ? voiceDescription(selectedVoice) : "当前模型不可用"} · 已失效`
    : selectedVoice
      ? voiceDescription(selectedVoice)
      : "请选择目录音色";
  const filtersActive = Boolean(query.trim() || languageFilter || genderFilter);
  const filteredAvailableCount = filteredEntries.filter((entry) => !entry.disabled).length;

  const closeMenu = useCallback((restoreFocus = false) => {
    setOpen(false);
    setQuery("");
    setLanguageFilter(null);
    setGenderFilter(null);
    setActiveIndex(0);
    setPopoverStyle(null);
    if (restoreFocus) triggerRef.current?.focus();
  }, []);

  const updatePopoverPosition = useCallback(() => {
    const trigger = triggerRef.current;
    if (!trigger) return;
    const rect = trigger.getBoundingClientRect();
    const viewportPadding = 12;
    const desiredWidth = popoverWidth ?? Math.max(rect.width, 484);
    const width = Math.min(desiredWidth, window.innerWidth - viewportPadding * 2);
    const left = Math.min(
      Math.max(viewportPadding, rect.right - width),
      window.innerWidth - width - viewportPadding,
    );
    const top = rect.bottom + 6;
    setPopoverStyle({
      top,
      left,
      width,
      maxHeight: Math.max(280, window.innerHeight - top - viewportPadding),
    });
  }, [popoverWidth]);

  useEffect(() => {
    if (!open) return;
    updatePopoverPosition();
    const handleOutsideClick = (event: MouseEvent) => {
      const target = event.target as Node;
      if (!rootRef.current?.contains(target) && !popoverRef.current?.contains(target)) closeMenu();
    };
    const handleEscape = (event: KeyboardEvent) => {
      if (event.key === "Escape") closeMenu(true);
    };
    document.addEventListener("mousedown", handleOutsideClick);
    document.addEventListener("keydown", handleEscape);
    window.addEventListener("resize", updatePopoverPosition);
    window.addEventListener("scroll", updatePopoverPosition, true);
    return () => {
      document.removeEventListener("mousedown", handleOutsideClick);
      document.removeEventListener("keydown", handleEscape);
      window.removeEventListener("resize", updatePopoverPosition);
      window.removeEventListener("scroll", updatePopoverPosition, true);
    };
  }, [closeMenu, open, updatePopoverPosition]);

  useEffect(() => {
    if (open && popoverStyle) searchRef.current?.focus();
  }, [open, popoverStyle]);

  useEffect(() => {
    setActiveIndex((current) => Math.min(current, Math.max(filteredEntries.length - 1, 0)));
  }, [filteredEntries.length]);

  function toggleMenu() {
    if (disabled) return;
    if (open) {
      closeMenu();
      return;
    }
    const selectedIndex = entries.findIndex((entry) => entry.voiceType === selectedVoiceType);
    setActiveIndex(selectedIndex >= 0 ? selectedIndex : 0);
    setOpen(true);
  }

  function chooseEntry(entry: VoiceSelectEntry) {
    if (entry.disabled) return;
    onChange(entry.voiceType);
    closeMenu(true);
  }

  function handleSearchKeyDown(event: ReactKeyboardEvent<HTMLInputElement>) {
    if (!filteredEntries.length) return;
    if (event.key === "ArrowDown") {
      event.preventDefault();
      setActiveIndex((current) => Math.min(current + 1, filteredEntries.length - 1));
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      setActiveIndex((current) => Math.max(current - 1, 0));
    } else if (event.key === "Enter") {
      event.preventDefault();
      const entry = filteredEntries[activeIndex];
      if (entry) chooseEntry(entry);
    }
  }

  const popover = open && popoverStyle ? (
    <div className="voiceCatalogPopover" ref={popoverRef} style={popoverStyle}>
      <div className="voiceCatalogPopoverHeader">
        <strong>选择音色</strong>
        <span>{filtersActive ? `${filteredAvailableCount} / ${voices.length} 个可用` : `${voices.length} 个可用`}</span>
      </div>
      <input
        ref={searchRef}
        type="search"
        role="searchbox"
        aria-label="搜索音色"
        aria-activedescendant={filteredEntries[activeIndex] ? `${listboxId}-${activeIndex}` : undefined}
        placeholder="搜索名称、中文描述或标签"
        value={query}
        onChange={(event) => {
          setQuery(event.target.value);
          setActiveIndex(0);
        }}
        onKeyDown={handleSearchKeyDown}
      />
      <div className="voiceCatalogFilters">
        <div className="voiceCatalogFilterGroup" role="group" aria-label="按语言筛选音色">
          <span>语言</span>
          <div className="voiceCatalogFilterTags">
            {voiceLanguageFilters.map((filter) => (
              <button
                type="button"
                className={languageFilter === filter.value ? "active" : ""}
                aria-pressed={languageFilter === filter.value}
                title={filter.title}
                key={filter.value}
                onClick={() => {
                  setLanguageFilter((current) => current === filter.value ? null : filter.value);
                  setActiveIndex(0);
                }}
              >
                {filter.label}
              </button>
            ))}
          </div>
        </div>
        <div className="voiceCatalogFilterGroup" role="group" aria-label="按声线筛选音色">
          <span>声线</span>
          <div className="voiceCatalogFilterTags">
            {voiceGenderFilters.map((filter) => (
              <button
                type="button"
                className={genderFilter === filter.value ? "active" : ""}
                aria-pressed={genderFilter === filter.value}
                key={filter.value}
                onClick={() => {
                  setGenderFilter((current) => current === filter.value ? null : filter.value);
                  setActiveIndex(0);
                }}
              >
                {filter.label}
              </button>
            ))}
          </div>
        </div>
      </div>
      <div id={listboxId} className="voiceCatalogList" role="listbox" aria-label="可用音色">
        {filteredEntries.map((entry, index) => (
          <button
            id={`${listboxId}-${index}`}
            type="button"
            role="option"
            aria-selected={entry.voiceType === selectedVoiceType}
            aria-disabled={entry.disabled ? "true" : undefined}
            className={`voiceCatalogOption${entry.voiceType === selectedVoiceType ? " selected" : ""}${entry.disabled ? " invalid" : ""}${index === activeIndex ? " active" : ""}`}
            key={entry.key}
            onMouseEnter={() => setActiveIndex(index)}
            onClick={() => chooseEntry(entry)}
          >
            <strong>{entry.name}</strong>
            <span className="voiceCatalogOptionDescription">{entry.description}</span>
            <span className="voiceCatalogOptionTags">{entry.tags.join(" · ") || "目录音色"}</span>
          </button>
        ))}
        {!filteredEntries.length && <p className="voiceCatalogEmpty">没有匹配的音色</p>}
      </div>
    </div>
  ) : null;

  return (
    <div className={`voiceCatalogSelect ${variant === "compact" ? "compact" : "detailed"}`} ref={rootRef}>
      <button
        ref={triggerRef}
        type="button"
        role="combobox"
        aria-label="音色"
        aria-expanded={open}
        aria-controls={open ? listboxId : undefined}
        aria-haspopup="listbox"
        aria-invalid={invalid || undefined}
        className={`voiceCatalogTrigger${invalid ? " invalid" : ""}`}
        disabled={disabled}
        onClick={toggleMenu}
      >
        {variant === "compact" ? (
          <span className="voiceCatalogTriggerCompact">{invalid ? `${triggerName}（已失效）` : triggerName}</span>
        ) : (
          <span className="voiceCatalogTriggerCopy">
            <span className="voiceCatalogTriggerLabel">音色</span>
            <strong>{triggerName}</strong>
            <small>{triggerDescription}</small>
          </span>
        )}
        <span className={`voiceCatalogChevron${open ? " open" : ""}`} aria-hidden="true" />
      </button>
      {popover && createPortal(popover, document.body)}
    </div>
  );
}

function voiceSelectEntry(voice: VoiceCatalogEntry, disabled = false): VoiceSelectEntry {
  const tags = voiceTagLabels(voice);
  const searchableLabels = [
    voice.name,
    voice.description,
    voice.gender || "",
    voice.age || "",
    ...tags,
    ...voice.normal_labels,
    ...voice.special_labels,
    ...collectCatalogStrings(voice.categories),
  ];
  return {
    key: voice.voice_id,
    voiceType: voice.voice_type,
    name: voice.name,
    description: voiceDescription(voice),
    tags,
    searchText: searchableLabels.join("\n").toLocaleLowerCase(),
    languageGroup: voiceLanguageGroup(voice),
    genderGroup: voiceGenderGroup(voice.gender),
    disabled,
  };
}

function missingVoiceSelectEntry(voiceType: string, label: string): VoiceSelectEntry {
  const name = label || voiceType;
  return {
    key: `missing-${voiceType}`,
    voiceType,
    name,
    description: "当前模型不可用",
    tags: ["已失效"],
    searchText: `${name}\n${voiceType}\n已失效`.toLocaleLowerCase(),
    languageGroup: "multilingual",
    genderGroup: null,
    disabled: true,
  };
}

function voiceLanguageGroup(voice: VoiceCatalogEntry): VoiceLanguageFilter {
  const languageCodes = extractLanguageOptions(voice.languages)
    .map((option) => option.value.trim().toLocaleLowerCase())
    .filter(Boolean);
  if (languageCodes.length && languageCodes.every(isChineseLanguageCode)) return "chinese";
  if (languageCodes.length && languageCodes.every(isEnglishLanguageCode)) return "english";
  return "multilingual";
}

function isChineseLanguageCode(value: string) {
  return value === "zh" || value.startsWith("zh-");
}

function isEnglishLanguageCode(value: string) {
  return value === "en" || value.startsWith("en-");
}

function voiceGenderGroup(value: string | null): VoiceGenderFilter | null {
  if (!value) return null;
  const normalized = value.trim().toLocaleLowerCase();
  if (["male", "man", "男", "男性", "男声"].includes(normalized)) return "male";
  if (["female", "woman", "女", "女性", "女声"].includes(normalized)) return "female";
  return null;
}

function voiceDescription(voice: VoiceCatalogEntry) {
  return voice.description.trim()
    || voice.normal_labels.find((label) => label.trim())
    || "暂无中文描述";
}

function voiceTagLabels(voice: VoiceCatalogEntry) {
  return uniqueStrings([
    genderLabel(voice.gender),
    ageLabel(voice.age),
    ...extractLanguageOptions(voice.languages).map((option) => option.label),
  ].filter((label): label is string => Boolean(label)));
}

function genderLabel(value: string | null) {
  if (!value) return "";
  const normalized = value.trim().toLocaleLowerCase();
  const labels: Record<string, string> = { male: "男", man: "男", female: "女", woman: "女" };
  return labels[normalized] || value.trim();
}

function ageLabel(value: string | null) {
  if (!value) return "";
  const normalized = value.trim().toLocaleLowerCase().replaceAll("_", "-");
  const labels: Record<string, string> = {
    child: "儿童",
    kid: "儿童",
    teen: "少年/少女",
    teenager: "少年/少女",
    youth: "青年",
    young: "青年",
    adult: "成年",
    "middle-aged": "中年",
    senior: "老年",
    elderly: "老年",
  };
  return labels[normalized] || value.trim();
}

function collectCatalogStrings(value: unknown): string[] {
  if (typeof value === "string") return value.trim() ? [value.trim()] : [];
  if (Array.isArray(value)) return value.flatMap(collectCatalogStrings);
  if (!value || typeof value !== "object") return [];
  return Object.values(value as Record<string, unknown>).flatMap(collectCatalogStrings);
}

function uniqueStrings(values: string[]) {
  return values.filter((value, index) => values.indexOf(value) === index);
}

export function extractLanguageOptions(value: unknown): Array<{ value: string; label: string }> {
  if (!Array.isArray(value)) return [];
  const options = value.flatMap((item): Array<{ value: string; label: string }> => {
    if (typeof item === "string" && item.trim()) {
      const code = item.trim();
      return [{ value: code, label: languageLabel(code) }];
    }
    if (!item || typeof item !== "object" || Array.isArray(item)) return [];
    const record = item as Record<string, unknown>;
    const rawCode = [record.Language, record.language, record.Value, record.value]
      .find((entry) => typeof entry === "string");
    if (typeof rawCode !== "string" || !rawCode.trim()) return [];
    const code = rawCode.trim();
    return [{ value: code, label: languageLabel(code) }];
  });
  return options.filter((option, index) => (
    options.findIndex((candidate) => candidate.value.toLocaleLowerCase() === option.value.toLocaleLowerCase()) === index
  ));
}

export function languageLabel(value: string) {
  const normalized = value.trim().toLowerCase();
  const labels: Record<string, string> = {
    zh: "中文",
    "zh-cn": "简体中文",
    "zh-tw": "繁体中文",
    en: "英语",
    "en-us": "英语（美国）",
    "en-gb": "英语（英国）",
    id: "印尼语",
    "pt-br": "巴西葡萄牙语",
    ja: "日语",
    "ja-jp": "日语",
    mx: "墨西哥西语",
    vi: "越南语",
    th: "泰语",
    "es-mx": "西班牙语",
    fil: "菲律宾语",
    fr: "法语",
    ru: "俄语",
    de: "德语",
    ko: "韩语",
    "ko-kr": "韩语",
    ms: "马来语",
    ar: "阿拉伯语",
    it: "意大利语",
    "*": "自动识别",
  };
  return labels[normalized] || value;
}
