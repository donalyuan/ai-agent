import { Pause, Play } from "lucide-react";
import { useRef, useState } from "react";

type AssetAudioPlayerProps = {
  audioPath: string | null;
  durationMs?: number | null;
  disabled?: boolean;
  playing: boolean;
  onToggle: () => void;
  onEnded: () => void;
  onExpired?: () => void;
};

function AssetAudioPlayer({
  audioPath,
  durationMs,
  disabled = false,
  playing,
  onToggle,
  onEnded,
  onExpired,
}: AssetAudioPlayerProps) {
  const audioRef = useRef<HTMLAudioElement>(null);
  const [position, setPosition] = useState(0);
  const duration = Math.max(0, durationMs ?? 0) / 1000;

  const seek = (value: number) => {
    setPosition(value);
    if (audioRef.current) audioRef.current.currentTime = value;
  };

  return (
    <div
      className="flex items-center gap-3 rounded-md border border-border bg-muted/40 p-2"
      aria-label="音频试听"
    >
      <button
        className="inline-flex size-10 items-center justify-center rounded-md border border-border bg-background text-foreground hover:bg-accent disabled:pointer-events-none disabled:opacity-50"
        title={playing ? "暂停试听" : "试听"}
        aria-label={playing ? "暂停试听" : "试听"}
        onClick={onToggle}
        disabled={disabled}
      >
        {playing ? <Pause size={16} /> : <Play size={16} />}
      </button>
      <div className="flex h-6 flex-1 items-center gap-0.5" aria-hidden="true">
        {Array.from({ length: 10 }, (_, index) => (
          <span
            className="w-full rounded-full bg-primary/35"
            key={index}
            style={{ height: `${25 + ((index * 29) % 70)}%` }}
          />
        ))}
      </div>
      <input
        aria-label="音频进度"
        type="range"
        min={0}
        max={duration || 1}
        step={0.01}
        value={audioPath ? Math.min(position, duration || 1) : 0}
        disabled={!audioPath || disabled}
        onChange={(event) => seek(Number(event.target.value))}
      />
      <span className="font-mono text-xs text-muted-foreground">
        {duration ? `${Math.round(duration)}s` : "--"}
      </span>
      {audioPath && (
        <audio
          ref={audioRef}
          className="hidden"
          src={audioPath}
          autoPlay
          onTimeUpdate={(event) => setPosition(event.currentTarget.currentTime)}
          onEnded={onEnded}
          onError={onExpired}
        />
      )}
    </div>
  );
}

export { AssetAudioPlayer };
export type { AssetAudioPlayerProps };
