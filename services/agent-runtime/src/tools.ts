import type { AgentHarnessTool, ExecutionToolContext } from "@earendil-works/pi-agent-core";
import { Type } from "@earendil-works/pi-ai";

function textResult(text: string, details: Record<string, unknown> = {}) {
  return { content: [{ type: "text" as const, text }], details };
}

function valueOrThrow<T>(result: { ok: true; value: T } | { ok: false; error: Error }): T {
  if (!result.ok) throw result.error;
  return result.value;
}

export function createWorkspaceTools(): AgentHarnessTool<ExecutionToolContext>[] {
  const readParameters = Type.Object({
    path: Type.String({ description: "Workspace-relative or absolute file path" }),
    offset: Type.Optional(Type.Integer({ minimum: 1 })),
    limit: Type.Optional(Type.Integer({ minimum: 1, maximum: 10_000 })),
  });
  const read: AgentHarnessTool<ExecutionToolContext, typeof readParameters> = {
    name: "read",
    label: "Read file",
    description: "Read a UTF-8 file from the configured workspace.",
    parameters: readParameters,
    execute: async (_id, input, signal, _onUpdate, { env }) => {
      const content = valueOrThrow(await env.readTextFile(input.path, signal));
      const lines = content.split("\n");
      const offset = input.offset ?? 1;
      const selected = lines.slice(offset - 1, input.limit ? offset - 1 + input.limit : undefined).join("\n");
      return textResult(selected, { path: input.path, offset, line_count: selected.split("\n").length });
    },
  };

  const writeParameters = Type.Object({ path: Type.String(), content: Type.String() });
  const write: AgentHarnessTool<ExecutionToolContext, typeof writeParameters> = {
    name: "write",
    label: "Write file",
    description: "Create or overwrite a UTF-8 file in the configured workspace.",
    parameters: writeParameters,
    executionMode: "sequential",
    execute: async (_id, input, signal, _onUpdate, { env }) => {
      valueOrThrow(await env.writeFile(input.path, input.content, signal));
      return textResult(`Wrote ${Buffer.byteLength(input.content)} bytes to ${input.path}`, {
        path: input.path,
        bytes: Buffer.byteLength(input.content),
      });
    },
  };

  const editParameters = Type.Object({ path: Type.String(), old_text: Type.String(), new_text: Type.String() });
  const edit: AgentHarnessTool<ExecutionToolContext, typeof editParameters> = {
    name: "edit",
    label: "Edit file",
    description: "Replace one exact text occurrence in a UTF-8 workspace file.",
    parameters: editParameters,
    executionMode: "sequential",
    execute: async (_id, input, signal, _onUpdate, { env }) => {
      if (input.old_text.length === 0) throw new Error("old_text must not be empty");
      const content = valueOrThrow(await env.readTextFile(input.path, signal));
      const first = content.indexOf(input.old_text);
      if (first < 0) throw new Error("old_text was not found");
      if (content.indexOf(input.old_text, first + input.old_text.length) >= 0) {
        throw new Error("old_text is not unique");
      }
      const next = content.slice(0, first) + input.new_text + content.slice(first + input.old_text.length);
      valueOrThrow(await env.writeFile(input.path, next, signal));
      return textResult(`Edited ${input.path}`, { path: input.path });
    },
  };

  const bashParameters = Type.Object({
    command: Type.String(),
    timeout_seconds: Type.Optional(Type.Number({ exclusiveMinimum: 0, maximum: 3_600 })),
  });
  const bash: AgentHarnessTool<ExecutionToolContext, typeof bashParameters> = {
    name: "bash",
    label: "Run command",
    description: "Run a shell command with the configured workspace as its working directory.",
    parameters: bashParameters,
    executionMode: "sequential",
    execute: async (_id, input, signal, onUpdate, { env }) => {
      let stdout = "";
      let stderr = "";
      const result = valueOrThrow(
        await env.exec(input.command, {
          cwd: env.cwd,
          ...(signal ? { abortSignal: signal } : {}),
          ...(input.timeout_seconds ? { timeout: input.timeout_seconds } : {}),
          onStdout: (chunk) => {
            stdout += chunk;
            onUpdate?.(textResult(stdout + stderr, { stdout, stderr, running: true }));
          },
          onStderr: (chunk) => {
            stderr += chunk;
            onUpdate?.(textResult(stdout + stderr, { stdout, stderr, running: true }));
          },
        }),
      );
      return textResult(
        `${result.stdout}${result.stderr}${result.exitCode === 0 ? "" : `\nProcess exited with code ${result.exitCode}`}`,
        result,
      );
    },
  };

  return [read, write, edit, bash];
}
