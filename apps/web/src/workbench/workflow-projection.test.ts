import { describe, expect, it } from "vitest";
import { projectWorkflowNodes } from "./workflow-projection-model";

describe("workflow projection", () => {
  it("projects all nodes while keeping frozen scope and selection data", () => {
    const scope = {
      projectId: "project-a",
      episodeId: "episode-a",
      sceneId: "scene-a",
      shotId: "shot-a",
    };
    const selection = {
      selectionSnapshotId: "selection-a",
      provider: "mock",
    };
    const source = {
      key: "fixture.node.001",
      scope,
      selectionSnapshot: selection,
      ports: { input: "fixture.input.v1" },
    };

    const projection = projectWorkflowNodes(
      Array.from({ length: 300 }, (_, index) =>
        index === 0 ? source : { key: `fixture.node.${index + 1}` },
      ),
    );

    expect(projection.nodes).toHaveLength(300);
    expect(projection.edges).toHaveLength(299);
    expect(projection.nodes[0].data).toEqual(
      expect.objectContaining({
        source,
        scope,
        selectionSnapshot: selection,
        frozenScope: scope,
        frozenSelection: selection,
        label: "流程节点 1",
      }),
    );
    expect(projection.nodes[0]).toEqual(
      expect.objectContaining({
        draggable: false,
        connectable: false,
        deletable: false,
      }),
    );
  });
});
