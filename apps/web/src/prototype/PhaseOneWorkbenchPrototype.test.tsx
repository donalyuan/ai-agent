import { fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { App, queryClient } from "../App";

afterEach(() => {
  vi.unstubAllGlobals();
  queryClient.clear();
  window.history.pushState({}, "", "/projects");
});

describe("阶段一组件库原型", () => {
  it("在独立路由呈现静态生产状态，并且不调用网络", () => {
    window.history.pushState({}, "", "/prototype");
    const fetchMock = vi.fn();
    vi.stubGlobal("fetch", fetchMock);

    render(<App />);

    const applicationMenu = screen.getByRole("complementary", {
      name: "应用菜单",
    });

    expect(within(applicationMenu).getByText("帧间制片")).toBeVisible();
    expect(applicationMenu).not.toHaveTextContent("雾港来信");
    expect(applicationMenu).not.toHaveTextContent("项目菜单");
    expect(
      within(screen.getByRole("banner")).getByRole("heading", {
        name: "雾港来信",
      }),
    ).toBeVisible();
    expect(screen.getByText("文本审核")).toBeVisible();
    expect(screen.getAllByText("时间线")[0]).toBeVisible();
    expect(screen.getByText("版本状态")).toBeVisible();
    expect(applicationMenu).toBeVisible();
    expect(screen.getByRole("navigation", { name: "项目导航" })).toBeVisible();
    expect(screen.getByRole("complementary", { name: "拍摄板" })).toBeVisible();
    expect(screen.getByRole("region", { name: "镜头工作区" })).toBeVisible();
    expect(screen.getByRole("button", { name: "项目工作台" })).toHaveAttribute(
      "aria-current",
      "page",
    );

    fireEvent.click(screen.getByRole("button", { name: "候选审核" }));

    expect(screen.getByRole("button", { name: "候选审核" })).toHaveAttribute(
      "aria-current",
      "page",
    );

    fireEvent.click(screen.getByRole("button", { name: /第 2 场/ }));

    expect(
      screen.getByRole("heading", { name: "信号塔下的会面" }),
    ).toBeVisible();
    expect(fetchMock).not.toHaveBeenCalled();
  });
});
