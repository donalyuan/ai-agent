# Fix AI Model Default Request Method Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. Repository rules prohibit `git add`, `git commit`, and `git push` without explicit confirmation, so commit steps are omitted.

**Goal:** Make the Admin “设为默认” action call the deployed `POST /api/admin/models/:model_id/default` contract and refresh successfully.

**Architecture:** Keep the Rust route and PostgreSQL atomic default-switch transaction unchanged. Correct the HTTP method at the Admin API client boundary, lock the contract with client and page tests, then verify the deployed workflow against the real API.

**Tech Stack:** Next.js 14, TypeScript, React Testing Library, Vitest, Rust/Axum API, PostgreSQL, Docker Compose, OpenSpec.

---

### Task 1: Lock The Admin Request Contract With Failing Tests

**Files:**
- Modify: `admin/app/lib/api.test.ts`
- Modify: `admin/app/models/page.test.tsx`

- [x] **Step 1: Assert the API client sends POST**

After the existing `setDefaultAiModel()` invocation, add:

```typescript
expect(fetchMock.mock.calls[3][1]).toMatchObject({
  method: "POST",
  body: JSON.stringify({ version: 3 }),
});
```

- [x] **Step 2: Assert the page action sends POST and reloads**

Add a page test with the existing default model and one enabled replacement:

```typescript
it("设为默认使用 POST 并在成功后刷新列表", async () => {
  const replacement = {
    ...textModel,
    model_id: "22222222-2222-4222-8222-222222222222",
    display_name: "GPT Backup",
    is_default: false,
    version: 3,
  };
  const fetcher = vi.fn(() => jsonResponse({ models: [textModel, replacement] }));
  render(
    <ModelManagementPage
      client={createApiClient({ baseUrl: "http://api.test", fetcher })}
    />,
  );
  await screen.findByText("GPT Backup");

  fireEvent.click(screen.getByRole("button", { name: "设为默认" }));

  await waitFor(() => {
    expect(fetcher).toHaveBeenCalledWith(
      `http://api.test/api/admin/models/${replacement.model_id}/default`,
      expect.objectContaining({
        method: "POST",
        body: JSON.stringify({ version: 3 }),
      }),
    );
  });
  await waitFor(() => expect(fetcher).toHaveBeenCalledTimes(3));
  expect(screen.queryByRole("alert")).not.toBeInTheDocument();
});
```

- [x] **Step 3: Run focused tests and verify RED**

Run:

```bash
docker compose -f /server/docker-compose.yml exec -T ai-agent-admin sh -lc \
  'cd /app && npm test -- app/lib/api.test.ts app/models/page.test.tsx'
```

Expected: both new assertions fail because the request method is currently `PUT`.

### Task 2: Correct The Client Method And Verify The Workflow

**Files:**
- Modify: `admin/app/lib/api.ts`
- Modify: `openspec/changes/fix-ai-model-default-request-method/tasks.md`

- [x] **Step 1: Change only the default action to POST**

```typescript
return request<AiModel>(client, `/api/admin/models/${modelId}/default`, {
  method: "POST",
  body: payload,
});
```

- [x] **Step 2: Run focused tests and verify GREEN**

Run the focused command from Task 1. Expected: all selected tests pass.

- [x] **Step 3: Run Admin full verification**

```bash
docker compose -f /server/docker-compose.yml exec -T ai-agent-admin sh -lc \
  'cd /app && npm test && npm run lint && npm run build'
```

Expected: all commands exit `0`.

- [x] **Step 4: Validate OpenSpec and diff hygiene**

```bash
openspec validate fix-ai-model-default-request-method --strict
openspec instructions apply --change fix-ai-model-default-request-method --json
git diff --check
```

- [x] **Step 5: Rebuild Admin and verify the real default switch**

```bash
docker compose -f /server/docker-compose.yml up -d --build ai-agent-admin
curl -X POST \
  http://127.0.0.1:18180/api/admin/models/21ca4433-b8a3-430c-a72d-7092a00bc44e/default \
  -H 'Content-Type: application/json' \
  --data-binary '{"version":3}'
```

Assert the response and a fresh model list show `xx.is_default=true`, the previous image model is no longer default, and the target version increments once.
