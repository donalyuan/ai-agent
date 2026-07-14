import { useEffect, useState } from "react";

type ImagePreviewDialogProps = {
  alt: string;
  imageUrl: string;
  subtitle?: string;
  title: string;
  onClose: () => void;
};

export function ImagePreviewDialog({
  alt,
  imageUrl,
  subtitle,
  title,
  onClose,
}: ImagePreviewDialogProps) {
  const [zoom, setZoom] = useState(100);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        onClose();
      }
    };
    document.addEventListener("keydown", onKeyDown);
    return () => document.removeEventListener("keydown", onKeyDown);
  }, [onClose]);

  return (
    <div
      className="materialImageLightbox"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget) {
          onClose();
        }
      }}
    >
      <div aria-label="图片大图预览" aria-modal="true" className="materialImageDialog" role="dialog">
        <header>
          <div>
            <strong>{title}</strong>
            {subtitle ? <span>{subtitle}</span> : null}
          </div>
          <button aria-label="关闭大图预览" type="button" onClick={onClose}>×</button>
        </header>
        <div className="materialImageViewport">
          {/* eslint-disable-next-line @next/next/no-img-element */}
          <img alt={alt} src={imageUrl} style={{ transform: `scale(${zoom / 100})` }} />
        </div>
        <div aria-label="大图缩放" className="materialImageZoomControls">
          <button
            aria-label="缩小图片"
            disabled={zoom <= 50}
            type="button"
            onClick={() => setZoom((current) => Math.max(50, current - 25))}
          >
            −
          </button>
          <strong>{zoom}%</strong>
          <button
            aria-label="放大图片"
            disabled={zoom >= 200}
            type="button"
            onClick={() => setZoom((current) => Math.min(200, current + 25))}
          >
            +
          </button>
        </div>
      </div>
    </div>
  );
}
