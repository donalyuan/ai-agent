import Hls from "hls.js";
import { Application, Assets, Sprite } from "pixi.js";
import { Pause, Play, RotateCcw, RotateCw } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import WaveSurfer from "wavesurfer.js";
import { Button } from "../shared/ui";

export type MediaPreviewState = "loading" | "ready" | "error" | "blocked";
export type MediaPreviewKind = "pixi" | "waveform" | "hls";

export function sameOriginMediaUrl(
  value: string,
):
  | { ok: true; url: string }
  | { ok: false; state: "error" | "blocked"; message: string } {
  if (!value.trim())
    return {
      ok: false,
      state: "error",
      message: "Preview unavailable: missing proxy URL",
    };
  try {
    const url = new URL(value, window.location.origin);
    if (url.origin !== window.location.origin)
      return {
        ok: false,
        state: "blocked",
        message: "Preview blocked: URL is not same-origin",
      };
    if (url.protocol !== "http:" && url.protocol !== "https:")
      return {
        ok: false,
        state: "blocked",
        message: "Preview blocked: unsupported URL scheme",
      };
    return { ok: true, url: url.toString() };
  } catch {
    return {
      ok: false,
      state: "error",
      message: "Preview unavailable: invalid proxy URL",
    };
  }
}

function PreviewFallback({
  state,
  message,
}: {
  state: MediaPreviewState;
  message: string;
}) {
  return (
    <div
      className="grid min-h-40 place-items-center bg-muted px-4 text-center text-sm text-muted-foreground"
      data-testid="media-preview-fallback"
      role={state === "error" || state === "blocked" ? "alert" : "status"}
    >
      <span>
        <strong className="block text-foreground">{state.toUpperCase()}</strong>
        {message}
      </span>
    </div>
  );
}

