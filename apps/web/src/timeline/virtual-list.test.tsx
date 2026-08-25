import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { VirtualList } from "../shared/ui";

describe("Timeline VirtualList integration", () => {
  it("keeps the rendered DOM bounded for a long list", () => {
    render(
      <VirtualList
        items={Array.from({ length: 1_000 }, (_, index) => index)}
        estimateSize={32}
        height={160}
        getKey={(item) => String(item)}
        ariaLabel="long test list"
        renderItem={(item) => <span>{item}</span>}
      />,
    );
    expect(
      screen.getByRole("list").querySelectorAll('[role="listitem"]').length,
    ).toBeLessThan(100);
  });
});
