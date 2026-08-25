import {
  Badge,
  Button,
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
  Progress,
  Separator,
  Tabs,
  TabsContent,
  TabsList,
  TabsTrigger,
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "../shared/ui";
import {
  CheckCircle2,
  Clapperboard,
  Clock3,
  FileCheck2,
  Film,
  FolderKanban,
  ImageIcon,
  Layers3,
  LockKeyhole,
  Play,
  Send,
  Settings2,
  Sparkles,
  Upload,
} from "lucide-react";
import { useState } from "react";

type PrototypeShot = {
  id: string;
  scene: number;
  title: string;
  image: string;
  alt: string;
  status: "已就绪" | "待审核" | "未开始";
  duration: string;
  model: string;
};

const stages = [
  { label: "文本审核", detail: "批次已接受", icon: FileCheck2, state: "done" },
  {
    label: "镜头素材",
    detail: "2 / 3 已就绪",
    icon: ImageIcon,
    state: "active",
  },
  { label: "时间线", detail: "等待素材闭合", icon: Layers3, state: "waiting" },
  { label: "导出", detail: "尚未组装", icon: Send, state: "waiting" },
] as const;

const shots: PrototypeShot[] = [
  {
    id: "shot-01",
    scene: 1,
    title: "潮声抵港",
    image: "/prototype/harbor.jpg",
    alt: "夜色海面与浪花",
    status: "已就绪",
    duration: "00:04:12",
    model: "agnes-video / v1",
  },
  {
    id: "shot-02",
    scene: 2,
    title: "信号塔下的会面",
    image: "/prototype/lighthouse.jpg",
    alt: "海岸灯塔",
    status: "待审核",
    duration: "00:06:00",
    model: "agnes-video / v1",
  },
  {
    id: "shot-03",
    scene: 3,
    title: "未寄出的信",
    image: "/prototype/letter.jpg",
    alt: "信件与桌面",
    status: "未开始",
    duration: "00:03:18",
    model: "冻结后选择",
  },
];

const primaryNavigation = [
  { id: "workbench", label: "项目工作台", icon: FolderKanban },
  { id: "review", label: "候选审核", icon: FileCheck2 },
  { id: "assets", label: "项目资产", icon: Upload },
  { id: "timeline", label: "集时间线", icon: Layers3 },
  { id: "exports", label: "项目导出", icon: Send },
  { id: "settings", label: "模型设置", icon: Settings2 },
] as const;

function statusVariant(status: PrototypeShot["status"]) {
  if (status === "已就绪") return "success" as const;
  if (status === "待审核") return "warning" as const;
  return "secondary" as const;
}

export function PhaseOneWorkbenchPrototype() {
  const [selectedShotId, setSelectedShotId] = useState(shots[0].id);
  const [selectedSection, setSelectedSection] = useState("workbench");
  const selectedShot =
    shots.find((shot) => shot.id === selectedShotId) ?? shots[0];

  return (
    <TooltipProvider delayDuration={200}>
      <div className="min-h-screen bg-muted/40 text-foreground lg:flex lg:h-dvh lg:overflow-hidden">
        <aside
          aria-label="应用菜单"
          className="flex flex-col border-b border-border bg-background p-4 sm:p-6 lg:w-56 lg:shrink-0 lg:border-r lg:border-b-0 lg:px-4 lg:py-5"
        >
          <div className="flex min-w-0 items-center gap-3">
            <span className="grid size-10 shrink-0 place-items-center rounded-md bg-primary text-primary-foreground">
              <Clapperboard aria-hidden="true" className="size-5" />
            </span>
            <span className="truncate text-base font-semibold">帧间制片</span>
          </div>

          <nav
            aria-label="项目导航"
            className="mt-7 grid content-start gap-2 lg:min-h-0 lg:overflow-y-auto"
          >
            <div className="grid grid-cols-2 gap-1 lg:grid-cols-1">
              {primaryNavigation.map(({ id, label, icon: Icon }) => (
                <Button
                  aria-current={selectedSection === id ? "page" : undefined}
                  className="justify-start px-3"
                  key={id}
                  onClick={() => setSelectedSection(id)}
                  variant={selectedSection === id ? "secondary" : "ghost"}
                >
                  <Icon aria-hidden="true" className="size-4" />
                  <span className="truncate">{label}</span>
                </Button>
              ))}
            </div>
          </nav>
        </aside>

        <div className="min-w-0 lg:flex lg:min-h-0 lg:flex-1 lg:flex-col">
          <header className="shrink-0 border-b border-border bg-background">
            <div className="flex w-full items-center justify-between gap-4 px-4 py-4 sm:px-6 lg:px-8">
              <div className="min-w-0">
                <div className="flex flex-wrap items-center gap-2">
                  <h1 className="truncate text-xl font-semibold">雾港来信</h1>
                  <Badge variant="outline">第 01 / 08 集</Badge>
                </div>
              </div>
              <div className="flex shrink-0 items-center gap-2">
                <span className="hidden items-center gap-2 text-sm text-muted-foreground sm:flex">
                  <Clock3 aria-hidden="true" className="size-4" />
                  最近检查 10:42
                </span>
                <Badge variant="success">
                  <CheckCircle2 aria-hidden="true" className="size-3.5" />{" "}
                  本地测试
                </Badge>
                <Tooltip>
                  <TooltipTrigger asChild>
                    <Button disabled size="sm" variant="outline">
                      <LockKeyhole aria-hidden="true" /> 等待确认
                    </Button>
                  </TooltipTrigger>
                  <TooltipContent>原型确认前不执行正式业务操作</TooltipContent>
                </Tooltip>
              </div>
            </div>
          </header>

          <main className="flex w-full flex-col gap-5 p-4 sm:p-6 lg:p-8 lg:min-h-0 lg:flex-1">
            <section aria-label="生产交接" className="overflow-x-auto">
              <ol className="grid min-w-[44rem] grid-cols-4 divide-x divide-border overflow-hidden rounded-lg border border-border bg-card">
                {stages.map(({ label, detail, icon: Icon, state }) => (
                  <li className="min-w-0 p-4" key={label}>
                    <div className="flex items-center gap-2">
                      <span
                        className={
                          state === "done"
                            ? "grid size-7 place-items-center rounded bg-success/10 text-success"
                            : state === "active"
                              ? "grid size-7 place-items-center rounded bg-primary text-primary-foreground"
                              : "grid size-7 place-items-center rounded bg-muted text-muted-foreground"
                        }
                      >
                        <Icon aria-hidden="true" className="size-4" />
                      </span>
                      <span className="truncate text-sm font-semibold">
                        {label}
                      </span>
                    </div>
                    <p className="mt-3 truncate text-xs text-muted-foreground">
                      {detail}
                    </p>
                  </li>
                ))}
              </ol>
            </section>

            <section className="grid items-start gap-5 lg:min-h-0 lg:flex-1 lg:grid-cols-[13rem_minmax(0,1fr)_18rem] lg:items-stretch">
              <aside
                className="grid content-start gap-2 lg:min-h-0 lg:overflow-y-auto"
                aria-label="拍摄板"
              >
                <p className="px-1 text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                  拍摄板
                </p>
                {shots.map((shot) => (
                  <Button
                    aria-label={`选择第 ${shot.scene} 场：${shot.title}`}
                    className="h-auto min-h-20 justify-start px-3 py-3 text-left"
                    key={shot.id}
                    onClick={() => setSelectedShotId(shot.id)}
                    variant={
                      selectedShot.id === shot.id ? "secondary" : "ghost"
                    }
                  >
                    <span className="grid size-7 shrink-0 place-items-center rounded bg-background font-mono text-xs text-muted-foreground">
                      {String(shot.scene).padStart(2, "0")}
                    </span>
                    <span className="min-w-0">
                      <span className="block truncate text-sm font-semibold">
                        {shot.title}
                      </span>
                      <span className="mt-1 block truncate text-xs text-muted-foreground">
                        {shot.status}
                      </span>
                    </span>
                  </Button>
                ))}
              </aside>

              <section
                aria-label="镜头工作区"
                className="min-w-0 lg:min-h-0 lg:overflow-y-auto lg:pr-2"
              >
                <Tabs defaultValue="board">
                  <div className="flex flex-wrap items-center justify-between gap-3 border-b border-border pb-3">
                    <TabsList aria-label="原型工作区视图">
                      <TabsTrigger value="board">镜头板</TabsTrigger>
                      <TabsTrigger value="review">审核</TabsTrigger>
                      <TabsTrigger value="timeline">时间线</TabsTrigger>
                    </TabsList>
                    <Badge variant={statusVariant(selectedShot.status)}>
                      {selectedShot.status}
                    </Badge>
                  </div>

                  <TabsContent className="pt-4" value="board">
                    <Card>
                      <div className="grid min-h-72 sm:grid-cols-[minmax(0,1.4fr)_minmax(16rem,0.9fr)] lg:h-[calc(100dvh-24rem)]">
                        <img
                          alt={selectedShot.alt}
                          className="h-72 w-full object-cover sm:h-full"
                          src={selectedShot.image}
                        />
                        <CardContent className="flex flex-col justify-between p-5">
                          <div>
                            <div className="flex items-center gap-2 text-xs font-semibold uppercase tracking-wide text-primary">
                              <Film aria-hidden="true" className="size-4" /> 场{" "}
                              {String(selectedShot.scene).padStart(2, "0")}
                            </div>
                            <h2 className="mt-3 text-xl font-semibold">
                              {selectedShot.title}
                            </h2>
                          </div>
                          <div className="mt-6 grid gap-3 text-sm">
                            <div className="flex justify-between gap-3">
                              <span className="text-muted-foreground">
                                镜头时长
                              </span>
                              <span className="font-mono">
                                {selectedShot.duration}
                              </span>
                            </div>
                            <div className="flex justify-between gap-3">
                              <span className="text-muted-foreground">
                                执行模型
                              </span>
                              <span className="font-mono text-xs">
                                {selectedShot.model}
                              </span>
                            </div>
                          </div>
                        </CardContent>
                      </div>
                    </Card>
                  </TabsContent>

                  <TabsContent className="pt-4" value="review">
                    <Card>
                      <CardHeader>
                        <CardTitle>文本审核批次</CardTitle>
                        <CardDescription>
                          全部候选接受后，镜头素材才可进入可用状态。
                        </CardDescription>
                      </CardHeader>
                      <CardContent className="grid gap-4">
                        <div className="flex flex-wrap items-center justify-between gap-3">
                          <Badge variant="success">已接受</Badge>
                          <span className="font-mono text-xs text-muted-foreground">
                            审核批次 00017
                          </span>
                        </div>
                        <Separator />
                        <p className="text-sm text-muted-foreground">
                          故事、剧本、场次、镜头与素材设定在同一批次中闭合。
                        </p>
                      </CardContent>
                    </Card>
                  </TabsContent>

                  <TabsContent className="pt-4" value="timeline">
                    <Card>
                      <CardHeader>
                        <CardTitle>当前 Cut</CardTitle>
                        <CardDescription>
                          时间线保持等待状态，直到所有必需素材就绪。
                        </CardDescription>
                      </CardHeader>
                      <CardContent className="grid gap-4">
                        <Progress value={67} />
                        <div className="grid grid-cols-3 gap-2 text-center text-xs text-muted-foreground">
                          <span>视频 02</span>
                          <span>音频 01</span>
                          <span>字幕 03</span>
                        </div>
                      </CardContent>
                    </Card>
                  </TabsContent>
                </Tabs>
              </section>

              <aside
                className="grid content-start gap-4 lg:min-h-0 lg:overflow-y-auto"
                aria-label="版本检查"
              >
                <Card>
                  <CardHeader>
                    <CardTitle>下一步</CardTitle>
                    <CardDescription>
                      先确认第 2 场素材审核，再进入时间线。
                    </CardDescription>
                  </CardHeader>
                  <CardContent>
                    <Button className="w-full" disabled>
                      <Play aria-hidden="true" /> 等待正式确认
                    </Button>
                  </CardContent>
                </Card>
                <Card>
                  <CardHeader>
                    <CardTitle>版本状态</CardTitle>
                  </CardHeader>
                  <CardContent className="grid gap-3 text-sm">
                    <div>
                      <p className="text-xs text-muted-foreground">素材版本</p>
                      <p className="mt-1 text-sm">版本 8</p>
                    </div>
                    <Separator />
                    <div>
                      <p className="text-xs text-muted-foreground">
                        时间线版本
                      </p>
                      <p className="mt-1 font-mono text-xs">未发布</p>
                    </div>
                    <Separator />
                    <div>
                      <p className="text-xs text-muted-foreground">导出状态</p>
                      <p className="mt-1 text-sm">渲染器未配置</p>
                    </div>
                  </CardContent>
                </Card>
                <div className="flex items-start gap-2 rounded-md border border-warning/30 bg-warning/10 p-3 text-sm text-warning-foreground">
                  <Sparkles
                    aria-hidden="true"
                    className="mt-0.5 size-4 shrink-0"
                  />
                  <span>外部服务和渲染能力在正式页面中由业务数据决定。</span>
                </div>
              </aside>
            </section>
          </main>
        </div>
      </div>
    </TooltipProvider>
  );
}
