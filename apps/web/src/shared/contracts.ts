import type * as DomainContracts from "@video-agent/contracts";

/** 跨层领域事实只从共享契约包读取。 */
export type DomainContractModule = typeof DomainContracts;