export function MediaPreview({
  kind,
  url = "",
  title = "Media preview",
}: {
  kind: MediaPreviewKind;
  url?: string;
  title?: string;
}) {
  const resolved = sameOriginMediaUrl(url);
  const [state, setState] = useState<MediaPreviewState>(
    resolved.ok ? "loading" : resolved.state,
  );
  const [message, setMessage] = useState(
    resolved.ok ? "Loading same-origin proxy…" : resolved.message,
  );
  const [playing, setPlaying] = useState(false);
  const [progress, setProgress] = useState(0);
  const pixiRef = useRef<HTMLCanvasElement>(null);
  const waveformRef = useRef<HTMLDivElement>(null);
  const waveformInstanceRef = useRef<WaveSurfer | null>(null);
  const videoRef = useRef<HTMLVideoElement>(null);
  const resolvedUrl = resolved.ok ? resolved.url : "";

  useEffect(() => {
    if (!resolved.ok) return;
    let disposed = false;
    let app: Application | undefined;
    let waveform: WaveSurfer | undefined;
    let hls: Hls | undefined;

    const fail = (error: unknown) => {
      if (!disposed) {
        setState("error");
        setMessage(
          error instanceof Error ? error.message : "Preview failed to load",
        );
      }
    };

    if (kind === "pixi" && pixiRef.current) {
      app = new Application();
      void app
        .init({
          canvas: pixiRef.current,
          width: 640,
          height: 360,
          backgroundColor: 0x26332f,
        })
        .then(async () => {
          if (disposed || !app) return;
          const texture = await Assets.load(resolvedUrl);
          if (disposed) return;
          const sprite = Sprite.from(texture);
          sprite.width = 640;
          sprite.height = 360;
          app.stage.addChild(sprite);
          setState("ready");
          setMessage("Pixi preview ready");
        })
        .catch(fail);
    }

    if (kind === "waveform" && waveformRef.current) {
      try {
        waveform = WaveSurfer.create({
          container: waveformRef.current,
          url: resolvedUrl,
          height: 96,
          waveColor: "#9eb9ad",
          progressColor: "#c95f48",
          normalize: true,
        });
        waveformInstanceRef.current = waveform;
        waveform.on("ready", () => {
          setState("ready");
          setMessage("Waveform ready");
        });
        waveform.on("error", fail);
        waveform.on("audioprocess", (time) => {
          const duration = waveform?.getDuration() ?? 0;
          if (duration > 0) setProgress(time / duration);
        });
      } catch (error) {
        fail(error);
      }
    }

    if (kind === "hls" && videoRef.current) {
      const video = videoRef.current;
      const ready = () => {
        setState("ready");
        setMessage("HLS proxy ready");
      };
      const failed = () => fail(new Error("HLS proxy returned a media error"));
      video.addEventListener("loadedmetadata", ready);
      video.addEventListener("error", failed);
      if (Hls.isSupported()) {
        hls = new Hls({ enableWorker: false });
        hls.on(Hls.Events.MANIFEST_PARSED, ready);
        hls.on(Hls.Events.ERROR, (_event, data) => {
          if (data.fatal) failed();
        });
        hls.loadSource(resolvedUrl);
        hls.attachMedia(video);
      } else {
        video.src = resolvedUrl;
        video.load();
      }
      return () => {
        disposed = true;
        video.removeEventListener("loadedmetadata", ready);
        video.removeEventListener("error", failed);
        hls?.destroy();
        waveformInstanceRef.current = null;
      };
    }

    return () => {
      disposed = true;
      waveform?.destroy();
      waveformInstanceRef.current = null;
      app?.destroy(true, {
        children: true,
        texture: true,
        textureSource: true,
      });
    };
  }, [kind, resolved.ok, resolvedUrl]);

  const mediaElement =
    state === "ready" || state === "loading" ? (
      <>
        {kind === "pixi" && (
          <canvas
            ref={pixiRef}
            className="aspect-video w-full"
            aria-label={title}
          />
        )}
        {kind === "waveform" && (
          <div
            ref={waveformRef}
            className="min-h-24 w-full"
            aria-label={title}
          />
        )}
        {kind === "hls" && (
          <video
            ref={videoRef}
            className="aspect-video w-full bg-black"
            aria-label={title}
            controls
          />
        )}
      </>
    ) : null;

  const seek = (offset: number) => {
    if (kind === "waveform") {
      const waveform = waveformInstanceRef.current;
      if (!waveform) return;
      const duration = waveform.getDuration();
      waveform.setTime(
        Math.max(0, Math.min(duration, waveform.getCurrentTime() + offset)),
      );
      return;
    }
    const video = videoRef.current;
    if (video && Number.isFinite(video.duration)) {
      video.currentTime = Math.max(
        0,
        Math.min(video.duration, video.currentTime + offset),
      );
    }
  };

  return (
    <div className="grid gap-2" data-testid="media-preview" data-state={state}>
      {mediaElement ?? <PreviewFallback state={state} message={message} />}
      {state === "ready" && kind !== "pixi" && (
        <div className="flex items-center gap-2" aria-label="媒体播放控制">
          <Button
            type="button"
            size="icon-sm"
            variant="outline"
            aria-label={playing ? "暂停媒体" : "播放媒体"}
            onClick={() => {
              if (kind === "waveform") {
                waveformInstanceRef.current?.playPause();
                setPlaying((value) => !value);
                return;
              }
              const video = videoRef.current;
              if (!video) return;
              if (playing) void video.pause();
              else void video.play();
              setPlaying(!playing);
            }}
          >
            {playing ? <Pause /> : <Play />}
          </Button>
          <Button
            type="button"
            size="icon-sm"
            variant="ghost"
            aria-label="向后 seek"
            onClick={() => seek(-1)}
          >
            <RotateCcw />
          </Button>
          <Button
            type="button"
            size="icon-sm"
            variant="ghost"
            aria-label="向前 seek"
            onClick={() => seek(1)}
          >
            <RotateCw />
          </Button>
          <progress
            className="h-1 min-w-24 flex-1"
            max={1}
            value={progress}
            aria-label="媒体进度"
          />
        </div>
      )}
    </div>
  );
}
