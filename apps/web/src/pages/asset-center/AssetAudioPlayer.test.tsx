import { fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { AssetAudioPlayer } from "./AssetAudioPlayer";

afterEach(() => vi.unstubAllGlobals());

describe("AssetAudioPlayer", () => {
  it("只消费 opaque grant path，并在 seek 时保持无请求副作用", () => {
    const onToggle = vi.fn();
    const onEnded = vi.fn();
    const fetchMock = vi.fn();
    vi.stubGlobal("fetch", fetchMock);

    render(
      <AssetAudioPlayer
        audioPath="/api/v1/asset-media-grants/opaque-token"
        durationMs={3000}
        playing={false}
        onToggle={onToggle}
        onEnded={onEnded}
      />,
    );

    expect(document.querySelector("audio")).toHaveAttribute(
      "src",
      "/api/v1/asset-media-grants/opaque-token",
    );
    fireEvent.click(screen.getByRole("button", { name: "试听" }));
    fireEvent.change(screen.getByRole("slider", { name: "音频进度" }), {
      target: { value: "1.5" },
    });

    expect(onToggle).toHaveBeenCalledOnce();
    expect(fetchMock).not.toHaveBeenCalled();
  });
});
