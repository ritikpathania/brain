export type AppEvent =
  | { type: 'QueryStarted'; query: string }
  | { type: 'QueryFinished'; success: boolean }
  | { type: 'ThemeChanged'; theme: string }
  | { type: 'HistoryAdded'; command: string }
  | { type: 'MetricsUpdated'; metrics: any }
  | { type: 'ToastAdded'; message: string }
  | { type: 'ClearPrompt' };

export type EventCallback = (event: AppEvent) => void;

class EventBusService {
  private listeners: Set<EventCallback> = new Set();

  public subscribe(callback: EventCallback): () => void {
    this.listeners.add(callback);
    return () => {
      this.listeners.delete(callback);
    };
  }

  public publish(event: AppEvent): void {
    for (const listener of this.listeners) {
      listener(event);
    }
  }
}

export const EventBus = new EventBusService();
