import { InteractiveWidget } from '../components/widgets/base/InteractiveWidget';

class FocusManagerService {
  private widgets: InteractiveWidget[] = [];
  private activeIndex: number = -1;
  private changeListeners: Set<(activeWidget: InteractiveWidget | null) => void> = new Set();

  public register(widget: InteractiveWidget): void {
    if (this.widgets.some(w => w.id === widget.id)) return;
    this.widgets.push(widget);
    if (widget.onMount) {
      widget.onMount();
    }
    // If it's the first widget registered, focus it
    if (this.activeIndex === -1) {
      this.activeIndex = 0;
      if (widget.onFocus) {
        widget.onFocus();
      }
      this.notify();
    }
  }

  public unregister(id: string): void {
    const idx = this.widgets.findIndex(w => w.id === id);
    if (idx === -1) return;

    const wasActive = idx === this.activeIndex;
    if (wasActive && this.widgets[idx].onBlur) {
      this.widgets[idx].onBlur!();
    }

    this.widgets.splice(idx, 1);

    if (this.widgets.length === 0) {
      this.activeIndex = -1;
    } else if (wasActive) {
      this.activeIndex = Math.min(this.activeIndex, this.widgets.length - 1);
      const newActive = this.widgets[this.activeIndex];
      if (newActive.onFocus) {
        newActive.onFocus();
      }
    } else if (idx < this.activeIndex) {
      this.activeIndex--;
    }
    this.notify();
  }

  public focusNext(): void {
    if (this.widgets.length <= 1) return;
    const prevWidget = this.getActiveWidget();
    if (prevWidget && prevWidget.onBlur) {
      prevWidget.onBlur();
    }
    this.activeIndex = (this.activeIndex + 1) % this.widgets.length;
    const newWidget = this.getActiveWidget();
    if (newWidget && newWidget.onFocus) {
      newWidget.onFocus();
    }
    this.notify();
  }

  public focusPrevious(): void {
    if (this.widgets.length <= 1) return;
    const prevWidget = this.getActiveWidget();
    if (prevWidget && prevWidget.onBlur) {
      prevWidget.onBlur();
    }
    this.activeIndex = (this.activeIndex - 1 + this.widgets.length) % this.widgets.length;
    const newWidget = this.getActiveWidget();
    if (newWidget && newWidget.onFocus) {
      newWidget.onFocus();
    }
    this.notify();
  }

  public focusWidget(id: string): void {
    const idx = this.widgets.findIndex(w => w.id === id);
    if (idx === -1 || idx === this.activeIndex) return;
    const prevWidget = this.getActiveWidget();
    if (prevWidget && prevWidget.onBlur) {
      prevWidget.onBlur();
    }
    this.activeIndex = idx;
    const newWidget = this.getActiveWidget();
    if (newWidget && newWidget.onFocus) {
      newWidget.onFocus();
    }
    this.notify();
  }

  public getActiveWidget(): InteractiveWidget | null {
    if (this.activeIndex >= 0 && this.activeIndex < this.widgets.length) {
      return this.widgets[this.activeIndex];
    }
    return null;
  }

  public onChange(listener: (activeWidget: InteractiveWidget | null) => void): () => void {
    this.changeListeners.add(listener);
    // Initial call
    listener(this.getActiveWidget());
    return () => {
      this.changeListeners.delete(listener);
    };
  }

  public reset(): void {
    this.widgets = [];
    this.activeIndex = -1;
    this.changeListeners.clear();
  }

  private notify() {
    const active = this.getActiveWidget();
    for (const listener of this.changeListeners) {
      listener(active);
    }
  }
}

export const FocusManager = new FocusManagerService();
