import {
  createSdkMcpServer,
  tool,
} from "@anthropic-ai/claude-agent-sdk";
import { z } from "zod";
import { VibeLightsClient } from "./vibelights-client.js";

/**
 * Create an in-process SDK MCP server with VibeLights tools.
 * Uses 3 meta-tools (not 70+) to keep context consumption low.
 */
export function createVibeLightsMcpServer(client: VibeLightsClient) {
  const commandTool = tool(
    "vibelights_command",
    "Execute a VibeLights command. Use vibelights_help to discover available commands and their schemas.",
    {
      command: z.string().describe("The command name (e.g. 'get_show', 'add_effect')"),
      params: z
        .record(z.unknown())
        .optional()
        .describe("Command parameters as a JSON object"),
    },
    async (args) => {
      try {
        const result = await client.executeTool(
          args.command,
          args.params ?? {},
        );
        return {
          content: [
            {
              type: "text" as const,
              text:
                typeof result === "string"
                  ? result
                  : JSON.stringify(result, null, 2),
            },
          ],
        };
      } catch (err) {
        return {
          content: [
            {
              type: "text" as const,
              text: `Error: ${err instanceof Error ? err.message : String(err)}`,
            },
          ],
          isError: true,
        };
      }
    },
  );

  const helpTool = tool(
    "vibelights_help",
    "Discover available VibeLights commands. Call with no arguments to see categories, or with a topic to see commands in that category.",
    {
      topic: z
        .string()
        .optional()
        .describe(
          "Category name or command name to get details for (e.g. 'edit', 'query', 'add_effect')",
        ),
    },
    async (args) => {
      try {
        const result = await client.executeTool("help", {
          topic: args.topic,
        });
        return {
          content: [
            {
              type: "text" as const,
              text:
                typeof result === "string"
                  ? result
                  : JSON.stringify(result, null, 2),
            },
          ],
        };
      } catch {
        // Fallback: fetch tool schemas directly
        try {
          const schemas = await client.getToolSchemas();
          if (args.topic) {
            const filtered = schemas.filter(
              (s) =>
                s.category === args.topic ||
                s.name.toLowerCase() === args.topic?.toLowerCase(),
            );
            return {
              content: [
                {
                  type: "text" as const,
                  text: JSON.stringify(filtered, null, 2),
                },
              ],
            };
          }
          const categories = [
            ...new Set(schemas.map((s) => s.category)),
          ];
          return {
            content: [
              {
                type: "text" as const,
                text: `Available categories: ${categories.join(", ")}\n\nUse vibelights_help({topic: "category_name"}) to see commands in a category.`,
              },
            ],
          };
        } catch (err2) {
          return {
            content: [
              {
                type: "text" as const,
                text: `Error: ${err2 instanceof Error ? err2.message : String(err2)}`,
              },
            ],
            isError: true,
          };
        }
      }
    },
  );

  const batchTool = tool(
    "vibelights_batch",
    "Execute multiple VibeLights commands as a single undo step. Use this for batch edits.",
    {
      description: z
        .string()
        .describe("Human-readable description of the batch edit"),
      commands: z
        .array(
          z.object({
            command: z.string(),
            params: z.record(z.unknown()).optional(),
          }),
        )
        .describe("Array of commands to execute"),
    },
    async (args) => {
      try {
        const result = await client.batchEdit({
          description: args.description,
          commands: args.commands.map((c) => ({
            action: c.command,
            params: c.params ?? {},
          })),
        });
        return {
          content: [
            {
              type: "text" as const,
              text:
                typeof result === "string"
                  ? result
                  : JSON.stringify(result, null, 2),
            },
          ],
        };
      } catch (err) {
        return {
          content: [
            {
              type: "text" as const,
              text: `Error: ${err instanceof Error ? err.message : String(err)}`,
            },
          ],
          isError: true,
        };
      }
    },
  );

  return createSdkMcpServer({
    name: "vibelights",
    version: "0.1.0",
    tools: [commandTool, helpTool, batchTool],
  });
}
