import { QueryClient } from "@tanstack/react-query";

// A single client keeps owner projections coherent across route-level pages.
export const queryClient = new QueryClient({
  defaultOptions: { queries: { retry: false, staleTime: 5_000 } },
});
