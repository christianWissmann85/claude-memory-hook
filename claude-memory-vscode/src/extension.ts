/**
 * extension.ts — VS Code extension entry point.
 *
 * Registers:
 *  1. `@memory` chat participant — invoke inside Copilot Chat to save history.
 *  2. `Claude Memory: Save Current Chat Session` command (command palette /
 *     keyboard shortcut).
 *  3. Window-state change listener — auto-saves when VS Code loses focus
 *     (if `claudeMemory.autoSaveOnWindowClose` is enabled) using the most
 *     recently captured history snapshot.
 *
 * ## Why not fully passive?
 *
 * VS Code's stable chat API only exposes `context.history` *inside* a chat
 * participant's `requestHandler`. There is no public event that fires whenever
 * Copilot produces a response. The `@memory` participant therefore acts as
 * the capture trigger — the user invokes it once per session (or lets the
 * window-close auto-save handle it).
 *
 * The auto-save path reuses the last history snapshot that was passed to the
 * participant's handler. If the participant has never been invoked in this
 * VS Code session, auto-save is a no-op.
 */
import * as vscode from 'vscode';

import { buildSession, CopilotSession } from './capture';
import { ingestSession } from './ingest';

/** The most recent history snapshot — updated each time @memory is invoked. */
let lastSession: CopilotSession | null = null;

export function activate(context: vscode.ExtensionContext): void {
  // -------------------------------------------------------------------------
  // 1. Chat participant — @memory
  // -------------------------------------------------------------------------
  const participant = vscode.chat.createChatParticipant(
    'claude-memory.memory',
    async (
      request: vscode.ChatRequest,
      chatContext: vscode.ChatContext,
      stream: vscode.ChatResponseStream,
      _token: vscode.CancellationToken,
    ) => {
      const session = buildSession(chatContext.history, request.prompt);

      if (!session) {
        stream.markdown(
          'No conversation history found to save. ' +
            'Have a chat with Copilot first, then invoke `@memory save`.',
        );
        return;
      }

      lastSession = session;

      stream.markdown('Saving session to claude-memory…');

      try {
        const msg = await ingestSession(session);
        stream.markdown(
          `✅ **Saved!** ${session.turns.filter((t) => t.role === 'user').length} user turn(s) stored.\n\n` +
            `> ${msg}`,
        );
      } catch (err: unknown) {
        const message = err instanceof Error ? err.message : String(err);
        stream.markdown(`❌ **Failed to save:** ${message}`);
        vscode.window.showErrorMessage(`claude-memory: ${message}`);
      }
    },
  );

  participant.iconPath = new vscode.ThemeIcon('database');
  context.subscriptions.push(participant);

  // -------------------------------------------------------------------------
  // 2. Command — claude-memory.saveCurrentChat
  // -------------------------------------------------------------------------
  context.subscriptions.push(
    vscode.commands.registerCommand(
      'claude-memory.saveCurrentChat',
      async () => {
        if (!lastSession) {
          vscode.window.showInformationMessage(
            'claude-memory: No chat history captured yet. ' +
              'Use @memory in a Copilot Chat session first.',
          );
          return;
        }
        try {
          const msg = await ingestSession(lastSession);
          vscode.window.showInformationMessage(`claude-memory: ${msg}`);
        } catch (err: unknown) {
          const message = err instanceof Error ? err.message : String(err);
          vscode.window.showErrorMessage(`claude-memory: ${message}`);
        }
      },
    ),
  );

  // -------------------------------------------------------------------------
  // 3. Auto-save on window focus loss
  // -------------------------------------------------------------------------
  context.subscriptions.push(
    vscode.window.onDidChangeWindowState(async (state: vscode.WindowState) => {
      if (state.focused) {
        return; // Only act when focus is *lost*
      }

      const config = vscode.workspace.getConfiguration('claudeMemory');
      const autoSave: boolean = config.get('autoSaveOnWindowClose', true);
      if (!autoSave || !lastSession) {
        return;
      }

      try {
        await ingestSession(lastSession);
        // Silent success — don't interrupt the user who just switched windows
      } catch {
        // Also silent on error — we don't want a popup when the user is leaving
      }
    }),
  );
}

export function deactivate(): void {
  // Nothing to clean up — subscriptions are disposed via context.subscriptions
}
