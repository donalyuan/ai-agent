"use client";

import { useEffect, useMemo, useState } from "react";
import { Group, Image as KonvaImage, Layer, Rect, Stage, Text } from "react-konva/es/ReactKonvaCore";
import "konva/lib/shapes/Image";
import "konva/lib/shapes/Rect";
import "konva/lib/shapes/Text";
import type { Material, MaterialType } from "../../lib/api";
import { getMaterialPreview, materialStatusLabels, materialTypeLabels } from "./materialModel";

export type MaterialCanvasStageProps = {
  detailOpen: boolean;
  materials: Material[];
  selectedMaterialId: string | null;
  width: number;
  height: number;
  onSelectMaterial: (materialId: string) => void;
};

export type CanvasNode = {
  material: Material;
  x: number;
  y: number;
};

export function MaterialCanvasStage({
  detailOpen,
  materials,
  selectedMaterialId,
  width,
  height,
  onSelectMaterial,
}: MaterialCanvasStageProps) {
  const nodes = useMemo(
    () => materialCanvasNodes(materials, width, detailOpen),
    [detailOpen, materials, width],
  );

  return (
    <Stage className="materialKonvaStage" height={height} width={width}>
      <Layer>
        <Rect
          fill="#f7fafc"
          height={height}
          stroke="#d8e0e8"
          strokeWidth={1}
          width={width}
          x={0}
          y={0}
        />
        {nodes.map((node) => (
          <MaterialCanvasNode
            key={node.material.material_id}
            node={node}
            selected={node.material.material_id === selectedMaterialId}
            onSelect={onSelectMaterial}
          />
        ))}
      </Layer>
    </Stage>
  );
}

export const MATERIAL_CANVAS_NODE_WIDTH = 206;
export const MATERIAL_CANVAS_NODE_HEIGHT = 154;

export function materialCanvasNodes(
  materials: Material[],
  width: number,
  detailOpen: boolean,
): CanvasNode[] {
  const leftInset = 340;
  const rightInset = detailOpen ? 376 : 24;
  const columnGap = 24;
  const rowGap = 30;
  const availableWidth = Math.max(
    MATERIAL_CANVAS_NODE_WIDTH,
    width - leftInset - rightInset,
  );
  const columnCount = Math.max(
    1,
    Math.floor((availableWidth + columnGap) / (MATERIAL_CANVAS_NODE_WIDTH + columnGap)),
  );

  return materials.map((material, index) => ({
    material,
    x: leftInset + (index % columnCount) * (MATERIAL_CANVAS_NODE_WIDTH + columnGap),
    y: 96 + Math.floor(index / columnCount) * (MATERIAL_CANVAS_NODE_HEIGHT + rowGap),
  }));
}

function MaterialCanvasNode({
  node,
  selected,
  onSelect,
}: {
  node: CanvasNode;
  selected: boolean;
  onSelect: (materialId: string) => void;
}) {
  const preview = getMaterialPreview(node.material);
  const image = usePreviewImage(preview.imageUrl);
  const typeLabel = materialTypeLabels[node.material.material_type];

  return (
    <Group
      onClick={() => onSelect(node.material.material_id)}
      onTap={() => onSelect(node.material.material_id)}
      x={node.x}
      y={node.y}
    >
      <Rect
        cornerRadius={8}
        fill="#ffffff"
        height={MATERIAL_CANVAS_NODE_HEIGHT}
        shadowBlur={selected ? 14 : 4}
        shadowColor={selected ? "#2f6df6" : "#9aa7b2"}
        shadowOpacity={selected ? 0.22 : 0.12}
        stroke={selected ? "#2f6df6" : "#cfd8e3"}
        strokeWidth={selected ? 2 : 1}
        width={MATERIAL_CANVAS_NODE_WIDTH}
      />
      {image ? (
        <KonvaImage height={76} image={image} width={182} x={12} y={12} />
      ) : (
        <Rect
          cornerRadius={6}
          fill={materialTypeColor(node.material.material_type)}
          height={76}
          width={182}
          x={12}
          y={12}
        />
      )}
      <Text
        fill="#f8fafc"
        fontSize={14}
        fontStyle="bold"
        height={76}
        text={image ? "" : typeLabel}
        verticalAlign="middle"
        width={182}
        x={12}
        y={12}
      />
      <Text
        ellipsis
        fill="#172033"
        fontSize={12}
        fontStyle="bold"
        height={32}
        lineHeight={1.15}
        text={node.material.file_name}
        width={182}
        wrap="char"
        x={12}
        y={96}
      />
      <Text
        fill="#5f6c7b"
        fontSize={10}
        text={`${typeLabel} · ${materialStatusLabels[node.material.status]}`}
        width={182}
        x={12}
        y={134}
      />
    </Group>
  );
}

function usePreviewImage(imageUrl: string | null) {
  const [image, setImage] = useState<HTMLImageElement | null>(null);

  useEffect(() => {
    if (!imageUrl) {
      setImage(null);
      return;
    }
    let active = true;
    const nextImage = new window.Image();
    nextImage.crossOrigin = "anonymous";
    nextImage.onload = () => {
      if (active) {
        setImage(nextImage);
      }
    };
    nextImage.onerror = () => {
      if (active) {
        setImage(null);
      }
    };
    nextImage.src = imageUrl;
    return () => {
      active = false;
    };
  }, [imageUrl]);

  return image;
}

function materialTypeColor(type: MaterialType) {
  switch (type) {
    case "video":
      return "#315d9a";
    case "image":
      return "#2f8f83";
    case "audio":
      return "#8a5b2e";
    case "subtitle":
      return "#5c6299";
  }
}
