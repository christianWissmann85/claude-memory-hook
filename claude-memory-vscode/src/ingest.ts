/**
 * ingest.ts — pipe a CopilotSession payload into the claude-memory binary.
 */
import * as child_process from 'child_process';
import * as vscode from 'vscode';

import { CopilotSession } from './capture';

/**
 * Call `<binaryPath> ingest --format copilot` and write the session JSON to
 * its stdin.  Rejects with a descriptive error if the binary is not found or
 * exits non-zero.
 */
export async function ingestSession(session: CopilotSession): Promise<string> {
  const config = vscode.workspace.getConfiguration('claudeMemory');
  const binaryPath: string = config.get('binaryPath', 'claude-memory');
  const json = JSON.stringify(session);

  return new Promise((resolve, reject) => {
    const proc = child_process.spawn(
      binaryPath,
      ['ingest', '--format', 'copilot'],
      {
        cwd: session.cwd || undefined,
        stdio: ['pipe', 'pipe', 'pipe'],
      },
    );

    let stderr = '';
    let stdout = '';

    proc.stdout.on('data', (d: Buffer) => {
      stdout += d.toString();
    });
    proc.stderr.on('data', (d: Buffer) => {
      stderr += d.toString();
    });

    proc.on('error', (err: NodeJS.ErrnoException) => {
      if (err.code === 'ENOENT') {
        reject(
          new Error(
            `claude-memory binary not found at '${binaryPath}'. ` +
              `Install it with 'cargo install claude-memory' or set ` +
              `'claudeMemory.binaryPath' in VS Code settings.`,
          ),
        );
      } else {
        reject(new Error(`Failed to launch '${binaryPath}': ${err.message}`));
      }
    });

    proc.on('close', (code: number | null) => {
      if (code === 0) {
        // claude-memory writes a confirmation line to stderr
        resolve(stderr.trim() || stdout.trim() || 'Session ingested.');
      } else {
        reject(
          new Error(
            `claude-memory exited with code ${code ?? '?'}. ${stderr.trim()}`,
          ),
        );
      }
    });

    proc.stdin.write(json);
    proc.stdin.end();
  });
}
