import { registerBundledSkill } from '../../vendor/claude/skills/bundledSkills.js';

const DESCRIPTION =
  'Diagramming know-how for Artifacts — when a picture earns its place, how to draw one that shows the real mechanism, and the inline-SVG mechanics that keep it legible in both themes.';

export function registerArtifactDiagrammingSkill(): void {
  registerBundledSkill({
    name: 'artifact-diagramming',
    description: DESCRIPTION,
    isEnabled: () => false,
    userInvocable: true,
    files: {},
    async getPromptForCommand(args) {
      const parts: string[] = [DESCRIPTION];
      if (args) {
        parts.push(`## User Request\n\n${args}`);
      }
      return [{ type: 'text', text: parts.join('\n\n') }];
    },
  });
}
