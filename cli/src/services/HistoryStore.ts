import fs from 'fs';
import path from 'path';

class HistoryStoreService {
  private historyPath: string;
  private history: string[] = [];

  constructor() {
    const homeDir = process.env.HOME || '/tmp';
    const configDir = path.join(homeDir, '.brain');
    if (!fs.existsSync(configDir)) {
      fs.mkdirSync(configDir, { recursive: true });
    }
    this.historyPath = path.join(configDir, 'history.log');
    this.load();
  }

  public load(): string[] {
    try {
      if (fs.existsSync(this.historyPath)) {
        const content = fs.readFileSync(this.historyPath, 'utf8');
        this.history = content.split('\n').map(s => s.trim()).filter(Boolean);
        // enforce max limit
        if (this.history.length > 1000) {
          this.history = this.history.slice(-1000);
        }
      }
    } catch (e) {}
    return this.history;
  }

  public append(cmd: string): void {
    const trimmed = cmd.trim();
    if (!trimmed) return; // ignore blank

    // deduplicate consecutive duplicates
    if (this.history.length > 0 && this.history[this.history.length - 1] === trimmed) {
      return;
    }

    this.history.push(trimmed);

    if (this.history.length > 1000) {
      this.history = this.history.slice(-1000);
    }

    // Write asynchronously to prevent UI stutters
    fs.appendFile(this.historyPath, trimmed + '\n', () => {});
  }

  public getHistory(): string[] {
    return this.history;
  }

  public clear(): void {
    this.history = [];
    fs.writeFile(this.historyPath, '', () => {});
  }
}

export const HistoryStore = new HistoryStoreService();
