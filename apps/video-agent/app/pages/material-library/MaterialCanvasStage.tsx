"use client";

import { useEffect, useMemo, useState } from "react";
import { Group, Image as KonvaImage, Layer, Rect, Stage, Text } from "react-konva/es/ReactKonvaCore";
import "konva/lib/shapes/Image";
import "konva/lib/shapes/Rect";
import "konva/lib/shapes/Text";
import type { Material, MaterialType } from "../../lib/api";
import { getMaterialPreview, materialStatusLabels, materialTypeLabels } from "./materialModel";

export type MaterialCanvasStageProps = {
  materials: Material[];
  selectedMaterialId: string | null;
  width: number;
  height: number;
  onSelectMaterial: (materialId: string) => void;
};

type CanvasNode = {
  material: Material;
  x: number;
  y: number;
};

export function MaterialCanvasStage({
  materials,
  selectedMaterialId,
  width,
  height,
  onSelectMaterial,
}: MaterialCanvasStageProps) {
  const nodes = useMemo(() => materialCanvasNodes(materials), [materials]);

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

function materialCanvasNodes(materials: Material[]): CanvasNode[] {
  return materials.map((material, index) => ({
    material,
    x: 360 + (index % 3) * 210,
    y: 120 + Math.floor(index / 3) * 170,
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
        height={132}
        shadowBlur={selected ? 14 : 4}
        shadowColor={selected ? "#2f6df6" : "#9aa7b2"}
        shadowOpacity={selected ? 0.22 : 0.12}
        stroke={selected ? "#2f6df6" : "#cfd8e3"}
        strokeWidth={selected ? 2 : 1}
        width={176}
      />
      {image ? (
        <KonvaImage height={72} image={image} width={152} x={12} y={12} />
      ) : (
        <Rect
          cornerRadius={6}
          fill={materialTypeColor(node.material.material_type)}
          height={72}
          width={152}
          x={12}
          y={12}
        />
      )}
      <Text
        fill="#f8fafc"
        fontSize={14}
        fontStyle="bold"
        height={72}
        text={image ? "" : typeLabel}
        verticalAlign="middle"
        width={152}
        x={12}
        y={12}
      />
      <Text fill="#172033" fontSize={13} fontStyle="bold" text={node.material.file_name} width={152} x={12} y={92} />
      <Text
        fill="#5f6c7b"
        fontSize={11}
        text={`${typeLabel} · ${materialStatusLabels[node.material.status]}`}
        width={152}
        x={12}
        y={112}
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
