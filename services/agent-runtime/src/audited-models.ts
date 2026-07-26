import type {
  Api,
  AssistantMessage,
  Context,
  Model,
  ModelsApiStreamOptions,
  ModelsSimpleStreamOptions,
  MutableModels,
} from "@earendil-works/pi-ai";

export type CompleteSimpleNext = (
  context: Context,
  options?: ModelsSimpleStreamOptions,
) => Promise<AssistantMessage>;

export type CompleteSimpleGovernor = (
  model: Model<Api>,
  context: Context,
  options: ModelsSimpleStreamOptions | undefined,
  next: CompleteSimpleNext,
) => Promise<AssistantMessage>;

/** Public Models composition boundary that disables provider/SDK transparent retries. */
export class AuditedModels {
  private completeSimpleGovernor: CompleteSimpleGovernor | undefined;
  readonly getProviders: MutableModels["getProviders"];
  readonly getProvider: MutableModels["getProvider"];
  readonly getModels: MutableModels["getModels"];
  readonly getModel: MutableModels["getModel"];
  readonly refresh: MutableModels["refresh"];
  readonly checkAuth: MutableModels["checkAuth"];
  readonly getAvailable: MutableModels["getAvailable"];
  readonly getAuth: MutableModels["getAuth"];
  readonly login: MutableModels["login"];
  readonly logout: MutableModels["logout"];
  readonly setProvider: MutableModels["setProvider"];
  readonly deleteProvider: MutableModels["deleteProvider"];
  readonly clearProviders: MutableModels["clearProviders"];
  readonly stream: MutableModels["stream"];
  readonly complete: MutableModels["complete"];
  readonly streamSimple: MutableModels["streamSimple"];
  readonly completeSimple: MutableModels["completeSimple"];

  constructor(private readonly inner: MutableModels) {
    this.getProviders = inner.getProviders.bind(inner);
    this.getProvider = inner.getProvider.bind(inner);
    this.getModels = inner.getModels.bind(inner);
    this.getModel = inner.getModel.bind(inner);
    this.refresh = inner.refresh.bind(inner);
    this.checkAuth = inner.checkAuth.bind(inner);
    this.getAvailable = inner.getAvailable.bind(inner);
    this.getAuth = inner.getAuth.bind(inner);
    this.login = inner.login.bind(inner);
    this.logout = inner.logout.bind(inner);
    this.setProvider = inner.setProvider.bind(inner);
    this.deleteProvider = inner.deleteProvider.bind(inner);
    this.clearProviders = inner.clearProviders.bind(inner);
    this.stream = ((model: Model<Api>, context: Context, options?: ModelsApiStreamOptions<Api>) =>
      inner.stream(model, context, { ...options, maxRetries: 0 })) as MutableModels["stream"];
    this.complete = ((model: Model<Api>, context: Context, options?: ModelsApiStreamOptions<Api>) =>
      inner.complete(model, context, { ...options, maxRetries: 0 })) as MutableModels["complete"];
    this.streamSimple = ((model: Model<Api>, context: Context, options?: ModelsSimpleStreamOptions) =>
      inner.streamSimple(model, context, { ...options, maxRetries: 0 })) as MutableModels["streamSimple"];
    this.completeSimple = (async (model: Model<Api>, context: Context, options?: ModelsSimpleStreamOptions) => {
      const next: CompleteSimpleNext = (nextContext, nextOptions) =>
        inner.completeSimple(model, nextContext, { ...nextOptions, maxRetries: 0 });
      return this.completeSimpleGovernor
        ? this.completeSimpleGovernor(model, context, options, next)
        : next(context, options);
    }) as MutableModels["completeSimple"];
  }

  /** Installs the per-Harness governor for Pi's public standalone model calls. */
  governCompleteSimple(governor: CompleteSimpleGovernor): void {
    if (this.completeSimpleGovernor) throw new Error("completeSimple governor is already installed");
    this.completeSimpleGovernor = governor;
  }
}

export function createAuditedModels(inner: MutableModels): AuditedModels {
  return new AuditedModels(inner);
}
