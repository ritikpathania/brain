import React, { useState, useEffect } from 'react';
import { ThemedBox, ThemedText } from '../design-system';
import { WidgetContainer, WidgetHeader, WidgetBody, WidgetFooter } from './base/Widget';
import { InteractiveWidget, WidgetState } from './base/InteractiveWidget';
import { FocusManager } from '../../services/FocusManager';
import { EventBus } from '../../services/EventBus';

export class MultiStepFormWidget implements InteractiveWidget {
  public id = 'config-wizard';
  public title = 'Configuration Wizard';
  private setFocused: (val: boolean) => void;
  private onInputCallback: (input: string, key: any) => boolean;

  constructor(setFocused: (val: boolean) => void, onInputCallback: (input: string, key: any) => boolean) {
    this.setFocused = setFocused;
    this.onInputCallback = onInputCallback;
  }

  public handleInput(input: string, key: any): boolean {
    return this.onInputCallback(input, key);
  }

  public onFocus() {
    this.setFocused(true);
  }

  public onBlur() {
    this.setFocused(false);
  }

  public onMount() {
    this.setFocused(false);
  }
}

interface FormStep {
  title: string;
  type: 'select' | 'input';
  options?: string[];
  placeholder?: string;
  keyName: string;
}

export const MultiStepForm: React.FC = () => {
  const steps: FormStep[] = [
    {
      title: '1. Select LLM Provider',
      type: 'select',
      options: ['noop', 'openai', 'ollama', 'anthropic'],
      keyName: 'llm',
    },
    {
      title: '2. Select Embedding Provider',
      type: 'select',
      options: ['noop', 'openai', 'ollama', 'sentence-transformers'],
      keyName: 'embeddings',
    },
    {
      title: '3. Enter Sliding Window Cache Size',
      type: 'input',
      placeholder: 'Enter positive integer (default: 10)',
      keyName: 'windowSize',
    },
  ];

  const [currentStepIdx, setCurrentStepIdx] = useState(0);
  const [formData, setFormData] = useState<Record<string, string>>({
    llm: 'noop',
    embeddings: 'noop',
    windowSize: '10',
  });
  const [selectedOptIdx, setSelectedOptIdx] = useState(0);
  const [inputText, setInputText] = useState('');
  const [focused, setFocusedState] = useState(false);
  const [widgetState, setWidgetState] = useState<WidgetState>('idle');

  const currentStep = steps[currentStepIdx];

  useEffect(() => {
    // Reset selection index when moving between select steps
    if (currentStep.type === 'select') {
      const currentVal = formData[currentStep.keyName];
      const optIdx = currentStep.options?.indexOf(currentVal) ?? 0;
      setSelectedOptIdx(optIdx >= 0 ? optIdx : 0);
    } else {
      setInputText(formData[currentStep.keyName] || '');
    }
  }, [currentStepIdx]);

  const handleWidgetInput = (input: string, key: any): boolean => {
    if (currentStep.type === 'select') {
      if (key.upArrow) {
        setSelectedOptIdx((prev) => Math.max(0, prev - 1));
        return true;
      }
      if (key.downArrow) {
        setSelectedOptIdx((prev) => Math.min((currentStep.options?.length ?? 1) - 1, prev + 1));
        return true;
      }
      if (key.return) {
        const selectedVal = currentStep.options![selectedOptIdx];
        setFormData((prev) => ({ ...prev, [currentStep.keyName]: selectedVal }));

        if (currentStepIdx < steps.length - 1) {
          setCurrentStepIdx((prev) => prev + 1);
        } else {
          finishForm();
        }
        return true;
      }
    } else if (currentStep.type === 'input') {
      if (key.return) {
        const val = inputText.trim() || '10';
        setFormData((prev) => ({ ...prev, [currentStep.keyName]: val }));

        if (currentStepIdx < steps.length - 1) {
          setCurrentStepIdx((prev) => prev + 1);
        } else {
          // Trigger form completion asynchronously
          setFormData((prev) => {
            const finalData = { ...prev, [currentStep.keyName]: val };
            setTimeout(() => finishFormWithData(finalData), 0);
            return finalData;
          });
        }
        return true;
      }
      if (key.backspace || key.delete) {
        setInputText((prev) => prev.slice(0, -1));
        return true;
      }
      if (input && !key.ctrl && !key.meta && !key.escape && input !== '\r' && input !== '\n' && input !== '\t') {
        setInputText((prev) => prev + input);
        return true;
      }
    }

    // Go back to previous step on escape/left
    if (key.escape || key.leftArrow) {
      if (currentStepIdx > 0) {
        setCurrentStepIdx((prev) => prev - 1);
      } else {
        EventBus.publish({
          type: 'ToastAdded',
          message: 'Already at the first configuration step.',
        });
      }
      return true;
    }

    return false;
  };

  const finishForm = () => {
    finishFormWithData(formData);
  };

  const finishFormWithData = (data: Record<string, string>) => {
    setWidgetState('loading');
    setTimeout(() => {
      setWidgetState('idle');
      setCurrentStepIdx(0);
      EventBus.publish({
        type: 'ToastAdded',
        message: `Wizard config completed! LLM: ${data.llm} | Embeddings: ${data.embeddings} | Cache Size: ${data.windowSize}`,
      });
    }, 1000);
  };

  useEffect(() => {
    const widget = new MultiStepFormWidget(
      (val) => setFocusedState(val),
      (input, key) => handleWidgetInput(input, key)
    );
    FocusManager.register(widget);
    return () => {
      FocusManager.unregister(widget.id);
    };
  }, [currentStepIdx, selectedOptIdx, inputText, formData, entriesForRenderHack()]);

  // Hack helper to make hook re-register on options update
  function entriesForRenderHack() {
    return currentStep.options ? currentStep.options.join(',') : '';
  }

  const shortcuts = [
    { key: '↑/↓', description: 'Navigate choices' },
    { key: 'Enter', description: 'Next step / Submit' },
    { key: 'Esc/←', description: 'Previous step' },
  ];

  const currentValForStep = currentStep.type === 'select' 
    ? (currentStep.options ? currentStep.options[selectedOptIdx] : '')
    : inputText;

  return (
    <WidgetContainer isFocused={focused}>
      <WidgetHeader
        title="Plugin Configuration Wizard"
        isFocused={focused}
        state={widgetState}
      />
      <WidgetBody state={widgetState} loadingLabel="Applying settings...">
        <ThemedBox flexDirection="column">
          <ThemedText color="primary" bold marginBottom={1}>
            {currentStep.title}
          </ThemedText>

          {currentStep.type === 'select' ? (
            <ThemedBox flexDirection="column" marginLeft={1}>
              {currentStep.options?.map((opt, idx) => {
                const isSelected = idx === selectedOptIdx;
                return (
                  <ThemedText key={opt} color={isSelected ? 'claude' : 'text'} bold={isSelected}>
                    {isSelected ? '▶ ' : '  '}
                    {opt}
                  </ThemedText>
                );
              })}
            </ThemedBox>
          ) : (
            <ThemedBox flexDirection="row" borderStyle="classic" borderColor="subtle" padding={1}>
              <ThemedText color="inactive">{currentStep.placeholder || 'Type here...'}</ThemedText>
              <ThemedText color="text" bold marginLeft={2}>
                {inputText}
              </ThemedText>
              <ThemedText color="claude" bold>█</ThemedText>
            </ThemedBox>
          )}
        </ThemedBox>
      </WidgetBody>
      <WidgetFooter
        shortcuts={shortcuts}
        statusText={`Step ${currentStepIdx + 1} of ${steps.length} | Selection: "${currentValForStep}"`}
      />
    </WidgetContainer>
  );
};
