/**
 * capture.ts — serialise a VS Code chat history into the JSON format
 * accepted by `claude-memory ingest --format copilot`.
 */
import * as crypto from 'crypto';
import * as vscode from 'vscode';

export interface CopilotTurn {
  role: 'user' | 'assistant';
  content: string;
}

export interface CopilotSession {
  format: 'copilot';
  session_id: string;
  cwd: string;
  captured_at: string;
  model?: string;
  turns: CopilotTurn[];
}

/**
 * Convert VS Code chat history turns into a CopilotSession payload.
 *
 * @param history  The `context.history` array from a ChatRequestHandler.
 * @param extraPrompt  The user message that triggered this save (e.g.
 *                     "@memory save") — appended as a final user turn so it
 *                     doesn't get lost.
 */
export function buildSession(
  history: ReadonlyArray<vscode.ChatRequestTurn | vscode.ChatResponseTurn>,
  extraPrompt?: string,
): CopilotSession | null {
  const turns: CopilotTurn[] = [];

  for (const turn of history) {
    if (turn instanceof vscode.ChatRequestTurn) {
      const text = turn.prompt.trim();
      if (text) {
        turns.push({ role: 'user', content: text });
      }
    } else if (turn instanceof vscode.ChatResponseTurn) {
      // Collect markdown response fragments
      const text = turn.response
        .filter(
          (p: vscode.ChatResponsePart): p is vscode.ChatResponseMarkdownPart =>
            p instanceof vscode.ChatResponseMarkdownPart,
        )
        .map((p: vscode.ChatResponseMarkdownPart) => p.value.value)
        .join('');
      if (text.trim()) {
        turns.push({ role: 'assistant', content: text.trim() });
      }
    }
  }

  // Include the triggering prompt (e.g. "@memory save") as a user turn
  if (extraPrompt?.trim()) {
    turns.push({ role: 'user', content: extraPrompt.trim() });
  }

  const userTurns = turns.filter((t) => t.role === 'user');
  if (userTurns.length === 0) {
    // Nothing worth storing
    return null;
  }

  const cwd = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath ?? '';

  return {
    format: 'copilot',
    session_id: crypto.randomUUID(),
    cwd,
    captured_at: new Date().toISOString(),
    turns,
  };
}
