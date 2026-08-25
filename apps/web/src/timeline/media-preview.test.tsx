import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { MediaPreview, sameOriginMediaUrl } from "./media-preview";

describe("MediaPreview", () => {
  it("blocks cross-origin media and keeps a non-empty fallback", () => {
    expect(
      sameOriginMediaUrl("https://media.example.invalid/proxy.m3u8"),
    ).toMatchObject({
      ok: false,
      state: "blocked",
    });
    render(
      <MediaPreview
        kind="hls"
        url="https://media.example.invalid/proxy.m3u8"
      />,
    );
    expect(screen.getByTestId("media-preview")).toHaveAttribute(
      "data-state",
      "blocked",
    );
    expect(screen.getByTestId("media-preview-fallback")).toHaveTextContent(
      "Preview blocked",
    );
  });

  it("shows an explicit error state when the owner has no proxy URL", () => {
    render(<MediaPreview kind="waveform" />);
    expect(screen.getByTestId("media-preview")).toHaveAttribute(
      "data-state",
      "error",
    );
    expect(screen.getByTestId("media-preview-fallback")).toHaveTextContent(
      "missing proxy URL",
    );
  });
});
